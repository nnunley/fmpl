//! Native-backend coverage for the `FjallStore::is_empty()` override
//! shipped in ITER-0005a.6 T0.
//!
//! The in-source tests at `src/store.rs` cover the DEFAULT impl via
//! a synthetic `ScriptedStore`. This file covers the `FjallStore`
//! override (which calls fjall v3's native `Keyspace::is_empty()`)
//! against a real on-disk keyspace, closing the R-G-S-1 closing-PAR
//! gap.

#![cfg(feature = "fjall-backend")]

use fmpl_persistence::Store;
use fmpl_persistence::fjall_backend::FjallStore;

/// Fresh store reports `is_empty() == true`; after a single insert,
/// reports `false`. Tests the native-backend path end-to-end.
#[test]
fn fjall_store_is_empty_flips_on_first_insert() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FjallStore::open(dir.path()).expect("FjallStore::open");

    assert!(
        store.is_empty().expect("is_empty before insert"),
        "fresh FjallStore must report is_empty == true"
    );

    store.insert(b"k", b"v").expect("insert");

    assert!(
        !store.is_empty().expect("is_empty after insert"),
        "FjallStore with one record must report is_empty == false"
    );
}
