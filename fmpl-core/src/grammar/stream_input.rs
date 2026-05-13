//! Lazy, cons-cell input stream for grammar application.
//!
//! Each [`StreamPosition`] is an immutable node holding the value at one
//! cursor location plus a lazily-computed tail. The shape mirrors OMeta's
//! `OMInputStream` and maru's `<parser-stream>`: parsing advances by walking
//! cons cells, and a position can be retained as a backtrack anchor without
//! copying upstream input.
//!
//! Three source kinds are supported:
//! - `Async`: pulls from an mpsc channel with a configurable blocking
//!   timeout, optionally spilling old positions to a [`Store`][crate::persistence::Store]-
//!   backed overflow tier.
//! - `Static`: walks a fixed `Vec<Value>` with no blocking.
//! - `Empty`: a terminal sentinel.
//!
//! All positions carry a per-position memo table for packrat parsing; the
//! memo table optionally persists to a [`Store`][crate::persistence::Store]-
//! backed memo table so memoization survives across runs.

// Async carries channel state and an overflow handle; Static / Empty are
// thin. The size asymmetry is intentional.
#![allow(clippy::large_enum_variant)]

use crate::stream::{StreamEvent, StreamHandle};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

/// Default blocking-recv timeout for async stream sources. 30 s is chosen to
/// be long enough that a slow producer doesn't get falsely classified as
/// end-of-stream, but short enough that a wedged producer doesn't hang the
/// parser indefinitely. Use `None` for unbounded blocking.
pub const DEFAULT_STREAM_TIMEOUT: Option<Duration> = Some(Duration::from_secs(30));

/// One immutable cell in a lazy input stream.
///
/// A `StreamPosition` is the cursor type that grammars and packrat memo
/// tables key on. Each cell stores the value visible at this cursor plus a
/// lazily-computed pointer to the next cell. Multiple borrowing parsers can
/// hold `Rc<StreamPosition>` clones simultaneously (backtracking anchors,
/// memoized continuations) without copying upstream data.
///
/// # Lifetime / ownership
///
/// Cells are reference-counted (`Rc`). Interior mutability on `tail` and
/// `memo` is `RefCell` because parsing is single-threaded; the async source
/// uses `Mutex` only because the channel handle itself is `Send`.
///
/// # End of stream
///
/// `head == None` signals end-of-stream — a normal terminal state, *not*
/// corruption. Callers should treat it as a successful termination signal.
#[derive(Debug)]
pub struct StreamPosition {
    /// Value at this cursor; `None` iff this is the terminal cell.
    head: Option<Value>,
    /// Memoized next cell; populated on first call to [`tail`](Self::tail).
    tail: RefCell<Option<Rc<StreamPosition>>>,
    /// Zero-based position index used as a memo key and as the spill key.
    index: usize,
    /// Memo table keyed by rule name. Populated by packrat-style parsers.
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,
    /// Shared source descriptor — all cells of one stream point at the same
    /// `StreamSource`.
    source: Rc<StreamSource>,
    /// Optional persistent memo backing. When `Some`, [`get_memo`] /
    /// [`set_memo`] read through and write through to a Fjall partition so
    /// memoization survives process restarts.
    ///
    /// [`get_memo`]: Self::get_memo
    /// [`set_memo`]: Self::set_memo
    memo_store: Option<Arc<Mutex<MemoStore>>>,
}

/// Memoized outcome of one rule invocation at one position.
///
/// `InProgress` is a sentinel used to detect direct left recursion: when a
/// rule looks up its own memo at the position where it was just entered and
/// finds `InProgress`, the parser knows it must abort or apply seed-parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoEntry {
    /// This rule is currently being evaluated at this position. A second
    /// lookup of this entry indicates left recursion.
    InProgress,
    /// Rule completed. `(produced_value, end_position_index)`; a `None`
    /// produced value means the rule matched but produced no payload.
    Done(Option<Value>, usize),
}

/// Backing [`Store`][crate::persistence::Store] used to spill cold
/// positions out of memory. Trait-object form (`Arc<dyn Store + Send +
/// Sync>`) avoids a generic parameter cascade through `StreamSource`,
/// `StreamPosition`, and `Input::Position`.
struct OverflowStore(Arc<dyn crate::persistence::Store + Send + Sync>);

impl std::fmt::Debug for OverflowStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverflowStore").finish_non_exhaustive()
    }
}

/// Backing [`Store`][crate::persistence::Store] used to persist memo
/// entries. Trait-object form for the same reason as [`OverflowStore`].
struct MemoStore(Arc<dyn crate::persistence::Store + Send + Sync>);

impl std::fmt::Debug for MemoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoStore").finish_non_exhaustive()
    }
}

/// Backing source of stream values.
///
/// One `StreamSource` is shared (via `Rc`) by every cell of a single stream.
#[derive(Debug)]
enum StreamSource {
    /// Pulls values from an mpsc channel. Cell production is incremental and
    /// may block on the channel.
    Async {
        /// Receiver, behind a `Mutex` so cells can pull lazily from any
        /// thread holding the `Rc`.
        handle: Mutex<StreamHandle>,
        /// Per-recv timeout. `None` blocks indefinitely (use with care).
        timeout: Option<Duration>,
        /// In-memory position cache, scanned by index when a cell asks
        /// for its tail. Acts as the hot tier above the [`OverflowStore`]
        /// cold tier (see `overflow` below).
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        /// Cold tier; when `Some`, positions evicted from `positions` are
        /// spilled here so they can be re-materialized on a later access.
        overflow: Option<OverflowStore>,
        /// Maximum size of the in-memory cache before spilling. `None`
        /// disables spilling regardless of whether `overflow` is set.
        memory_limit: Option<usize>,
    },
    /// Fixed input. No blocking, no caching needed — cells walk `values`
    /// directly.
    Static(Vec<Value>),
    /// Already at end-of-stream.
    Empty,
}

impl StreamPosition {
    /// Returns a stream rooted on an async channel.
    ///
    /// Eagerly pulls the first value from `handle` so the returned cell has a
    /// concrete `head` (or `None` if the channel was already drained). All
    /// subsequent cells are produced lazily by [`tail`](Self::tail).
    ///
    /// `timeout` bounds each blocking receive. `None` blocks indefinitely.
    /// There is no overflow store backing this constructor — use
    /// [`from_async_with_store`](Self::from_async_with_store) for that.
    pub fn from_async(handle: StreamHandle, timeout: Option<Duration>) -> Rc<Self> {
        let source = Rc::new(StreamSource::Async {
            handle: Mutex::new(handle),
            timeout,
            positions: Mutex::new(Vec::new()),
            overflow: None,
            memory_limit: None,
        });

        // Pull the first value
        let head = Self::pull_next(&source);
        let pos = Rc::new(Self {
            head,
            tail: RefCell::new(None),
            index: 0,
            memo: RefCell::new(HashMap::new()),
            source: source.clone(),
            memo_store: None,
        });

        // Cache this position
        if let StreamSource::Async { positions, .. } = source.as_ref() {
            positions.lock().unwrap().push(pos.clone());
        }

        pos
    }

    /// Returns a stream rooted on an async channel, with cold positions
    /// spilled to a [`Store`][crate::persistence::Store].
    ///
    /// Behaves like [`from_async`](Self::from_async) but, when the in-memory
    /// position cache exceeds `memory_limit`, the oldest cells are
    /// serialized to `store` and reloaded on-demand. This bounds memory
    /// for long-running parses against streams whose backtrack window
    /// cannot be statically bounded.
    ///
    /// # Parameters
    ///
    /// * `handle`: source channel; eagerly drained for the head value.
    /// * `timeout`: per-recv blocking timeout; `None` blocks indefinitely.
    /// * `store`: overflow store. `None` disables spilling entirely
    ///   (equivalent to `from_async`).
    /// * `memory_limit`: cap on the in-memory position cache before spill.
    pub fn from_async_with_store(
        handle: StreamHandle,
        timeout: Option<Duration>,
        store: Option<Arc<dyn crate::persistence::Store + Send + Sync>>,
        memory_limit: usize,
    ) -> Rc<Self> {
        let overflow = store.map(OverflowStore);

        let source = Rc::new(StreamSource::Async {
            handle: Mutex::new(handle),
            timeout,
            positions: Mutex::new(Vec::new()),
            overflow,
            memory_limit: Some(memory_limit),
        });

        // Pull the first value
        let head = Self::pull_next(&source);
        let pos = Rc::new(Self {
            head,
            tail: RefCell::new(None),
            index: 0,
            memo: RefCell::new(HashMap::new()),
            source: source.clone(),
            memo_store: None,
        });

        // Cache this position
        if let StreamSource::Async { positions, .. } = source.as_ref() {
            positions.lock().unwrap().push(pos.clone());
        }

        pos
    }

    /// Returns a stream rooted on a fixed `Vec<Value>`.
    ///
    /// An empty `values` yields a terminal cell directly. No memo persistence;
    /// use [`from_values_with_memo_store`](Self::from_values_with_memo_store)
    /// for that.
    pub fn from_values(values: Vec<Value>) -> Rc<Self> {
        if values.is_empty() {
            return Self::end_of_stream(None);
        }

        let source = Rc::new(StreamSource::Static(values.clone()));
        Self::build_static_chain(source, &values, 0, None)
    }

    /// Returns a stream over fixed `values` with a [`Store`][crate::persistence::Store]-backed
    /// memo table.
    ///
    /// `store` is the memo backing. Memo entries written via
    /// [`set_memo`](Self::set_memo) are persisted there and surface
    /// again on subsequent runs against the same store — useful for
    /// incremental parses where prefix work should not be redone.
    pub fn from_values_with_memo_store(
        values: Vec<Value>,
        store: Arc<dyn crate::persistence::Store + Send + Sync>,
    ) -> Rc<Self> {
        if values.is_empty() {
            return Self::end_of_stream(None);
        }

        let memo_store = Some(Arc::new(Mutex::new(MemoStore(store))));

        let source = Rc::new(StreamSource::Static(values.clone()));
        Self::build_static_chain(source, &values, 0, memo_store)
    }

    /// Returns a terminal cell with no head and no source.
    fn end_of_stream(memo_store: Option<Arc<Mutex<MemoStore>>>) -> Rc<Self> {
        Rc::new(Self {
            head: None,
            tail: RefCell::new(None),
            index: 0,
            memo: RefCell::new(HashMap::new()),
            source: Rc::new(StreamSource::Empty),
            memo_store,
        })
    }

    /// Constructs a cell rooted at `values[index]`. Returns a terminal cell
    /// when `index >= values.len()`. The tail is left uncomputed; subsequent
    /// cells are produced on demand by [`tail`](Self::tail).
    fn build_static_chain(
        source: Rc<StreamSource>,
        values: &[Value],
        index: usize,
        memo_store: Option<Arc<Mutex<MemoStore>>>,
    ) -> Rc<Self> {
        if index >= values.len() {
            return Rc::new(Self {
                head: None,
                tail: RefCell::new(None),
                index,
                memo: RefCell::new(HashMap::new()),
                source,
                memo_store,
            });
        }

        Rc::new(Self {
            head: Some(values[index].clone()),
            tail: RefCell::new(None), // Lazily computed
            index,
            memo: RefCell::new(HashMap::new()),
            source,
            memo_store,
        })
    }

    /// Pulls one value from `source`.
    ///
    /// Returns `None` for any of: end-of-channel, timeout, error event, or a
    /// non-`Async` source. The distinction between these is not surfaced —
    /// from the parser's point of view all four mean "no further input
    /// available at this position".
    ///
    /// Tries a non-blocking `try_recv` first to avoid runtime overhead when
    /// the producer is keeping up, and only falls back to a blocking recv
    /// when the channel is momentarily empty.
    fn pull_next(source: &StreamSource) -> Option<Value> {
        match source {
            StreamSource::Async {
                handle, timeout, ..
            } => {
                let mut handle = handle.lock().unwrap();
                // First try non-blocking receive
                if let Ok(event) = handle.receiver.try_recv() {
                    return Self::event_to_value(event);
                }

                // Need to block with timeout
                let timeout = *timeout;
                Self::blocking_recv_with_timeout(&mut handle.receiver, timeout)
            }
            StreamSource::Static(_) | StreamSource::Empty => None,
        }
    }

    /// Blocks on `receiver.recv()` with an optional timeout.
    ///
    /// Reuses the ambient tokio runtime if one is available
    /// (via `block_in_place` to release the worker), otherwise builds a
    /// transient single-thread runtime for the duration of the call. This
    /// lets `StreamPosition` be driven equally from synchronous callers and
    /// from inside async tasks.
    ///
    /// Returns `None` on timeout, on a closed channel, or on a
    /// stream-terminating event ([`StreamEvent::Err`] / [`StreamEvent::Done`]
    /// — see [`event_to_value`](Self::event_to_value)).
    fn blocking_recv_with_timeout(
        receiver: &mut mpsc::Receiver<StreamEvent>,
        timeout: Option<Duration>,
    ) -> Option<Value> {
        // Check if we're already in a runtime
        if let Ok(rt_handle) = tokio::runtime::Handle::try_current() {
            // We're in a runtime - use block_in_place to allow blocking
            tokio::task::block_in_place(|| {
                rt_handle.block_on(async {
                    match timeout {
                        Some(duration) => {
                            match tokio::time::timeout(duration, receiver.recv()).await {
                                Ok(Some(event)) => Self::event_to_value(event),
                                Ok(None) => None, // Channel closed
                                Err(_) => None,   // Timeout
                            }
                        }
                        None => {
                            // Infinite blocking
                            match receiver.recv().await {
                                Some(event) => Self::event_to_value(event),
                                None => None, // Channel closed
                            }
                        }
                    }
                })
            })
        } else {
            // Not in a runtime - create a temporary one for the blocking call
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .ok()?;

            rt.block_on(async {
                match timeout {
                    Some(duration) => {
                        match tokio::time::timeout(duration, receiver.recv()).await {
                            Ok(Some(event)) => Self::event_to_value(event),
                            Ok(None) => None, // Channel closed
                            Err(_) => None,   // Timeout
                        }
                    }
                    None => {
                        // Infinite blocking
                        match receiver.recv().await {
                            Some(event) => Self::event_to_value(event),
                            None => None, // Channel closed
                        }
                    }
                }
            })
        }
    }

    /// Projects a [`StreamEvent`] onto the parser's view: `Some` for a value
    /// to consume, `None` for any terminator (error or done). The error vs.
    /// done distinction is intentionally lost — both terminate the stream
    /// for the grammar.
    fn event_to_value(event: StreamEvent) -> Option<Value> {
        match event {
            StreamEvent::Data(v) => Some(v),
            StreamEvent::Ok(v) => Some(v),
            StreamEvent::Err(_) => None, // Error terminates stream
            StreamEvent::Done => None,   // Done terminates stream with no value
        }
    }

    /// Returns the value at this cursor, or `None` if this is the terminal
    /// cell. The borrow is tied to the cell's lifetime; clone the
    /// [`Value`] if it must outlive `self`.
    pub fn head(&self) -> Option<&Value> {
        self.head.as_ref()
    }

    /// Returns `true` iff this cell is the terminal sentinel (no more input
    /// will become available from this cursor).
    pub fn is_at_end(&self) -> bool {
        self.head.is_none()
    }

    /// Returns the zero-based position index. Used by callers that need a
    /// stable key (memo tables, error reports, spill keys).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the next cell, computing and caching it on first call.
    ///
    /// The returned cell is shared (`Rc::clone` of the cached pointer) so
    /// repeated `tail()` calls on the same cell return the same successor
    /// — this is what makes packrat memo lookups across siblings work.
    ///
    /// For an `Async` source, this may block on the source channel for up to
    /// the configured timeout. It may also trigger spill-to-Fjall if the
    /// in-memory position cache exceeds `memory_limit`.
    pub fn tail(self: &Rc<Self>) -> Rc<Self> {
        // Check if tail is already computed
        if let Some(ref tail) = *self.tail.borrow() {
            return tail.clone();
        }

        // Compute tail based on source type
        let new_tail = match self.source.as_ref() {
            StreamSource::Async {
                positions,
                overflow,
                memory_limit,
                ..
            } => {
                let target_index = self.index + 1;

                // Check if we already have this position cached in memory
                {
                    let positions_guard = positions.lock().unwrap();
                    for pos in positions_guard.iter() {
                        if pos.index == target_index {
                            return pos.clone();
                        }
                    }
                }

                // Check if it's in the cold-tier overflow store
                if let Some(overflow_store) = overflow
                    && let Some(pos) = Self::restore_from_store(
                        overflow_store,
                        target_index,
                        self.source.clone(),
                        self.memo_store.clone(),
                    )
                {
                    // Cache it in memory (it will be spilled again if needed)
                    positions.lock().unwrap().push(pos.clone());
                    return pos;
                }

                // Not found anywhere - pull next value from async source
                let head = Self::pull_next(&self.source);
                let new_pos = Rc::new(Self {
                    head,
                    tail: RefCell::new(None),
                    index: target_index,
                    memo: RefCell::new(HashMap::new()),
                    source: self.source.clone(),
                    memo_store: self.memo_store.clone(),
                });

                // Cache the new position and potentially spill old ones
                {
                    let mut positions_guard = positions.lock().unwrap();
                    positions_guard.push(new_pos.clone());

                    // Spill the oldest positions when we exceed memory_limit.
                    if let Some(limit) = memory_limit
                        && positions_guard.len() > *limit
                        && let Some(overflow_store) = overflow
                    {
                        let spill_count = positions_guard.len() - *limit;
                        for pos in positions_guard.drain(0..spill_count) {
                            Self::spill_to_store(overflow_store, &pos);
                        }
                    }
                }

                new_pos
            }
            StreamSource::Static(values) => Self::build_static_chain(
                self.source.clone(),
                values,
                self.index + 1,
                self.memo_store.clone(),
            ),
            StreamSource::Empty => Self::end_of_stream(self.memo_store.clone()),
        };

        // Cache the computed tail
        *self.tail.borrow_mut() = Some(new_tail.clone());
        new_tail
    }

    /// Writes `pos.head` to the overflow store under
    /// [`PayloadKind::StreamPosition`].
    ///
    /// The on-disk payload is JSON-encoded `Option<Vec<u8>>` where the inner
    /// bytes are themselves a JSON-encoded [`Value`] — preserving the wire
    /// shape that [`restore_from_store`](Self::restore_from_store) reads
    /// back. This payload kind is deliberately distinct from
    /// `ParseState` so the two cannot alias in the store.
    ///
    /// # Panics
    ///
    /// Panics if serialization or the store write fails. Spill is invoked
    /// during normal traversal and there is no recovery path — a failure
    /// here is a storage-layer bug, surfaced loudly rather than swallowed.
    ///
    /// [`PayloadKind::StreamPosition`]: crate::persistence::schema::PayloadKind::StreamPosition
    fn spill_to_store(overflow: &OverflowStore, pos: &Rc<StreamPosition>) {
        use crate::persistence::envelope::write;
        use crate::persistence::schema::PayloadKind;
        use fmpl_types::Hash;

        let head_bytes = pos
            .head
            .as_ref()
            .map(|v| serde_json::to_vec(v).expect("failed to serialize value for store"));

        let key = pos.index.to_be_bytes();

        write(
            &*overflow.0,
            &key,
            &head_bytes,
            PayloadKind::StreamPosition,
            crate::VM_VERSION,
            Hash::NONE,
        )
        .expect("failed to write position envelope to store");
    }

    /// Reconstructs a previously spilled position from the store, or
    /// returns `None` if no record exists for `index` or the record
    /// fails any envelope integrity check (magic / CRC / VM-major /
    /// payload-kind / schema-version).
    ///
    /// The returned cell carries a fresh memo table and an uncomputed
    /// tail; it is observationally equivalent to a freshly-pulled cell
    /// at the same index. `source` and `memo_store` are propagated from
    /// the caller so the resurrected cell stays attached to the live
    /// stream.
    ///
    /// Records that fail to decode are treated as missing (the spilled
    /// position is re-pulled from the source). This is the same
    /// "graceful skip" semantics the loader applies elsewhere — a
    /// corrupted overflow record never panics the parser, since the
    /// source can always re-produce the value.
    fn restore_from_store(
        overflow: &OverflowStore,
        index: usize,
        source: Rc<StreamSource>,
        memo_store: Option<Arc<Mutex<MemoStore>>>,
    ) -> Option<Rc<StreamPosition>> {
        use crate::persistence::loader::{DecodeOutcome, decode};
        let key = index.to_be_bytes();

        let value_bytes = overflow.0.get(&key).ok().flatten()?;
        let (outcome, decoded) = decode(&value_bytes, crate::VM_VERSION.major);
        if outcome != DecodeOutcome::Loaded {
            return None;
        }
        let rec = decoded?;

        // The on-wire payload is JSON-encoded `Option<Vec<u8>>` where the
        // inner bytes are themselves a JSON-encoded `Value` — see
        // `spill_to_store` for the producer side. A decode failure here
        // would indicate envelope-internal schema drift (header passed,
        // payload didn't); skip rather than panic.
        let head_bytes: Option<Vec<u8>> = serde_json::from_slice(rec.payload).ok()?;
        let head = match head_bytes {
            None => None,
            Some(bytes) => Some(serde_json::from_slice(&bytes).ok()?),
        };

        Some(Rc::new(StreamPosition {
            head,
            tail: RefCell::new(None),
            index,
            memo: RefCell::new(HashMap::new()),
            source,
            memo_store,
        }))
    }

    /// Walks `n` cells forward and returns the resulting cursor.
    ///
    /// Stops early at end-of-stream — the returned cell is the terminal cell
    /// if `self` had fewer than `n` remaining values. Each step may pull /
    /// block on the underlying source (see [`tail`](Self::tail)).
    pub fn advance(self: &Rc<Self>, n: usize) -> Rc<Self> {
        let mut current = self.clone();
        for _ in 0..n {
            if current.is_at_end() {
                break;
            }
            current = current.tail();
        }
        current
    }

    /// Returns the memoized outcome of `rule` at this position, or `None`
    /// for a cache miss.
    ///
    /// Reads the in-memory memo table first; on a miss, falls through
    /// to the optional [`Store`][crate::persistence::Store]-backed memo
    /// table and promotes any hit into the in-memory table for
    /// subsequent lookups. Persisted records that fail envelope
    /// integrity checks (magic / CRC / VM-major / payload-kind) are
    /// treated as cache misses — the rule will be re-evaluated rather
    /// than silently returning a tampered or version-mismatched
    /// memoization.
    ///
    /// A `None` here is *only* a cache miss — it does not distinguish
    /// "not yet attempted" from "attempted and failed".
    pub fn get_memo(&self, rule: &SmolStr) -> Option<MemoEntry> {
        if let Some(entry) = self.memo.borrow().get(rule).cloned() {
            return Some(entry);
        }

        if let Some(ref memo_store) = self.memo_store {
            use crate::persistence::loader::{DecodeOutcome, decode};
            let memo_store_guard = memo_store.lock().unwrap();
            let key = format!("{}:{}", self.index, rule);
            if let Ok(Some(value_bytes)) = memo_store_guard.0.get(key.as_bytes()) {
                let (outcome, decoded) = decode(&value_bytes, crate::VM_VERSION.major);
                if outcome == DecodeOutcome::Loaded
                    && let Some(rec) = decoded
                    && let Ok(entry) = serde_json::from_slice::<MemoEntry>(rec.payload)
                {
                    self.memo.borrow_mut().insert(rule.clone(), entry.clone());
                    return Some(entry);
                }
            }
        }

        None
    }

    /// Inserts `entry` as the memoized outcome of `rule` at this position.
    ///
    /// Writes both the in-memory table and (when configured) the Fjall
    /// partition. Any previous entry for `rule` at this position is
    /// overwritten — packrat semantics require the latest `Done` result to
    /// win, and overwriting `InProgress` with `Done` is the normal
    /// completion path.
    ///
    /// # Panics
    ///
    /// Panics if the Fjall write fails; matches the spill path semantics —
    /// storage errors are not silently tolerated.
    pub fn set_memo(&self, rule: SmolStr, entry: MemoEntry) {
        self.memo.borrow_mut().insert(rule.clone(), entry.clone());

        if let Some(ref memo_store) = self.memo_store {
            use crate::persistence::envelope::write;
            use crate::persistence::schema::PayloadKind;
            use fmpl_types::Hash;

            let memo_store_guard = memo_store.lock().unwrap();
            let key = format!("{}:{}", self.index, rule);
            write(
                &*memo_store_guard.0,
                key.as_bytes(),
                &entry,
                PayloadKind::MemoTable,
                crate::VM_VERSION,
                Hash::NONE,
            )
            .expect("failed to write memo envelope to store");
        }
    }

    /// Drains the stream from this cursor to end-of-stream into a `Vec`.
    ///
    /// Intended for tests and debugging — forces full materialization of the
    /// remaining stream, defeating the laziness this type otherwise
    /// preserves. Not appropriate for production parsing of unbounded
    /// streams.
    pub fn collect_to_vec(self: &Rc<Self>) -> Vec<Value> {
        let mut result = Vec::new();
        let mut current = self.clone();
        while let Some(v) = current.head() {
            result.push(v.clone());
            current = current.tail();
        }
        result
    }
}

/// PegRuntime-compatible wrapper around a [`StreamPosition`].
///
/// Presents the indexed `value_at` / `is_at_end` interface that PEG-style
/// drivers expect, while retaining lazy stream semantics underneath. The
/// `start` position is shared (`Rc`), so cloning a `StreamInput` is cheap
/// and shares all upstream cells with the original.
#[derive(Debug)]
pub struct StreamInput {
    /// Root position of the underlying lazy stream.
    start: Rc<StreamPosition>,
    /// Cached copy of the source's blocking timeout for observability via
    /// [`timeout`](Self::timeout).
    timeout: Option<Duration>,
}

impl StreamInput {
    /// Returns a stream input over `handle` using [`DEFAULT_STREAM_TIMEOUT`].
    pub fn from_async(handle: StreamHandle) -> Self {
        Self::from_async_with_timeout(handle, DEFAULT_STREAM_TIMEOUT)
    }

    /// Returns a stream input over `handle` with a caller-chosen blocking
    /// timeout. `None` blocks indefinitely.
    pub fn from_async_with_timeout(handle: StreamHandle, timeout: Option<Duration>) -> Self {
        Self {
            start: StreamPosition::from_async(handle, timeout),
            timeout,
        }
    }

    /// Returns a stream input over a fixed `Vec<Value>`. The recorded
    /// `timeout` is the default — static streams never actually block, but
    /// the field is preserved so the inspection API is uniform.
    pub fn from_values(values: Vec<Value>) -> Self {
        Self {
            start: StreamPosition::from_values(values),
            timeout: DEFAULT_STREAM_TIMEOUT,
        }
    }

    /// Returns a shared handle to the root position. Clones cheaply.
    pub fn start(&self) -> Rc<StreamPosition> {
        self.start.clone()
    }

    /// Returns the configured blocking timeout, or `None` for unbounded.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns the cell at `index`, walking the stream from the root.
    ///
    /// O(index) in cell traversal; may block on the underlying source for
    /// uncomputed indices. For repeated indexed access, prefer holding the
    /// returned [`Rc<StreamPosition>`] and using [`StreamPosition::tail`].
    pub fn position_at(&self, index: usize) -> Rc<StreamPosition> {
        self.start.advance(index)
    }

    /// Returns the value at `index`, or `None` if `index` is past
    /// end-of-stream. Equivalent to `position_at(index).head().cloned()`.
    pub fn value_at(&self, index: usize) -> Option<Value> {
        let pos = self.position_at(index);
        pos.head().cloned()
    }

    /// Returns `true` iff `index` falls at or past end-of-stream. May block
    /// while pulling cells up to `index`.
    pub fn is_at_end(&self, index: usize) -> bool {
        let pos = self.position_at(index);
        pos.is_at_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    // Store-backed overflow + memo integration tests live at
    // `fmpl-persistence/tests/stream_input_store.rs`. They construct
    // a `FjallStore` directly (which the no-fjall-in-fmpl-core gate
    // forbids in `fmpl-core/src/`) and exercise spill/restore + memo
    // round-trips through the loader's integrity gates.

    #[test]
    fn test_static_stream_basic() {
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let stream = StreamPosition::from_values(values);

        assert_eq!(stream.head(), Some(&Value::Int(1)));
        assert_eq!(stream.index(), 0);

        let tail = stream.tail();
        assert_eq!(tail.head(), Some(&Value::Int(2)));
        assert_eq!(tail.index(), 1);

        let tail2 = tail.tail();
        assert_eq!(tail2.head(), Some(&Value::Int(3)));

        let end = tail2.tail();
        assert!(end.is_at_end());
    }

    #[test]
    fn test_static_stream_collect() {
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let stream = StreamPosition::from_values(values.clone());
        let collected = stream.collect_to_vec();
        assert_eq!(collected, values);
    }

    #[test]
    fn test_static_stream_advance() {
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let stream = StreamPosition::from_values(values);

        let pos2 = stream.advance(2);
        assert_eq!(pos2.head(), Some(&Value::Int(3)));
        assert_eq!(pos2.index(), 2);
    }

    #[test]
    fn test_empty_stream() {
        let stream = StreamPosition::from_values(vec![]);
        assert!(stream.is_at_end());
    }

    #[test]
    fn test_memoization() {
        let values = vec![Value::Int(1)];
        let stream = StreamPosition::from_values(values);

        // Initially no memo
        assert!(stream.get_memo(&SmolStr::new("test")).is_none());

        // Set memo
        stream.set_memo(
            SmolStr::new("test"),
            MemoEntry::Done(Some(Value::Int(42)), 1),
        );

        // Should retrieve memo
        let memo = stream.get_memo(&SmolStr::new("test"));
        assert!(matches!(
            memo,
            Some(MemoEntry::Done(Some(Value::Int(42)), 1))
        ));
    }

    // The memo-persists-across-reopen scenario lives in
    // `fmpl-persistence/tests/stream_input_store.rs` as the
    // `memo_persists_across_store_reopen` integration test.

    #[test]
    fn test_stream_input_interface() {
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let input = StreamInput::from_values(values);

        assert_eq!(input.value_at(0), Some(Value::Int(1)));
        assert_eq!(input.value_at(1), Some(Value::Int(2)));
        assert_eq!(input.value_at(2), Some(Value::Int(3)));
        assert_eq!(input.value_at(3), None);

        assert!(!input.is_at_end(0));
        assert!(input.is_at_end(3));
    }

    // Async stream tests require multi-threaded runtime for block_in_place
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_async_stream_basic() {
        let (tx, rx) = mpsc::channel(10);

        // Send some events
        tx.send(StreamEvent::Data(Value::Int(1))).await.unwrap();
        tx.send(StreamEvent::Data(Value::Int(2))).await.unwrap();
        tx.send(StreamEvent::Ok(Value::Int(3))).await.unwrap();
        drop(tx); // Close channel

        let handle = crate::stream::StreamHandle::new(rx, 1);
        let stream = StreamPosition::from_async(handle, Some(Duration::from_secs(1)));

        // Note: This test runs synchronously within the tokio runtime
        let collected = stream.collect_to_vec();
        assert_eq!(
            collected,
            vec![Value::Int(1), Value::Int(2), Value::Int(3),]
        );
    }
}
