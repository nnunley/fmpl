//! End-to-end integration tests for `SourceStore` against a real
//! on-disk FjallStore.
//!
//! Unit tests against the same SourceStore live in-source at
//! `fmpl-persistence/src/source_store.rs::tests`. These tests
//! duplicate a subset of those scenarios to confirm behavior holds
//! when the store is opened/closed across the test boundary
//! (catches any quirks in fjall's persistence story).

#![cfg(feature = "fjall-backend")]

use fmpl_persistence::{Hash, SourceStore};
use fmpl_types::Hash as FmplHash;

// Re-export the Hash type for clarity. fmpl-persistence's prelude
// re-exports it from fmpl-types via the public API; this `use` line
// pins the test's expectation of the type's origin.
#[allow(dead_code)]
fn _hash_type_check(_: FmplHash) {}

#[test]
fn open_put_get_survives_within_one_process() {
    let dir = tempfile::tempdir().unwrap();
    let store = SourceStore::open(dir.path()).unwrap();
    let h = store.put(b"end-to-end source bytes").unwrap();
    let read_back = store.get(h).unwrap();
    assert_eq!(read_back.as_deref(), Some(&b"end-to-end source bytes"[..]));
}

#[test]
fn put_persists_across_store_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();

    let saved_hash = {
        let store = SourceStore::open(&path).unwrap();
        store.put(b"persisted across reopen").unwrap()
    };

    // Reopen the same path; the source must still be there.
    let store2 = SourceStore::open(&path).unwrap();
    let read_back = store2.get(saved_hash).unwrap();
    assert_eq!(
        read_back.as_deref(),
        Some(&b"persisted across reopen"[..]),
        "SourceStore must survive a close-reopen cycle"
    );
}

#[test]
fn dedup_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();

    let h1 = {
        let store = SourceStore::open(&path).unwrap();
        store.put(b"identical bytes").unwrap()
    };
    let h2 = {
        let store = SourceStore::open(&path).unwrap();
        store.put(b"identical bytes").unwrap()
    };
    assert_eq!(h1, h2, "content-hash must match across store sessions");

    // Confirm there's still exactly one record after the second put.
    // We can't directly count without going through the fjall
    // keyspace, so use compact with the known hash as the only
    // referenced one and verify removed == 0 (= no extras).
    let store3 = SourceStore::open(&path).unwrap();
    let stats = store3.compact([h1]).unwrap();
    assert_eq!(
        stats.removed, 0,
        "second put of identical bytes must not have created a second record"
    );
}

#[test]
fn compact_persists_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();

    let keep_h;
    let drop_h;
    {
        let store = SourceStore::open(&path).unwrap();
        keep_h = store.put(b"survive").unwrap();
        drop_h = store.put(b"perish").unwrap();
        let stats = store.compact([keep_h]).unwrap();
        assert_eq!(stats.removed, 1);
    }

    // Reopen — the deletion must have persisted.
    let store2 = SourceStore::open(&path).unwrap();
    assert_eq!(
        store2.get(keep_h).unwrap().as_deref(),
        Some(&b"survive"[..])
    );
    assert_eq!(
        store2.get(drop_h).unwrap(),
        None,
        "compacted record must stay gone across reopen"
    );
}

/// Sanity: caller can hand back the Hash from put() and recover the
/// exact bytes. Content-address invariant.
#[test]
fn content_address_round_trip_holds() {
    let dir = tempfile::tempdir().unwrap();
    let store = SourceStore::open(dir.path()).unwrap();
    let original = b"the content-address invariant: bytes -> hash -> bytes is identity";
    let h = store.put(original).unwrap();
    let recovered = store.get(h).unwrap().expect("just put it; must be present");
    assert_eq!(&recovered[..], original);
    assert_eq!(h, Hash::from_bytes(*blake3::hash(original).as_bytes()));
}
