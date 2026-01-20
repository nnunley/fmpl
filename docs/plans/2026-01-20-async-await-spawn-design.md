# Async/Await/Spawn Design

## Overview

This document describes the async execution model for FMPL, enabling external tool calls (HTTP, LLM APIs, human approval) for agentic workflows.

**Focus:** External tool calls, not distributed object communication.

---

## Core Semantics

The `<-` operator makes an async call and returns a **stream**. Streams emit zero or more data events, then terminate with `%{ok: value}` or `%{err: error}`.

```fmpl
-- Async call returns a stream
let (%{source: body} = <- curl.get("https://api.example.com/data")) in
  body @ json.value

-- LLM completion streams tokens
let (%{source: tokens} = <- curl.post(llm_endpoint, prompt)) in
  tokens @ output_parser
```

The `$` (sync call) operator remains for same-vat calls where no suspension is needed.

---

## Sources and Sinks

**Source (stream):** Emits values, terminates with ok/err.

**Sink:** Receives values from a stream via `|>`.

```fmpl
-- Pipe a stream into a sink
source |> sink

-- Bidirectional connection (WebSocket)
let (%{source: input, sink: output} = <- curl.connect(ws_url)) in
  input @ message_handler |> output
```

When you write `stream |> sink`, the VM:
1. Subscribes to the stream
2. For each emitted value, calls the sink
3. On terminal event, closes the sink

---

## The `curl` Built-in

The `curl` object provides URL-based network access using curl.rs, supporting HTTP, HTTPS, FTP, SFTP, WebSocket, and other protocols.

```fmpl
-- HTTP GET: source only
let (%{source: body} = <- curl.get(url)) in
  body @ json.value

-- HTTP POST: source (response) with body
let (%{source: resp} = <- curl.post(url, %{data: payload})) in
  resp @ json.value

-- WebSocket: bidirectional
let (%{source: input, sink: output} = <- curl.connect(ws_url)) in
  input @ handler |> output

-- FTP download: source only
let (%{source: file} = <- curl.get("ftp://files.example.com/data.txt")) in
  file |> local_sink
```

The returned object has `source` and `sink` properties; either may be `nil` depending on the protocol.

---

## Pattern Destructuring in Let

The `let` binding supports pattern matching:

```fmpl
let (<pattern> = <expr>) in <body>
```

Examples:

```fmpl
-- Map destructuring
let (%{source: s, sink: k} = <- curl.connect(url)) in ...

-- List destructuring
let ([head | tail] = some_list) in
  process(head, tail)

-- Nested patterns
let (%{data: [first, second | rest]} = response) in ...
```

If the pattern doesn't match, a runtime error is raised.

---

## Error Handling

### Result Values (Primitive)

Streams terminate with `%{ok: value}` or `%{err: error}`. In grammar context, pattern match on these:

```fmpl
result_handler =
  | %{ok: %{tool: t, args: a}} => <- execute(t, a) @ result_handler
  | %{err: e} &{ retryable(e) } => retry()
  | %{err: e} => escalate(e)
```

### Try/Catch (Sugar)

For imperative code, `try`/`catch` unwraps success or throws on error:

```fmpl
try {
  let (%{source: body} = <- curl.get(url)) in
  body @ json.value
} catch e {
  %{fallback: true, error: e}
}
```

Inside a `try` block:
- Stream terminating with `%{err: e}` throws the error
- Pattern match failure throws an error
- The `catch` block receives the error value

`try`/`catch` is an expression - it evaluates to the body's value on success, or the catch block's value on error:

```fmpl
let (result = try {
  <- curl.get(url) |> collect()
} catch e {
  %{error: e}
}) in
  process(result)
```

No `finally` block for now (no resources requiring cleanup yet).

---

## VM Architecture

### Runtime Handle Injection

The VM receives a `tokio::runtime::Handle` at construction:

```rust
pub struct Vm {
    // ... existing fields ...
    runtime: Option<tokio::runtime::Handle>,
}

impl Vm {
    pub fn new() -> Self { /* runtime = None */ }
    pub fn with_runtime(handle: tokio::runtime::Handle) -> Self { /* ... */ }
    pub fn set_runtime(&mut self, handle: tokio::runtime::Handle) { /* ... */ }
}
```

This allows:
- fmpl-web passes its Axum runtime handle
- fmpl-cli creates its own runtime
- Tests use `#[tokio::test]` and pass that handle

### Async Execution Flow

When `<-` executes:
1. VM checks it has a runtime handle (error if not)
2. Creates a channel pair (mpsc for stream events)
3. Spawns an async task via the handle
4. Returns an `AsyncStreamHandle` immediately
5. Task performs the operation, sends events through channel
6. Consumer (grammar, sink, collect) receives events

### Value Types

```rust
pub enum Value {
    // ... existing variants ...
    Stream(StreamHandle),
    Sink(SinkHandle),
}

pub struct StreamHandle {
    receiver: tokio::sync::mpsc::Receiver<StreamEvent>,
    id: u64,
}

pub struct SinkHandle {
    sender: tokio::sync::mpsc::Sender<Value>,
    id: u64,
}

pub enum StreamEvent {
    Data(Value),    // Intermediate value
    Ok(Value),      // Terminal success
    Err(Value),     // Terminal error
}
```

---

## Stream Protocol

Streams emit a sequence of events:

| Event | Meaning |
|-------|---------|
| `Data(v)` | Intermediate value (chunk, token, etc.) |
| `Ok(v)` | Terminal success with final value |
| `Err(e)` | Terminal failure with error |

Examples:

```
-- HTTP streaming response
Data("chunk1"), Data("chunk2"), Ok(nil)

-- LLM completion
Data("The"), Data(" answer"), Data(" is"), Data(" 42"), Ok("The answer is 42")

-- Simple JSON API (no streaming)
Ok(%{data: [1, 2, 3]})

-- Failed request
Err(%{code: 500, message: "Internal Server Error"})
```

---

## Testing Strategy

### Unit Tests (fmpl-core)

1. **Lexer/Parser** - `try`, `catch` tokens and AST nodes
2. **Pattern destructuring in let** - various patterns, match failures
3. **Stream values** - create, emit, terminate, collect
4. **Sink protocol** - accept values, close

### Integration Tests (Mock Runtime)

Test helpers for injecting mock streams:

```rust
// Create a stream that emits predetermined values
fn mock_stream(events: Vec<StreamEvent>) -> StreamHandle

// Create a sink that collects values
fn collecting_sink() -> (SinkHandle, Vec<Value>)
```

### End-to-End Tests (fmpl-web)

Use a local test server (wiremock) for actual HTTP:

```rust
#[tokio::test]
async fn test_curl_get() {
    let server = MockServer::start().await;
    server.register(
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
    );

    let mut vm = Vm::with_runtime(Handle::current());
    let result = eval(&mut vm, &format!(r#"
        let (%{{source: body}} = <- curl.get("{}")) in
        body |> collect()
    "#, server.uri())).unwrap();

    assert_eq!(result, Value::String("hello".into()));
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure
- Add `Stream` and `Sink` value types
- Implement runtime handle injection
- Add `try`/`catch` to parser and compiler
- Add pattern destructuring to `let`

### Phase 2: Curl Integration
- Implement `curl` built-in using curl.rs
- HTTP GET/POST with streaming support
- Return `%{source: ..., sink: ...}` objects

### Phase 3: Bidirectional Streams
- WebSocket support via curl
- Sink implementation and `|>` into sinks
- Connection lifecycle (close, error handling)

### Phase 4: Testing
- Mock stream helpers
- Unit tests for all components
- Integration tests with wiremock

---

## References

- [FMPL Revival Design](2025-12-19-fmpl-revival-design.md) - Overall language vision
- [Unified Grammars and Agents](2026-01-19-unified-grammars-and-agents-design.md) - Agentic patterns
- [curl.rs](https://docs.rs/curl/) - Multi-protocol URL transfer library
- [Spritely Goblins](https://spritely.institute/goblins/) - Async object capability patterns
