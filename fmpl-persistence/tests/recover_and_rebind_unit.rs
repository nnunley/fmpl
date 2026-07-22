//! Unit-style integration tests for `fmpl_core::recover_and_rebind`.
//!
//! These exercise the orchestrator's contract in isolation from the
//! full SCENARIO-0102 journey:
//! - it forwards an incompatible record's source bytes to a fresh
//!   `eval_persistent` call and counts it as `recovered_from_source`;
//! - it propagates UTF-8 errors on the iteration key through
//!   `RecoveryError::recompile(...)` into `recompile_failed`;
//! - multi-record cardinality (pure incompatible at N>1; mixed
//!   compatible + incompatible in one keyspace) — stress-tests the
//!   iterator-during-mutation pattern in `recover_incompatible`.
//!
//! The full journey (rebind + execute under the original key) is
//! covered by SCENARIO-0102 in `scenario_0102_recover_incompatible.rs`.

#![cfg(feature = "fjall-backend")]

use fmpl_core::vm::Vm;
use fmpl_persistence::envelope::write;
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::schema::PayloadKind;
use fmpl_persistence::{SourceStore, Store};
use fmpl_types::VmVersion;

/// The future-major envelope used to drive `SkippedIncompatible`. The
/// orchestrator reads the running VM major from `fmpl_core::VM_VERSION_MAJOR`.
fn future_vm() -> VmVersion {
    VmVersion::new(fmpl_core::VM_VERSION_MAJOR.wrapping_add(1), 0, 0)
}

#[test]
fn recover_and_rebind_recovers_single_incompatible_record_with_recoverable_source() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Seed: source store holds the user-source bytes that the
    // incompatible envelope's source_hash will point at.
    let source_bytes = b"1 + 2";
    let h = source_store.put(source_bytes).unwrap();

    // Write an envelope with the future VM major so the loader treats
    // it as `SkippedIncompatible::VmMajorMismatch`. Payload is a
    // placeholder string — the recovery path never reads it.
    let key = "answer";
    write(
        &store,
        key.as_bytes(),
        &"stale-payload-bytes",
        PayloadKind::CompiledCode,
        future_vm(),
        h,
    )
    .unwrap();

    let mut vm = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    assert_eq!(stats.recovered_from_source, 1);
    assert_eq!(stats.recompile_failed, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.total(), 1);

    // The rebound envelope at the original key now has the current VM
    // major (so a follow-up loader pass would see it as Loaded).
    let raw = store.get(key.as_bytes()).unwrap().unwrap();
    let (hdr, _) =
        <fmpl_persistence::envelope::EnvelopeHeader as zerocopy::FromBytes>::ref_from_prefix(&raw)
            .unwrap();
    assert_eq!(hdr.vm_version_major.get(), fmpl_core::VM_VERSION_MAJOR);
}

#[test]
fn recover_and_rebind_counts_non_utf8_key_as_recompile_failure() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Seed source store with bytes the future-VM record will reference.
    let h = source_store.put(b"1 + 2").unwrap();

    // Write an incompatible record at a key that's not valid UTF-8.
    // `recover_and_rebind`'s closure must surface this as a recompile
    // failure (not panic, not propagate as a Store error).
    let bad_key = b"\xFFnot-utf8\xFE";
    write(
        &store,
        bad_key,
        &"stale-payload-bytes",
        PayloadKind::CompiledCode,
        future_vm(),
        h,
    )
    .unwrap();

    let mut vm = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    assert_eq!(stats.recompile_failed, 1);
    assert_eq!(stats.recovered_from_source, 0);
    // Asserting all remaining counters too — if a wrong implementation
    // routed the non-UTF-8 key failure through `unrecoverable_source_
    // missing` or `unrecoverable_no_source`, the `recompile_failed == 1`
    // and `total() == 1` assertions could still pass while the actual
    // classification was wrong. These extra assertions pin the
    // discrimination.
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.total(), 1);
}

#[test]
fn recover_and_rebind_counts_non_utf8_source_as_recompile_failure() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Seed the source store with bytes that are NOT valid UTF-8. The
    // incompatible record's `source_hash` will point at these bytes, so
    // when `recover_and_rebind`'s closure fetches them and tries to
    // decode as `&str` it must surface this as a recompile failure
    // (mirror of the non-UTF-8 KEY case above, but injected on the
    // SOURCE bytes side).
    let h = source_store.put(&[0xFF, 0xFE]).unwrap();

    // A valid UTF-8 key — the failure must come from the source bytes,
    // not the key.
    let key = "some-key";
    write(
        &store,
        key.as_bytes(),
        &"stale-payload-bytes",
        PayloadKind::CompiledCode,
        future_vm(),
        h,
    )
    .unwrap();

    let mut vm = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    assert_eq!(stats.recompile_failed, 1);
    assert_eq!(stats.recovered_from_source, 0);
    // Asserting all remaining counters too — if a wrong implementation
    // routed the non-UTF-8 source failure through `unrecoverable_source_
    // missing` or `unrecoverable_no_source`, the `recompile_failed == 1`
    // and `total() == 1` assertions could still pass while the actual
    // classification was wrong. These extra assertions pin the
    // discrimination.
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.total(), 1);
}

/// N=10 incompatible records, all recoverable via source.
///
/// Stress-tests the iterator-during-mutation pattern in
/// `recover_incompatible`: the closure rebinds each key (via
/// `eval_persistent`) while `store.iter()` is still walking the same
/// keyspace. If fjall's iterator does not snapshot-isolate, the
/// rewritten records could be re-emitted by the iterator, double-
/// counting `recovered_from_source` or producing a `loaded_passthrough`
/// pass-through on the same key.
#[test]
fn recover_and_rebind_multi_incompatible_stress() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Seed N=10 distinct incompatible records, each with its own source
    // and key. Sources are distinct so each gets a unique source_hash;
    // keys are zero-padded so iteration order is stable for debugging.
    let mut keys: Vec<String> = Vec::with_capacity(10);
    for i in 1..=10u32 {
        let source = format!("{} + {}", 2 * i - 1, 2 * i);
        let h = source_store.put(source.as_bytes()).unwrap();
        let key = format!("key-{:02}", i);
        write(
            &store,
            key.as_bytes(),
            &"stale-payload-bytes",
            PayloadKind::CompiledCode,
            future_vm(),
            h,
        )
        .unwrap();
        keys.push(key);
    }

    let mut vm = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    assert_eq!(stats.recovered_from_source, 10);
    assert_eq!(stats.recompile_failed, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.total(), 10);

    // Every rebound envelope now stamps the current VM major. If any
    // key still carries the future-major header, the rebind did not
    // land at that key.
    for key in &keys {
        let raw = store
            .get(key.as_bytes())
            .unwrap()
            .unwrap_or_else(|| panic!("rebound record missing at key {}", key));
        let (hdr, _) =
            <fmpl_persistence::envelope::EnvelopeHeader as zerocopy::FromBytes>::ref_from_prefix(
                &raw,
            )
            .unwrap();
        assert_eq!(
            hdr.vm_version_major.get(),
            fmpl_core::VM_VERSION_MAJOR,
            "rebound envelope at {} has wrong vm_version_major",
            key
        );
    }
}

/// Mixed cardinality: K=5 already-compatible records co-resident with
/// N=10 incompatible records in the same keyspace.
///
/// This is the production-realistic stress that the single-shape tests
/// would miss: `recover_incompatible`'s iterator must yield both
/// `Loaded` and `SkippedIncompatible` outcomes interleaved while the
/// closure mutates `SkippedIncompatible` entries via `Store::insert`.
/// `loaded_passthrough` must stay at K (compatible records are not
/// re-emitted via mutation) and `recovered_from_source` must land
/// exactly N (no incompatible record is double-counted).
#[test]
fn recover_and_rebind_multi_mixed_cardinality() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // K=5 already-compatible records. `eval_persistent` writes through
    // the current VM version, so each lands as `DecodeOutcome::Loaded`.
    {
        let mut seed_vm = Vm::new();
        for i in 1..=5u32 {
            let source = format!("{} + {}", 100 + 2 * i - 1, 100 + 2 * i);
            let key = format!("clean-{:02}", i);
            fmpl_core::eval_persistent(&mut seed_vm, &source, &store, &source_store, &key)
                .unwrap_or_else(|e| panic!("seed clean record {} failed: {}", key, e));
        }
    }

    // N=10 incompatible records sharing the keyspace.
    for i in 1..=10u32 {
        let source = format!("{} + {}", 2 * i - 1, 2 * i);
        let h = source_store.put(source.as_bytes()).unwrap();
        let key = format!("stale-{:02}", i);
        write(
            &store,
            key.as_bytes(),
            &"stale-payload-bytes",
            PayloadKind::CompiledCode,
            future_vm(),
            h,
        )
        .unwrap();
    }

    let mut vm = Vm::new();
    let stats = fmpl_core::recover_and_rebind(&mut vm, &store, &source_store).unwrap();

    assert_eq!(stats.loaded_passthrough, 5);
    assert_eq!(stats.recovered_from_source, 10);
    assert_eq!(stats.recompile_failed, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.total(), 15);
}
