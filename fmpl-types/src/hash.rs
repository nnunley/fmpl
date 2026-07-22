//! Content-hash newtype.
//!
//! Carrier for blake3-derived 32-byte content hashes. The hashing
//! primitive (`Hash::compute(bytes)`) is deferred to ITER-0005b when
//! the source store is wired and a real caller exists; this iteration
//! ships only the carrier type + the [`Hash::NONE`] sentinel.

use serde::{Deserialize, Serialize};

/// 32-byte content hash.
///
/// Newtype around `[u8; 32]`. Does NOT derive zerocopy traits —
/// envelope-header struct fields stay as `[u8; 32]` for layout
/// compatibility; `Hash` is used at API-edge positions and converts
/// to/from the raw array via [`Hash::from_bytes`] / [`Hash::as_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash, Serialize, Deserialize)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Sentinel hash for "no source associated with this record."
    /// All-zero bytes. Envelope writers stamp this when no source
    /// content addressing applies (yet).
    pub const NONE: Hash = Hash([0u8; 32]);

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Type alias for the `source_hash` field's API-edge form.
pub type SourceHash = Hash;

/// Helper returning [`Hash::NONE`] in API positions expecting a `Hash`.
/// Convenience for callers that previously wrote `NO_SOURCE_HASH` as
/// a raw `[u8; 32]` literal.
pub const fn no_source_hash() -> Hash {
    Hash::NONE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_all_zeros() {
        assert_eq!(Hash::NONE.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn round_trip_bytes() {
        let bytes = [42u8; 32];
        let h = Hash::from_bytes(bytes);
        assert_eq!(h.as_bytes(), &bytes);
        assert_eq!(h.into_bytes(), bytes);
    }

    #[test]
    fn const_construction() {
        const SENTINEL: Hash = Hash::NONE;
        const FROM_LITERAL: Hash = Hash::from_bytes([1u8; 32]);
        assert_eq!(SENTINEL, Hash::NONE);
        assert_ne!(FROM_LITERAL, Hash::NONE);
    }

    #[test]
    fn serde_round_trip() {
        let h = Hash::from_bytes([7u8; 32]);
        let json = serde_json::to_string(&h).unwrap();
        let recovered: Hash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, recovered);
    }

    #[test]
    fn no_source_hash_helper_returns_none() {
        assert_eq!(no_source_hash(), Hash::NONE);
    }
}
