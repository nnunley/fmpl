//! SCENARIO-0099 evidence — Loader skips records with incompatible VM
//! version, unknown payload kind, unknown schema version, nonzero
//! reserved flags, and corrupt checksum without aborting iteration.
//!
//! Two complementary tests live in this file:
//!
//! - `scenario_0099_six_record_skip_journey` (always built): a
//!   decode-pathway integration test. Builds six envelope-encoded
//!   byte values in-memory, feeds each to `decode`, and asserts on
//!   the typed `DecodeOutcome` per record. Counts loaded/skipped in
//!   harness-local `u32` variables — a typed invariant assertion.
//!
//! - `scenario_0099_iter_store_aggregates_stats` (only built with
//!   `--features fjall-backend`): a store-iteration test. The
//!   same six records are inserted into a real `FjallStore`, then
//!   `loader::iter_store` is invoked. Asserts on the public
//!   `LoaderStats` (aggregate counters + sub-reason histograms) so
//!   the entire skip taxonomy is observable to an operator via the
//!   sanctioned API.
//!
//! The decode-pathway test is preserved alongside the iter-store
//! test because the two cover different seams: `decode` (the
//! per-record classification function) vs `iter_store` (the
//! store traversal that aggregates classifications into stats).

use fmpl_core::VM_VERSION_MAJOR;
use fmpl_persistence::envelope::EnvelopeHeader;
use fmpl_persistence::loader::{
    CorruptionReason, DecodeOutcome, IncompatibilityReason, UnknownKindReason, decode,
};
use fmpl_persistence::schema::PayloadKind;
use fmpl_types::{Hash, VmVersion};
use zerocopy::IntoBytes;
use zerocopy::little_endian::U16;

/// VM version used for tests — the running fmpl-core's compile-time
/// version. Production callers pass `fmpl_core::VM_VERSION`.
fn test_vm_version() -> VmVersion {
    fmpl_core::VM_VERSION
}

/// Construct a complete, well-formed envelope value for `kind` carrying
/// `payload`.
fn build_record(kind: PayloadKind, payload: &[u8]) -> Vec<u8> {
    let hdr = EnvelopeHeader::new(test_vm_version(), kind, payload.len() as u32, Hash::NONE)
        .finalize_checksum(payload);
    let mut value = Vec::with_capacity(56 + payload.len());
    value.extend_from_slice(hdr.as_bytes());
    value.extend_from_slice(payload);
    value
}

#[test]
fn scenario_0099_six_record_skip_journey() {
    // Six records cover every taxonomy class the loader must classify:
    //
    // - Record A: well-formed → DecodeOutcome::Loaded.
    // - Record B: vm_version major one ahead → SkippedIncompatible(VmMajorMismatch).
    // - Record C: unknown payload_kind → SkippedUnknownKind(UnknownPayloadKind).
    // - Record D: corrupted payload byte → SkippedCorrupt(ChecksumMismatch).
    // - Record E: unknown schema_version for a known kind → SkippedUnknownKind(UnknownSchemaVersion).
    // - Record F: nonzero reserved flags → SkippedUnknownKind(NonzeroReservedFlags).
    //
    // The three SkippedUnknownKind sub-cases assert on the specific
    // sub-reason variant per record, not just the umbrella outcome,
    // so a regression that collapsed the three sub-reasons would be
    // caught here.

    // Record A — well-formed current VM, current schema.
    let record_a = build_record(PayloadKind::CompiledCode, b"payload A");

    // Record B — vm_version major one ahead. Construct manually so the
    // header carries a future major; finalize_checksum still uses our
    // current `compute()` so the checksum is well-formed for the header
    // as written.
    let record_b = {
        let payload = b"payload B";
        let mut hdr = EnvelopeHeader::new(
            test_vm_version(),
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.vm_version_major = U16::new(VM_VERSION_MAJOR.wrapping_add(1));
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::with_capacity(56 + payload.len());
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        value
    };

    // Record C — unknown payload_kind.
    let record_c = {
        let payload = b"payload C";
        let mut hdr = EnvelopeHeader::new(
            test_vm_version(),
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.payload_kind = 0xEE; // not in the taxonomy
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::with_capacity(56 + payload.len());
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        value
    };

    // Record D — CRC32 deliberately corrupted (tamper with a payload
    // byte; the stamped checksum no longer matches).
    let record_d = {
        let mut value = build_record(PayloadKind::CompiledCode, b"payload D");
        let last = value.len() - 1;
        value[last] ^= 0xFF;
        value
    };

    // Record E — unknown schema_version for a known payload_kind.
    let record_e = {
        let payload = b"payload E";
        let mut hdr = EnvelopeHeader::new(
            test_vm_version(),
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        // Bump schema_version to a far-future value the current
        // PayloadKind::CompiledCode does not recognize.
        hdr.schema_version = U16::new(0xFFFF);
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::with_capacity(56 + payload.len());
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        value
    };

    // Record F — nonzero reserved flags (reserved-must-be-zero).
    let record_f = {
        let payload = b"payload F";
        let mut hdr = EnvelopeHeader::new(
            test_vm_version(),
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.flags = 0x01;
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::with_capacity(56 + payload.len());
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        value
    };

    // Simulate a store iterator yielding these six records in order.
    let records: [(&str, &[u8]); 6] = [
        ("a", &record_a),
        ("b", &record_b),
        ("c", &record_c),
        ("d", &record_d),
        ("e", &record_e),
        ("f", &record_f),
    ];

    // Harness-local counters. The iter-store variant below
    // proves the same observables via the public `LoaderStats` API.
    let mut loaded: u32 = 0;
    let mut skipped_incompatible: u32 = 0;
    let mut skipped_unknown_kind: u32 = 0;
    let mut skipped_corrupt: u32 = 0;

    for (key, value) in records {
        let (outcome, decoded) = decode(value, VM_VERSION_MAJOR);
        match outcome {
            DecodeOutcome::Loaded => {
                let rec = decoded.expect("loaded record yields DecodedRecord");
                // Sanity: only record A should reach here.
                assert_eq!(key, "a");
                assert_eq!(rec.kind, PayloadKind::CompiledCode);
                assert_eq!(rec.payload, b"payload A");
                loaded += 1;
            }
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::VmMajorMismatch) => {
                assert_eq!(key, "b");
                assert!(decoded.is_none());
                skipped_incompatible += 1;
            }
            DecodeOutcome::SkippedIncompatible(other) => {
                panic!("record {key} skipped incompatible for unexpected reason: {other:?}");
            }
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownPayloadKind) => {
                assert_eq!(key, "c");
                skipped_unknown_kind += 1;
            }
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownSchemaVersion) => {
                assert_eq!(key, "e");
                skipped_unknown_kind += 1;
            }
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::NonzeroReservedFlags) => {
                assert_eq!(key, "f");
                skipped_unknown_kind += 1;
            }
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ChecksumMismatch) => {
                assert_eq!(key, "d");
                skipped_corrupt += 1;
            }
            DecodeOutcome::SkippedCorrupt(other) => {
                panic!("record {key} skipped corrupt for unexpected reason: {other:?}");
            }
        }
    }

    // SCENARIO-0099's expected observables.
    assert_eq!(loaded, 1, "exactly one record (A) should load");
    assert_eq!(
        skipped_incompatible, 1,
        "exactly one record (B) should skip-incompatible"
    );
    assert_eq!(
        skipped_unknown_kind, 3,
        "exactly three records (C, E, F) should skip-unknown-kind"
    );
    assert_eq!(
        skipped_corrupt, 1,
        "exactly one record (D) should skip-corrupt"
    );
}

// ---- iter_store pathway: same six records, public LoaderStats API ----

#[cfg(feature = "fjall-backend")]
mod iter_store_pathway {
    use super::*;
    use fmpl_persistence::Store;
    use fmpl_persistence::fjall_backend::FjallStore;
    use fmpl_persistence::loader::iter_store;

    /// Open a fresh `FjallStore` on a tempdir for this scenario.
    fn fresh_store() -> (tempfile::TempDir, FjallStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FjallStore::open(dir.path()).expect("FjallStore open");
        (dir, store)
    }

    /// Construct the same six-record corpus as the decode-pathway test,
    /// returned as `(key, value)` pairs ready for `store.insert`.
    fn six_record_corpus() -> Vec<(&'static [u8], Vec<u8>)> {
        // Record A — well-formed.
        let record_a = build_record(PayloadKind::CompiledCode, b"payload A");

        // Record B — vm_version major one ahead.
        let record_b = {
            let payload = b"payload B";
            let mut hdr = EnvelopeHeader::new(
                test_vm_version(),
                PayloadKind::CompiledCode,
                payload.len() as u32,
                Hash::NONE,
            );
            hdr.vm_version_major = U16::new(VM_VERSION_MAJOR.wrapping_add(1));
            let hdr = hdr.finalize_checksum(payload);
            let mut value = Vec::with_capacity(56 + payload.len());
            value.extend_from_slice(hdr.as_bytes());
            value.extend_from_slice(payload);
            value
        };

        // Record C — unknown payload_kind.
        let record_c = {
            let payload = b"payload C";
            let mut hdr = EnvelopeHeader::new(
                test_vm_version(),
                PayloadKind::CompiledCode,
                payload.len() as u32,
                Hash::NONE,
            );
            hdr.payload_kind = 0xEE;
            let hdr = hdr.finalize_checksum(payload);
            let mut value = Vec::with_capacity(56 + payload.len());
            value.extend_from_slice(hdr.as_bytes());
            value.extend_from_slice(payload);
            value
        };

        // Record D — corrupted last byte.
        let record_d = {
            let mut value = build_record(PayloadKind::CompiledCode, b"payload D");
            let last = value.len() - 1;
            value[last] ^= 0xFF;
            value
        };

        // Record E — unknown schema_version for a known kind.
        let record_e = {
            let payload = b"payload E";
            let mut hdr = EnvelopeHeader::new(
                test_vm_version(),
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
        };

        // Record F — nonzero reserved flags.
        let record_f = {
            let payload = b"payload F";
            let mut hdr = EnvelopeHeader::new(
                test_vm_version(),
                PayloadKind::CompiledCode,
                payload.len() as u32,
                Hash::NONE,
            );
            hdr.flags = 0x01;
            let hdr = hdr.finalize_checksum(payload);
            let mut value = Vec::with_capacity(56 + payload.len());
            value.extend_from_slice(hdr.as_bytes());
            value.extend_from_slice(payload);
            value
        };

        vec![
            (b"a".as_slice(), record_a),
            (b"b".as_slice(), record_b),
            (b"c".as_slice(), record_c),
            (b"d".as_slice(), record_d),
            (b"e".as_slice(), record_e),
            (b"f".as_slice(), record_f),
        ]
    }

    #[test]
    fn scenario_0099_iter_store_aggregates_stats() {
        let (_dir, store) = fresh_store();
        for (key, value) in six_record_corpus() {
            store.insert(key, &value).expect("insert");
        }

        // Capture which keys triggered the callback. Only record A
        // (the well-formed one) should arrive here.
        let mut loaded_keys: Vec<Vec<u8>> = Vec::new();
        let mut loaded_payloads: Vec<Vec<u8>> = Vec::new();
        let stats = iter_store(&store, VM_VERSION_MAJOR, |key, rec| {
            loaded_keys.push(key.to_vec());
            loaded_payloads.push(rec.payload.to_vec());
        })
        .expect("iter_store ok");

        // Aggregate counters — same observables as the decode-pathway test.
        assert_eq!(stats.loaded, 1, "exactly record A loads");
        assert_eq!(stats.skipped_incompatible, 1, "record B is incompatible");
        assert_eq!(
            stats.skipped_unknown_kind, 3,
            "records C, E, F are unknown-kind"
        );
        assert_eq!(stats.skipped_corrupt, 1, "record D is corrupt");
        assert_eq!(stats.total_processed(), 6);
        assert!(stats.check_invariants().is_ok());

        // Sub-reason histograms — pinpoint the exact sub-reason per skip.
        assert_eq!(stats.incompatible_reasons.vm_major_mismatch, 1);
        assert_eq!(stats.incompatible_reasons.unknown_envelope_format, 0);
        assert_eq!(stats.unknown_kind_reasons.unknown_payload_kind, 1);
        assert_eq!(stats.unknown_kind_reasons.unknown_schema_version, 1);
        assert_eq!(stats.unknown_kind_reasons.nonzero_reserved_flags, 1);
        assert_eq!(stats.corrupt_reasons.checksum_mismatch, 1);
        assert_eq!(stats.corrupt_reasons.value_too_short, 0);
        assert_eq!(stats.corrupt_reasons.bad_magic, 0);
        assert_eq!(stats.corrupt_reasons.payload_length_mismatch, 0);

        // Callback was invoked exactly once, for record A.
        assert_eq!(loaded_keys, vec![b"a".to_vec()]);
        assert_eq!(loaded_payloads, vec![b"payload A".to_vec()]);
    }
}
