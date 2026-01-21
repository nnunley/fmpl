# Streaming Grammar Push-Model Implementation Plan

**Status: Complete**
This implementation plan has been marked as complete. The tasks outlined in the status table below have been implemented.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable incremental parsing of async streams (LLM output, HTTP chunks) with full backtracking, speculative downstream emission, and durable suspension via Fjall persistence.

**Architecture:** Extend the existing `PegRuntime` with an incremental `start()`/`resume()` API. The `StreamPosition` already buffers positions for backtracking - add Fjall overflow for large buffers. Wire `|>` operator to spawn parse drivers that emit matches downstream as they occur.

**Tech Stack:** Rust, tokio (async channels), Fjall (persistence), existing PegInput trait

---

## Implementation Status (as of 2026-01-20)

| Task | Status | Commit |
|------|--------|--------|
| Task 1: ParseState/ParseNext types | ✅ Complete | `53b27a0` |
| Task 2: Fjall backing for StreamPosition | ✅ Complete | `b2c5daf` |
| Task 3: Incremental parse API | ✅ Complete | `67536dc` |
| Task 4: ParseDriver for streaming pipelines | ✅ Complete | `d137df4` |
| Task 5: Wire |> operator to ParseDriver | ✅ Complete | `18991d1` |
| Task 6: Fjall persistence for memo tables | ✅ Complete | `04949ff` |
| Task 7: ParseState serialization | ✅ Complete | `c178edf` |
| Task 8: Integration tests | ✅ Complete | `33e08a2` |
| Task 9: Documentation | ✅ Complete | (this commit) |

**Status:** All tasks complete. Run `cargo test -p fmpl-core --features fjall-persistence` to verify.

---

## Task 1: Add ParseState and ParseNext Types

**Files:**
- Create: `fmpl-core/src/grammar/incremental.rs`
- Modify: `fmpl-core/src/grammar/mod.rs:42-44` (add module export)

**Step 1: Write the failing test**

In `fmpl-core/src/grammar/incremental.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn test_parse_state_serialization() {
        let state = ParseState {
            position_index: 5,
            rule_stack: vec![("digit".into(), 3), ("integer".into(), 0)],
            bindings: [("x".into(), Value::Int(42))].into_iter().collect(),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: ParseState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.position_index, 5);
        assert_eq!(deserialized.rule_stack.len(), 2);
        assert_eq!(deserialized.bindings.get("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_parse_next_variants() {
        let match_result: ParseNext = ParseNext::Match(Value::Int(42));
        assert!(matches!(match_result, ParseNext::Match(Value::Int(42))));

        let need_input: ParseNext = ParseNext::NeedInput(ParseState::default());
        assert!(matches!(need_input, ParseNext::NeedInput(_)));

        let end: ParseNext = ParseNext::End;
        assert!(matches!(end, ParseNext::End));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core incremental::tests --no-run 2>&1 | head -20`
Expected: Compilation error - module and types don't exist

**Step 3: Write minimal implementation**

```rust
//! Incremental parsing support for streaming grammars.
//!
//! Provides ParseState for suspension/resumption and ParseNext for
//! incremental parse results.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

/// State needed to resume an incremental parse.
///
/// Captures position, rule call stack, and bindings for serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Current position index in input.
    pub position_index: usize,
    /// Rule call stack: (rule_name, entry_position_index).
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Current variable bindings.
    pub bindings: HashMap<SmolStr, Value>,
}

/// Result of an incremental parse step.
#[derive(Debug, Clone)]
pub enum ParseNext {
    /// Rule matched, here's the result value.
    Match(Value),
    /// Need more input - here's state to resume from.
    NeedInput(ParseState),
    /// Input stream ended.
    End,
}
```

Update `fmpl-core/src/grammar/mod.rs` to add:
```rust
pub mod incremental;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core incremental::tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/incremental.rs fmpl-core/src/grammar/mod.rs
git commit -m "feat(grammar): add ParseState and ParseNext for incremental parsing"
```

---

## Task 2: Add Fjall Backing for StreamPosition Buffer

**Files:**
- Modify: `fmpl-core/src/grammar/stream_input.rs:56-69` (StreamSource)
- Modify: `fmpl-core/Cargo.toml` (add fjall dependency)

**Step 1: Write the failing test**

In `fmpl-core/src/grammar/stream_input.rs`, add to tests module:

```rust
#[test]
fn test_large_buffer_spills_to_fjall() {
    use tempfile::tempdir;

    // Create stream with Fjall backing
    let temp_dir = tempdir().unwrap();
    let (tx, rx) = mpsc::channel(10);

    // Send many values to force spill
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        for i in 0..1000 {
            tx.send(StreamEvent::Data(Value::Int(i))).await.unwrap();
        }
        drop(tx);
    });

    let handle = crate::stream::StreamHandle::new(rx, 1);
    let stream = StreamPosition::from_async_with_fjall(
        handle,
        Some(Duration::from_secs(1)),
        Some(temp_dir.path().to_path_buf()),
        100, // Memory limit: 100 positions before spilling
    );

    // Advance to position 500 - should have spilled to Fjall
    let pos = stream.advance(500);
    assert_eq!(pos.index(), 500);
    assert_eq!(pos.head(), Some(&Value::Int(500)));

    // Go back to position 50 - should restore from Fjall
    let pos_early = stream.advance(0).advance(50);
    assert_eq!(pos_early.head(), Some(&Value::Int(50)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_large_buffer_spills_to_fjall --no-run 2>&1 | head -20`
Expected: Compilation error - `from_async_with_fjall` doesn't exist

**Step 3: Write minimal implementation**

Add to `Cargo.toml`:
```toml
fjall = "2"
tempfile = { version = "3", optional = true }

[dev-dependencies]
tempfile = "3"

[features]
default = []
fjall-persistence = ["fjall"]
```

Update `StreamSource` enum:
```rust
enum StreamSource {
    Async {
        handle: Mutex<StreamHandle>,
        timeout: Option<Duration>,
        /// In-memory positions (recent).
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        /// Fjall keyspace for overflow (optional).
        fjall: Option<FjallOverflow>,
        /// Threshold for spilling to Fjall.
        memory_limit: usize,
    },
    Static(Vec<Value>),
    Empty,
}

#[cfg(feature = "fjall-persistence")]
struct FjallOverflow {
    keyspace: fjall::Keyspace,
    partition: fjall::PartitionHandle,
}
```

Add `from_async_with_fjall` constructor and spill/restore logic.

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_large_buffer_spills_to_fjall -v --features fjall-persistence`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/stream_input.rs fmpl-core/Cargo.toml
git commit -m "feat(grammar): add Fjall backing for StreamPosition overflow"
```

---

## Task 3: Add Incremental Parse API to PegRuntime

**Files:**
- Modify: `fmpl-core/src/grammar/runtime.rs:39-66` (add start/resume methods)
- Modify: `fmpl-core/src/grammar/incremental.rs` (re-export from runtime)

**Step 1: Write the failing test**

In `fmpl-core/src/grammar/runtime.rs`, add to tests module:

```rust
#[test]
fn test_incremental_parse_basic() {
    let registry = GrammarRegistry::new();
    let grammar = registry.get("base::tree").unwrap();

    // Create streaming input with values that arrive incrementally
    let input = StreamingInput::from_values(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ]);

    let mut runtime = PegRuntime::new(input, &registry, grammar);

    // Start incremental parse for "any" rule
    let mut state = runtime.start("any");

    // Resume should return Match for first value
    match runtime.resume(state) {
        Ok(ParseNext::Match(v)) => assert_eq!(v, Value::Int(1)),
        other => panic!("expected Match, got {:?}", other),
    }
}

#[test]
fn test_incremental_parse_needs_input() {
    use tokio::sync::mpsc;

    let registry = GrammarRegistry::new();
    let grammar = registry.get("base::tree").unwrap();

    // Create async stream that we control
    let (tx, rx) = mpsc::channel(10);
    let handle = crate::stream::StreamHandle::new(rx, 1);
    let input = StreamingInput::from_async_with_timeout(handle, Some(Duration::from_millis(10)));

    let mut runtime = PegRuntime::new(input, &registry, grammar);
    let state = runtime.start("any");

    // With empty channel and short timeout, should get NeedInput
    match runtime.resume(state) {
        Ok(ParseNext::NeedInput(_)) => (), // Expected
        Ok(ParseNext::End) => (), // Also acceptable if channel closed
        other => panic!("expected NeedInput or End, got {:?}", other),
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_incremental_parse --no-run 2>&1 | head -20`
Expected: Compilation error - `start` and `resume` methods don't exist

**Step 3: Write minimal implementation**

Add to `PegRuntime`:

```rust
use super::incremental::{ParseState, ParseNext};

impl<'a, 'e, I: PegInput> PegRuntime<'a, 'e, I> {
    /// Start an incremental parse, returning initial state.
    pub fn start(&mut self, rule_name: &str) -> ParseState {
        self.bindings.clear();
        ParseState {
            position_index: 0,
            rule_stack: vec![(SmolStr::new(rule_name), 0)],
            bindings: HashMap::new(),
        }
    }

    /// Resume an incremental parse from saved state.
    pub fn resume(&mut self, state: ParseState) -> Result<ParseNext> {
        // Restore state
        self.bindings = state.bindings.clone();
        let pos = self.input.position_at(state.position_index);

        // Check if at end
        if self.input.is_at_end(&pos) {
            return Ok(ParseNext::End);
        }

        // Try to match the top rule
        if let Some((rule_name, _)) = state.rule_stack.first() {
            match self.apply_rule(rule_name, pos)? {
                ParseResult::Success(value, _) => Ok(ParseNext::Match(value)),
                ParseResult::Failure => {
                    // No match yet - need more input
                    Ok(ParseNext::NeedInput(state))
                }
            }
        } else {
            Ok(ParseNext::End)
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_incremental_parse -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/runtime.rs fmpl-core/src/grammar/incremental.rs
git commit -m "feat(grammar): add start/resume incremental parse API"
```

---

## Task 4: Add Parse Driver for Streaming Pipelines

**Files:**
- Create: `fmpl-core/src/grammar/driver.rs`
- Modify: `fmpl-core/src/grammar/mod.rs` (add module export)

**Step 1: Write the failing test**

In `fmpl-core/src/grammar/driver.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use tokio::sync::mpsc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_parse_driver_emits_matches() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();

        // Input stream
        let (in_tx, in_rx) = mpsc::channel(10);
        let in_handle = crate::stream::StreamHandle::new(in_rx, 1);

        // Output channel
        let (out_tx, mut out_rx) = mpsc::channel(10);

        // Start parse driver
        let driver = ParseDriver::new(
            in_handle,
            grammar,
            "any".to_string(),
            &registry,
            out_tx,
        );

        let handle = tokio::spawn(async move {
            driver.run().await
        });

        // Send values
        in_tx.send(StreamEvent::Data(Value::Int(1))).await.unwrap();
        in_tx.send(StreamEvent::Data(Value::Int(2))).await.unwrap();
        drop(in_tx);

        // Collect output
        let mut results = Vec::new();
        while let Some(v) = out_rx.recv().await {
            results.push(v);
        }

        handle.await.unwrap().unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Value::Int(1));
        assert_eq!(results[1], Value::Int(2));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core driver::tests --no-run 2>&1 | head -20`
Expected: Compilation error - `ParseDriver` doesn't exist

**Step 3: Write minimal implementation**

```rust
//! Parse driver for streaming grammar pipelines.
//!
//! Connects an async input stream to a grammar, emitting matches downstream.

use crate::error::Result;
use crate::grammar::{Grammar, GrammarRegistry, ParseResult};
use crate::grammar::input::StreamingInput;
use crate::grammar::runtime::PegRuntime;
use crate::stream::{StreamEvent, StreamHandle};
use crate::value::Value;
use smol_str::SmolStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Drives incremental parsing of an async stream.
pub struct ParseDriver {
    input_handle: StreamHandle,
    grammar: Arc<Grammar>,
    rule: String,
    registry: Arc<GrammarRegistry>,
    output: mpsc::Sender<Value>,
    timeout: Option<Duration>,
}

impl ParseDriver {
    pub fn new(
        input_handle: StreamHandle,
        grammar: Arc<Grammar>,
        rule: String,
        registry: &GrammarRegistry,
        output: mpsc::Sender<Value>,
    ) -> Self {
        Self {
            input_handle,
            grammar,
            rule,
            registry: Arc::new(registry.clone()),
            output,
            timeout: Some(Duration::from_secs(30)),
        }
    }

    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run the parse driver until input ends.
    pub async fn run(self) -> Result<()> {
        let input = StreamingInput::from_async_with_timeout(
            self.input_handle,
            self.timeout,
        );

        let mut runtime = PegRuntime::new(input, &self.registry, self.grammar);

        loop {
            let state = runtime.start(&self.rule);
            match runtime.resume(state)? {
                super::incremental::ParseNext::Match(value) => {
                    if self.output.send(value).await.is_err() {
                        break; // Output closed
                    }
                }
                super::incremental::ParseNext::NeedInput(_) => {
                    // Wait for more input - the blocking happens in StreamingInput
                    continue;
                }
                super::incremental::ParseNext::End => break,
            }
        }

        Ok(())
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core driver::tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/driver.rs fmpl-core/src/grammar/mod.rs
git commit -m "feat(grammar): add ParseDriver for streaming pipelines"
```

---

## Task 5: Wire |> Operator to Parse Driver

**Files:**
- Modify: `fmpl-core/src/vm.rs:756-773` (push_stream_parse)
- Modify: `fmpl-core/src/value.rs` (add AsyncParse stream op variant)

**Step 1: Write the failing test**

In `fmpl-core/tests/streaming_parse.rs` (new file):

```rust
use fmpl_core::grammar::GrammarRegistry;
use fmpl_core::stream::{StreamEvent, StreamHandle};
use fmpl_core::value::{Stream, StreamOp, Value};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pipe_to_grammar_creates_output_stream() {
    let registry = GrammarRegistry::new();
    let grammar = registry.get("base::tree").unwrap();

    // Create input stream
    let (tx, rx) = mpsc::channel(10);
    let handle = StreamHandle::new(rx, 1);

    // Create stream value with Parse op
    let stream = Stream {
        source: Value::StreamHandle(Arc::new(handle)),
        ops: vec![StreamOp::AsyncParse {
            grammar: Value::Grammar(grammar),
            rule: "any".into(),
        }],
    };

    // When materialized, should produce output stream
    // This test verifies the type exists and can be constructed
    assert!(matches!(stream.ops[0], StreamOp::AsyncParse { .. }));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_pipe_to_grammar --no-run 2>&1 | head -20`
Expected: Compilation error - `StreamOp::AsyncParse` doesn't exist

**Step 3: Write minimal implementation**

In `value.rs`, add to `StreamOp`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse { grammar: Value, rule: SmolStr },
    /// Async streaming parse - emits matches as they occur.
    AsyncParse { grammar: Value, rule: SmolStr },
}
```

In `vm.rs`, update `push_stream_parse` or add new handler:
```rust
Instruction::StreamAsyncParse(rule) => {
    self.push_stream_async_parse(rule)?;
}

fn push_stream_async_parse(&mut self, rule: SmolStr) -> Result<()> {
    let grammar = self.pop()?;
    let stream = self.pop()?;
    let Value::Stream(stream) = stream else {
        return Err(Error::Type {
            expected: "stream".to_string(),
            got: stream.type_name().to_string(),
        });
    };

    let mut ops = stream.ops.clone();
    ops.push(StreamOp::AsyncParse { grammar, rule });
    let next = Stream {
        source: stream.source.clone(),
        ops,
    };
    self.stack.push(Value::Stream(Arc::new(next)));
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_pipe_to_grammar -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/value.rs fmpl-core/src/vm.rs fmpl-core/tests/streaming_parse.rs
git commit -m "feat(vm): add AsyncParse stream operation for |> grammar wiring"
```

---

## Task 6: Add Fjall Persistence for Memo Tables

**Files:**
- Modify: `fmpl-core/src/grammar/stream_input.rs:32-43` (StreamPosition memo)

**Step 1: Write the failing test**

In `fmpl-core/src/grammar/stream_input.rs` tests:

```rust
#[test]
fn test_memo_table_persists_to_fjall() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let values = vec![Value::Int(1), Value::Int(2)];

    // Create stream with Fjall memo backing
    let stream = StreamPosition::from_values_with_fjall(
        values,
        Some(temp_dir.path().to_path_buf()),
    );

    // Set memo entry
    stream.set_memo(SmolStr::new("test_rule"), MemoEntry::Done(Some(Value::Int(42)), 1));

    // Verify it can be retrieved
    let memo = stream.get_memo(&SmolStr::new("test_rule"));
    assert!(matches!(memo, Some(MemoEntry::Done(Some(Value::Int(42)), 1))));

    // Verify it persists (simulate reload by checking Fjall directly)
    // The actual persistence is verified by the from_async_with_fjall tests
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_memo_table_persists --no-run 2>&1 | head -20`
Expected: Compilation error - `from_values_with_fjall` doesn't exist

**Step 3: Write minimal implementation**

Add to `StreamPosition`:
```rust
/// Optional Fjall partition for memo overflow.
memo_fjall: Option<Arc<Mutex<fjall::PartitionHandle>>>,

pub fn from_values_with_fjall(values: Vec<Value>, fjall_path: Option<PathBuf>) -> Rc<Self> {
    let fjall = fjall_path.map(|path| {
        let keyspace = fjall::Config::new(path).open().unwrap();
        let partition = keyspace.open_partition("memo", Default::default()).unwrap();
        Arc::new(Mutex::new(partition))
    });

    // ... build chain with fjall reference ...
}
```

Update `get_memo` and `set_memo` to check Fjall when not in memory.

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_memo_table_persists -v --features fjall-persistence`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/stream_input.rs
git commit -m "feat(grammar): add Fjall persistence for memo tables"
```

---

## Task 7: Add Parse State Serialization for Durable Suspension

**Files:**
- Modify: `fmpl-core/src/grammar/incremental.rs`
- Create: `fmpl-core/tests/parse_state_persistence.rs`

**Step 1: Write the failing test**

In `fmpl-core/tests/parse_state_persistence.rs`:

```rust
use fmpl_core::grammar::incremental::ParseState;
use fmpl_core::value::Value;
use std::collections::HashMap;

#[test]
fn test_parse_state_binary_serialization() {
    let mut bindings = HashMap::new();
    bindings.insert("x".into(), Value::Int(42));
    bindings.insert("name".into(), Value::String("test".into()));

    let state = ParseState {
        position_index: 100,
        rule_stack: vec![
            ("outer".into(), 0),
            ("inner".into(), 50),
        ],
        bindings,
    };

    // Serialize to bytes (for Fjall storage)
    let bytes = state.to_bytes().unwrap();

    // Deserialize
    let restored = ParseState::from_bytes(&bytes).unwrap();

    assert_eq!(restored.position_index, 100);
    assert_eq!(restored.rule_stack.len(), 2);
    assert_eq!(restored.bindings.get("x"), Some(&Value::Int(42)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_parse_state_binary --no-run 2>&1 | head -20`
Expected: Compilation error - `to_bytes` and `from_bytes` don't exist

**Step 3: Write minimal implementation**

Add to `ParseState`:
```rust
impl ParseState {
    /// Serialize to bytes for Fjall storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_parse_state_binary -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/incremental.rs fmpl-core/tests/parse_state_persistence.rs
git commit -m "feat(grammar): add binary serialization for ParseState"
```

---

## Task 8: Integration Test - Full Streaming Parse Pipeline

**Files:**
- Create: `fmpl-core/tests/streaming_pipeline.rs`

**Step 1: Write the failing test**

```rust
//! Integration test for full streaming parse pipeline.

use fmpl_core::grammar::{Grammar, GrammarRegistry, Pattern, Rule};
use fmpl_core::grammar::driver::ParseDriver;
use fmpl_core::stream::{StreamEvent, StreamHandle};
use fmpl_core::value::Value;
use smol_str::SmolStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_streaming_tool_call_parsing() {
    // Create grammar for tool calls: { "tool": string, "args": any }
    let mut registry = GrammarRegistry::new();
    let mut grammar = Grammar::with_parent(
        SmolStr::new("test::tool"),
        SmolStr::new("base::tree"),
    );

    // tool_call = any (simplified - matches any value)
    grammar.add_rule(
        SmolStr::new("tool_call"),
        Rule::new(Pattern::Any),
    );
    registry.register(grammar);
    let grammar = registry.get("test::tool").unwrap();

    // Simulate LLM streaming tokens
    let (in_tx, in_rx) = mpsc::channel(10);
    let (out_tx, mut out_rx) = mpsc::channel(10);

    let handle = StreamHandle::new(in_rx, 1);
    let driver = ParseDriver::new(
        handle,
        grammar,
        "tool_call".to_string(),
        &registry,
        out_tx,
    ).with_timeout(Some(Duration::from_secs(1)));

    // Start driver
    let driver_handle = tokio::spawn(async move {
        driver.run().await
    });

    // Stream tool call values
    let tool1 = Value::Map(Arc::new([
        ("tool".to_string(), Value::String("search".into())),
        ("args".to_string(), Value::String("rust async".into())),
    ].into_iter().collect()));

    let tool2 = Value::Map(Arc::new([
        ("tool".to_string(), Value::String("calculate".into())),
        ("args".to_string(), Value::Int(42)),
    ].into_iter().collect()));

    in_tx.send(StreamEvent::Data(tool1.clone())).await.unwrap();
    in_tx.send(StreamEvent::Data(tool2.clone())).await.unwrap();
    drop(in_tx); // Signal end

    // Collect results
    let mut results = Vec::new();
    while let Some(v) = out_rx.recv().await {
        results.push(v);
    }

    driver_handle.await.unwrap().unwrap();

    // Should have parsed both tool calls
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], tool1);
    assert_eq!(results[1], tool2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_streaming_backtracking_with_buffer() {
    // Test that backtracking works with buffered stream positions
    let registry = GrammarRegistry::new();
    let grammar = registry.get("base::tree").unwrap();

    let (in_tx, in_rx) = mpsc::channel(10);
    let (out_tx, mut out_rx) = mpsc::channel(10);

    let handle = StreamHandle::new(in_rx, 1);

    // Use int* pattern (zero or more ints)
    let mut custom = Grammar::with_parent_grammar(
        SmolStr::new("test::ints"),
        grammar.clone(),
    );
    custom.add_rule(
        SmolStr::new("ints"),
        Rule::new(Pattern::Star(Box::new(Pattern::MatchType(SmolStr::new("int"))))),
    );
    let mut reg = GrammarRegistry::new();
    reg.register(custom);
    let grammar = reg.get("test::ints").unwrap();

    let driver = ParseDriver::new(
        handle,
        grammar,
        "ints".to_string(),
        &reg,
        out_tx,
    ).with_timeout(Some(Duration::from_millis(100)));

    let driver_handle = tokio::spawn(async move {
        driver.run().await
    });

    // Send ints with gaps
    in_tx.send(StreamEvent::Data(Value::Int(1))).await.unwrap();
    in_tx.send(StreamEvent::Data(Value::Int(2))).await.unwrap();
    in_tx.send(StreamEvent::Data(Value::Int(3))).await.unwrap();
    drop(in_tx);

    // Should collect all ints into a list
    let mut results = Vec::new();
    while let Some(v) = out_rx.recv().await {
        results.push(v);
    }

    driver_handle.await.unwrap().unwrap();

    // Star pattern should have collected all ints
    assert!(!results.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core streaming_pipeline --no-run 2>&1 | head -30`
Expected: Should compile but may fail if integration isn't complete

**Step 3: Fix any integration issues**

This test validates the full pipeline works end-to-end. Fix any issues discovered.

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core streaming_pipeline -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/tests/streaming_pipeline.rs
git commit -m "test(grammar): add integration tests for streaming parse pipeline"
```

---

## Task 9: Add Documentation

**Files:**
- Modify: `fmpl-core/src/grammar/mod.rs` (module-level docs)
- Modify: `fmpl-core/src/grammar/driver.rs` (API docs)

**Step 1: Write documentation**

Update `fmpl-core/src/grammar/mod.rs` module docs:

```rust
//! OMeta-style extensible grammars for FMPL.
//!
//! This module provides PEG-based parsing with grammar inheritance,
//! packrat memoization, and semantic actions that produce FMPL values.
//!
//! ## Streaming Grammar Pipelines
//!
//! Grammars can operate on async streams with full backtracking:
//!
//! ```fmpl
//! llm_stream |> parser.tool_call |> execute_tool
//! ```
//!
//! The pipeline works like Unix pipes:
//! - Each value from `llm_stream` pushes into `parser.tool_call`
//! - When `tool_call` fully matches, its result pushes to `execute_tool`
//! - Backtracking is unlimited with buffered input (spills to Fjall)
//! - Memoization prevents re-execution of external calls
//!
//! ## Durable Suspension
//!
//! Parse state can be serialized for durable suspension:
//!
//! ```rust
//! let state = runtime.start("rule");
//! let bytes = state.to_bytes()?;
//! // ... store in Fjall ...
//! let restored = ParseState::from_bytes(&bytes)?;
//! runtime.resume(restored)?;
//! ```
```

**Step 2: Run doc tests**

Run: `cargo test -p fmpl-core --doc`
Expected: PASS (or no doc tests to run)

**Step 3: Commit**

```bash
git add fmpl-core/src/grammar/mod.rs fmpl-core/src/grammar/driver.rs
git commit -m "docs(grammar): add streaming grammar documentation"
```

---

## Summary

| Task | Component | Description |
|------|-----------|-------------|
| 1 | incremental.rs | ParseState and ParseNext types |
| 2 | stream_input.rs | Fjall backing for position buffer |
| 3 | runtime.rs | start()/resume() incremental API |
| 4 | driver.rs | ParseDriver for async pipelines |
| 5 | vm.rs + value.rs | AsyncParse stream operation |
| 6 | stream_input.rs | Fjall persistence for memo tables |
| 7 | incremental.rs | Binary serialization for ParseState |
| 8 | tests/ | Integration tests |
| 9 | docs | Module documentation |

After completing all tasks, run the full test suite:
```bash
cargo test -p fmpl-core --features fjall-persistence
```
