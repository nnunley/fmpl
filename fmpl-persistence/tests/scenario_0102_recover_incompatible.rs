//! SCENARIO-0102 — Loader recovers from incompatible payload via
//! source recompilation.
//!
//! Owning stories: STORY-0100 (AC-6).
//! Proof seam: integration.
//!
//! Preconditions:
//! - A keyspace contains a `CompiledCode` record whose envelope has
//!   a known magic but a `schema_version` (here: VM major bumped by
//!   one) the current loader does not understand.
//! - The envelope's `source_hash` resolves to `"1 + 2"` in the
//!   source store.
//!
//! Action:
//! - Run `recover_incompatible` over the keyspace.
//!
//! Expected observables:
//! - The payload decode fails (incompatible).
//! - The loader detects the present `source_hash` and attempts
//!   recovery.
//! - The recovery path resolves the hash, fetches `"1 + 2"`,
//!   recompiles via the caller's closure.
//! - The closure receives the original key + the original source
//!   bytes byte-for-byte.
//! - Stats: `recovered_from_source = 1`, everything else 0.

#![cfg(feature = "fjall-backend")]

use fmpl_persistence::envelope::write;
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::schema::PayloadKind;
use fmpl_persistence::{RecoveryError, SourceStore, recover_incompatible};
use fmpl_types::{Hash, VmVersion};

/// The current VM major version, as far as this test is concerned.
const CURRENT_VM_MAJOR: u16 = 0;
/// A future VM major version one ahead of CURRENT — records stamped
/// with this version decode as `SkippedIncompatible::VmMajorMismatch`.
const FUTURE_VM: VmVersion = VmVersion::new(CURRENT_VM_MAJOR.wrapping_add(1), 0, 0);

#[test]
fn scenario_0102_recover_incompatible_payload_via_source_recompile() {
    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Seed the source store with `"1 + 2"`.
    let source_bytes = b"1 + 2";
    let hash = source_store.put(source_bytes).unwrap();
    assert_ne!(hash, Hash::NONE);

    // Write a CompiledCode envelope stamped with the FUTURE_VM major
    // so the current loader treats it as SkippedIncompatible. The
    // payload is a placeholder string — `recover_incompatible` never
    // looks at the payload bytes on skip outcomes, it only reads the
    // envelope header.
    let key = b"future-record";
    let stale_payload = "stale-payload-bytes-from-an-older-build";
    write(
        &store,
        key,
        &stale_payload,
        PayloadKind::CompiledCode,
        FUTURE_VM,
        hash,
    )
    .unwrap();

    // Caller's recompile closure: record what we receive.
    let mut received_keys: Vec<Vec<u8>> = Vec::new();
    let mut received_sources: Vec<Vec<u8>> = Vec::new();
    let recompile = |key: &[u8], src: &[u8]| -> Result<(), RecoveryError> {
        received_keys.push(key.to_vec());
        received_sources.push(src.to_vec());
        Ok(())
    };

    let stats = recover_incompatible(&store, &source_store, CURRENT_VM_MAJOR, recompile).unwrap();

    // The single record was incompatible + had source + closure
    // succeeded → recovered_from_source = 1. Everything else 0.
    assert_eq!(stats.recovered_from_source, 1);
    assert_eq!(stats.loaded_passthrough, 0);
    assert_eq!(stats.recompile_failed, 0);
    assert_eq!(stats.unrecoverable_no_source, 0);
    assert_eq!(stats.unrecoverable_source_missing, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.total(), 1);

    // Closure observed the right key + the original source bytes.
    assert_eq!(received_keys, vec![key.to_vec()]);
    assert_eq!(received_sources, vec![source_bytes.to_vec()]);
}

/// Composability check: walking the same store twice (once with the
/// happy path via decode, once with recover_incompatible) yields
/// consistent and disjoint coverage. Verifies the recovery pass
/// does not interfere with iter_store and vice versa.
#[test]
fn scenario_0102_composes_with_iter_store_for_full_keyspace_coverage() {
    use fmpl_persistence::loader::iter_store;

    let dir = tempfile::tempdir().unwrap();
    let store = FjallStore::open(dir.path().join("bytecode")).unwrap();
    let source_store = SourceStore::open(dir.path().join("sources")).unwrap();

    // Write two records: one compatible, one incompatible.
    let happy_source = source_store.put(b"happy source").unwrap();
    write(
        &store,
        b"happy",
        &"payload-a",
        PayloadKind::CompiledCode,
        VmVersion::new(CURRENT_VM_MAJOR, 0, 0),
        happy_source,
    )
    .unwrap();

    let stale_source = source_store.put(b"stale source").unwrap();
    write(
        &store,
        b"stale",
        &"payload-b",
        PayloadKind::CompiledCode,
        FUTURE_VM,
        stale_source,
    )
    .unwrap();

    // First pass: iter_store sees the happy record.
    let mut loaded_keys: Vec<Vec<u8>> = Vec::new();
    let loader_stats = iter_store(&store, CURRENT_VM_MAJOR, |key, _record| {
        loaded_keys.push(key.to_vec());
    })
    .unwrap();
    assert_eq!(loader_stats.loaded, 1);
    assert_eq!(loader_stats.skipped_incompatible, 1);
    assert_eq!(loaded_keys, vec![b"happy".to_vec()]);

    // Second pass: recover_incompatible recovers the stale record.
    let mut recovered_keys: Vec<Vec<u8>> = Vec::new();
    let stats = recover_incompatible(&store, &source_store, CURRENT_VM_MAJOR, |key, _| {
        recovered_keys.push(key.to_vec());
        Ok(())
    })
    .unwrap();
    assert_eq!(stats.loaded_passthrough, 1, "happy record passes through");
    assert_eq!(stats.recovered_from_source, 1, "stale record recovers");
    assert_eq!(recovered_keys, vec![b"stale".to_vec()]);

    // Disjoint: the loaded set and recovered set don't overlap.
    let loaded_set: std::collections::HashSet<_> = loaded_keys.into_iter().collect();
    let recovered_set: std::collections::HashSet<_> = recovered_keys.into_iter().collect();
    assert!(
        loaded_set.is_disjoint(&recovered_set),
        "iter_store and recover_incompatible must produce disjoint coverage"
    );
}
