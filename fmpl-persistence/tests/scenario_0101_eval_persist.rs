//! Scenario-0101-eval-persist — `eval_persistent` drives compile+execute+persist
//! end-to-end through the evaluator's persistence-aware sibling entry.
//!
//! Owning stories: STORY-0100 (AC-2).
//! Proof seam: integration. Impact: journey.
//!
//! Preconditions:
//! - FjallStore-backed bytecode keyspace available via tempdir.
//! - SourceStore initialized at a sibling subdir.
//! - A fresh `Vm`.
//!
//! Action:
//! - Call `fmpl_core::eval_persistent(&mut vm, "1 + 2", &store, &source_store, "answer")`.
//!
//! Expected observables:
//! - Return value equals `Value::Int(3)` (the journey actually executes).
//! - The bytecode store holds an envelope at key `"answer"`.
//! - The envelope's `source_hash` resolves in the source store to the
//!   original source bytes (`b"1 + 2"`).
//!
//! This proves the AC-2 contract: a `journey`-impact, `integration`-seam
//! observation that the eval-seam wires through to persistence.

#![cfg(feature = "fjall-backend")]

use fmpl_core::value::Value;
use fmpl_core::vm::Vm;
use fmpl_persistence::envelope::EnvelopeHeader;
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::{Hash, SourceStore, Store};
use zerocopy::FromBytes;

#[test]
fn scenario_0101_eval_persist_writes_envelope_and_returns_value() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    let mut vm = Vm::new();
    let source = "1 + 2";
    let key = "answer";

    let value =
        fmpl_core::eval_persistent(&mut vm, source, &store, &source_store, key).expect("eval");
    assert_eq!(value, Value::Int(3), "journey actually executes");

    // Read the envelope written under the binding key.
    let raw = store
        .get(key.as_bytes())
        .unwrap()
        .expect("envelope present at the key");
    let (hdr, _) =
        EnvelopeHeader::ref_from_prefix(&raw[..]).expect("value frames a complete envelope header");

    // The envelope's source_hash must be non-NONE and must resolve in the
    // source store to the original bytes — the AC-2 contract.
    let source_hash = Hash::from_bytes(hdr.source_hash);
    assert_ne!(
        source_hash,
        Hash::NONE,
        "eval_persistent must stamp a real source_hash (not the no-source sentinel)"
    );
    let recovered = source_store
        .get(source_hash)
        .unwrap()
        .expect("source store must hold the bytes under the stamped hash");
    assert_eq!(
        recovered.as_slice(),
        source.as_bytes(),
        "source_hash must resolve to the original source bytes"
    );
}

/// Sanity property: two evaluations of byte-identical source against
/// independent VMs (but a shared persistence pair) produce identical
/// `source_hash` stamps and one SourceStore record. Proves the
/// content-addressing property AC-2 promises is observable at the
/// eval-seam, not just at the lower-level `save_to_store` API.
#[test]
fn scenario_0101_eval_persist_dedups_identical_sources_at_eval_seam() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    let mut vm1 = Vm::new();
    let mut vm2 = Vm::new();
    let source = "2 + 3";

    let v1 = fmpl_core::eval_persistent(&mut vm1, source, &store, &source_store, "first").unwrap();
    let v2 = fmpl_core::eval_persistent(&mut vm2, source, &store, &source_store, "second").unwrap();
    assert_eq!(v1, Value::Int(5));
    assert_eq!(v2, Value::Int(5));

    let raw_a = store.get(b"first").unwrap().unwrap();
    let raw_b = store.get(b"second").unwrap().unwrap();
    let (hdr_a, _) = EnvelopeHeader::ref_from_prefix(&raw_a[..]).unwrap();
    let (hdr_b, _) = EnvelopeHeader::ref_from_prefix(&raw_b[..]).unwrap();
    assert_eq!(
        hdr_a.source_hash, hdr_b.source_hash,
        "byte-identical sources stamped via the eval-seam must yield identical hashes"
    );

    // SourceStore holds exactly one record for that hash.
    let h = Hash::from_bytes(hdr_a.source_hash);
    let stats = source_store.compact([h]).unwrap();
    assert_eq!(stats.retained, 1);
    assert_eq!(stats.removed, 0, "eval-seam must dedup at the source store");
}
