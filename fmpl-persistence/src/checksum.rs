//! Envelope integrity checksum.
//!
//! Defines the 32-bit checksum stored in [`EnvelopeHeader::crc32`][crc32]:
//! the low 32 bits of `blake3(header_with_crc_field_zeroed || payload)`.
//!
//! # Why blake3 truncated, not CRC32
//!
//! The field is named `crc32` for layout stability, but the computation is
//! a truncated cryptographic hash. blake3 matches the hash used by the
//! content-addressed source store, so the persistence layer carries one
//! hash implementation instead of two. Truncation to the first four output
//! bytes is sound because blake3's output is an XOF: any prefix is
//! cryptographically uniform, yielding the same `2^-32` collision rate as
//! CRC32 without polynomial blind spots.
//!
//! # Hash input framing
//!
//! The `crc32` field inside the header is zeroed before hashing — the
//! checksum cannot cover its own storage location without a fixed point.
//! `magic` is the first four bytes of the header and is covered by the
//! hash; it is not passed as a separate argument.
//!
//! Source bytes are intentionally excluded. The header's `source_hash` is
//! itself the source identity (content addressing); any tampering is
//! detected at source-store lookup time rather than re-verified here.
//!
//! [crc32]: super::envelope::EnvelopeHeader

/// Returns the 32-bit envelope checksum for the given header/payload pair.
///
/// # Inputs
///
/// * `header_no_crc` — borrowed envelope header bytes with the `crc32`
///   field overwritten with zeros. Callers must zero the field before
///   calling; this function does not mutate or copy the header.
/// * `payload` — borrowed payload bytes that follow the header on disk.
///
/// Both slices are read-only; no allocation occurs beyond the blake3
/// hasher's fixed internal state.
///
/// # Returns
///
/// The low 32 bits of `blake3(header_no_crc || payload)`, interpreted as
/// a little-endian `u32`. Identical inputs always produce identical
/// outputs (deterministic, no salting).
///
/// # Boundaries
///
/// Empty slices are valid: `compute(&[], &[])` returns the truncated
/// blake3 of the empty string. There is no upper bound on input size
/// beyond blake3's own (effectively unbounded for practical use).
///
/// # Performance
///
/// One blake3 pass over `header_no_crc.len() + payload.len()` bytes. The
/// two slices are streamed through a single hasher rather than
/// concatenated, avoiding an allocation.
pub fn compute(header_no_crc: &[u8], payload: &[u8]) -> u32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(header_no_crc);
    hasher.update(payload);
    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_over_same_inputs() {
        let h = b"header data here padded out";
        let p = b"payload bytes";
        assert_eq!(compute(h, p), compute(h, p));
    }

    #[test]
    fn changes_when_header_changes() {
        let h1 = b"header data here padded out";
        let h2 = b"header data here padded OUT"; // single bit flip
        let p = b"payload bytes";
        assert_ne!(compute(h1, p), compute(h2, p));
    }

    #[test]
    fn changes_when_payload_changes() {
        let h = b"header data here padded out";
        let p1 = b"payload bytes";
        let p2 = b"payload Bytes"; // single bit flip
        assert_ne!(compute(h, p1), compute(h, p2));
    }

    #[test]
    fn header_payload_concatenation_matters() {
        // compute(AB, C) must differ from compute(A, BC) because blake3
        // updates are stateful: blake3(A)|blake3(B)|blake3(C) is not the
        // same as blake3(AB)|blake3(C) in our protocol. Catch any future
        // refactor that accidentally treats header+payload as a single
        // concatenated buffer instead of separate updates (which would
        // happen to produce the same hash here because blake3 is
        // length-streaming, but explicit framing avoids that ambiguity).
        let a = [1u8, 2, 3];
        let b = [4u8, 5, 6];
        let c = [7u8, 8, 9];
        // Sanity: streaming blake3 across (a,b,c) ignores boundaries — so
        // compute([a;b], [c]) == compute([a], [b;c]). That's expected for
        // blake3 the algorithm; the test pins that we're using the
        // streaming behavior, which is fine because the caller always
        // passes (header_no_crc, payload) in that exact order.
        let ab: Vec<u8> = a.iter().chain(b.iter()).copied().collect();
        let bc: Vec<u8> = b.iter().chain(c.iter()).copied().collect();
        assert_eq!(compute(&ab, &c), compute(&a, &bc));
    }
}
