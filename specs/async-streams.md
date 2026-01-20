# Async Streams

Async stream primitives with pipe operator.

**Location**: [fmpl-core/src/stream.rs](../fmpl-core/src/stream.rs), [fmpl-core/src/value.rs](../fmpl-core/src/value.rs)

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

### Grammar Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `parse(g.rule)` | Blocking parse | `stream \|> parse(json.value)` |
| `async_parse(g.rule)` | Incremental parse | `stream \|> async_parse(tool.call)` |

---

## Key Types

### StreamHandle

```rust
pub struct StreamHandle {
    pub rx: tokio::sync::mpsc::Receiver<StreamEvent>,
}
```

### SinkHandle

```rust
pub struct SinkHandle {
    pub tx: tokio::sync::mpsc::Sender<StreamEvent>,
}
```

### StreamEvent

```rust
pub enum StreamEvent {
    Value(Value),
    End,
    Error(String),
}
```

### StreamOp

```rust
pub enum StreamOp {
    Map { f: Value },
    Filter { f: Value },
    Parse { grammar: Value, rule: SmolStr },
    AsyncParse { grammar: Value, rule: SmolStr },
    Collect,
    Take { n: usize },
    Drop { n: usize },
    FlatMap { f: Value },
}
```

---

## Value Representation

```rust
pub enum Value {
    // Stream values
    Stream(StreamHandle),
    Sink(SinkHandle),
    StreamOp(Box<StreamOp>),
    Promise(PromiseHandle),
    // ...
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

## Related Specs

- [streaming-grammar.md](./streaming-grammar.md) — Incremental parsing
- [vm.md](./vm.md) — Async VM support
- [fmpl-core.md](./fmpl-core.md) — Core runtime
