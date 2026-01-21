# Streaming Grammar

Push-based incremental parsing for async streams.

**Location**: [fmpl-core/src/grammar/](../fmpl-core/src/grammar/)

---

## Overview

Extends the grammar system to handle async streams (LLM output, HTTP chunks) with:

- **Push-based parsing** — Values arrive asynchronously, grammar emits matches
- **Unlimited backtracking** — OMeta-style cons-cell positions with Fjall overflow
- **Packrat memoization** — Per-position memo tables with optional Fjall backing
- **Incremental API** — `start()`/`resume()` for durable parse states

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    ParseDriver                          │
│  driver.rs:24                                           │
│  - Collects values from async stream                    │
│  - Runs grammar against each value                      │
│  - Emits matched values downstream                      │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│                   PegRuntime                            │
│  runtime.rs:900                                         │
│  - start(rule) → ParseState                             │
│  - resume(state) → ParseNext                            │
│  - Per-position packrat memoization                     │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│               StreamPosition                            │
│  stream_input.rs:42                                     │
│  - Immutable cons-cell with lazy tail                   │
│  - Per-position memo table                              │
│  - Fjall overflow in StreamSource::Async                │
└─────────────────────────────────────────────────────────┘
```

---

## Key Types

### ParseState (`incremental.rs:15`)

Represents suspended parse state:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Current position index in input
    pub position_index: usize,
    /// Rule call stack: (rule_name, entry_position_index)
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Current variable bindings
    pub bindings: HashMap<SmolStr, Value>,
}
```

Serialization methods (`incremental.rs:65-97`, feature-gated):
- `to_bytes()` / `from_bytes()` — rkyv serialization
- `save_to_fjall()` / `load_from_fjall()` — durable persistence

### ParseNext (`incremental.rs:26`)

Result of incremental parse step:

```rust
pub enum ParseNext {
    /// Rule matched, here's the result value
    Match(Value),
    /// Need more input - here's state to resume from
    NeedInput(ParseState),
    /// Input stream ended
    End,
}
```

### StreamPosition (`stream_input.rs:42`)

OMeta-style immutable cons-cell for streaming input:

```rust
pub struct StreamPosition {
    /// The value at this position (None = end of stream)
    head: Option<Value>,
    /// The next position (lazily computed)
    tail: RefCell<Option<Rc<StreamPosition>>>,
    /// Position index (for memoization keys)
    index: usize,
    /// Per-position memoization table
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,
    /// Source reference for pulling more data
    source: Rc<StreamSource>,
    /// Optional Fjall partition for memo persistence
    #[cfg(feature = "fjall-persistence")]
    memo_fjall: Option<Arc<Mutex<MemoFjall>>>,
}
```

### MemoEntry (`stream_input.rs:59`)

Cached parse result for packrat memoization:

```rust
pub enum MemoEntry {
    /// Parsing in progress (left recursion detection)
    InProgress,
    /// Completed: (value, end_position_index)
    Done(Option<Value>, usize),
}
```

---

## Usage

### Pipeline Syntax

```fmpl
-- LLM stream → parser → handler
llm_stream |> parser.tool_call |> execute_tool

-- With async parse operator
llm_stream |> AsyncParse { grammar: ToolParser, rule: "output" } |> handler
```

### Incremental API (`runtime.rs:900-945`)

```rust
// Start parsing
let input = StreamingInput::from_values(values);
let mut runtime = PegRuntime::new(input, &registry, grammar);
let state = runtime.start("rule_name");

// Resume and get result
match runtime.resume(state)? {
    ParseNext::Match(value) => {
        // Successfully matched - use value
    }
    ParseNext::NeedInput(state) => {
        // Need more input - state can be saved for later
    }
    ParseNext::End => {
        // Input stream ended
    }
}
```

---

## Fjall Backing

### Stream Source Overflow (`stream_input.rs:99-116`)

For async streams, positions can spill to Fjall when memory is limited:

```rust
enum StreamSource {
    Async {
        handle: Mutex<StreamHandle>,
        timeout: Option<Duration>,
        /// Cached positions for index lookup
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        /// Fjall overflow for spilled positions
        #[cfg(feature = "fjall-persistence")]
        fjall: Option<FjallOverflow>,
        /// Memory limit before spilling
        #[cfg(feature = "fjall-persistence")]
        memory_limit: Option<usize>,
    },
    Static(Vec<Value>),
    Empty,
}
```

### Per-Position Memo Persistence (`stream_input.rs:635-684`)

Each `StreamPosition` has its own memo table with optional Fjall backing:

```rust
impl StreamPosition {
    fn get_memo(&self, rule: &SmolStr) -> Option<MemoEntry> {
        // Check in-memory first
        if let Some(entry) = self.memo.borrow().get(rule) {
            return Some(entry.clone());
        }
        // Fall back to Fjall if configured
        #[cfg(feature = "fjall-persistence")]
        if let Some(fjall) = &self.memo_fjall {
            // Key format: "{position_index}:{rule_name}"
            return self.read_memo_from_fjall(fjall, rule);
        }
        None
    }

    fn set_memo(&self, rule: SmolStr, entry: MemoEntry) {
        self.memo.borrow_mut().insert(rule.clone(), entry.clone());
        // Also persist to Fjall if configured
        #[cfg(feature = "fjall-persistence")]
        if let Some(fjall) = &self.memo_fjall {
            self.write_memo_to_fjall(fjall, &rule, &entry);
        }
    }
}
```

---

## ParseDriver (`driver.rs:24`)

Async driver connecting streams to grammars:

```rust
pub struct ParseDriver {
    input_handle: StreamHandle,
    grammar: Arc<Grammar>,
    rule: String,
    registry: GrammarRegistry,
    output: mpsc::Sender<Value>,
    timeout: Option<Duration>,
}
```

The `run()` method (`driver.rs:71-139`) collects input values then parses each independently:

```rust
impl ParseDriver {
    pub async fn run(mut self) -> Result<()> {
        // 1. Collect values from async stream
        let mut values = Vec::new();
        loop {
            match recv_with_timeout(&mut self.input_handle, self.timeout).await {
                Some(StreamEvent::Data(value)) => values.push(value),
                Some(StreamEvent::Ok(value)) => { values.push(value); break; }
                _ => break,
            }
        }

        // 2. Parse each value with a fresh runtime
        let mut results = Vec::new();
        for value in values {
            let input = StreamingInput::from_values(vec![value]);
            let mut runtime = PegRuntime::new(input, &self.registry, self.grammar.clone());
            let state = runtime.start(&self.rule);
            if let ParseNext::Match(matched) = runtime.resume(state)? {
                results.push(matched);
            }
        }

        // 3. Send matched results
        for result in results {
            self.output.send(result).await.ok();
        }
        Ok(())
    }
}
```

---

## StreamOp Variants (`value.rs:87`)

```rust
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse { grammar: Value, rule: SmolStr },      // Blocking parse
    AsyncParse { grammar: Value, rule: SmolStr }, // Incremental parse
}
```

---

## Implementation Status

| Component | Status |
|-----------|--------|
| ParseState/ParseNext types | Complete |
| StreamPosition with Fjall | Complete |
| Incremental API (start/resume) | Complete |
| ParseDriver | Complete |
| AsyncParse StreamOp | Complete |
| Memo table persistence | Complete |
| ParseState serialization | Complete |
| Integration tests | Complete |

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — Base grammar system
- [persistence.md](./persistence.md) — Fjall storage
- [fmpl-core.md](./fmpl-core.md) — Core runtime
