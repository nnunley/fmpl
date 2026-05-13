//! SCENARIO-0100 — Compiled bytecode persists with a content-addressed
//! source reference.
//!
//! Owning stories: STORY-0100 (AC-1, AC-2).
//! Proof seam: integration.
//!
//! Preconditions:
//! - FjallStore-backed bytecode keyspace available via tempdir
//! - SourceStore initialized at a sibling subdir
//!
//! Action:
//! - Compile `"1 + 2"` via the lexer + parser + compiler.
//! - Save the resulting CompiledCode with `source = Some(b"1 + 2")`.
//! - Compile + save the same source again under a different key.
//!
//! Expected observables:
//! - Both CompiledCode envelopes carry the same `source_hash` (blake3
//!   of `"1 + 2"`).
//! - The SourceStore contains exactly one entry under that hash.
//! - Reading the SourceStore at that hash returns `"1 + 2"`
//!   byte-for-byte.
//! - Identical sources are deduplicated (no double-write at the
//!   storage level).

#![cfg(feature = "fjall-backend")]

use fmpl_core::compiler::{CompiledCode, Compiler};
use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;
use fmpl_persistence::envelope::{ENVELOPE_HEADER_SIZE, EnvelopeHeader};
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::{Hash, SourceStore, Store, hash_bytes};
use zerocopy::FromBytes;

fn compile(src: &str) -> CompiledCode {
    let tokens = Lexer::new(src).tokenize().expect("lex");
    let ast = Parser::with_source(&tokens, src).parse().expect("parse");
    Compiler::new().compile(&ast).expect("compile")
}

#[test]
fn scenario_0100_identical_source_deduplicates_in_source_store() {
    let dir = tempfile::tempdir().unwrap();
    let bytecode_store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    let source = "1 + 2";
    let source_bytes = source.as_bytes();
    let expected_hash = hash_bytes(source_bytes);
    assert_ne!(expected_hash, Hash::NONE);

    // First save.
    let code_a = compile(source);
    code_a
        .save_to_store(&bytecode_store, &source_store, "first", Some(source_bytes))
        .unwrap();

    // Second save with byte-identical source.
    let code_b = compile(source);
    code_b
        .save_to_store(&bytecode_store, &source_store, "second", Some(source_bytes))
        .unwrap();

    // Read both envelopes; their source_hash fields must match each
    // other and the expected hash.
    let raw_a = bytecode_store.get(b"first").unwrap().unwrap();
    let raw_b = bytecode_store.get(b"second").unwrap().unwrap();
    let (hdr_a, _) = EnvelopeHeader::ref_from_prefix(&raw_a[..]).unwrap();
    let (hdr_b, _) = EnvelopeHeader::ref_from_prefix(&raw_b[..]).unwrap();

    assert_eq!(
        hdr_a.source_hash, hdr_b.source_hash,
        "byte-identical sources must yield identical envelope source_hashes"
    );
    assert_eq!(
        hdr_a.source_hash,
        *expected_hash.as_bytes(),
        "envelope source_hash must equal the blake3 of the source bytes"
    );

    // SourceStore returns the original bytes at that hash.
    let recovered = source_store.get(expected_hash).unwrap().unwrap();
    assert_eq!(
        recovered.as_slice(),
        source_bytes,
        "SourceStore must return the original source bytes verbatim"
    );

    // SourceStore stored exactly one entry. Use compact with the
    // single referenced hash and verify nothing was removed (= no
    // extra records existed under SourceStore-shaped keys).
    let stats = source_store.compact([expected_hash]).unwrap();
    assert_eq!(stats.retained, 1);
    assert_eq!(
        stats.removed, 0,
        "duplicate source insertion must not have created a second record in the source store"
    );

    // Bonus property the test confirms empirically FOR THIS SPECIFIC
    // INPUT: byte-identical source compiled twice yields byte-
    // identical envelope bytes.
    //
    // CAVEAT (closing-PAR R-M-S-1, 2026-05-14): this is NOT a
    // universal property of FMPL's compiler. `"1 + 2"` is an
    // arithmetic expression with NO grammar rules; its CompiledCode
    // has an empty `rule_entry_points: HashMap<SmolStr, InstrIndex>`
    // map, and serde_json serializes an empty HashMap deterministically
    // as `{}`. For grammar-bearing programs (non-empty rule_entry_points),
    // Rust's HashMap uses a randomized hasher (SipHash with a per-process
    // seed) → serde_json emits keys in hash order → different process
    // invocations produce different envelope bytes even from identical
    // source. To make this property universal, swap HashMap for BTreeMap
    // in CompiledCode (filed as an open follow-up; out of ITER-0005b
    // scope). For this iteration the test is scoped to grammar-free
    // input and the deterministic-compiler claim in the iteration-log
    // is narrowed to match.
    assert_eq!(
        raw_a, raw_b,
        "for this specific grammar-free input, byte-identical source yields byte-identical envelope contents"
    );
    let _ = ENVELOPE_HEADER_SIZE; // silence unused-import lint
}

/// Sanity: hashing the same byte slice twice externally also yields
/// the same hash — proves the dedup property is rooted in
/// `hash_bytes`'s determinism, not in the SourceStore's logic.
#[test]
fn scenario_0100_hash_bytes_is_the_dedup_primitive() {
    let h1 = hash_bytes(b"the answer is 42");
    let h2 = hash_bytes(b"the answer is 42");
    let h3 = hash_bytes(b"the answer is 43");
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}
