//! SCENARIO-0112 evidence — Operators can detect silent data loss
//! after a VM upgrade by inspecting `LoaderStats` sub-reason histograms.
//!
//! The narrative under test:
//!
//!     Pre-upgrade: a store contains records that all loaded cleanly.
//!     Post-upgrade: the same store is opened by a new VM build.
//!     Some records now skip — incompatibility, schema drift, disk
//!     corruption, etc. — and a naive read path silently drops them.
//!
//!     The public `LoaderStats` returned by `iter_store` MUST let an
//!     operator pinpoint each cause: not just that 5 records dropped,
//!     but that 2 were disk-corruption (call sysadmin), 2 were
//!     post-upgrade schema drift (recompile / re-extract), 1 was VM
//!     incompatibility (downgrade or migrate), etc.
//!
//! The test constructs a mixed-validity corpus designed to exercise
//! THREE distinct operator-actionable signals at once and asserts the
//! histogram pinpoints each. The aggregate-only counters cannot
//! distinguish these signals — the sub-reason histograms are the
//! proof.

#![cfg(feature = "fjall-backend")]

use fmpl_core::{VM_VERSION, VM_VERSION_MAJOR};
use fmpl_persistence::Store;
use fmpl_persistence::envelope::EnvelopeHeader;
use fmpl_persistence::fjall_backend::FjallStore;
use fmpl_persistence::loader::iter_store;
use fmpl_persistence::schema::PayloadKind;
use fmpl_types::{Hash, VmVersion};
use zerocopy::IntoBytes;
use zerocopy::little_endian::U16;

fn fresh_store() -> (tempfile::TempDir, FjallStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("fjall open");
    (dir, store)
}

fn well_formed(kind: PayloadKind, payload: &[u8]) -> Vec<u8> {
    let hdr = EnvelopeHeader::new(VM_VERSION, kind, payload.len() as u32, Hash::NONE)
        .finalize_checksum(payload);
    let mut value = Vec::with_capacity(56 + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

/// Returns a record whose `vm_version_major` is one ahead of this
/// build. Simulates a record written by a newer VM the operator hasn't
/// rolled out yet (or that this build is a regression from).
fn vm_future(payload: &[u8]) -> Vec<u8> {
    let future_version = VmVersion::new(
        VM_VERSION_MAJOR.wrapping_add(1),
        VM_VERSION.minor,
        VM_VERSION.patch,
    );
    let hdr = EnvelopeHeader::new(
        future_version,
        PayloadKind::CompiledCode,
        payload.len() as u32,
        Hash::NONE,
    )
    .finalize_checksum(payload);
    let mut value = Vec::with_capacity(56 + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

/// Returns a record whose `schema_version` is `0xFFFF` for a known
/// kind. Simulates post-upgrade schema drift: the kind is recognized
/// but its on-disk version is from a future schema this build doesn't
/// know how to interpret.
fn schema_drift(payload: &[u8]) -> Vec<u8> {
    let mut hdr = EnvelopeHeader::new(
        VM_VERSION,
        PayloadKind::CompiledCode,
        payload.len() as u32,
        Hash::NONE,
    );
    hdr.schema_version = U16::new(0xFFFF);
    let hdr = hdr.finalize_checksum(payload);
    let mut value = Vec::with_capacity(56 + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

/// Returns a well-formed record with its last payload byte XORed,
/// breaking the stamped CRC. Simulates on-disk bit-rot.
fn disk_corruption(payload: &[u8]) -> Vec<u8> {
    let mut value = well_formed(PayloadKind::CompiledCode, payload);
    let last = value.len() - 1;
    value[last] ^= 0xFF;
    value
}

#[test]
fn scenario_0112_operator_detects_silent_data_loss() {
    let (_dir, store) = fresh_store();

    // The corpus:
    //   3 valid records (would have loaded pre-upgrade — still load now).
    //   2 vm-major-future records (incompatibility — call admin to downgrade).
    //   2 unknown-schema-version records (schema drift — re-extract / recompile).
    //   1 checksum-corrupt record (disk corruption — call sysadmin).
    //
    // Aggregates only:
    //   loaded=3, skipped=5
    // … tells the operator NOTHING about the cause.
    //
    // Histograms pinpoint:
    //   incompatible_reasons.vm_major_mismatch=2
    //   unknown_kind_reasons.unknown_schema_version=2
    //   corrupt_reasons.checksum_mismatch=1
    // … each of which dispatches to a different operator runbook.

    for i in 0..3 {
        let key = format!("valid_{i}");
        let payload = format!("valid-{i}");
        store
            .insert(
                key.as_bytes(),
                &well_formed(PayloadKind::CompiledCode, payload.as_bytes()),
            )
            .unwrap();
    }
    for i in 0..2 {
        let key = format!("vm_future_{i}");
        let payload = format!("vm-future-{i}");
        store
            .insert(key.as_bytes(), &vm_future(payload.as_bytes()))
            .unwrap();
    }
    for i in 0..2 {
        let key = format!("schema_drift_{i}");
        let payload = format!("schema-drift-{i}");
        store
            .insert(key.as_bytes(), &schema_drift(payload.as_bytes()))
            .unwrap();
    }
    store
        .insert(b"disk_corrupt", &disk_corruption(b"corrupt-record"))
        .unwrap();

    let stats = iter_store(&store, VM_VERSION_MAJOR, |_key, _rec| {}).expect("iter_store ok");

    // Aggregate observables.
    assert_eq!(stats.loaded, 3);
    assert_eq!(stats.skipped_incompatible, 2);
    assert_eq!(stats.skipped_unknown_kind, 2);
    assert_eq!(stats.skipped_corrupt, 1);
    assert_eq!(stats.total_processed(), 8);
    assert!(stats.check_invariants().is_ok());

    // Sub-reason histograms — the operator-actionable signal that
    // distinguishes "5 records dropped" from "2 disk corruption, 2
    // schema drift, 1 VM incompatibility".
    assert_eq!(
        stats.incompatible_reasons.vm_major_mismatch, 2,
        "VM-major-mismatch is the dispatch key for 'downgrade or migrate'"
    );
    assert_eq!(stats.incompatible_reasons.unknown_envelope_format, 0);

    assert_eq!(
        stats.unknown_kind_reasons.unknown_schema_version, 2,
        "unknown_schema_version is the dispatch key for 'recompile / re-extract'"
    );
    assert_eq!(stats.unknown_kind_reasons.unknown_payload_kind, 0);
    assert_eq!(stats.unknown_kind_reasons.nonzero_reserved_flags, 0);

    assert_eq!(
        stats.corrupt_reasons.checksum_mismatch, 1,
        "checksum_mismatch is the dispatch key for 'call sysadmin'"
    );
    assert_eq!(stats.corrupt_reasons.value_too_short, 0);
    assert_eq!(stats.corrupt_reasons.bad_magic, 0);
    assert_eq!(stats.corrupt_reasons.payload_length_mismatch, 0);
}

/// Aggregate-only observability fails the operator-actionability test:
/// two distinct corpora can produce the same `(loaded, skipped_*)`
/// totals while requiring entirely different operator responses. The
/// histograms are what makes them distinguishable. This test pins
/// that property: corpus X and corpus Y agree on aggregates but
/// disagree on histograms.
#[test]
fn scenario_0112_histograms_distinguish_isomorphic_aggregates() {
    let (_dir_x, store_x) = fresh_store();
    let (_dir_y, store_y) = fresh_store();

    // Corpus X: 3 corrupt records (all checksum-mismatch).
    for i in 0..3 {
        let key = format!("x_{i}");
        let payload = format!("x-{i}");
        store_x
            .insert(key.as_bytes(), &disk_corruption(payload.as_bytes()))
            .unwrap();
    }

    // Corpus Y: 1 checksum, 1 magic-corrupt, 1 length-mismatch.
    store_y
        .insert(b"checksum", &disk_corruption(b"y0"))
        .unwrap();

    let mut magic_corrupt = well_formed(PayloadKind::CompiledCode, b"y1");
    magic_corrupt[0] = b'X'; // tamper with magic
    store_y.insert(b"magic", &magic_corrupt).unwrap();

    let mut length_mismatch = well_formed(PayloadKind::CompiledCode, b"y2-padded");
    length_mismatch.pop(); // truncate payload by 1
    store_y.insert(b"length", &length_mismatch).unwrap();

    let stats_x = iter_store(&store_x, VM_VERSION_MAJOR, |_, _| {}).expect("iter X");
    let stats_y = iter_store(&store_y, VM_VERSION_MAJOR, |_, _| {}).expect("iter Y");

    // Aggregates agree.
    assert_eq!(stats_x.loaded, stats_y.loaded);
    assert_eq!(stats_x.skipped_corrupt, stats_y.skipped_corrupt);
    assert_eq!(stats_x.skipped_corrupt, 3);

    // Histograms disagree — this is the point of the test.
    assert_eq!(stats_x.corrupt_reasons.checksum_mismatch, 3);
    assert_eq!(stats_x.corrupt_reasons.bad_magic, 0);
    assert_eq!(stats_x.corrupt_reasons.payload_length_mismatch, 0);

    assert_eq!(stats_y.corrupt_reasons.checksum_mismatch, 1);
    assert_eq!(stats_y.corrupt_reasons.bad_magic, 1);
    assert_eq!(stats_y.corrupt_reasons.payload_length_mismatch, 1);

    assert_ne!(stats_x.corrupt_reasons, stats_y.corrupt_reasons);
}
