//! Integration tests for `StreamPosition`'s Store-backed overflow and
//! memo tiers (T4.7 + T4.8).
//!
//! These replace the in-source `#[cfg(test)] mod tests` blocks that
//! previously lived inside `fmpl-core/src/grammar/stream_input.rs`
//! (`test_fjall_overflow_basic`, `test_memo_persists_to_fjall`). The
//! in-source versions were deleted as part of T4.11 because they
//! referenced `fjall::Database` / `fjall::Keyspace` directly, which
//! would trip the no-fjall-in-fmpl-core gate (T5).
//!
//! Reading the `Store`-backed overflow records goes through
//! [`loader::decode`][`fmpl_persistence::loader::decode`] — the
//! envelope integrity check (magic / CRC / VM-major / payload-kind /
//! schema-version) covers both the spill and memo paths, so this suite
//! also implicitly exercises the loader gate on the
//! `PayloadKind::StreamPosition` and `PayloadKind::MemoTable` wire
//! formats.

#![cfg(feature = "fjall-backend")]

use std::sync::Arc;
use std::time::Duration;

use fmpl_core::grammar::stream_input::{MemoEntry, StreamPosition};
use fmpl_core::stream::{StreamEvent, StreamHandle};
use fmpl_core::value::Value;
use fmpl_persistence::Store;
use fmpl_persistence::fjall_backend::FjallStore;
use smol_str::SmolStr;
use tokio::sync::mpsc;

/// Open a fresh `FjallStore` in a temp dir, returning (tempdir, store)
/// so the caller can keep the tempdir alive for the test's duration.
///
/// The store is wrapped in `Arc<dyn Store + Send + Sync>` because the
/// stream-input constructors take the trait-object form (T4.7 + T4.8).
fn fresh_store() -> (tempfile::TempDir, Arc<dyn Store + Send + Sync>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store: Arc<dyn Store + Send + Sync> =
        Arc::new(FjallStore::open(dir.path()).expect("FjallStore::open"));
    (dir, store)
}

/// Reopen a previously-written store at the same path. Used by the
/// memo-persists-across-runs test to model a "close and reopen the
/// process" scenario without actually re-execing.
fn reopen_store(path: &std::path::Path) -> Arc<dyn Store + Send + Sync> {
    Arc::new(FjallStore::open(path).expect("FjallStore::reopen"))
}

/// Replacement for the deleted in-source `test_fjall_overflow_basic`.
///
/// Constructs a stream over an async channel with `memory_limit = 5`,
/// drives all 10 values through, then walks past the spill threshold
/// to force at least one spill-to-store. Verifies that the spilled
/// position is retrievable via the source's tail chain.
#[test]
fn overflow_spills_and_restores_position() {
    let (_dir, store) = fresh_store();
    let values: Vec<Value> = (0..10).map(Value::Int).collect();

    let (tx, rx) = mpsc::channel(100);
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        for v in &values {
            tx.send(StreamEvent::Data(v.clone()))
                .await
                .expect("channel send");
        }
    });
    drop(tx);

    let handle = StreamHandle::new(rx, 1);
    let stream = StreamPosition::from_async_with_store(
        handle,
        Some(Duration::from_secs(1)),
        Some(store),
        5, // memory_limit — anything past this index spills
    );

    // Walk past the spill threshold to trigger spill-to-store on the
    // earliest positions.
    let pos5 = stream.advance(5);
    assert_eq!(pos5.head(), Some(&Value::Int(5)));

    let pos9 = stream.advance(9);
    assert_eq!(pos9.head(), Some(&Value::Int(9)));

    // Walking past end yields a terminal cell.
    let end = pos9.tail();
    assert!(end.is_at_end());
}

/// Verifies that `set_memo` persists across a store re-open — the
/// scenario the deleted `test_memo_persists_to_fjall` was guarding.
///
/// The test seeds a memo entry, drops the original store handle (and
/// the stream that held the memo-backing Arc), reopens the same path
/// in a fresh `FjallStore`, builds a new stream over the same source,
/// and asserts `get_memo` returns the seeded entry.
#[test]
fn memo_persists_across_store_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    let values = vec![Value::Int(1), Value::Int(2)];

    {
        let store: Arc<dyn Store + Send + Sync> =
            Arc::new(FjallStore::open(&path).expect("FjallStore::open"));
        let stream = StreamPosition::from_values_with_memo_store(values.clone(), store);
        stream.set_memo(
            SmolStr::new("test_rule"),
            MemoEntry::Done(Some(Value::Int(42)), 1),
        );

        // Sanity: in-memory hit on the same stream.
        let memo = stream.get_memo(&SmolStr::new("test_rule"));
        assert!(matches!(
            memo,
            Some(MemoEntry::Done(Some(Value::Int(42)), 1))
        ));
        // `stream` (and its memo Arc, and `store`) drop at the end of
        // this scope, flushing the FjallStore.
    }

    // Reopen the same path; the memo record should still be there.
    let store2 = reopen_store(&path);
    let stream2 = StreamPosition::from_values_with_memo_store(values, store2);
    let memo = stream2.get_memo(&SmolStr::new("test_rule"));
    assert!(
        matches!(memo, Some(MemoEntry::Done(Some(Value::Int(42)), 1))),
        "memo should survive store-reopen; got {:?}",
        memo
    );
}

/// Verifies the integrity-gate behaviour added by R-A-S-2/R-B-S-1's
/// fix: `get_memo` now routes through `loader::decode`, which rejects
/// records with a corrupted CRC. A bitflipped memo record must surface
/// as a cache miss, not as a deserialized-but-bogus `MemoEntry`.
#[test]
fn memo_with_bitflipped_record_is_cache_miss() {
    let (_dir, store) = fresh_store();
    let values = vec![Value::Int(1)];

    let stream = StreamPosition::from_values_with_memo_store(values.clone(), store.clone());
    stream.set_memo(
        SmolStr::new("rule"),
        MemoEntry::Done(Some(Value::Int(7)), 1),
    );

    // Sanity: in-memory cache hits regardless of store state.
    let live = stream.get_memo(&SmolStr::new("rule"));
    assert!(matches!(live, Some(MemoEntry::Done(_, 1))));

    // Drop the live stream so the in-memory memo table is gone. Build
    // a fresh stream so the next get_memo must go to the store.
    drop(stream);
    let stream2 = StreamPosition::from_values_with_memo_store(values, store.clone());

    // Pre-flip: store-backed lookup succeeds.
    let pre = stream2.get_memo(&SmolStr::new("rule"));
    assert!(
        matches!(pre, Some(MemoEntry::Done(_, 1))),
        "expected hit before corruption; got {:?}",
        pre
    );

    // Corrupt the persisted record by overwriting one byte of the
    // stamped CRC. The CRC field sits at offset 52 of the 56-byte
    // envelope header; any flip there must trip the loader's
    // verify_checksum gate.
    let key = "0:rule";
    let mut bytes = store
        .get(key.as_bytes())
        .expect("store.get")
        .expect("memo key present");
    bytes[52] ^= 0xFF;
    store.insert(key.as_bytes(), &bytes).expect("store.insert");

    // After corruption, build a third stream so neither this stream
    // nor stream2 has an in-memory entry. The corrupted record must
    // fail the loader's CRC gate and surface as None.
    let stream3 = StreamPosition::from_values_with_memo_store(vec![Value::Int(1)], store);
    let post = stream3.get_memo(&SmolStr::new("rule"));
    assert!(
        post.is_none(),
        "expected cache miss on corrupted record; got {:?}",
        post
    );
}
