//! Envelope-aware loader for persisted keyspaces.
//!
//! Iterates a Fjall keyspace, decodes each value as an
//! [`EnvelopeHeader`]-prefixed record, and routes each entry into one of:
//!
//! - **Loaded** — magic matches, format version recognized, VM major
//!   matches, `flags == 0`, payload kind known with current schema
//!   version, checksum verifies. Header + payload are exposed by
//!   reference to the caller.
//! - **Skipped (incompatible)** — VM major mismatch or envelope format
//!   version unrecognized. These are "future" records this build cannot
//!   safely interpret; the writer that produced them used a newer
//!   binary format.
//! - **Skipped (unknown kind)** — unknown `payload_kind`, unknown
//!   schema version for a known kind, or nonzero reserved `flags`.
//!   These are records of a category this build doesn't recognize.
//! - **Skipped (corrupt)** — magic mismatch, value shorter than the
//!   fixed-size envelope header, declared payload length doesn't match
//!   the actual byte length, or checksum mismatch.
//!
//! Iteration is non-fatal on per-record errors: skips are recorded into
//! [`LoaderStats`] and the next entry is processed. Only fjall iterator
//! errors abort. No byte arithmetic outside the header strip — each
//! fjall value is a self-contained envelope-prefixed record.
//!
//! [`EnvelopeHeader`]: super::envelope::EnvelopeHeader

use super::envelope::{ENVELOPE_HEADER_SIZE, EnvelopeHeader};
use super::schema::{ENVELOPE_FORMAT_VERSION, PayloadKind};
use crate::{Store, StoreError};
use zerocopy::FromBytes;

/// Outcome classifying one keyspace entry.
///
/// When [`Loaded`](Self::Loaded), [`decode`] returns the borrowed
/// header + payload alongside this outcome. For any skip variant, the
/// payload is **not** exposed: the caller must not act on the bytes
/// because at least one envelope precondition was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeOutcome {
    /// Header passed every validation and checksum verified. The
    /// payload bytes are safe to feed to the schema's deserializer.
    Loaded,
    /// Record exists but this build cannot interpret it: it was
    /// written by an incompatible VM major or envelope format.
    SkippedIncompatible(IncompatibilityReason),
    /// Record exists but its category is unknown to this build:
    /// unknown payload kind, unknown schema version, or nonzero
    /// reserved `flags`.
    SkippedUnknownKind(UnknownKindReason),
    /// Record is malformed: magic mismatch, length mismatch, or
    /// checksum mismatch. The bytes on disk are not a valid envelope.
    SkippedCorrupt(CorruptionReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompatibilityReason {
    /// VM major version on disk differs from this build's
    /// `VM_VERSION_MAJOR`.
    VmMajorMismatch,
    /// Envelope format version on disk is not 1 (or whatever the
    /// current `ENVELOPE_FORMAT_VERSION` is).
    UnknownEnvelopeFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownKindReason {
    /// `payload_kind` byte does not map to any known `PayloadKind`.
    UnknownPayloadKind,
    /// `schema_version` does not match the current value for the
    /// known `payload_kind`.
    UnknownSchemaVersion,
    /// `flags` byte is nonzero (reserved must-be-zero).
    NonzeroReservedFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorruptionReason {
    /// Value shorter than [`ENVELOPE_HEADER_SIZE`].
    ValueTooShort,
    /// Magic bytes are not `b"FMPL"`.
    BadMagic,
    /// `header.payload_len` does not equal `value.len() - 56`.
    PayloadLengthMismatch,
    /// Stamped CRC field doesn't match the recomputed blake3 truncation.
    ChecksumMismatch,
}

/// Decode a single keyspace value into a typed outcome plus, when
/// `Loaded`, a borrowed header + payload pair.
///
/// The VM-major compatibility check uses `expected_vm_major` (typically
/// `fmpl_core::VM_VERSION_MAJOR`) rather than a const stamped at this
/// crate's build time. Per ITER-0005a.5 R3-C1: this crate is
/// version-agnostic; the running VM's identity flows in by parameter.
///
/// Borrowing rather than copying preserves the zero-copy benefit of the
/// [`zerocopy`]-derived header. Callers needing an owned header should
/// `*` the reference.
pub fn decode<'v>(
    value: &'v [u8],
    expected_vm_major: u16,
) -> (DecodeOutcome, Option<DecodedRecord<'v>>) {
    if value.len() < ENVELOPE_HEADER_SIZE {
        return (
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ValueTooShort),
            None,
        );
    }

    let Ok((header, payload)) = EnvelopeHeader::ref_from_prefix(value) else {
        // ref_from_prefix only fails on size; we already checked. This
        // arm exists for defensive completeness.
        return (
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ValueTooShort),
            None,
        );
    };

    if header.magic != super::envelope::MAGIC {
        return (
            DecodeOutcome::SkippedCorrupt(CorruptionReason::BadMagic),
            None,
        );
    }

    if header.envelope_format_version.get() != ENVELOPE_FORMAT_VERSION {
        return (
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::UnknownEnvelopeFormat),
            None,
        );
    }

    if header.vm_version_major.get() != expected_vm_major {
        return (
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::VmMajorMismatch),
            None,
        );
    }

    if header.flags != 0 {
        return (
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::NonzeroReservedFlags),
            None,
        );
    }

    let Some(kind) = PayloadKind::from_byte(header.payload_kind) else {
        return (
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownPayloadKind),
            None,
        );
    };

    if header.schema_version.get() != kind.current_schema_version() {
        return (
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownSchemaVersion),
            None,
        );
    }

    if header.payload_len.get() as usize != payload.len() {
        return (
            DecodeOutcome::SkippedCorrupt(CorruptionReason::PayloadLengthMismatch),
            None,
        );
    }

    if !header.verify_checksum(payload) {
        return (
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ChecksumMismatch),
            None,
        );
    }

    (
        DecodeOutcome::Loaded,
        Some(DecodedRecord {
            header,
            payload,
            kind,
        }),
    )
}

/// Borrowed header + payload from a successfully decoded record.
#[derive(Debug)]
pub struct DecodedRecord<'v> {
    pub header: &'v EnvelopeHeader,
    pub payload: &'v [u8],
    pub kind: PayloadKind,
}

// =========================================================================
// Per-keyspace loader statistics
// =========================================================================

/// Per-skip-reason histogram. One `u32` counter per
/// [`IncompatibilityReason`] / [`UnknownKindReason`] / [`CorruptionReason`]
/// variant. The sum of all counters in a histogram equals the aggregate
/// counter in [`LoaderStats`] for the same category.
///
/// Histograms preserve the sub-reasons of [`DecodeOutcome`] so operators
/// can distinguish "5 records all checksum-mismatch" (disk corruption
/// signal) from "5 records all unknown-schema-version" (post-upgrade
/// schema drift signal) without losing data in the aggregate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IncompatibilityReasonCounts {
    /// VM major version on disk differs from this build's
    /// [`VM_VERSION_MAJOR`]. Operationally: writer was a different
    /// VM generation; reader cannot interpret the payload safely.
    pub vm_major_mismatch: u32,
    /// Envelope format version on disk is not [`ENVELOPE_FORMAT_VERSION`].
    pub unknown_envelope_format: u32,
}

impl IncompatibilityReasonCounts {
    fn record(&mut self, reason: IncompatibilityReason) {
        match reason {
            IncompatibilityReason::VmMajorMismatch => self.vm_major_mismatch += 1,
            IncompatibilityReason::UnknownEnvelopeFormat => self.unknown_envelope_format += 1,
        }
    }

    /// Sum of all sub-reason counters (must equal the aggregate counter
    /// in [`LoaderStats::skipped_incompatible`]).
    pub fn total(&self) -> u32 {
        self.vm_major_mismatch + self.unknown_envelope_format
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnknownKindReasonCounts {
    /// `payload_kind` byte does not map to any known [`PayloadKind`].
    pub unknown_payload_kind: u32,
    /// `schema_version` does not match the current value for a known kind.
    pub unknown_schema_version: u32,
    /// `flags` byte is nonzero. The reserved flags field must be zero
    /// in this envelope version; a nonzero value signals a future
    /// extension this build does not understand.
    pub nonzero_reserved_flags: u32,
}

impl UnknownKindReasonCounts {
    fn record(&mut self, reason: UnknownKindReason) {
        match reason {
            UnknownKindReason::UnknownPayloadKind => self.unknown_payload_kind += 1,
            UnknownKindReason::UnknownSchemaVersion => self.unknown_schema_version += 1,
            UnknownKindReason::NonzeroReservedFlags => self.nonzero_reserved_flags += 1,
        }
    }

    pub fn total(&self) -> u32 {
        self.unknown_payload_kind + self.unknown_schema_version + self.nonzero_reserved_flags
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CorruptionReasonCounts {
    /// Value shorter than [`ENVELOPE_HEADER_SIZE`].
    pub value_too_short: u32,
    /// Magic bytes are not `b"FMPL"`.
    pub bad_magic: u32,
    /// `header.payload_len` does not equal `value.len() - 56`.
    pub payload_length_mismatch: u32,
    /// Stamped CRC field doesn't match the recomputed blake3 truncation.
    pub checksum_mismatch: u32,
}

impl CorruptionReasonCounts {
    fn record(&mut self, reason: CorruptionReason) {
        match reason {
            CorruptionReason::ValueTooShort => self.value_too_short += 1,
            CorruptionReason::BadMagic => self.bad_magic += 1,
            CorruptionReason::PayloadLengthMismatch => self.payload_length_mismatch += 1,
            CorruptionReason::ChecksumMismatch => self.checksum_mismatch += 1,
        }
    }

    pub fn total(&self) -> u32 {
        self.value_too_short
            + self.bad_magic
            + self.payload_length_mismatch
            + self.checksum_mismatch
    }
}

/// Per-keyspace loader statistics.
///
/// Headline aggregate counters (`loaded`, `skipped_incompatible`,
/// `skipped_corrupt`, `skipped_unknown_kind`) plus per-sub-reason
/// histograms so operators can pinpoint silent data loss after a VM
/// upgrade. The aggregate counters equal the totals of their
/// corresponding histograms (invariant; see
/// [`LoaderStats::check_invariants`]).
///
/// Returned by [`iter_keyspace`]. The point-key load paths
/// (`CompiledCode::load_from_fjall`, `Object::load_from_fjall`, etc.)
/// will gain `&mut LoaderStats` accumulator parameters in a future
/// iteration; until then those paths construct their own
/// `LoaderStats::default()` per call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LoaderStats {
    /// Aggregate count of records that decoded cleanly to [`DecodeOutcome::Loaded`].
    pub loaded: u32,
    /// Aggregate count of records skipped via [`DecodeOutcome::SkippedIncompatible`].
    /// Equals `incompatible_reasons.total()`.
    pub skipped_incompatible: u32,
    /// Aggregate count of records skipped via [`DecodeOutcome::SkippedCorrupt`].
    /// Equals `corrupt_reasons.total()`.
    pub skipped_corrupt: u32,
    /// Aggregate count of records skipped via [`DecodeOutcome::SkippedUnknownKind`].
    /// Equals `unknown_kind_reasons.total()`.
    pub skipped_unknown_kind: u32,

    /// Per-sub-reason histogram for `SkippedIncompatible` outcomes.
    pub incompatible_reasons: IncompatibilityReasonCounts,
    /// Per-sub-reason histogram for `SkippedCorrupt` outcomes.
    pub corrupt_reasons: CorruptionReasonCounts,
    /// Per-sub-reason histogram for `SkippedUnknownKind` outcomes.
    pub unknown_kind_reasons: UnknownKindReasonCounts,
}

impl LoaderStats {
    /// Total records processed (loaded + skipped).
    pub fn total_processed(&self) -> u32 {
        self.loaded + self.skipped_incompatible + self.skipped_corrupt + self.skipped_unknown_kind
    }

    /// Verify the aggregate counters equal their sub-reason histogram
    /// totals. Returns `Ok(())` if consistent.
    pub fn check_invariants(&self) -> Result<(), &'static str> {
        if self.skipped_incompatible != self.incompatible_reasons.total() {
            return Err("skipped_incompatible != incompatible_reasons.total()");
        }
        if self.skipped_corrupt != self.corrupt_reasons.total() {
            return Err("skipped_corrupt != corrupt_reasons.total()");
        }
        if self.skipped_unknown_kind != self.unknown_kind_reasons.total() {
            return Err("skipped_unknown_kind != unknown_kind_reasons.total()");
        }
        Ok(())
    }

    /// Record a single decode outcome into the stats.
    pub fn record(&mut self, outcome: DecodeOutcome) {
        match outcome {
            DecodeOutcome::Loaded => self.loaded += 1,
            DecodeOutcome::SkippedIncompatible(reason) => {
                self.skipped_incompatible += 1;
                self.incompatible_reasons.record(reason);
            }
            DecodeOutcome::SkippedCorrupt(reason) => {
                self.skipped_corrupt += 1;
                self.corrupt_reasons.record(reason);
            }
            DecodeOutcome::SkippedUnknownKind(reason) => {
                self.skipped_unknown_kind += 1;
                self.unknown_kind_reasons.record(reason);
            }
        }
    }
}

/// Iterate a store, decode each value via [`decode`], invoke
/// `on_record` for `DecodeOutcome::Loaded` outcomes, accumulate stats
/// into [`LoaderStats`]. Returns the accumulated stats at
/// end-of-iteration or propagates `StoreError` from the underlying
/// iterator.
///
/// `expected_vm_major` is the running VM's major version (typically
/// `fmpl_core::VM_VERSION_MAJOR`); records whose stamped major differs
/// from this are skipped as incompatible. Per ITER-0005a.5 R3-C1.
///
/// The callback's `&[u8]` (key) and `DecodedRecord<'_>` (header +
/// payload) borrows live within one iteration step. `on_record` fires
/// only on `Loaded`. Skip outcomes are recorded into `LoaderStats`
/// without callback invocation.
pub fn iter_store<S: Store + ?Sized, F>(
    store: &S,
    expected_vm_major: u16,
    mut on_record: F,
) -> Result<LoaderStats, StoreError>
where
    F: FnMut(&[u8], DecodedRecord<'_>),
{
    let mut stats = LoaderStats::default();
    for item in store.iter() {
        let (key, value) = item?;
        let (outcome, decoded) = decode(&value, expected_vm_major);
        stats.record(outcome);
        if let DecodeOutcome::Loaded = outcome
            && let Some(record) = decoded
        {
            on_record(&key, record);
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::super::envelope::EnvelopeHeader;
    use super::super::schema::PayloadKind;
    use super::*;
    use fmpl_types::{Hash, VmVersion};
    use zerocopy::IntoBytes;
    use zerocopy::little_endian::U16;

    // Test VM version. Real callers pass fmpl_core::VM_VERSION.
    const TEST_VM_VERSION: VmVersion = VmVersion::new(0, 1, 0);
    // Major-version-mismatch case stamps records with a major one ahead
    // of TEST_VM_VERSION.major so decode() returns SkippedIncompatible.
    const TEST_VM_VERSION_AHEAD: VmVersion =
        VmVersion::new(TEST_VM_VERSION.major.wrapping_add(1), 1, 0);

    /// Build a complete envelope value (header + payload) for a given
    /// kind. Used by tests to construct synthetic records the loader
    /// iterates over.
    fn build_record(kind: PayloadKind, payload: &[u8]) -> Vec<u8> {
        let hdr = EnvelopeHeader::new(TEST_VM_VERSION, kind, payload.len() as u32, Hash::NONE)
            .finalize_checksum(payload);
        let mut out = Vec::with_capacity(ENVELOPE_HEADER_SIZE + payload.len());
        out.extend_from_slice(hdr.as_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn well_formed_record_loads() {
        let payload = b"hello payload";
        let value = build_record(PayloadKind::CompiledCode, payload);
        let (outcome, decoded) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(outcome, DecodeOutcome::Loaded);
        let rec = decoded.expect("loaded record should yield a DecodedRecord");
        assert_eq!(rec.kind, PayloadKind::CompiledCode);
        assert_eq!(rec.payload, payload);
    }

    #[test]
    fn short_value_skipped_corrupt() {
        let value = vec![0u8; 10];
        let (outcome, decoded) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ValueTooShort)
        );
        assert!(decoded.is_none());
    }

    #[test]
    fn bad_magic_skipped_corrupt() {
        let payload = b"x";
        let mut value = build_record(PayloadKind::ObjectIndex, payload);
        value[0] = b'X';
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedCorrupt(CorruptionReason::BadMagic)
        );
    }

    #[test]
    fn vm_major_mismatch_skipped_incompatible() {
        let payload = b"x";
        let hdr = EnvelopeHeader::new(
            TEST_VM_VERSION_AHEAD,
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        )
        .finalize_checksum(payload);
        let mut value = Vec::new();
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::VmMajorMismatch)
        );
    }

    #[test]
    fn unknown_envelope_format_skipped_incompatible() {
        let payload = b"x";
        let mut hdr = EnvelopeHeader::new(
            TEST_VM_VERSION,
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.envelope_format_version = U16::new(0xFFFF);
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::new();
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::UnknownEnvelopeFormat)
        );
    }

    #[test]
    fn unknown_payload_kind_skipped_unknown_kind() {
        let payload = b"x";
        let mut hdr = EnvelopeHeader::new(
            TEST_VM_VERSION,
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.payload_kind = 0xEE;
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::new();
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownPayloadKind)
        );
    }

    #[test]
    fn unknown_schema_version_skipped_unknown_kind() {
        let payload = b"x";
        let mut hdr = EnvelopeHeader::new(
            TEST_VM_VERSION,
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.schema_version = U16::new(0xFFFF);
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::new();
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownSchemaVersion)
        );
    }

    #[test]
    fn nonzero_flags_skipped_unknown_kind() {
        let payload = b"x";
        let mut hdr = EnvelopeHeader::new(
            TEST_VM_VERSION,
            PayloadKind::CompiledCode,
            payload.len() as u32,
            Hash::NONE,
        );
        hdr.flags = 0x01;
        let hdr = hdr.finalize_checksum(payload);
        let mut value = Vec::new();
        value.extend_from_slice(hdr.as_bytes());
        value.extend_from_slice(payload);
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::NonzeroReservedFlags)
        );
    }

    #[test]
    fn checksum_mismatch_skipped_corrupt() {
        let payload = b"x";
        let mut value = build_record(PayloadKind::CompiledCode, payload);
        let last = value.len() - 1;
        value[last] ^= 0xFF;
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ChecksumMismatch)
        );
    }

    #[test]
    fn payload_length_mismatch_skipped_corrupt() {
        let payload = b"hello payload";
        let mut value = build_record(PayloadKind::CompiledCode, payload);
        value.pop();
        let (outcome, _) = decode(&value, TEST_VM_VERSION.major);
        assert_eq!(
            outcome,
            DecodeOutcome::SkippedCorrupt(CorruptionReason::PayloadLengthMismatch)
        );
    }

    /// Every `DecodeOutcome` value used by tests, covering both the
    /// `Loaded` arm and every sub-reason of each skip variant.
    /// Exhaustiveness check: if a new sub-reason is added without
    /// extending this list, [`stats_invariants_hold_across_all_outcomes`]
    /// will compile but no longer prove the invariant for the new
    /// variant. The list lives next to the `record`-routing tests so
    /// the gap is visible.
    fn all_outcome_variants() -> Vec<DecodeOutcome> {
        vec![
            DecodeOutcome::Loaded,
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::VmMajorMismatch),
            DecodeOutcome::SkippedIncompatible(IncompatibilityReason::UnknownEnvelopeFormat),
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownPayloadKind),
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::UnknownSchemaVersion),
            DecodeOutcome::SkippedUnknownKind(UnknownKindReason::NonzeroReservedFlags),
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ValueTooShort),
            DecodeOutcome::SkippedCorrupt(CorruptionReason::BadMagic),
            DecodeOutcome::SkippedCorrupt(CorruptionReason::PayloadLengthMismatch),
            DecodeOutcome::SkippedCorrupt(CorruptionReason::ChecksumMismatch),
        ]
    }

    #[test]
    fn default_stats_pass_invariants() {
        let stats = LoaderStats::default();
        assert_eq!(stats.total_processed(), 0);
        assert!(stats.check_invariants().is_ok());
    }

    /// Proof-like: for every single-outcome sequence built from every
    /// outcome variant we know about, the aggregate-vs-histogram
    /// invariant must hold. This is the universal invariant of
    /// `record`, not just a pointwise test of one variant.
    #[test]
    fn stats_invariants_hold_across_all_outcomes() {
        for outcome in all_outcome_variants() {
            let mut stats = LoaderStats::default();
            stats.record(outcome);
            assert!(
                stats.check_invariants().is_ok(),
                "invariants broken after recording {:?}: stats = {:?}",
                outcome,
                stats
            );
            assert_eq!(stats.total_processed(), 1);
        }
    }

    /// Proof-like: cumulative recording over every variant preserves
    /// the invariant and grows `total_processed` by exactly the
    /// sequence length.
    #[test]
    fn cumulative_record_preserves_invariants() {
        let mut stats = LoaderStats::default();
        let outcomes = all_outcome_variants();
        for (i, outcome) in outcomes.iter().enumerate() {
            stats.record(*outcome);
            assert!(
                stats.check_invariants().is_ok(),
                "invariants broken after step {}: stats = {:?}",
                i,
                stats
            );
            assert_eq!(stats.total_processed(), (i + 1) as u32);
        }
    }

    #[test]
    fn record_routes_loaded_to_aggregate_only() {
        let mut stats = LoaderStats::default();
        stats.record(DecodeOutcome::Loaded);
        assert_eq!(stats.loaded, 1);
        assert_eq!(stats.skipped_incompatible, 0);
        assert_eq!(stats.skipped_corrupt, 0);
        assert_eq!(stats.skipped_unknown_kind, 0);
        assert_eq!(stats.incompatible_reasons.total(), 0);
        assert_eq!(stats.corrupt_reasons.total(), 0);
        assert_eq!(stats.unknown_kind_reasons.total(), 0);
    }

    #[test]
    fn record_routes_incompatible_to_both_aggregate_and_histogram() {
        let mut stats = LoaderStats::default();
        stats.record(DecodeOutcome::SkippedIncompatible(
            IncompatibilityReason::VmMajorMismatch,
        ));
        stats.record(DecodeOutcome::SkippedIncompatible(
            IncompatibilityReason::UnknownEnvelopeFormat,
        ));
        stats.record(DecodeOutcome::SkippedIncompatible(
            IncompatibilityReason::VmMajorMismatch,
        ));

        assert_eq!(stats.skipped_incompatible, 3);
        assert_eq!(stats.incompatible_reasons.vm_major_mismatch, 2);
        assert_eq!(stats.incompatible_reasons.unknown_envelope_format, 1);
        assert_eq!(stats.incompatible_reasons.total(), 3);
    }

    #[test]
    fn record_routes_unknown_kind_to_both_aggregate_and_histogram() {
        let mut stats = LoaderStats::default();
        stats.record(DecodeOutcome::SkippedUnknownKind(
            UnknownKindReason::UnknownPayloadKind,
        ));
        stats.record(DecodeOutcome::SkippedUnknownKind(
            UnknownKindReason::UnknownSchemaVersion,
        ));
        stats.record(DecodeOutcome::SkippedUnknownKind(
            UnknownKindReason::NonzeroReservedFlags,
        ));
        stats.record(DecodeOutcome::SkippedUnknownKind(
            UnknownKindReason::UnknownPayloadKind,
        ));

        assert_eq!(stats.skipped_unknown_kind, 4);
        assert_eq!(stats.unknown_kind_reasons.unknown_payload_kind, 2);
        assert_eq!(stats.unknown_kind_reasons.unknown_schema_version, 1);
        assert_eq!(stats.unknown_kind_reasons.nonzero_reserved_flags, 1);
        assert_eq!(stats.unknown_kind_reasons.total(), 4);
    }

    #[test]
    fn record_routes_corrupt_to_both_aggregate_and_histogram() {
        let mut stats = LoaderStats::default();
        stats.record(DecodeOutcome::SkippedCorrupt(
            CorruptionReason::ValueTooShort,
        ));
        stats.record(DecodeOutcome::SkippedCorrupt(CorruptionReason::BadMagic));
        stats.record(DecodeOutcome::SkippedCorrupt(
            CorruptionReason::PayloadLengthMismatch,
        ));
        stats.record(DecodeOutcome::SkippedCorrupt(
            CorruptionReason::ChecksumMismatch,
        ));
        stats.record(DecodeOutcome::SkippedCorrupt(
            CorruptionReason::ChecksumMismatch,
        ));

        assert_eq!(stats.skipped_corrupt, 5);
        assert_eq!(stats.corrupt_reasons.value_too_short, 1);
        assert_eq!(stats.corrupt_reasons.bad_magic, 1);
        assert_eq!(stats.corrupt_reasons.payload_length_mismatch, 1);
        assert_eq!(stats.corrupt_reasons.checksum_mismatch, 2);
        assert_eq!(stats.corrupt_reasons.total(), 5);
    }

    #[test]
    fn check_invariants_catches_aggregate_drift() {
        // Construct a deliberately-corrupted stats value: aggregate
        // says one skipped_incompatible, but the histogram says zero.
        // `check_invariants` must reject this. We can't reach this
        // state via `record` (which is the point of the invariant),
        // so build it by hand.
        let stats = LoaderStats {
            skipped_incompatible: 1,
            ..LoaderStats::default()
        };
        assert!(stats.check_invariants().is_err());
    }

    #[test]
    fn check_invariants_catches_corrupt_drift() {
        let stats = LoaderStats {
            skipped_corrupt: 2,
            ..LoaderStats::default()
        };
        assert!(stats.check_invariants().is_err());
    }

    #[test]
    fn check_invariants_catches_unknown_kind_drift() {
        let stats = LoaderStats {
            skipped_unknown_kind: 3,
            ..LoaderStats::default()
        };
        assert!(stats.check_invariants().is_err());
    }

    // iter_keyspace tests live in `tests/iter_keyspace.rs` as integration
    // tests rather than in this `mod tests` block. They require real
    // `fjall::Keyspace::insert` calls; the `persistence_envelope_invariant`
    // gate scans `fmpl-core/src/` for raw `keyspace.insert(`/`partition.insert(`
    // substrings and would treat test-helper inserts as production-side
    // writer-helper bypasses. Keeping the fjall-touching tests in
    // `tests/` preserves the gate's invariant without weakening it.
}
