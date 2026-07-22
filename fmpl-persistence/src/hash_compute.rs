//! Content-hash computation primitive.
//!
//! [`fmpl_types::Hash`] is a carrier-only newtype; the actual blake3
//! hashing lives here in fmpl-persistence because [`crate::source_store::
//! SourceStore`] is the first (and currently only) consumer. Placing
//! `hash_bytes` here keeps fmpl-types free of implementation code —
//! fmpl-types depends only on `serde`, while fmpl-persistence already
//! depends on `blake3` for the envelope checksum.
//!
//! Per ITER-0005b R-I-S-2 PAR finding: do NOT put `Hash::compute` in
//! fmpl-types. fmpl-types stays a zero-impl carrier crate.

use fmpl_types::Hash;

/// Compute the blake3 content hash of `bytes`.
///
/// Two byte slices with identical content yield the same [`Hash`];
/// different content yields different hashes modulo astronomically
/// unlikely collisions. The output is the 32-byte blake3 digest of
/// the input, wrapped in the [`Hash`] newtype.
///
/// Used by [`crate::source_store::SourceStore::put`] to derive
/// content-addressed keys from source bytes. Idempotent: hashing the
/// same input twice always yields equal hashes.
pub fn hash_bytes(bytes: &[u8]) -> Hash {
    Hash::from_bytes(*blake3::hash(bytes).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known blake3 test vector for `b"hello world"`. Confirms we're
    /// reading the right bytes out of the blake3 hasher and wrapping
    /// them into Hash without reordering.
    ///
    /// Source: `blake3sum` cli: `echo -n "hello world" | b3sum`.
    #[test]
    fn known_vector_hello_world() {
        let h = hash_bytes(b"hello world");
        let hex: String = h.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex, "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24",
            "blake3 of \"hello world\" must match the published vector"
        );
    }

    #[test]
    fn idempotent_same_input_same_hash() {
        let a = hash_bytes(b"some source bytes");
        let b = hash_bytes(b"some source bytes");
        assert_eq!(
            a, b,
            "hash_bytes must be deterministic over identical input"
        );
    }

    #[test]
    fn different_input_different_hash() {
        let a = hash_bytes(b"input one");
        let b = hash_bytes(b"input two");
        assert_ne!(a, b, "distinct inputs must produce distinct hashes");
    }

    /// Even the empty input has a defined blake3 hash that is NOT
    /// Hash::NONE (the all-zeros sentinel). Confirms the empty case
    /// doesn't accidentally collide with the "no source" sentinel.
    #[test]
    fn empty_input_is_not_none_sentinel() {
        let h = hash_bytes(b"");
        assert_ne!(
            h,
            Hash::NONE,
            "blake3 of empty input is well-defined and must NOT equal Hash::NONE"
        );
    }

    /// Hash::NONE is reserved for "no source attached" and must never
    /// be the natural hash of any concrete input. (Blake3 collision
    /// with all-zeros is astronomically improbable; this test pins
    /// the conceptual contract.)
    #[test]
    fn no_realistic_input_collides_with_none() {
        for sample in [b"x".as_slice(), b"\x00", b"\x00\x00", b"0", &[0u8; 32]] {
            assert_ne!(
                hash_bytes(sample),
                Hash::NONE,
                "hash_bytes({:?}) collided with Hash::NONE sentinel — investigate",
                sample
            );
        }
    }
}
