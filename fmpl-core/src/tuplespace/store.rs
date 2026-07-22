//! Tuple space store with blocking operations and optional Fjall
//! persistence.
//!
//! Two construction paths:
//!
//! - [`TupleSpace::new`] — pure in-memory. No I/O, no store. `out` calls
//!   with `durable: true` are an error (no store to write to).
//!
//! - [`TupleSpace::open`] — opens (or creates) a fjall-backed keyspace
//!   at the given path and replays any existing tuples into the
//!   in-memory `BTreeMap` on construction. Subsequent `out` calls with
//!   `durable: true` write an envelope record under
//!   [`Tuple::store_key`]; `in` / `inp` remove the matched record from
//!   the store as well as the in-memory map. Only available when the
//!   `persistence` feature is enabled.
//!
//! The in-memory `BTreeMap` remains authoritative for queries; the
//! store is a write-through durability layer. Pattern-match queries
//! never touch the store — a deliberate scoping decision: pattern
//! matching against stored records is its own concern (would need a
//! separate abstraction, not part of the `Store` trait).

use crate::error::{Error, Result};
use crate::stream::StreamHandle;
use crate::tuplespace::{Tuple, TuplePattern};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Subscriber to tuple space events.
#[derive(Debug)]
pub struct TupleSubscriber {
    pattern: TuplePattern,
    sender: tokio::sync::mpsc::Sender<crate::stream::StreamEvent>,
}

/// In-memory tuple space with optional Fjall persistence.
///
/// The in-memory `BTreeMap` is always authoritative for queries (`rd`,
/// `in`, `inp`, `rdp`). When `backing` is `Some`, durable `out` calls
/// also write a `PayloadKind::Tuple` envelope record, and destructive
/// reads remove from both the map and the store.
pub struct TupleSpace {
    /// Next sequence number
    next_seq: Arc<AtomicU64>,
    /// Tuples by (namespace, type, seq)
    tuples: BTreeMap<(Option<SmolStr>, SmolStr, u64), Tuple>,
    /// Stream subscribers
    subscribers: Arc<Mutex<Vec<TupleSubscriber>>>,
    /// Optional fjall-backed durable store. `None` for in-memory spaces
    /// created via [`Self::new`]. `Some` for spaces opened via
    /// [`Self::open`] (only available with the `persistence` feature).
    #[cfg(feature = "persistence")]
    backing: Option<fmpl_persistence::fjall_backend::FjallStore>,
}

// `FjallStore` doesn't derive `Debug` (fjall keyspaces hold OS handles
// that don't either), so we hand-write a Debug impl that prints
// observable shape without recursing into the backing store.
impl std::fmt::Debug for TupleSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("TupleSpace");
        d.field("tuple_count", &self.tuples.len())
            .field("next_seq", &self.next_seq.load(Ordering::SeqCst));
        #[cfg(feature = "persistence")]
        d.field("durable", &self.backing.is_some());
        d.finish()
    }
}

impl TupleSpace {
    /// Create a new pure-in-memory tuple space. `out` with `durable:
    /// true` will error: no store to write to.
    pub fn new() -> Self {
        Self {
            next_seq: Arc::new(AtomicU64::new(1)),
            tuples: BTreeMap::new(),
            subscribers: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "persistence")]
            backing: None,
        }
    }

    /// Open (or create) a durable tuple space at `path`. Replays any
    /// existing persisted tuples into the in-memory map on construction;
    /// `next_seq` is bumped past the highest seen `seq` so new tuples
    /// don't collide.
    ///
    /// Only available with the `persistence` feature. The path is
    /// opened as a fjall keyspace; fjall takes a single-writer file
    /// lock, so two processes cannot hold the same path open
    /// simultaneously.
    #[cfg(feature = "persistence")]
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        use fmpl_persistence::fjall_backend::FjallStore;
        use fmpl_persistence::loader::iter_store;
        use fmpl_persistence::schema::PayloadKind;

        let store = FjallStore::open(path.as_ref()).map_err(|e| {
            Error::Runtime(format!("tuplespace.open({}): {e}", path.as_ref().display()))
        })?;

        // Replay: walk every record, filter to PayloadKind::Tuple,
        // deserialize the payload, and populate the in-memory map.
        // Other PayloadKinds in the same keyspace (if any are ever
        // mixed in by future work) are skipped harmlessly via the
        // loader's already-tested unknown-kind-skip path.
        //
        // `iter_store`'s callback can't return Result, so we capture
        // any deserialize error in a local Option and check it after.
        let mut tuples: BTreeMap<(Option<SmolStr>, SmolStr, u64), Tuple> = BTreeMap::new();
        let mut max_seq: u64 = 0;
        let mut deserialize_error: Option<String> = None;

        iter_store(&store, crate::VM_VERSION.major, |_key, record| {
            if deserialize_error.is_some() {
                return;
            }
            if record.kind != PayloadKind::Tuple {
                return;
            }
            match serde_json::from_slice::<Tuple>(record.payload) {
                Ok(tuple) => {
                    max_seq = max_seq.max(tuple.seq);
                    let key = (tuple.namespace.clone(), tuple.type_name.clone(), tuple.seq);
                    tuples.insert(key, tuple);
                }
                Err(e) => {
                    deserialize_error = Some(format!("tuplespace.open replay deserialize: {e}"));
                }
            }
        })
        .map_err(|e| Error::Runtime(format!("tuplespace.open replay: {e}")))?;

        if let Some(msg) = deserialize_error {
            return Err(Error::Runtime(msg));
        }

        Ok(Self {
            next_seq: Arc::new(AtomicU64::new(max_seq + 1)),
            tuples,
            subscribers: Arc::new(Mutex::new(Vec::new())),
            backing: Some(store),
        })
    }

    /// Returns `true` if this space has a durable backing store.
    #[cfg(feature = "persistence")]
    pub fn is_durable(&self) -> bool {
        self.backing.is_some()
    }

    /// Returns `false` when the feature is off (no way to be durable).
    #[cfg(not(feature = "persistence"))]
    pub fn is_durable(&self) -> bool {
        false
    }

    /// Subscribe to tuples matching a pattern.
    ///
    /// Returns a StreamHandle that will receive matching tuples as they are added.
    pub fn subscribe(&self, pattern: TuplePattern) -> StreamHandle {
        use crate::stream::{StreamSource, next_id};

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let id = next_id();
        let handle = StreamHandle::with_source(rx, id, StreamSource::Ephemeral);

        let subscriber = TupleSubscriber {
            pattern,
            sender: tx,
        };
        self.subscribers.lock().unwrap().push(subscriber);

        handle
    }

    /// Write a tuple to the space.
    ///
    /// In-memory `BTreeMap` is always updated. When the tuple is
    /// `durable: true` AND this space has a backing store, the tuple
    /// is also written through to the store as a `PayloadKind::Tuple`
    /// envelope record under `tuple.store_key()`.
    ///
    /// Errors:
    /// - `durable: true` with no backing store: hard error (would
    ///   silently drop durability — better to fail loudly).
    /// - Store write failure: surfaces as a runtime error; the
    ///   in-memory write has already landed so the space is observably
    ///   ahead of disk. Phantoms are tolerated by the replay path;
    ///   stricter durability semantics belong to a future change.
    pub fn out(&mut self, mut tuple: Tuple) -> Result<()> {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        // Stamp the assigned seq onto the tuple BEFORE we hash its key
        // for the backing store. Otherwise the store key would be
        // derived from seq=0 (the default), collide for every tuple,
        // and replay would produce a single-entry map.
        tuple.seq = seq;

        // Durability check + write-through. Performed before the
        // in-memory insert so a "durable: true without backing" error
        // doesn't leave the in-memory map ahead of the user's intent.
        if tuple.durable {
            #[cfg(feature = "persistence")]
            {
                let backing = self.backing.as_ref().ok_or_else(|| {
                    Error::Runtime(
                        "tuple has durable: true but tuplespace has no backing store; \
                         use tuplespace.open(path) to create a durable space"
                            .to_string(),
                    )
                })?;
                use fmpl_persistence::Hash;
                use fmpl_persistence::envelope::write as envelope_write;
                use fmpl_persistence::schema::PayloadKind;
                envelope_write(
                    backing,
                    &tuple.store_key(),
                    &tuple,
                    PayloadKind::Tuple,
                    crate::VM_VERSION,
                    Hash::NONE,
                )
                .map_err(|e| Error::Runtime(format!("durable out write: {e}")))?;
            }
            #[cfg(not(feature = "persistence"))]
            {
                return Err(Error::Runtime(
                    "tuple has durable: true but the persistence feature is not enabled"
                        .to_string(),
                ));
            }
        }

        let key = (tuple.namespace.clone(), tuple.type_name.clone(), seq);
        self.tuples.insert(key, tuple.clone());

        // Notify subscribers
        self.notify_subscribers(&tuple);

        Ok(())
    }

    /// Notify all subscribers of a new tuple.
    fn notify_subscribers(&self, tuple: &Tuple) {
        use crate::stream::StreamEvent;
        use crate::value::Value;
        use std::collections::HashMap;
        use std::sync::Arc;

        let mut subscribers = self.subscribers.lock().unwrap();
        let mut i = 0;
        while i < subscribers.len() {
            let subscriber = &subscribers[i];
            if subscriber.sender.is_closed() {
                // Remove closed subscribers
                subscribers.remove(i);
            } else if subscriber.pattern.matches(tuple) {
                // Convert tuple to Value for streaming
                let mut map = HashMap::new();
                map.insert(SmolStr::new("type"), Value::String(tuple.type_name.clone()));
                if let Some(ns) = &tuple.namespace {
                    map.insert(SmolStr::new("namespace"), Value::String(ns.clone()));
                }
                map.insert(SmolStr::new("data"), tuple.data.clone());

                let _ = subscriber
                    .sender
                    .try_send(StreamEvent::Data(Value::Map(Arc::new(map))));
                i += 1;
            } else {
                i += 1;
            }
        }
    }

    /// Remove a matching tuple (blocking).
    pub fn r#in(&mut self, pattern: &TuplePattern) -> Result<Tuple> {
        // For now, non-blocking implementation
        self.inp(pattern)?
            .ok_or_else(|| Error::Runtime("no matching tuple found".to_string()))
    }

    /// Read a matching tuple (blocking, non-destructive).
    pub fn rd(&mut self, pattern: &TuplePattern) -> Result<Tuple> {
        // For now, non-blocking implementation
        self.rdp(pattern)?
            .ok_or_else(|| Error::Runtime("no matching tuple found".to_string()))
    }

    /// Non-blocking remove (returns None if no match).
    ///
    /// When this space has a backing store AND the matched tuple was
    /// stored as durable, also removes the on-disk record under
    /// `tuple.store_key()`.
    ///
    /// Note: the in-memory remove happens FIRST. If the on-disk remove
    /// then fails, the in-memory map is observably ahead of disk: the
    /// caller sees the tuple consumed, but a future replay would
    /// resurrect it. This is the same tolerated-phantom story as the
    /// out path: replay-on-open can read a phantom tuple as a real
    /// one; it remains queryable and `in`-able again, which is the
    /// least-surprise behavior. Per scope doc: aggressive consistency
    /// is a future iteration.
    pub fn inp(&mut self, pattern: &TuplePattern) -> Result<Option<Tuple>> {
        // Find the match position without holding a borrow during the
        // mutate. Walking iter() then calling remove() is fine because
        // we exit the loop on first match.
        let found_key = self
            .tuples
            .iter()
            .find(|(_, tuple)| pattern.matches(tuple))
            .map(|((ns, type_name, seq), tuple)| {
                ((ns.clone(), type_name.clone(), *seq), tuple.clone())
            });

        let Some((map_key, tuple)) = found_key else {
            return Ok(None);
        };

        self.tuples.remove(&map_key);

        #[cfg(feature = "persistence")]
        if tuple.durable
            && let Some(backing) = self.backing.as_ref()
        {
            // Reach for the fjall escape hatch the same way
            // SourceStore::compact does. The Store trait deliberately
            // doesn't expose remove; the documented escape hatch on
            // FjallStore is the principled call for consumers that
            // need delete semantics.
            backing
                .keyspace()
                .remove(tuple.store_key())
                .map_err(|e| Error::Runtime(format!("durable in remove: {e}")))?;
        }

        Ok(Some(tuple))
    }

    /// Non-blocking read (returns None if no match).
    pub fn rdp(&mut self, pattern: &TuplePattern) -> Result<Option<Tuple>> {
        for tuple in self.tuples.values() {
            if pattern.matches(tuple) {
                return Ok(Some(tuple.clone()));
            }
        }
        Ok(None)
    }
}

impl Default for TupleSpace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use crate::tuplespace::Pattern;
    use crate::value::Value;
    use smol_str::SmolStr;

    #[test]
    fn test_out_and_in() {
        let mut space = TupleSpace::new();
        let tuple = Tuple::new(SmolStr::new("test"), Value::Int(42));
        space.out(tuple.clone()).unwrap();

        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("test"),
            data: Pattern::Wildcard,
        };

        let result = space.r#in(&pattern).unwrap();
        assert_eq!(result.data, Value::Int(42));
    }

    #[test]
    fn test_in_returns_fifo() {
        let mut space = TupleSpace::new();
        space
            .out(Tuple::new(SmolStr::new("test"), Value::Int(1)))
            .unwrap();
        space
            .out(Tuple::new(SmolStr::new("test"), Value::Int(2)))
            .unwrap();

        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("test"),
            data: Pattern::Wildcard,
        };

        assert_eq!(space.r#in(&pattern).unwrap().data, Value::Int(1));
        assert_eq!(space.r#in(&pattern).unwrap().data, Value::Int(2));
    }

    #[test]
    fn test_rdp_non_blocking() {
        let mut space = TupleSpace::new();

        let pattern = TuplePattern::Any;
        assert!(space.rdp(&pattern).unwrap().is_none());

        space
            .out(Tuple::new(SmolStr::new("test"), Value::Int(42)))
            .unwrap();
        assert!(space.rdp(&pattern).unwrap().is_some());
    }

    #[test]
    fn test_rd_is_non_destructive() {
        let mut space = TupleSpace::new();
        space
            .out(Tuple::new(SmolStr::new("test"), Value::Int(42)))
            .unwrap();

        let pattern = TuplePattern::Any;
        let result1 = space.rd(&pattern).unwrap();
        assert_eq!(result1.data, Value::Int(42));

        let result2 = space.rd(&pattern).unwrap();
        assert_eq!(result2.data, Value::Int(42));
    }

    #[test]
    fn test_in_is_destructive() {
        let mut space = TupleSpace::new();
        space
            .out(Tuple::new(SmolStr::new("test"), Value::Int(42)))
            .unwrap();

        let pattern = TuplePattern::Any;
        let result1 = space.r#in(&pattern).unwrap();
        assert_eq!(result1.data, Value::Int(42));

        let result2 = space.inp(&pattern).unwrap();
        assert!(result2.is_none());
    }

    #[test]
    fn test_subscribe_receives_matching_tuples() {
        let mut space = TupleSpace::new();

        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: Pattern::Wildcard,
        };
        let mut handle = space.subscribe(pattern.clone());

        // Add a matching tuple
        let tuple = Tuple::new(SmolStr::new("log"), Value::String(SmolStr::new("error")));
        space.out(tuple).unwrap();

        // Check subscriber received the tuple
        let event = handle.recv_blocking().unwrap();
        match event {
            StreamEvent::Data(Value::Map(map)) => {
                assert_eq!(map.get("type"), Some(&Value::String(SmolStr::new("log"))));
            }
            _ => panic!("Expected Data event with Map"),
        }
    }

    #[test]
    fn test_subscribe_filters_non_matching_tuples() {
        let mut space = TupleSpace::new();

        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: Pattern::Wildcard,
        };
        let mut handle = space.subscribe(pattern);

        // Add a non-matching tuple
        let tuple = Tuple::new(SmolStr::new("event"), Value::String(SmolStr::new("click")));
        space.out(tuple).unwrap();

        // Channel should be empty (non-blocking check)
        // We can't easily test this without tokio runtime, so we just verify no panic
        // The tuple was added but subscriber didn't receive it
        assert!(handle.receiver.try_recv().is_err());
    }

    #[test]
    fn test_subscribe_multiple_subscribers() {
        let mut space = TupleSpace::new();

        let pattern1 = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: Pattern::Wildcard,
        };
        let mut handle1 = space.subscribe(pattern1);

        let pattern2 = TuplePattern::Any;
        let mut handle2 = space.subscribe(pattern2);

        // Add a log tuple
        let tuple = Tuple::new(SmolStr::new("log"), Value::String(SmolStr::new("info")));
        space.out(tuple).unwrap();

        // Both subscribers should receive it
        let event1 = handle1.recv_blocking().unwrap();
        assert!(matches!(event1, StreamEvent::Data(_)));

        let event2 = handle2.recv_blocking().unwrap();
        assert!(matches!(event2, StreamEvent::Data(_)));
    }

    #[cfg(feature = "persistence")]
    mod durable {
        //! Durable TupleSpace round-trip through an on-disk fjall
        //! keyspace.
        use super::*;
        use tempfile::tempdir;

        #[test]
        fn durable_out_persists_across_reopen() {
            let dir = tempdir().unwrap();
            // First incarnation: open, out a durable tuple, drop.
            {
                let mut space = TupleSpace::open(dir.path()).unwrap();
                let tuple = Tuple::new(
                    SmolStr::new("greeting"),
                    Value::String(SmolStr::new("hello")),
                )
                .with_durable(true);
                space.out(tuple).unwrap();
            }
            // Second incarnation: open same path, rd the tuple.
            let mut space = TupleSpace::open(dir.path()).unwrap();
            let pattern = TuplePattern::TypeAndData {
                type_name: SmolStr::new("greeting"),
                data: Pattern::Wildcard,
            };
            let recovered = space.rd(&pattern).unwrap();
            assert_eq!(recovered.data, Value::String(SmolStr::new("hello")));
            assert!(recovered.durable);
        }

        #[test]
        fn non_durable_out_does_not_persist() {
            let dir = tempdir().unwrap();
            {
                let mut space = TupleSpace::open(dir.path()).unwrap();
                // durable=false: should NOT be written through to disk.
                let tuple = Tuple::new(SmolStr::new("ephemeral"), Value::Int(42));
                space.out(tuple).unwrap();
            }
            let mut space = TupleSpace::open(dir.path()).unwrap();
            let pattern = TuplePattern::Any;
            assert!(
                space.rdp(&pattern).unwrap().is_none(),
                "non-durable tuples must not survive process restart"
            );
        }

        #[test]
        fn durable_in_removes_from_store() {
            let dir = tempdir().unwrap();
            {
                let mut space = TupleSpace::open(dir.path()).unwrap();
                let t = Tuple::new(SmolStr::new("event"), Value::Int(1)).with_durable(true);
                space.out(t).unwrap();
                let pattern = TuplePattern::TypeAndData {
                    type_name: SmolStr::new("event"),
                    data: Pattern::Wildcard,
                };
                let consumed = space.r#in(&pattern).unwrap();
                assert_eq!(consumed.data, Value::Int(1));
            }
            // Reopen: the tuple should be gone from disk too.
            let mut space = TupleSpace::open(dir.path()).unwrap();
            let pattern = TuplePattern::Any;
            assert!(
                space.rdp(&pattern).unwrap().is_none(),
                "in()'d durable tuple must not resurrect on reopen"
            );
        }

        #[test]
        fn durable_out_on_non_durable_space_errors() {
            let mut space = TupleSpace::new();
            let tuple = Tuple::new(SmolStr::new("x"), Value::Null).with_durable(true);
            let err = space.out(tuple).unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.contains("durable") && msg.contains("no backing store"),
                "expected durable+no-backing error, got: {msg}"
            );
        }

        #[test]
        fn fifo_order_preserved_across_reopen() {
            // Property: tuples come back in the same FIFO order. The
            // store_key big-endian seq encoding + fjall's byte-ordered
            // iter guarantee this.
            let dir = tempdir().unwrap();
            {
                let mut space = TupleSpace::open(dir.path()).unwrap();
                for i in 1..=5 {
                    let t = Tuple::new(SmolStr::new("n"), Value::Int(i)).with_durable(true);
                    space.out(t).unwrap();
                }
            }
            let mut space = TupleSpace::open(dir.path()).unwrap();
            let pattern = TuplePattern::TypeAndData {
                type_name: SmolStr::new("n"),
                data: Pattern::Wildcard,
            };
            for expected in 1..=5 {
                let t = space.r#in(&pattern).unwrap();
                assert_eq!(t.data, Value::Int(expected), "FIFO order");
            }
        }
    }
}
