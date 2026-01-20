# Streaming Grammar

Push-based incremental parsing for async streams.

**Location**: [fmpl-core/src/grammar/](../fmpl-core/src/grammar/)

---

## Overview

Extends the grammar system to handle async streams (LLM output, HTTP chunks) with:

- **Push-based parsing** — Values arrive asynchronously, grammar emits matches
- **Unlimited backtracking** — Buffered positions with Fjall overflow
- **Packrat memoization** — Persisted memo tables survive suspension
- **Incremental API** — `start()`/`resume()` for durable parse states

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    ParseDriver                          │
│  - Connects async stream to grammar                     │
│  - Manages ParseState suspension/resumption             │
│  - Emits parsed values downstream                       │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│                   PegRuntime                            │
│  - start(rule) → ParseState                             │
│  - resume(state) → ParseNext                            │
│  - Packrat memoization                                  │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│               StreamPosition                            │
│  - In-memory buffer for hot path                        │
│  - Fjall overflow for large streams                     │
│  - Position tracking with unlimited backtrack           │
└─────────────────────────────────────────────────────────┘
```

---

## Key Types

### ParseState

Represents suspended parse state:

```rust
pub struct ParseState {
    /// Position in input stream
    pub position_index: usize,
    /// Stack of (rule_name, position) for backtracking
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Bound variables from pattern matching
    pub bindings: HashMap<SmolStr, Value>,
}
```

### ParseNext

Result of incremental parse step:

```rust
pub enum ParseNext {
    /// Successful match with value
    Match(Value),
    /// Need more input, save state
    NeedInput(ParseState),
    /// End of input reached
    End,
}
```

### StreamPosition

Position in a potentially infinite stream:

```rust
pub struct StreamPosition {
    /// In-memory buffer for recent positions
    buffer: Vec<Value>,
    /// Start offset (positions before this are in Fjall)
    start_offset: usize,
    /// Current read position
    position: usize,
    /// Fjall partition for overflow (optional)
    #[cfg(feature = "fjall-persistence")]
    overflow: Option<FjallOverflow>,
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

### Incremental API

```rust
// Start parsing
let mut runtime = PegRuntime::new(grammar, registry);
let state = runtime.start("rule_name");

// Resume with more input
loop {
    match runtime.resume(state) {
        ParseNext::Match(value) => emit(value),
        ParseNext::NeedInput(s) => {
            state = s;
            let chunk = stream.next().await;
            runtime.feed(chunk);
        }
        ParseNext::End => break,
    }
}
```

---

## Fjall Backing

### Stream Position Overflow

When buffer exceeds threshold, older positions spill to disk:

```rust
impl StreamPosition {
    fn push(&mut self, value: Value) {
        self.buffer.push(value);
        if self.buffer.len() > THRESHOLD {
            self.spill_to_fjall();
        }
    }

    fn get(&self, pos: usize) -> Option<Value> {
        if pos < self.start_offset {
            self.read_from_fjall(pos)
        } else {
            self.buffer.get(pos - self.start_offset).cloned()
        }
    }
}
```

### Memo Table Persistence

Memoization results persist across suspension:

```rust
// Key: (position, rule_name)
// Value: ParseResult

#[cfg(feature = "fjall-persistence")]
impl MemoTable {
    fn get(&self, pos: usize, rule: &str) -> Option<ParseResult> {
        // Check in-memory first
        if let Some(result) = self.hot.get(&(pos, rule)) {
            return Some(result);
        }
        // Fall back to Fjall
        self.cold.get(&(pos, rule))
    }
}
```

---

## ParseDriver

Async driver connecting streams to grammars:

```rust
pub struct ParseDriver {
    runtime: PegRuntime,
    state: Option<ParseState>,
    input_rx: Receiver<Value>,
    output_tx: Sender<Value>,
}

impl ParseDriver {
    pub async fn run(&mut self) {
        loop {
            match self.runtime.resume(self.state.take().unwrap()) {
                ParseNext::Match(value) => {
                    self.output_tx.send(value).await.ok();
                }
                ParseNext::NeedInput(state) => {
                    self.state = Some(state);
                    match self.input_rx.recv().await {
                        Some(chunk) => self.runtime.feed(chunk),
                        None => break,
                    }
                }
                ParseNext::End => break,
            }
        }
    }
}
```

---

## StreamOp Variants

```rust
pub enum StreamOp {
    // Existing
    Map { f: Value },
    Filter { f: Value },
    Parse { grammar: Value, rule: SmolStr },      // Blocking parse

    // Streaming
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
| ParseState serialization | In Progress |
| Integration tests | Planned |

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — Base grammar system
- [persistence.md](./persistence.md) — Fjall storage
- [fmpl-core.md](./fmpl-core.md) — Core runtime
