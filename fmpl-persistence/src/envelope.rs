//! Versioned envelope header prepended to every persisted record.
//!
//! Every value stored in the Fjall keyspace is `EnvelopeHeader || payload`.
//! The header is a fixed 56-byte, unaligned `#[repr(C)]` struct decoded
//! zero-copy via [`zerocopy`]; the payload is opaque to this module and
//! framed/parsed by the caller (typically `serde_json`).
//!
//! # Wire layout
//!
//! 56 bytes, little-endian, no padding:
//!
//! ```text
//! offset  size  field
//!      0     4  magic                    b"FMPL" — sentinel for "is this an envelope?"
//!      4     2  envelope_format_version  U16<LE>, bumped on wire-format breaks
//!      6     1  payload_kind             discriminant; see persistence::schema::PayloadKind
//!      7     1  flags                    must be zero; reserved for forward-compatible micro-bumps
//!      8     2  vm_version_major         U16<LE>; major-version mismatch => loader rejects
//!     10     2  vm_version_minor         U16<LE>; informational
//!     12     2  vm_version_patch         U16<LE>; informational
//!     14     2  schema_version           U16<LE>; per-payload-kind schema version
//!     16     4  payload_len              U32<LE>; bytes of payload following the header
//!     20    32  source_hash              blake3(source); all-zeros = NO_SOURCE_HASH sentinel
//!     52     4  crc32                    U32<LE>; low 32 bits of blake3(header[crc=0] || payload)
//! ```
//!
//! # Alignment
//!
//! The header is unaligned (`align_of == 1`) so it can be decoded from
//! any byte slice without copying. It is intentionally not padded to a
//! power of two: Fjall stores each value as an independent buffer, so
//! there is no contiguous-record alignment win to chase.
//!
//! # Integrity
//!
//! `crc32` covers the header itself (with the `crc32` field zeroed during
//! hashing) concatenated with the payload bytes. `magic` is included in
//! that hash because it sits at offset 0 of the header; it is not hashed
//! separately. The "CRC" name is preserved for field-name stability; the
//! actual algorithm is the low 32 bits of blake3 — see
//! [`super::checksum::compute`].

use fmpl_types::{Hash, VmVersion};
use zerocopy::little_endian::{U16, U32};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

/// Magic bytes (`b"FMPL"`) at offset 0 of every envelope. Used by the
/// loader as a fast "is this our framing?" gate before any further
/// parsing.
pub const MAGIC: [u8; 4] = *b"FMPL";

/// Sentinel `source_hash` value meaning "this record has no associated
/// source bytes." All-zero is a safe sentinel because a real blake3 hash
/// is never all-zero in practice. Used until the content-addressed source
/// store lands; non-zero values will then resolve to source blobs.
pub const NO_SOURCE_HASH: [u8; 32] = [0u8; 32];

/// Fixed-layout, zero-copy-decodable header for a persisted record.
///
/// # Usage
///
/// Construction is a two-phase pattern; the `crc32` field is *not* valid
/// until phase two:
///
/// 1. [`EnvelopeHeader::new`] stamps everything except the
///    checksum (which is left as zero).
/// 2. [`EnvelopeHeader::finalize_checksum`] consumes the partial header
///    plus the payload bytes and returns the finalized header.
///
/// The two phases exist because the checksum covers the rest of the
/// header — it has to be the last field written.
///
/// To decode bytes coming back off disk, use
/// [`zerocopy::FromBytes::ref_from_prefix`] on the first
/// [`ENVELOPE_HEADER_SIZE`] bytes of the keyspace value; then call
/// [`verify_checksum`](Self::verify_checksum) with the remaining payload.
///
/// # Layout
///
/// `#[repr(C)]`, no padding, alignment 1 (`Unaligned`), little-endian
/// integers. See the module-level docs for the full byte map.
#[derive(Debug, Clone, Copy, KnownLayout, Immutable, FromBytes, IntoBytes, Unaligned)]
#[repr(C)]
pub struct EnvelopeHeader {
    /// Always [`MAGIC`] (`b"FMPL"`). First field so that any record can
    /// be fingerprinted in O(1) without parsing the rest of the header.
    pub magic: [u8; 4],
    /// Wire-format version of the envelope itself. Bumped only on
    /// breaking changes to this struct's layout or semantics; per-payload
    /// schema changes use [`schema_version`](Self::schema_version) instead.
    pub envelope_format_version: U16,
    /// Discriminant for the payload bytes that follow. See
    /// [`PayloadKind`](super::schema::PayloadKind).
    pub payload_kind: u8,
    /// Reserved-must-be-zero byte. The loader rejects nonzero values so
    /// that this slot can later carry forward-compatible feature bits
    /// without bumping the envelope format version.
    pub flags: u8,
    /// VM major version that wrote the record. The loader compares this
    /// against the running VM's major version and rejects mismatches —
    /// a major bump signals an incompatible VM contract.
    pub vm_version_major: U16,
    /// VM minor version that wrote the record. Informational; no
    /// compatibility check is performed.
    pub vm_version_minor: U16,
    /// VM patch version that wrote the record. Informational; no
    /// compatibility check is performed.
    pub vm_version_patch: U16,
    /// Per-payload-kind schema version, queried from
    /// [`PayloadKind::current_schema_version`](super::schema::PayloadKind::current_schema_version)
    /// at write time. Lets one payload kind evolve its encoding without
    /// bumping any unrelated kind.
    pub schema_version: U16,
    /// Byte length of the payload immediately following the header.
    /// `payload_len as usize == total_bytes - ENVELOPE_HEADER_SIZE`.
    pub payload_len: U32,
    /// blake3 hash of the record's source bytes, or [`NO_SOURCE_HASH`]
    /// (all zeros) when no source is attached. Reserved for the
    /// content-addressed source store; today every writer passes the
    /// sentinel.
    pub source_hash: [u8; 32],
    /// Truncated blake3 checksum: low 32 bits of
    /// `blake3(header_with_this_field_zeroed || payload)`.
    ///
    /// The field name predates the algorithm choice — the value is *not*
    /// a CRC-32 polynomial. Stored as a little-endian `U32` for wire
    /// stability. See [`super::checksum::compute`].
    pub crc32: U32,
}

// Compile-time guards: any field reorder, retype, or addition that
// breaks the on-disk layout fails to build instead of corrupting data
// silently at runtime.
const _: () = assert!(::core::mem::size_of::<EnvelopeHeader>() == 56);
const _: () = assert!(::core::mem::align_of::<EnvelopeHeader>() == 1);

/// Wire size of the envelope header in bytes. Equal to
/// `size_of::<EnvelopeHeader>()`; exposed as a const so callers can size
/// buffers without naming the type.
pub const ENVELOPE_HEADER_SIZE: usize = 56;

impl EnvelopeHeader {
    /// Build phase-one of a fresh header: every field is stamped except
    /// `crc32`, which is left as zero.
    ///
    /// VM version is supplied by the caller via [`VmVersion`] — typically
    /// `fmpl_core::VM_VERSION`. Per ITER-0005a.5 R3-C1, the version is a
    /// parameter rather than a constant so this crate stays
    /// version-agnostic; otherwise `env!("CARGO_PKG_VERSION")` would
    /// resolve to fmpl-persistence's package version, not the running
    /// VM's. The carrier struct lives in `fmpl-types` so it can travel
    /// across crate boundaries without dragging fmpl-persistence in.
    ///
    /// `source_hash` is the API-edge form: a [`Hash`] (32-byte newtype)
    /// rather than the raw `[u8; 32]` stored in the header struct.
    /// Callers with no source attached pass [`Hash::NONE`].
    ///
    /// The schema version is queried from `kind` so each payload kind
    /// controls its own evolution. `flags` is forced to zero.
    ///
    /// The caller must follow up with [`Self::finalize_checksum`] before
    /// writing the header to durable storage — a header whose `crc32` is
    /// zero will not survive [`Self::verify_checksum`].
    pub fn new(
        vm_version: VmVersion,
        kind: super::schema::PayloadKind,
        payload_len: u32,
        source_hash: Hash,
    ) -> Self {
        Self {
            magic: MAGIC,
            envelope_format_version: U16::new(super::schema::ENVELOPE_FORMAT_VERSION),
            payload_kind: kind.as_byte(),
            flags: 0,
            vm_version_major: U16::new(vm_version.major),
            vm_version_minor: U16::new(vm_version.minor),
            vm_version_patch: U16::new(vm_version.patch),
            schema_version: U16::new(kind.current_schema_version()),
            payload_len: U32::new(payload_len),
            source_hash: source_hash.into_bytes(),
            crc32: U32::new(0),
        }
    }

    /// Phase-two of header construction: hash `header_with_crc_zeroed
    /// || payload` and stamp the result into the `crc32` field.
    ///
    /// # Preconditions
    ///
    /// `self.crc32` must be zero on entry — i.e. `self` was produced by
    /// [`Self::new`] and has not been mutated since. The
    /// debug assertion below catches accidental re-finalization.
    ///
    /// # Performance
    ///
    /// One blake3 pass over `ENVELOPE_HEADER_SIZE + payload.len()` bytes;
    /// no allocation.
    pub fn finalize_checksum(mut self, payload: &[u8]) -> Self {
        debug_assert_eq!(
            self.crc32.get(),
            0,
            "finalize_checksum requires a header with crc32 == 0"
        );
        let crc = super::checksum::compute(self.as_bytes(), payload);
        self.crc32 = U32::new(crc);
        self
    }

    /// Recompute the checksum over `header_with_crc_zeroed || payload`
    /// and compare it to the stamped `crc32`.
    ///
    /// Returns `true` iff the stamp matches — i.e. neither the header
    /// (excluding `crc32` itself) nor `payload` has been tampered with
    /// since [`Self::finalize_checksum`] ran.
    ///
    /// Cost is the same single blake3 pass as `finalize_checksum`.
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        let mut zeroed = *self;
        zeroed.crc32 = U32::new(0);
        let expected = super::checksum::compute(zeroed.as_bytes(), payload);
        self.crc32.get() == expected
    }
}

/// Failure modes of [`write`]: payload serialization (`serde_json`) or
/// store I/O (abstracted via [`StoreError`](crate::StoreError)).
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeWriteError {
    /// `serde_json::to_vec` failed on the payload value.
    #[error("envelope write: payload serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    /// Inserting the framed bytes into the store failed.
    #[error("envelope write: store insert failed: {0}")]
    Store(#[from] crate::StoreError),
}

/// Serialize `value`, frame it under an [`EnvelopeHeader`], and insert
/// the result into `keyspace` at `key`.
///
/// This is the single sanctioned path for persisting any value: it
/// guarantees the on-disk record obeys the envelope layout and carries a
/// finalized checksum. An invariant gate elsewhere in the tree forbids
/// bypass writes (raw `keyspace.insert(.., serde_json::to_vec(..))`).
///
/// # Parameters
///
/// - `kind`: drives both the `payload_kind` byte and the `schema_version`
///   stamped in the header.
/// - `source_hash`: blake3 of the originating source bytes, or
///   [`NO_SOURCE_HASH`] when no source is attached.
///
/// # Postcondition
///
/// On `Ok`, the value stored at `key` is exactly
/// `ENVELOPE_HEADER_SIZE + serde_json::to_vec(value)?.len()` bytes:
/// header followed by payload, with `crc32` covering both.
pub fn write<T: serde::Serialize, S: crate::Store + ?Sized>(
    store: &S,
    key: &[u8],
    value: &T,
    kind: super::schema::PayloadKind,
    vm_version: VmVersion,
    source_hash: Hash,
) -> Result<(), EnvelopeWriteError> {
    let payload = serde_json::to_vec(value)?;
    let header = EnvelopeHeader::new(vm_version, kind, payload.len() as u32, source_hash)
        .finalize_checksum(&payload);
    let mut bytes = Vec::with_capacity(ENVELOPE_HEADER_SIZE + payload.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(&payload);
    store.insert(key, &bytes)?;
    Ok(())
}

/// Write a CompiledCode payload through the envelope writer.
///
/// Kind-specific convenience wrapper around [`write`] for test code in
/// non-schema-aware modules (e.g. `recovery.rs` test helpers) that would
/// otherwise need to name the payload-kind variant directly and trip the
/// schema-format anti-rot ratchet. `envelope.rs` is in the ratchet's
/// exemption set, so the variant reference inside this module body is
/// allowed.
#[cfg(test)]
pub(crate) fn write_compiled_code<T, S>(
    store: &S,
    key: &[u8],
    value: &T,
    vm_version: VmVersion,
    source_hash: Hash,
) -> Result<(), EnvelopeWriteError>
where
    T: serde::Serialize,
    S: crate::Store + ?Sized,
{
    write(
        store,
        key,
        value,
        super::schema::PayloadKind::CompiledCode,
        vm_version,
        source_hash,
    )
}

#[cfg(test)]
mod tests {
    use super::super::schema::PayloadKind;
    use super::*;

    // Test VM version. Real callers pass fmpl_core::VM_VERSION; tests
    // use a fixed value to keep assertions stable across
    // fmpl-persistence version bumps.
    const TEST_VM_VERSION: VmVersion = VmVersion::new(0, 1, 0);

    #[test]
    fn header_size_is_56_bytes() {
        assert_eq!(::core::mem::size_of::<EnvelopeHeader>(), 56);
        assert_eq!(ENVELOPE_HEADER_SIZE, 56);
    }

    #[test]
    fn header_is_unaligned() {
        assert_eq!(::core::mem::align_of::<EnvelopeHeader>(), 1);
    }

    #[test]
    fn new_stamps_magic_and_version() {
        let hdr = EnvelopeHeader::new(TEST_VM_VERSION, PayloadKind::CompiledCode, 42, Hash::NONE);
        assert_eq!(hdr.magic, MAGIC);
        assert_eq!(hdr.envelope_format_version.get(), 1);
        assert_eq!(hdr.payload_kind, PayloadKind::CompiledCode.as_byte());
        assert_eq!(hdr.flags, 0);
        assert_eq!(hdr.vm_version_major.get(), TEST_VM_VERSION.major);
        assert_eq!(hdr.vm_version_minor.get(), TEST_VM_VERSION.minor);
        assert_eq!(hdr.vm_version_patch.get(), TEST_VM_VERSION.patch);
        assert_eq!(hdr.schema_version.get(), 1);
        assert_eq!(hdr.payload_len.get(), 42);
        assert_eq!(hdr.source_hash, NO_SOURCE_HASH);
        assert_eq!(hdr.crc32.get(), 0); // unstamped
    }

    fn test_header(kind: PayloadKind, payload: &[u8]) -> EnvelopeHeader {
        EnvelopeHeader::new(TEST_VM_VERSION, kind, payload.len() as u32, Hash::NONE)
            .finalize_checksum(payload)
    }

    #[test]
    fn finalize_and_verify_checksum_roundtrips() {
        let payload = b"some payload bytes";
        let hdr = test_header(PayloadKind::CompiledCode, payload);
        assert_ne!(hdr.crc32.get(), 0, "checksum should be nonzero in practice");
        assert!(hdr.verify_checksum(payload));
    }

    #[test]
    fn verify_checksum_fails_on_payload_corruption() {
        let payload = b"some payload bytes";
        let hdr = test_header(PayloadKind::CompiledCode, payload);
        let corrupted = b"some payload bYtes";
        assert!(!hdr.verify_checksum(corrupted));
    }

    #[test]
    fn verify_checksum_fails_on_header_corruption() {
        let payload = b"some payload bytes";
        let mut hdr = test_header(PayloadKind::CompiledCode, payload);
        hdr.flags = 0x42;
        assert!(!hdr.verify_checksum(payload));
    }

    #[test]
    fn header_roundtrips_through_zerocopy_bytes() {
        let payload = b"";
        let hdr = test_header(PayloadKind::ObjectIndex, payload);
        let bytes = hdr.as_bytes();
        assert_eq!(bytes.len(), 56);
        let (decoded, _rest) = EnvelopeHeader::ref_from_prefix(bytes).unwrap();
        assert_eq!(decoded.magic, hdr.magic);
        assert_eq!(decoded.payload_kind, hdr.payload_kind);
        assert_eq!(decoded.crc32.get(), hdr.crc32.get());
        assert!(decoded.verify_checksum(payload));
    }

    #[test]
    fn ref_from_prefix_fails_on_short_buffer() {
        let too_short = [0u8; 55];
        let result = EnvelopeHeader::ref_from_prefix(&too_short);
        assert!(result.is_err());
    }

    // Integration tests against a real fjall-backed Store live in
    // `fmpl-persistence/tests/` because they require the `fjall-backend`
    // feature gate on dev-deps.
}
