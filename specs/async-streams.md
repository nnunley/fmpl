# Async Streams

Async stream primitives with pipe operator.

**Location**: [fmpl-core/src/stream.rs](../fmpl-core/src/stream.rs), [fmpl-core/src/value.rs](../fmpl-core/src/value.rs)

**Key types**:
- `StreamHandle` — `stream.rs:158`
- `SinkHandle` — `stream.rs:236`
- `StreamEvent` — `stream.rs:19`
- `StreamOp` — `value.rs:87`
- `Stream` — `value.rs:80`

---

## Overview

Streams are first-class values supporting:

- **Async sources** — HTTP, WebSocket, LLM output
- **Pipe operator** — `|>` for chaining transformations
- **Lazy evaluation** — Operators compose without immediate execution
- **Grammar integration** — Parse streams with grammars

---

## Stream Creation

### From Async Calls

```fmpl
let stream = <- http.get(url)
let stream = <- llm.complete(prompt)
let stream = <- websocket.messages()
```

### From Literals

```fmpl
let stream = stream { [1, 2, 3] }
```

---

## Pipe Operator

Chain transformations left-to-right:

```fmpl
input |> f |> g |> h

-- Equivalent to:
h(g(f(input)))
```

### With Streams

```fmpl
<- http.get(url)
  |> map(|chunk| parse_json(chunk))
  |> filter(|x| x.valid)
  |> collect
```

---

## Stream Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `map(f)` | Transform each element | `stream \|> map(\|x\| x + 1)` |
| `filter(f)` | Keep elements where f is true | `stream \|> filter(\|x\| x > 0)` |
| `collect` | Gather all elements into list | `stream \|> collect` |
| `take(n)` | Take first n elements | `stream \|> take(5)` |
| `drop(n)` | Skip first n elements | `stream \|> drop(3)` |
| `flat_map(f)` | Map and flatten | `stream \|> flat_map(\|x\| x.items)` |
| `reduce(f)` | Reduce with accumulator | `stream \|> reduce(\|acc, x\| acc + x)` |

### Grammar Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `parse(g.rule)` | Blocking parse | `stream \|> parse(json.value)` |
| `async_parse(g.rule)` | Incremental parse | `stream \|> async_parse(tool.call)` |

---

## Key Types

### StreamHandle (`stream.rs:158-193`)

```rust
pub struct StreamHandle {
    pub(crate) receiver: mpsc::Receiver<StreamEvent>,
    pub(crate) id: u64,
    pub(crate) source: StreamSource,  // For durable suspension
}
```

Methods: `new`, `with_source`, `id()`, `source()`, `recv_blocking()`

### SinkHandle (`stream.rs:236-274`)

```rust
pub struct SinkHandle {
    pub(crate) sender: mpsc::Sender<Value>,  // Note: Value, not StreamEvent
    pub(crate) id: u64,
    pub(crate) source: SinkSource,  // For durable suspension
}
```

Methods: `new`, `with_source`, `id()`, `source()`, `send_blocking()`

### StreamEvent (`stream.rs:19-26`)

```rust
pub enum StreamEvent {
    Data(Value),   // Intermediate data
    Ok(Value),     // Terminal success
    Err(Value),    // Terminal error (Value, not String)
}
```

### StreamOp (`value.rs:87-95`)

```rust
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse { grammar: Value, rule: SmolStr },
    AsyncParse { grammar: Value, rule: SmolStr },
}
```

Note: `Collect`, `Take`, `Drop` are not implemented as StreamOp variants.

---

## Value Representation (`value.rs:16-49`)

```rust
pub enum Value {
    // Grammar value
    Grammar(Arc<Grammar>),

    // Lazy stream pipeline
    Stream(Arc<Stream>),           // value.rs:32

    // Live handles (with serialization support)
    AsyncStream(Arc<Mutex<StreamHandle>>),  // value.rs:37
    Sink(Arc<SinkHandle>),                  // value.rs:42

    // Suspended handles (for resume after deserialize)
    SuspendedStream(StreamSource),  // value.rs:45
    SuspendedSink(SinkSource),      // value.rs:48
    // ...
}
```

### Stream Struct (`value.rs:80-84`)

```rust
pub struct Stream {
    pub source: Value,
    pub ops: Vec<StreamOp>,  // Lazy operation pipeline
}
```

---

## Async Operators

### spawn

```fmpl
let task = spawn(expensive_computation())
-- task is a Promise
```

### await (<-)

```fmpl
let result = <- promise
let stream = <- async_source
```

### Pipe with Async

```fmpl
-- Spawn returns promise, await returns stream
<- spawn(compute()) |> process()

-- LLM output streaming
<- llm.complete(prompt) |> parser.tool_call |> executor
```

---

## Grammar Integration

Streams can be parsed with grammars:

```fmpl
grammar ToolParser <: base::tree {
  output =
    | %{tool: t, args: a} => %{tool: t, args: a}
    | %{text: t}          => %{text: t}
}

<- llm.stream(prompt)
  |> ToolParser.output
  |> handle_output
```

### Blocking vs Incremental

```fmpl
-- Blocking: waits for complete input
stream |> parse(grammar.rule)

-- Incremental: processes chunk by chunk
stream |> async_parse(grammar.rule)
```

---

## Error Handling

Streams propagate errors:

```fmpl
<- http.get(url)
  |> map(process)
  |> catch(|e| %{error: e})
  |> collect
```

### Pattern Matching

```fmpl
<- async_operation() @ {
  %{ok: result} => handle_success(result)
  %{error: e}   => handle_error(e)
}
```

---

## Execution Model

### Lazy Composition

```fmpl
let pipeline = stream |> map(f) |> filter(g)
-- Nothing executed yet

let results = <- pipeline |> collect
-- Now executes
```

### Push vs Pull

- **Sources** push values into streams
- **Operators** transform lazily
- **Sinks** (collect, etc.) pull and consume

---

## ParseStream Combinators

In addition to async streams, FMPL provides `ParseStream` — a synchronous, combinator-based parsing API for building parsers from FMPL lambdas.

See [parse-stream.md](./parse-stream.md) for the full specification.

### Quick Reference

```fmpl
let s = stream::new("hello 42")

-- Primitive matchers
stream::match_char(s, "h")          -- Match exact character
stream::match_class(s, "a-z")       -- Match character class

-- Combinators
stream::choice(s, [rule1, rule2])    -- Ordered choice with backtracking
stream::star(s, rule)                -- Zero-or-more
stream::plus(s, rule)                -- One-or-more
stream::seq(s, [r1, r2, r3])        -- Sequence (all must match)
stream::not(s, rule)                 -- Negative lookahead
stream::lookahead(s, rule)           -- Positive lookahead
stream::optional(s, rule)            -- Zero-or-one

-- Memoization
s.apply(rule)                        -- Call rule with packrat caching

-- Failure
stream::fail("message")             -- Explicit parse failure
```

---

## Related Specs

- [parse-stream.md](./parse-stream.md) — ParseStream with combinators and packrat memoization
- [grammar-system.md](./grammar-system.md) — Streaming and incremental parsing
- [vm.md](./vm.md) — Async VM support
- [fmpl-core.md](./fmpl-core.md) — Core runtime
