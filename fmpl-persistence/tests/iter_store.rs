//! Integration tests for `loader::iter_store`.
//!
//! These run end-to-end against a real `FjallStore`. The pure-bytes
//! `decode`/`LoaderStats` tests live in
//! `fmpl-persistence/src/loader.rs::tests` and never touch fjall.
//!
//! These tests are in `tests/` (not in the unit-test module of
//! `loader.rs`) because the `persistence_envelope_invariant` gate scans
//! `fmpl-persistence/src/` for raw `keyspace.insert(`/`partition.insert(`
//! substrings and would flag test-helper inserts as production
//! envelope-bypasses. Keeping the store-touching tests outside
//! `src/` preserves the gate's grep invariant without weakening it.

#![cfg(feature = "fjall-backend")]

use fmpl_core::VM_VERSION_MAJOR;
use fmpl_persistence::envelope::{ENVELOPE_HEADER_SIZE, EnvelopeHeader};
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::loader::{LoaderStats, iter_store};
use fmpl_persistence::schema::PayloadKind;
use fmpl_types::{Hash, VmVersion};
use zerocopy::IntoBytes;
use zerocopy::little_endian::U16;

/// VM version used by tests. Matches the running fmpl-core's
/// `VM_VERSION_MAJOR` so well-formed records load successfully; the
/// minor/patch components are informational and may diverge.
const TEST_VM_VERSION: VmVersion = VmVersion::new(VM_VERSION_MAJOR, 0, 0);

fn fresh_store() -> (tempfile::TempDir, FjallStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("fjall open");
    (dir, store)
}

/// Construct a complete, well-formed envelope value carrying `payload`
/// under `kind`.
fn build_record(kind: PayloadKind, payload: &[u8]) -> Vec<u8> {
    let hdr = EnvelopeHeader::new(TEST_VM_VERSION, kind, payload.len() as u32, Hash::NONE)
        .finalize_checksum(payload);
    let mut value = Vec::with_capacity(ENVELOPE_HEADER_SIZE + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

/// Construct an envelope value whose `vm_version_major` is one ahead of
/// the current build, decoding to `SkippedIncompatible(VmMajorMismatch)`.
fn build_vm_mismatched_record(payload: &[u8]) -> Vec<u8> {
    let mut hdr = EnvelopeHeader::new(
        TEST_VM_VERSION,
        PayloadKind::CompiledCode,
        payload.len() as u32,
        Hash::NONE,
    );
    hdr.vm_version_major = U16::new(VM_VERSION_MAJOR.wrapping_add(1));
    let hdr = hdr.finalize_checksum(payload);
    let mut value = Vec::with_capacity(ENVELOPE_HEADER_SIZE + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

#[test]
fn iter_store_empty_returns_default_stats() {
    let (_dir, store) = fresh_store();
    let mut callback_count = 0;
    let stats = iter_store(&store, VM_VERSION_MAJOR, |_key, _rec| {
        callback_count += 1;
    })
    .expect("iter_store should not fail on empty store");
    assert_eq!(stats, LoaderStats::default());
    assert_eq!(stats.total_processed(), 0);
    assert_eq!(callback_count, 0);
}

#[test]
fn iter_store_single_valid_record_loads() {
    let (_dir, store) = fresh_store();
    let payload = b"single-record-payload";
    let value = build_record(PayloadKind::CompiledCode, payload);
    store.keyspace().insert(b"k1", &value).expect("insert");

    let mut seen_keys: Vec<Vec<u8>> = Vec::new();
    let mut seen_payloads: Vec<Vec<u8>> = Vec::new();
    let stats = iter_store(&store, VM_VERSION_MAJOR, |key, rec| {
        seen_keys.push(key.to_vec());
        seen_payloads.push(rec.payload.to_vec());
    })
    .expect("iter_store ok");

    assert_eq!(stats.loaded, 1);
    assert_eq!(stats.skipped_incompatible, 0);
    assert_eq!(stats.skipped_corrupt, 0);
    assert_eq!(stats.skipped_unknown_kind, 0);
    assert!(stats.check_invariants().is_ok());
    assert_eq!(seen_keys, vec![b"k1".to_vec()]);
    assert_eq!(seen_payloads, vec![payload.to_vec()]);
}

#[test]
fn iter_store_mixed_validity_aggregates_correctly() {
    let (_dir, store) = fresh_store();

    // Valid record.
    let valid_payload = b"valid";
    store
        .keyspace()
        .insert(
            b"k_valid",
            build_record(PayloadKind::CompiledCode, valid_payload),
        )
        .expect("insert");

    // VM major mismatch — counts as SkippedIncompatible/VmMajorMismatch.
    store
        .keyspace()
        .insert(b"k_vm_future", build_vm_mismatched_record(b"future-vm"))
        .expect("insert");

    // Checksum corruption — counts as SkippedCorrupt/ChecksumMismatch.
    let mut corrupt = build_record(PayloadKind::CompiledCode, b"corrupt");
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xFF;
    store
        .keyspace()
        .insert(b"k_corrupt", corrupt)
        .expect("insert");

    let mut callback_payloads: Vec<Vec<u8>> = Vec::new();
    let stats = iter_store(&store, VM_VERSION_MAJOR, |_key, rec| {
        callback_payloads.push(rec.payload.to_vec());
    })
    .expect("iter_store ok");

    // Aggregate counters.
    assert_eq!(stats.loaded, 1);
    assert_eq!(stats.skipped_incompatible, 1);
    assert_eq!(stats.skipped_corrupt, 1);
    assert_eq!(stats.skipped_unknown_kind, 0);
    assert_eq!(stats.total_processed(), 3);

    // Sub-reason histograms pinpoint the exact cause per skip.
    assert_eq!(stats.incompatible_reasons.vm_major_mismatch, 1);
    assert_eq!(stats.incompatible_reasons.unknown_envelope_format, 0);
    assert_eq!(stats.corrupt_reasons.checksum_mismatch, 1);
    assert_eq!(stats.corrupt_reasons.value_too_short, 0);
    assert_eq!(stats.corrupt_reasons.bad_magic, 0);
    assert_eq!(stats.corrupt_reasons.payload_length_mismatch, 0);

    // Aggregate-vs-histogram invariant.
    assert!(stats.check_invariants().is_ok());

    // Callback fires exactly once, for the valid record.
    assert_eq!(callback_payloads, vec![valid_payload.to_vec()]);
}

/// Callback is invoked exactly once per `Loaded` outcome — never
/// for skipped records. Verified by counting in a many-record mix.
#[test]
fn iter_store_callback_fires_only_on_loaded() {
    let (_dir, store) = fresh_store();
    // 3 valid, 2 incompatible — callback must fire exactly 3 times.
    for i in 0..3 {
        let payload = format!("v{}", i);
        store
            .keyspace()
            .insert(
                format!("v_{i}").as_bytes(),
                build_record(PayloadKind::CompiledCode, payload.as_bytes()),
            )
            .expect("insert");
    }
    for i in 0..2 {
        store
            .keyspace()
            .insert(
                format!("bad_{i}").as_bytes(),
                build_vm_mismatched_record(format!("bad-{i}").as_bytes()),
            )
            .expect("insert");
    }

    let mut fires = 0usize;
    let stats =
        iter_store(&store, VM_VERSION_MAJOR, |_key, _rec| fires += 1).expect("iter_store ok");

    assert_eq!(fires, 3);
    assert_eq!(stats.loaded, 3);
    assert_eq!(stats.skipped_incompatible, 2);
    assert_eq!(stats.incompatible_reasons.vm_major_mismatch, 2);
    assert!(stats.check_invariants().is_ok());
}
