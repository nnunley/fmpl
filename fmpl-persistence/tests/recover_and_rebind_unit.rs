//! Unit-style integration tests for `fmpl_core::recover_and_rebind`.
//!
//! These exercise the orchestrator's contract in isolation from the
//! full SCENARIO-0102 journey:
//! - it forwards an incompatible record's source bytes to a fresh
//!   `eval_persistent` call and counts it as `recovered_from_source`;
//! - it propagates UTF-8 errors on the iteration key through
//!   `RecoveryError::recompile(...)` into `recompile_failed`.
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
    assert_eq!(stats.total(), 1);
}
