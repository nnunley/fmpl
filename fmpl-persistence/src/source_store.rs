//! Content-addressed source store.
//!
//! Stores arbitrary source bytes keyed by their blake3 content hash.
//! Same bytes → same hash → one stored copy (de-duplication). Used by
//! the envelope writer to attach a `source_hash` to every persisted
//! record that has source provenance, and by
//! [`crate::loader::recover_incompatible`] to recover values whose
//! payload schema has drifted but whose source is still recoverable.
//!
//! Per ITER-0005b STORY-0100 AC-1 + the iteration's R-J-C-1 PAR
//! resolution: `SourceStore` wraps `FjallStore` concretely (not via a
//! generic `Store + Send + Sync` bound) because `compact()` requires
//! the native `fjall::Keyspace::remove` API and promoting `remove`
//! into the `Store` trait would burden every future backend impl
//! with delete semantics. If a non-fjall backend is ever wanted,
//! either promote `remove` to the trait OR introduce a
//! `RemovableStore` supertrait — both are breaking API changes that
//! deserve their own iteration.

#![cfg(feature = "fjall-backend")]

use crate::Store;
use crate::StoreError;
use crate::fjall_backend::FjallStore;
use crate::hash_compute::hash_bytes;
use fmpl_types::Hash;
use std::collections::HashSet;
use std::path::Path;

/// Content-addressed wrapper over a [`FjallStore`].
pub struct SourceStore {
    fjall: FjallStore,
}

/// Counts returned by [`SourceStore::compact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactStats {
    /// Number of distinct hashes the caller supplied as referenced.
    /// (Includes hashes that were already absent from the store —
    /// the caller may know about hashes the store never saw.)
    pub retained: usize,
    /// Number of records actually removed from the store because
    /// they were present AND not in the referenced set.
    pub removed: usize,
}

impl SourceStore {
    /// Open (or create) a `SourceStore` at `path`.
    ///
    /// The path is opened as a `FjallStore` keyspace — typically a
    /// sibling subdirectory of the caller's data directory (e.g.
    /// `SourceStore::open(&data_dir.join("sources"))`), mirroring
    /// the `ContinuationStore` / `ImageStore` layout pattern.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Ok(Self {
            fjall: FjallStore::open(path)?,
        })
    }

    /// Insert `bytes` into the source store. Returns the
    /// content-hash. Idempotent: re-inserting the same bytes returns
    /// the same hash and is observably a no-op (the store already
    /// has the record under that key).
    pub fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let h = hash_bytes(bytes);
        self.fjall.insert(h.as_bytes(), bytes)?;
        Ok(h)
    }

    /// Retrieve the source bytes stored under `hash`. Returns
    /// `Ok(None)` if absent; `Err` only on backend failure.
    pub fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        self.fjall.get(hash.as_bytes())
    }

    /// Remove every record in the store whose hash is NOT in
    /// `referenced`. Returns statistics: how many of the caller's
    /// supplied hashes were retained, and how many records the store
    /// actually deleted.
    ///
    /// The caller is responsible for collecting the set of
    /// still-referenced hashes from whatever keyspaces they care
    /// about — this method does not know how to walk envelopes;
    /// it just acts on the supplied set. The orchestration of
    /// "scan all keyspaces for referenced source_hash fields" is the
    /// AC-7 GC cycle and lives outside this primitive (deferred to
    /// ITER-0005b-GC).
    ///
    /// Uses the `#[doc(hidden)] FjallStore::keyspace()` escape hatch
    /// to access `fjall::Keyspace::remove` directly. The `Store`
    /// trait deliberately doesn't expose delete (per ITER-0005a.6 R-
    /// H-C-1's "trait stays sized to actual consumers" discipline);
    /// `compact()` is the one consumer that needs it.
    pub fn compact(
        &self,
        referenced: impl IntoIterator<Item = Hash>,
    ) -> Result<CompactStats, StoreError> {
        let keep: HashSet<[u8; 32]> = referenced.into_iter().map(|h| h.into_bytes()).collect();

        // First pass: collect keys that should be removed. We must NOT
        // remove while iterating — the fjall iterator's behavior under
        // concurrent delete isn't a contract we want to depend on.
        let mut to_remove: Vec<Vec<u8>> = Vec::new();
        for item in self.fjall.iter() {
            let (key, _value) = item?;
            // SourceStore keys are always 32-byte hashes by construction;
            // any non-32-byte key is foreign and we leave it alone.
            if key.len() == 32 {
                let arr: [u8; 32] = key
                    .as_slice()
                    .try_into()
                    .expect("len-checked above; infallible");
                if !keep.contains(&arr) {
                    to_remove.push(key);
                }
            }
        }

        let removed = to_remove.len();
        let ks = self.fjall.keyspace();
        for k in to_remove {
            ks.remove(k).map_err(StoreError::from)?;
        }

        Ok(CompactStats {
            retained: keep.len(),
            removed,
        })
    }
}

#[cfg(test)]
mod tests {
    //! In-source unit tests for SourceStore. End-to-end tests against
    //! a real on-disk FjallStore live in `tests/source_store.rs`.

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn put_then_get_roundtrips() {
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        let h = store.put(b"some source").unwrap();
        let back = store.get(h).unwrap();
        assert_eq!(back.as_deref(), Some(&b"some source"[..]));
    }

    #[test]
    fn put_same_bytes_twice_yields_same_hash_and_one_entry() {
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        let h1 = store.put(b"dup me").unwrap();
        let h2 = store.put(b"dup me").unwrap();
        assert_eq!(h1, h2, "identical bytes must yield identical hashes");

        // Count records under SourceStore keys (32-byte) to confirm
        // deduplication at the store level.
        let count = store
            .fjall
            .iter()
            .filter_map(|r| r.ok())
            .filter(|(k, _)| k.len() == 32)
            .count();
        assert_eq!(count, 1, "duplicate put must not insert a second record");
    }

    #[test]
    fn get_missing_hash_returns_none() {
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        let absent = Hash::from_bytes([0xAB; 32]);
        assert_eq!(store.get(absent).unwrap(), None);
    }

    #[test]
    fn compact_removes_unreferenced_keeps_referenced() {
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        let keep_h = store.put(b"keep this").unwrap();
        let drop_h = store.put(b"drop this").unwrap();
        assert_ne!(keep_h, drop_h);

        let stats = store.compact([keep_h]).unwrap();
        assert_eq!(stats.retained, 1);
        assert_eq!(stats.removed, 1);

        assert_eq!(
            store.get(keep_h).unwrap().as_deref(),
            Some(&b"keep this"[..])
        );
        assert_eq!(store.get(drop_h).unwrap(), None);
    }

    #[test]
    fn compact_with_empty_referenced_wipes_everything() {
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        for s in [&b"a"[..], b"b", b"c"] {
            store.put(s).unwrap();
        }
        let stats = store.compact(std::iter::empty()).unwrap();
        assert_eq!(stats.retained, 0);
        assert_eq!(stats.removed, 3);

        let remaining = store
            .fjall
            .iter()
            .filter_map(|r| r.ok())
            .filter(|(k, _)| k.len() == 32)
            .count();
        assert_eq!(
            remaining, 0,
            "compact with empty set must remove all records"
        );
    }

    #[test]
    fn compact_handles_referenced_hashes_not_in_store() {
        // Caller may supply hashes for sources the store never saw
        // (e.g. they were referenced by an envelope but the source
        // bytes were never persisted, or were already compacted out
        // in a prior run). `retained` counts the caller's supplied
        // set regardless of presence; `removed` counts only actually-
        // present-and-deleted records.
        let dir = tempdir().unwrap();
        let store = SourceStore::open(dir.path()).unwrap();
        let real_h = store.put(b"real").unwrap();
        let phantom_h = Hash::from_bytes([0x77; 32]);

        let stats = store.compact([real_h, phantom_h]).unwrap();
        assert_eq!(stats.retained, 2, "both supplied hashes count as retained");
        assert_eq!(
            stats.removed, 0,
            "no records to remove — only one was present and it's referenced"
        );

        assert_eq!(store.get(real_h).unwrap().as_deref(), Some(&b"real"[..]));
    }
}
