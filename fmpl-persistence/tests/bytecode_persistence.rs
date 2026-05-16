//! Tests for CompiledCode persistence to Store-backed storage.

#![cfg(feature = "fjall-backend")]

use fmpl_core::compiler::{CompiledCode, Compiler};
use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;
use fmpl_core::value::Value;
use fmpl_core::vm::Vm;
use fmpl_persistence::SourceStore;
use fmpl_persistence::fjall_backend::FjallStore;

fn compile(source: &str) -> CompiledCode {
    let tokens = Lexer::new(source).tokenize().expect("lex failed");
    let ast = Parser::with_source(&tokens, source)
        .parse()
        .expect("parse failed");
    Compiler::new().compile(&ast).expect("compile failed")
}

fn eval_code(code: &CompiledCode) -> Value {
    let mut vm = Vm::new();
    vm.run(code).expect("runtime error")
}

/// Open both a bytecode store and a source store in sibling subdirs
/// of the same tempdir — mirrors the production layout where every
/// data_dir hosts multiple FjallStore subdirs.
fn fresh_stores() -> (tempfile::TempDir, FjallStore, SourceStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();
    (dir, store, source_store)
}

/// AC-1: save_bytecode() serializes CompiledCode to the Store.
/// AC-2: load_bytecode() deserializes and returns usable CompiledCode.
/// AC-3: Restored bytecode executes correctly.
#[test]
fn bytecode_survives_save_restore() {
    let (_dir, store, source_store) = fresh_stores();

    let source = "1 + 2";
    let code = compile(source);
    code.save_to_store(&store, &source_store, "test_add", Some(source.as_bytes()))
        .unwrap();

    let restored = CompiledCode::load_from_store(&store, "test_add")
        .unwrap()
        .expect("should find saved bytecode");

    assert_eq!(eval_code(&restored), Value::Int(3));
}

/// AC-2: load returns None for missing key.
#[test]
fn load_missing_key_returns_none() {
    let (_dir, store, _source_store) = fresh_stores();

    let result = CompiledCode::load_from_store(&store, "nonexistent").unwrap();
    assert!(result.is_none());
}

/// AC-4: Round-trip works for programs with various instruction types.
#[test]
fn various_instruction_types_survive() {
    let (_dir, store, source_store) = fresh_stores();

    // Arithmetic
    let s = "10 * 3 + 5";
    let code = compile(s);
    code.save_to_store(&store, &source_store, "arith", Some(s.as_bytes()))
        .unwrap();
    let restored = CompiledCode::load_from_store(&store, "arith")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(35));

    // String
    let s = r#""hello" + " world""#;
    let code = compile(s);
    code.save_to_store(&store, &source_store, "string", Some(s.as_bytes()))
        .unwrap();
    let restored = CompiledCode::load_from_store(&store, "string")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::String("hello world".into()));

    // Boolean / conditional
    let s = "if true then 1 else 2";
    let code = compile(s);
    code.save_to_store(&store, &source_store, "cond", Some(s.as_bytes()))
        .unwrap();
    let restored = CompiledCode::load_from_store(&store, "cond")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(1));
}

/// AC-4: Lambdas (nested code) survive round-trip.
#[test]
fn nested_code_survives() {
    let (_dir, store, source_store) = fresh_stores();

    let s = "let f = \\x x + 1; f(41)";
    let code = compile(s);
    code.save_to_store(&store, &source_store, "lambda", Some(s.as_bytes()))
        .unwrap();
    let restored = CompiledCode::load_from_store(&store, "lambda")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(42));
}

/// Multiple keys coexist in the same Store.
#[test]
fn multiple_keys_coexist() {
    let (_dir, store, source_store) = fresh_stores();

    let s1 = "1 + 1";
    let s2 = "2 * 3";
    let code1 = compile(s1);
    let code2 = compile(s2);
    code1
        .save_to_store(&store, &source_store, "a", Some(s1.as_bytes()))
        .unwrap();
    code2
        .save_to_store(&store, &source_store, "b", Some(s2.as_bytes()))
        .unwrap();

    let r1 = CompiledCode::load_from_store(&store, "a").unwrap().unwrap();
    let r2 = CompiledCode::load_from_store(&store, "b").unwrap().unwrap();

    assert_eq!(eval_code(&r1), Value::Int(2));
    assert_eq!(eval_code(&r2), Value::Int(6));
}

/// AC-2 from STORY-0100: persisted CompiledCode carries a non-NONE
/// source_hash that points into the source store. Verifies the
/// `source: Some(...)` plumbing actually populated the envelope and
/// the source store has the original bytes at that hash.
#[test]
fn save_with_source_stamps_envelope_and_populates_source_store() {
    use fmpl_persistence::envelope::{ENVELOPE_HEADER_SIZE, EnvelopeHeader};
    use fmpl_persistence::{Hash, Store, hash_bytes};
    use zerocopy::FromBytes;

    let (_dir, store, source_store) = fresh_stores();
    let source = "21 * 2";
    let code = compile(source);
    code.save_to_store(&store, &source_store, "with_src", Some(source.as_bytes()))
        .unwrap();

    // Pull the raw stored bytes; decode the envelope header zero-copy.
    let raw = store
        .get(b"with_src")
        .unwrap()
        .expect("just inserted; must be present");
    assert!(raw.len() >= ENVELOPE_HEADER_SIZE);
    let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..]).unwrap();

    // Envelope's source_hash must match the hash of the source bytes.
    let expected = hash_bytes(source.as_bytes());
    assert_eq!(
        hdr.source_hash,
        *expected.as_bytes(),
        "envelope's source_hash must equal blake3 of supplied source"
    );
    assert_ne!(
        hdr.source_hash, [0u8; 32],
        "source_hash must NOT be Hash::NONE when source is supplied"
    );

    // Source store has the bytes at that hash.
    let recovered = source_store
        .get(Hash::from_bytes(hdr.source_hash))
        .unwrap()
        .expect("source must be in source store");
    assert_eq!(recovered.as_slice(), source.as_bytes());
}

/// `source: None` stamps Hash::NONE — preserves the pre-0005b
/// behavior for callers that don't have source provenance.
#[test]
fn save_without_source_stamps_none() {
    use fmpl_persistence::Store;
    use fmpl_persistence::envelope::{ENVELOPE_HEADER_SIZE, EnvelopeHeader};
    use zerocopy::FromBytes;

    let (_dir, store, source_store) = fresh_stores();
    let code = compile("99");
    code.save_to_store(&store, &source_store, "no_src", None)
        .unwrap();

    let raw = store.get(b"no_src").unwrap().unwrap();
    assert!(raw.len() >= ENVELOPE_HEADER_SIZE);
    let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..]).unwrap();
    assert_eq!(
        hdr.source_hash, [0u8; 32],
        "source: None must stamp Hash::NONE (all-zeros)"
    );
}

/// SCENARIO-0018: bytecode survives a drop+reopen of `FjallStore` at the
/// same path within the same process. Proves STORY-0014 AC-1's
/// "integration-seam" claim: after the store handle is closed and a fresh
/// handle is opened at the same path, the bytecode loads by key and runs.
///
/// Also asserts the envelope's `source_hash` and the corresponding source
/// bytes both survive drop+reopen — proves the envelope + source store
/// composition is durable, not just the payload.
///
/// The load goes through `&dyn Store` (exercising the `?Sized` relaxation
/// of `CompiledCode::load_from_store` shipped in the same iteration as its
/// first consumer).
#[test]
fn bytecode_survives_drop_and_reopen() {
    use fmpl_persistence::Store;
    use fmpl_persistence::envelope::EnvelopeHeader;
    use zerocopy::FromBytes;

    let dir = tempfile::tempdir().unwrap();
    let bytecode_path = dir.path().join("bytecode");
    let sources_path = dir.path().join("sources");

    let source = "1 + 2";
    let key = "k";

    // Phase 1: open stores, compile, save, drop.
    {
        let store = FjallStore::open(&bytecode_path).unwrap();
        let source_store = SourceStore::open(&sources_path).unwrap();
        let code = compile(source);
        code.save_to_store(&store, &source_store, key, Some(source.as_bytes()))
            .unwrap();
    } // store + source_store dropped here; fjall releases the single-writer lock

    // Phase 2: fresh handles at the SAME paths, load through `&dyn Store`,
    // execute on a fresh Vm.
    let store2 = FjallStore::open(&bytecode_path).unwrap();
    let source_store2 = SourceStore::open(&sources_path).unwrap();

    let store_dyn: &dyn Store = &store2;
    let restored = CompiledCode::load_from_store(store_dyn, key)
        .unwrap()
        .expect("bytecode must be loadable after drop+reopen");
    assert_eq!(
        eval_code(&restored),
        Value::Int(3),
        "restored bytecode must execute to the original value"
    );

    // Envelope's source_hash survives drop+reopen and resolves in source_store2.
    let raw = store2
        .get(key.as_bytes())
        .unwrap()
        .expect("envelope must be present at the key after drop+reopen");
    let (hdr, _) = EnvelopeHeader::ref_from_prefix(&raw[..])
        .expect("raw bytes must frame a complete envelope header");
    let source_hash = fmpl_persistence::Hash::from_bytes(hdr.source_hash);
    assert_ne!(
        source_hash,
        fmpl_persistence::Hash::NONE,
        "envelope source_hash must be non-NONE after drop+reopen"
    );
    let recovered = source_store2
        .get(source_hash)
        .unwrap()
        .expect("source bytes must be in source_store after drop+reopen");
    assert_eq!(
        recovered.as_slice(),
        source.as_bytes(),
        "source_hash must resolve to original source bytes after drop+reopen"
    );
}

/// SCENARIO-0018 second sub-test: a `nested: Vec<CompiledCode>`-bearing
/// program (lambda call) also survives drop+reopen. Closes the
/// "`nested` round-trip across drop+reopen" gap surfaced by pre-iteration
/// PAR.
#[test]
fn nested_code_survives_drop_and_reopen() {
    use fmpl_persistence::Store;

    let dir = tempfile::tempdir().unwrap();
    let bytecode_path = dir.path().join("bytecode");
    let sources_path = dir.path().join("sources");

    let source = "let f = \\x x + 1; f(41)";
    let key = "lambda";

    // Phase 1.
    {
        let store = FjallStore::open(&bytecode_path).unwrap();
        let source_store = SourceStore::open(&sources_path).unwrap();
        let code = compile(source);
        code.save_to_store(&store, &source_store, key, Some(source.as_bytes()))
            .unwrap();
    }

    // Phase 2.
    let store2 = FjallStore::open(&bytecode_path).unwrap();
    let store_dyn: &dyn Store = &store2;
    let restored = CompiledCode::load_from_store(store_dyn, key)
        .unwrap()
        .expect("nested bytecode must be loadable after drop+reopen");
    assert_eq!(
        eval_code(&restored),
        Value::Int(42),
        "lambda-bearing bytecode must execute correctly after drop+reopen"
    );
}
