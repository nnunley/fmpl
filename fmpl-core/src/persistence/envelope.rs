//! Versioned envelope header for every persisted record.
//!
//! Satisfies AC-1 of STORY-0099. The header is a fixed-layout, 56-byte,
//! unaligned `#[repr(C)]` struct decoded zero-copy via the
//! [`zerocopy`] crate. See `docs/superpowers/iterations/roadmap.md` →
//! ITER-0005a.1 on the `archive/agent-harness` branch for the design
//! rationale (the agent-dev docs were sidelined from `main`).
//!
//! Wire layout (56 bytes total, little-endian, no padding):
//!
//! ```text
//! offset  size  field
//!      0     4  magic                  // b"FMPL"
//!      4     2  envelope_format_version (U16<LE>)
//!      6     1  payload_kind            (see persistence::schema::PayloadKind)
//!      7     1  flags                   (must-be-zero in v1)
//!      8     2  vm_version_major        (U16<LE>)
//!     10     2  vm_version_minor        (U16<LE>)
//!     12     2  vm_version_patch        (U16<LE>)
//!     14     2  schema_version          (U16<LE>, per persistence::schema)
//!     16     4  payload_len             (U32<LE>)
//!     20    32  source_hash             (blake3; all-zeros = no source)
//!     52     4  crc32                   (U32<LE>; low 32 bits of blake3)
//! ```
//!
//! The header is intentionally NOT padded to a power of 2. Fjall is a
//! K/V store; each record's value is its own buffer, so there is no
//! contiguous-record alignment benefit. The `flags` byte is the
//! reserved-must-be-zero slot for micro-bumps.

use zerocopy::little_endian::{U16, U32};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

/// Magic bytes identifying an FMPL persistence envelope. ASCII "FMPL".
pub const MAGIC: [u8; 4] = *b"FMPL";

/// Sentinel value for `source_hash` meaning "no source for this record."
/// ITER-0005b's content-addressed source store will populate non-zero
/// hashes; in ITER-0005a.1 every record under test uses this sentinel.
pub const NO_SOURCE_HASH: [u8; 32] = [0u8; 32];

/// Fixed-layout envelope header. See module-level rustdoc for the wire
/// layout. Decoded zero-copy via [`zerocopy::FromBytes::ref_from_prefix`].
#[derive(Debug, Clone, Copy, KnownLayout, Immutable, FromBytes, IntoBytes, Unaligned)]
#[repr(C)]
pub struct EnvelopeHeader {
    /// Always [`MAGIC`] (`b"FMPL"`).
    pub magic: [u8; 4],
    /// Envelope wire-format version. See
    /// [`schema::ENVELOPE_FORMAT_VERSION`].
    ///
    /// [`schema::ENVELOPE_FORMAT_VERSION`]: super::schema::ENVELOPE_FORMAT_VERSION
    pub envelope_format_version: U16,
    /// Payload kind discriminator. See [`schema::PayloadKind`].
    ///
    /// [`schema::PayloadKind`]: super::schema::PayloadKind
    pub payload_kind: u8,
    /// Reserved flag bits. MUST be zero in v1. Loader rejects records
    /// with nonzero flags (AC-3 unknown-state skip path).
    pub flags: u8,
    /// VM major version that wrote this record. See
    /// [`schema::VM_VERSION_MAJOR`].
    ///
    /// [`schema::VM_VERSION_MAJOR`]: super::schema::VM_VERSION_MAJOR
    pub vm_version_major: U16,
    /// VM minor version that wrote this record.
    pub vm_version_minor: U16,
    /// VM patch version that wrote this record.
    pub vm_version_patch: U16,
    /// Per-payload-kind schema version. See
    /// [`PayloadKind::current_schema_version`].
    ///
    /// [`PayloadKind::current_schema_version`]: super::schema::PayloadKind::current_schema_version
    pub schema_version: U16,
    /// Byte length of the payload that follows this header.
    pub payload_len: U32,
    /// blake3 hash of the source bytes for this record. All-zeros
    /// ([`NO_SOURCE_HASH`]) means "no source"; non-zero hashes resolve
    /// against ITER-0005b's content-addressed source store.
    pub source_hash: [u8; 32],
    /// Envelope checksum — lower 32 bits of
    /// `blake3(header_with_crc_zeroed || payload)`. The `magic` bytes
    /// are already the first 4 bytes of `header_with_crc_zeroed`, so
    /// they are covered by the hash without being concatenated
    /// separately. See [`super::checksum::compute`]. Stored in the
    /// wire format as LE u32 for layout stability; the field name and
    /// width are preserved from AC-1's "CRC32" wording even though the
    /// algorithm is blake3.
    pub crc32: U32,
}

// Compile-time typed invariants. Per `feedback_prefer_proof_tests.md` form
// #1: failure here is a compile error, not a test failure. A future field
// reorder or addition is caught immediately.
const _: () = assert!(::core::mem::size_of::<EnvelopeHeader>() == 56);
const _: () = assert!(::core::mem::align_of::<EnvelopeHeader>() == 1);

/// Size of the envelope header on the wire. Convenience constant for
/// callers (matches `size_of::<EnvelopeHeader>()`).
pub const ENVELOPE_HEADER_SIZE: usize = 56;

impl EnvelopeHeader {
    /// Construct a fresh envelope header for the current VM and the
    /// given payload kind. The checksum is initially zero and must be
    /// filled in by the caller AFTER the payload bytes are known —
    /// see [`finalize_checksum`].
    pub fn new_for_current_vm(
        kind: super::schema::PayloadKind,
        payload_len: u32,
        source_hash: [u8; 32],
    ) -> Self {
        Self {
            magic: MAGIC,
            envelope_format_version: U16::new(super::schema::ENVELOPE_FORMAT_VERSION),
            payload_kind: kind.as_byte(),
            flags: 0,
            vm_version_major: U16::new(super::schema::VM_VERSION_MAJOR),
            vm_version_minor: U16::new(super::schema::VM_VERSION_MINOR),
            vm_version_patch: U16::new(super::schema::VM_VERSION_PATCH),
            schema_version: U16::new(kind.current_schema_version()),
            payload_len: U32::new(payload_len),
            source_hash,
            crc32: U32::new(0),
        }
    }

    /// Compute and stamp the checksum based on the header (with `crc32`
    /// already zeroed) and the supplied payload. Returns the final
    /// header bytes for writing to the keyspace.
    ///
    /// Standard CRC-of-itself pattern: zero the field, compute over the
    /// rest, write the field back. Because `new_for_current_vm` leaves
    /// `crc32` zero, no extra zeroing is needed here.
    pub fn finalize_checksum(mut self, payload: &[u8]) -> Self {
        let crc = super::checksum::compute(self.as_bytes(), payload);
        self.crc32 = U32::new(crc);
        self
    }

    /// Verify the checksum field against a re-computed value over the
    /// payload. Returns `true` if the stamp matches.
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        // Construct a "zeroed" copy for re-computation.
        let mut zeroed = *self;
        zeroed.crc32 = U32::new(0);
        let expected = super::checksum::compute(zeroed.as_bytes(), payload);
        self.crc32.get() == expected
    }
}

/// Error from [`write`]. Wraps the two failure modes of envelope writes:
/// serialization (`serde_json::Error`) and keyspace I/O (`fjall::Error`).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeWriteError {
    /// Serializing the payload to bytes failed.
    #[error("envelope write: payload serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    /// Inserting the framed bytes into the keyspace failed.
    #[error("envelope write: keyspace insert failed: {0}")]
    Keyspace(#[from] fjall::Error),
}

/// Write `value` to `keyspace` at `key`, wrapped in an
/// [`EnvelopeHeader`] for the given `kind` and `source_hash`. This is
/// the **single** path through which all persisted records flow per
/// STORY-0099 AC-5; the T5 invariant gate asserts that no raw
/// `keyspace.insert(.., serde_json::to_vec(..))` patterns survive
/// outside this module.
///
/// `source_hash` is the blake3 hash of the originating source bytes,
/// or [`NO_SOURCE_HASH`] (`[0u8; 32]`) when no source exists yet
/// (every caller in ITER-0005a.2 passes the sentinel — ITER-0005b
/// introduces actual source hashing).
///
/// On success the keyspace contains a value of exactly
/// `ENVELOPE_HEADER_SIZE + serialized_payload.len()` bytes.
#[cfg(not(target_arch = "wasm32"))]
pub fn write<T: serde::Serialize>(
    keyspace: &fjall::Keyspace,
    key: &[u8],
    value: &T,
    kind: super::schema::PayloadKind,
    source_hash: [u8; 32],
) -> Result<(), EnvelopeWriteError> {
    let payload = serde_json::to_vec(value)?;
    let header = EnvelopeHeader::new_for_current_vm(kind, payload.len() as u32, source_hash)
        .finalize_checksum(&payload);
    let mut bytes = Vec::with_capacity(ENVELOPE_HEADER_SIZE + payload.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(&payload);
    keyspace.insert(key, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::schema::{PayloadKind, VM_VERSION_MAJOR};
    use super::*;

    #[test]
    fn header_size_is_56_bytes() {
        assert_eq!(::core::mem::size_of::<EnvelopeHeader>(), 56);
        assert_eq!(ENVELOPE_HEADER_SIZE, 56);
    }

    #[test]
    fn header_is_unaligned() {
        // `Unaligned` derive guarantees alignment 1, but be defensive
        // against accidental field-type changes.
        assert_eq!(::core::mem::align_of::<EnvelopeHeader>(), 1);
    }

    #[test]
    fn new_for_current_vm_stamps_magic_and_version() {
        let hdr = EnvelopeHeader::new_for_current_vm(PayloadKind::CompiledCode, 42, NO_SOURCE_HASH);
        assert_eq!(hdr.magic, MAGIC);
        assert_eq!(hdr.envelope_format_version.get(), 1);
        assert_eq!(hdr.payload_kind, PayloadKind::CompiledCode.as_byte());
        assert_eq!(hdr.flags, 0);
        assert_eq!(hdr.vm_version_major.get(), VM_VERSION_MAJOR);
        assert_eq!(hdr.schema_version.get(), 1);
        assert_eq!(hdr.payload_len.get(), 42);
        assert_eq!(hdr.source_hash, NO_SOURCE_HASH);
        assert_eq!(hdr.crc32.get(), 0); // unstamped
    }

    #[test]
    fn finalize_and_verify_checksum_roundtrips() {
        let payload = b"some payload bytes";
        let hdr = EnvelopeHeader::new_for_current_vm(
            PayloadKind::CompiledCode,
            payload.len() as u32,
            NO_SOURCE_HASH,
        )
        .finalize_checksum(payload);
        assert_ne!(hdr.crc32.get(), 0, "checksum should be nonzero in practice");
        assert!(hdr.verify_checksum(payload));
    }

    #[test]
    fn verify_checksum_fails_on_payload_corruption() {
        let payload = b"some payload bytes";
        let hdr = EnvelopeHeader::new_for_current_vm(
            PayloadKind::CompiledCode,
            payload.len() as u32,
            NO_SOURCE_HASH,
        )
        .finalize_checksum(payload);
        let corrupted = b"some payload bYtes";
        assert!(!hdr.verify_checksum(corrupted));
    }

    #[test]
    fn verify_checksum_fails_on_header_corruption() {
        let payload = b"some payload bytes";
        let mut hdr = EnvelopeHeader::new_for_current_vm(
            PayloadKind::CompiledCode,
            payload.len() as u32,
            NO_SOURCE_HASH,
        )
        .finalize_checksum(payload);
        // Tamper with a non-crc field.
        hdr.flags = 0x42;
        assert!(!hdr.verify_checksum(payload));
    }

    #[test]
    fn header_roundtrips_through_zerocopy_bytes() {
        let payload = b"";
        let hdr = EnvelopeHeader::new_for_current_vm(PayloadKind::ObjectIndex, 0, NO_SOURCE_HASH)
            .finalize_checksum(payload);
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

    /// Open a fresh keyspace under `tempfile::tempdir`.
    /// Matches the test-harness pattern already used in
    /// `grammar/incremental.rs:178` (`fjall::Database::builder(path).open()`).
    fn fresh_keyspace() -> (tempfile::TempDir, fjall::Keyspace) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let keyspace = db
            .keyspace("envelope_test", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        (dir, keyspace)
    }

    #[test]
    fn write_then_decode_roundtrips_via_loader() {
        let (_dir, keyspace) = fresh_keyspace();
        let payload_value = vec![1i64, 2, 3, 42];

        write(
            &keyspace,
            b"my-key",
            &payload_value,
            PayloadKind::CompiledCode,
            NO_SOURCE_HASH,
        )
        .expect("envelope write should succeed");

        let raw = keyspace.get(b"my-key").unwrap().expect("key exists");
        let (outcome, decoded) = super::super::loader::decode(&raw);
        assert_eq!(outcome, super::super::loader::DecodeOutcome::Loaded);
        let rec = decoded.expect("loaded record");
        assert_eq!(rec.kind, PayloadKind::CompiledCode);
        let recovered: Vec<i64> = serde_json::from_slice(rec.payload).unwrap();
        assert_eq!(recovered, payload_value);
    }

    #[test]
    fn write_total_byte_count_is_header_size_plus_payload_size() {
        let (_dir, keyspace) = fresh_keyspace();
        let payload = "some-string-payload".to_string();
        write(
            &keyspace,
            b"k",
            &payload,
            PayloadKind::ObjectRecord,
            NO_SOURCE_HASH,
        )
        .unwrap();
        let raw = keyspace.get(b"k").unwrap().unwrap();
        let expected_payload_len = serde_json::to_vec(&payload).unwrap().len();
        assert_eq!(raw.len(), ENVELOPE_HEADER_SIZE + expected_payload_len);
    }

    #[test]
    fn write_two_different_kinds_yields_distinguishable_records() {
        let (_dir, keyspace) = fresh_keyspace();
        write(
            &keyspace,
            b"__object_ids__",
            &vec![1u64, 2, 3],
            PayloadKind::ObjectIndex,
            NO_SOURCE_HASH,
        )
        .unwrap();
        write(
            &keyspace,
            b"obj:1",
            &"{\"slot\":1}".to_string(),
            PayloadKind::ObjectRecord,
            NO_SOURCE_HASH,
        )
        .unwrap();

        let idx_raw = keyspace.get(b"__object_ids__").unwrap().unwrap();
        let obj_raw = keyspace.get(b"obj:1").unwrap().unwrap();

        let (idx_outcome, idx_decoded) = super::super::loader::decode(&idx_raw);
        let (obj_outcome, obj_decoded) = super::super::loader::decode(&obj_raw);

        assert_eq!(idx_outcome, super::super::loader::DecodeOutcome::Loaded);
        assert_eq!(obj_outcome, super::super::loader::DecodeOutcome::Loaded);
        assert_eq!(idx_decoded.unwrap().kind, PayloadKind::ObjectIndex);
        assert_eq!(obj_decoded.unwrap().kind, PayloadKind::ObjectRecord);
    }
}
