//! OMStream-style lazy input stream for grammar application.
//!
//! This module provides an immutable, cons-cell based input stream that supports:
//! - Lazy tail construction (blocks on async channel when needed)
//! - Per-position memoization for packrat parsing
//! - Configurable timeout for blocking operations
//!
//! Based on OMeta's OMInputStream design and maru's <parser-stream>.

use crate::stream::{StreamEvent, StreamHandle};
use crate::value::Value;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(feature = "fjall-persistence")]
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

/// Default timeout for blocking stream operations (30 seconds).
/// Use `None` for infinite blocking.
pub const DEFAULT_STREAM_TIMEOUT: Option<Duration> = Some(Duration::from_secs(30));

/// A position in a streaming input.
///
/// This is an immutable cons-cell with lazy tail construction.
/// Each position has:
/// - `head`: The value at this position (or None if at end)
/// - `tail`: The next position (lazily computed)
/// - `memo`: Per-position memoization table for packrat parsing
#[derive(Debug)]
pub struct StreamPosition {
    /// The value at this position (None = end of stream).
    head: Option<Value>,
    /// The next position (lazily computed).
    tail: RefCell<Option<Rc<StreamPosition>>>,
    /// Position index (for memoization keys).
    index: usize,
    /// Per-position memoization table.
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,
    /// Source reference for pulling more data.
    source: Rc<StreamSource>,
}

/// Cached parse result for memoization.
#[derive(Debug, Clone)]
pub enum MemoEntry {
    /// Parsing in progress (for left recursion detection).
    InProgress,
    /// Completed with result: (value, end_position_index).
    Done(Option<Value>, usize),
}

/// Fjall-backed overflow storage for spilled positions.
#[cfg(feature = "fjall-persistence")]
struct FjallOverflow {
    #[allow(dead_code)]
    keyspace: fjall::Keyspace,
    partition: fjall::PartitionHandle,
}

#[cfg(feature = "fjall-persistence")]
impl std::fmt::Debug for FjallOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallOverflow").finish_non_exhaustive()
    }
}

/// Source of streaming data.
#[derive(Debug)]
enum StreamSource {
    /// From an async stream with blocking receive.
    Async {
        handle: Mutex<StreamHandle>,
        /// Timeout for blocking recv (None = infinite).
        timeout: Option<Duration>,
        /// Cached positions for position index lookup.
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        /// Fjall overflow for spilled positions (optional).
        #[cfg(feature = "fjall-persistence")]
        fjall: Option<FjallOverflow>,
        /// Memory limit before spilling (default: no limit).
        #[cfg(feature = "fjall-persistence")]
        memory_limit: Option<usize>,
    },
    /// From a static list of values (no blocking needed).
    Static(Vec<Value>),
    /// Empty source (end of stream).
    Empty,
}

impl StreamPosition {
    /// Create a new stream from an async stream handle.
    ///
    /// `timeout` is the timeout for blocking recv operations.
    /// Use `None` for infinite blocking.
    pub fn from_async(handle: StreamHandle, timeout: Option<Duration>) -> Rc<Self> {
        let source = Rc::new(StreamSource::Async {
            handle: Mutex::new(handle),
            timeout,
            positions: Mutex::new(Vec::new()),
            #[cfg(feature = "fjall-persistence")]
            fjall: None,
            #[cfg(feature = "fjall-persistence")]
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
        });

        // Cache this position
        if let StreamSource::Async { positions, .. } = source.as_ref() {
            positions.lock().unwrap().push(pos.clone());
        }

        pos
    }

    /// Create a new stream from an async stream handle with Fjall overflow support.
    ///
    /// `timeout` is the timeout for blocking recv operations (use `None` for infinite blocking).
    /// `fjall_path` is the optional directory for Fjall storage (None = no overflow).
    /// `memory_limit` is the number of positions to keep in memory before spilling to Fjall.
    #[cfg(feature = "fjall-persistence")]
    pub fn from_async_with_fjall(
        handle: StreamHandle,
        timeout: Option<Duration>,
        fjall_path: Option<PathBuf>,
        memory_limit: usize,
    ) -> Rc<Self> {
        let fjall = fjall_path.map(|path| {
            let keyspace = fjall::Config::new(path)
                .open()
                .expect("failed to open fjall keyspace");
            let partition = keyspace
                .open_partition("positions", Default::default())
                .expect("failed to open positions partition");
            FjallOverflow {
                keyspace,
                partition,
            }
        });

        let source = Rc::new(StreamSource::Async {
            handle: Mutex::new(handle),
            timeout,
            positions: Mutex::new(Vec::new()),
            fjall,
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
        });

        // Cache this position
        if let StreamSource::Async { positions, .. } = source.as_ref() {
            positions.lock().unwrap().push(pos.clone());
        }

        pos
    }

    /// Create a new stream from a static list of values.
    pub fn from_values(values: Vec<Value>) -> Rc<Self> {
        if values.is_empty() {
            return Self::end_of_stream();
        }

        let source = Rc::new(StreamSource::Static(values.clone()));
        Self::build_static_chain(source, &values, 0)
    }

    /// Create an end-of-stream position.
    fn end_of_stream() -> Rc<Self> {
        Rc::new(Self {
            head: None,
            tail: RefCell::new(None),
            index: 0,
            memo: RefCell::new(HashMap::new()),
            source: Rc::new(StreamSource::Empty),
        })
    }

    /// Build a chain of positions from static values.
    fn build_static_chain(source: Rc<StreamSource>, values: &[Value], index: usize) -> Rc<Self> {
        if index >= values.len() {
            return Rc::new(Self {
                head: None,
                tail: RefCell::new(None),
                index,
                memo: RefCell::new(HashMap::new()),
                source,
            });
        }

        Rc::new(Self {
            head: Some(values[index].clone()),
            tail: RefCell::new(None), // Lazily computed
            index,
            memo: RefCell::new(HashMap::new()),
            source,
        })
    }

    /// Pull the next value from an async source.
    ///
    /// This uses try_recv first (non-blocking), and if that fails,
    /// uses blocking_recv with timeout via block_on.
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

    /// Blocking receive with optional timeout.
    ///
    /// If `timeout` is `None`, blocks indefinitely.
    /// Handles being called from within or outside a tokio runtime.
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

    /// Convert a stream event to an optional value.
    fn event_to_value(event: StreamEvent) -> Option<Value> {
        match event {
            StreamEvent::Data(v) => Some(v),
            StreamEvent::Ok(v) => Some(v),
            StreamEvent::Err(_) => None, // Error terminates stream
        }
    }

    /// Get the value at this position (None = end of stream).
    pub fn head(&self) -> Option<&Value> {
        self.head.as_ref()
    }

    /// Check if this position is at end of stream.
    pub fn is_at_end(&self) -> bool {
        self.head.is_none()
    }

    /// Get the position index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Get the tail (next position), computing lazily if needed.
    #[cfg(not(feature = "fjall-persistence"))]
    pub fn tail(self: &Rc<Self>) -> Rc<Self> {
        // Check if tail is already computed
        if let Some(ref tail) = *self.tail.borrow() {
            return tail.clone();
        }

        // Compute tail based on source type
        let new_tail = match self.source.as_ref() {
            StreamSource::Async { positions, .. } => {
                // Check if we already have this position cached
                let positions_guard = positions.lock().unwrap();
                if let Some(cached) = positions_guard.get(self.index + 1) {
                    cached.clone()
                } else {
                    drop(positions_guard); // Release lock before pulling

                    // Pull next value from async source
                    let head = Self::pull_next(&self.source);
                    let new_pos = Rc::new(Self {
                        head,
                        tail: RefCell::new(None),
                        index: self.index + 1,
                        memo: RefCell::new(HashMap::new()),
                        source: self.source.clone(),
                    });

                    // Cache the new position
                    if let StreamSource::Async { positions, .. } = self.source.as_ref() {
                        positions.lock().unwrap().push(new_pos.clone());
                    }

                    new_pos
                }
            }
            StreamSource::Static(values) => {
                Self::build_static_chain(self.source.clone(), values, self.index + 1)
            }
            StreamSource::Empty => Self::end_of_stream(),
        };

        // Cache the computed tail
        *self.tail.borrow_mut() = Some(new_tail.clone());
        new_tail
    }

    /// Get the tail (next position), computing lazily if needed.
    /// With Fjall support for overflow handling.
    #[cfg(feature = "fjall-persistence")]
    pub fn tail(self: &Rc<Self>) -> Rc<Self> {
        // Check if tail is already computed
        if let Some(ref tail) = *self.tail.borrow() {
            return tail.clone();
        }

        // Compute tail based on source type
        let new_tail = match self.source.as_ref() {
            StreamSource::Async {
                positions,
                fjall,
                memory_limit,
                ..
            } => {
                let target_index = self.index + 1;

                // Check if we already have this position cached in memory
                {
                    let positions_guard = positions.lock().unwrap();
                    // Try to find the position in memory
                    for pos in positions_guard.iter() {
                        if pos.index == target_index {
                            return pos.clone();
                        }
                    }
                }

                // Check if it's in Fjall overflow
                if let Some(fjall_overflow) = fjall {
                    if let Some(pos) =
                        Self::restore_from_fjall(fjall_overflow, target_index, self.source.clone())
                    {
                        // Cache it in memory (it will be spilled again if needed)
                        positions.lock().unwrap().push(pos.clone());
                        return pos;
                    }
                }

                // Not found anywhere - pull next value from async source
                let head = Self::pull_next(&self.source);
                let new_pos = Rc::new(Self {
                    head,
                    tail: RefCell::new(None),
                    index: target_index,
                    memo: RefCell::new(HashMap::new()),
                    source: self.source.clone(),
                });

                // Cache the new position and potentially spill old ones
                {
                    let mut positions_guard = positions.lock().unwrap();
                    positions_guard.push(new_pos.clone());

                    // Check if we need to spill to Fjall
                    if let Some(limit) = memory_limit {
                        if positions_guard.len() > *limit {
                            if let Some(fjall_overflow) = fjall {
                                // Spill the oldest positions
                                let spill_count = positions_guard.len() - *limit;
                                for pos in positions_guard.drain(0..spill_count) {
                                    Self::spill_to_fjall(fjall_overflow, &pos);
                                }
                            }
                        }
                    }
                }

                new_pos
            }
            StreamSource::Static(values) => {
                Self::build_static_chain(self.source.clone(), values, self.index + 1)
            }
            StreamSource::Empty => Self::end_of_stream(),
        };

        // Cache the computed tail
        *self.tail.borrow_mut() = Some(new_tail.clone());
        new_tail
    }

    /// Spill a position to Fjall storage.
    #[cfg(feature = "fjall-persistence")]
    fn spill_to_fjall(fjall: &FjallOverflow, pos: &Rc<StreamPosition>) {
        // Serialize the head value (if any) using serde_json
        let head_bytes = pos
            .head
            .as_ref()
            .map(|v| serde_json::to_vec(v).expect("failed to serialize value for fjall"));

        // Key is the position index as bytes
        let key = pos.index.to_be_bytes();

        // Value is Option<Vec<u8>> serialized
        let value =
            serde_json::to_vec(&head_bytes).expect("failed to serialize position for fjall");

        fjall
            .partition
            .insert(key, value)
            .expect("failed to insert position into fjall");
    }

    /// Restore a position from Fjall storage.
    #[cfg(feature = "fjall-persistence")]
    fn restore_from_fjall(
        fjall: &FjallOverflow,
        index: usize,
        source: Rc<StreamSource>,
    ) -> Option<Rc<StreamPosition>> {
        let key = index.to_be_bytes();

        if let Ok(Some(value_bytes)) = fjall.partition.get(key) {
            // Deserialize the Option<Vec<u8>>
            let head_bytes: Option<Vec<u8>> =
                serde_json::from_slice(&value_bytes).expect("failed to deserialize from fjall");

            // Deserialize the head value (if any)
            let head = head_bytes.map(|bytes| {
                serde_json::from_slice(&bytes).expect("failed to deserialize value from fjall")
            });

            Some(Rc::new(StreamPosition {
                head,
                tail: RefCell::new(None),
                index,
                memo: RefCell::new(HashMap::new()),
                source,
            }))
        } else {
            None
        }
    }

    /// Advance n positions, returning the new position.
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

    /// Get the memoization entry for a rule at this position.
    pub fn get_memo(&self, rule: &SmolStr) -> Option<MemoEntry> {
        self.memo.borrow().get(rule).cloned()
    }

    /// Set the memoization entry for a rule at this position.
    pub fn set_memo(&self, rule: SmolStr, entry: MemoEntry) {
        self.memo.borrow_mut().insert(rule, entry);
    }

    /// Collect values from current position to end (for testing/debugging).
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

/// Streaming input wrapper that can be used with PegRuntime.
///
/// This wraps StreamPosition to provide the same interface as Input,
/// but with streaming/lazy evaluation.
#[derive(Debug)]
pub struct StreamInput {
    /// The starting position in the stream.
    start: Rc<StreamPosition>,
    /// The timeout for blocking operations (None = infinite).
    timeout: Option<Duration>,
}

impl StreamInput {
    /// Create a streaming input from an async stream with default timeout.
    pub fn from_async(handle: StreamHandle) -> Self {
        Self::from_async_with_timeout(handle, DEFAULT_STREAM_TIMEOUT)
    }

    /// Create a streaming input from an async stream with custom timeout.
    ///
    /// Use `None` for infinite blocking.
    pub fn from_async_with_timeout(handle: StreamHandle, timeout: Option<Duration>) -> Self {
        Self {
            start: StreamPosition::from_async(handle, timeout),
            timeout,
        }
    }

    /// Create a streaming input from a static list of values.
    pub fn from_values(values: Vec<Value>) -> Self {
        Self {
            start: StreamPosition::from_values(values),
            timeout: DEFAULT_STREAM_TIMEOUT,
        }
    }

    /// Get the starting position.
    pub fn start(&self) -> Rc<StreamPosition> {
        self.start.clone()
    }

    /// Get the configured timeout for blocking operations.
    /// Returns `None` if infinite blocking is configured.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Get the position at a given index.
    ///
    /// This may block if the position hasn't been computed yet.
    pub fn position_at(&self, index: usize) -> Rc<StreamPosition> {
        self.start.advance(index)
    }

    /// Get the value at a given index.
    pub fn value_at(&self, index: usize) -> Option<Value> {
        let pos = self.position_at(index);
        pos.head().cloned()
    }

    /// Check if position is at end.
    pub fn is_at_end(&self, index: usize) -> bool {
        let pos = self.position_at(index);
        pos.is_at_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[cfg(feature = "fjall-persistence")]
    #[test]
    fn test_fjall_overflow_basic() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let values: Vec<Value> = (0..10).map(|i| Value::Int(i)).collect();

        // Create async stream with fjall backing
        let (tx, rx) = mpsc::channel(100);

        // Send all values
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            for v in values.iter() {
                tx.send(crate::stream::StreamEvent::Data(v.clone()))
                    .await
                    .unwrap();
            }
        });
        drop(tx);

        let handle = crate::stream::StreamHandle::new(rx, 1);
        // Use a memory limit of 5 positions - should trigger spilling after 5 positions
        let stream = StreamPosition::from_async_with_fjall(
            handle,
            Some(std::time::Duration::from_secs(1)),
            Some(temp_dir.path().to_path_buf()),
            5,
        );

        // Advance through all positions to trigger potential spilling
        let pos5 = stream.advance(5);
        assert_eq!(pos5.head(), Some(&Value::Int(5)));

        // Continue to the end
        let pos9 = stream.advance(9);
        assert_eq!(pos9.head(), Some(&Value::Int(9)));

        // Going beyond end should give None
        let end = pos9.tail();
        assert!(end.is_at_end());
    }

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
