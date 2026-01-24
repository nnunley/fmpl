# Stream Observability Design

**Goal:** Enable comprehensive observation and debugging of streaming agent interactions through a unified cursor-based interface that treats everything—network connections, agents, TUI panels, and cursors—as immutable streams.

**Core thesis:** Since all streams are immutable cons chains with head/tail structure, and stream buffers already persist to Fjall for backtracking, a cursor is simply a stream that starts at a specific position. `observe(stream)` returns a cursor, enabling live inspection, forking for alternative downstream experiments, and time-travel via position rewinding.

---

## Architecture

### Stream Foundations

FMPL streams are immutable cons chains with lazy operations:

```fmpl
Stream {
  source: Value (the producer)
  ops: Vec<StreamOp> (transformations: map, filter, parse, etc.)
}
```

Stream execution produces values through channels:
- **Data(v)** - Intermediate value
- **Ok(v)** - Terminal success
- **Err(e)** - Terminal failure

Streams backed by persistent sources (HTTP, WebSocket, files) spill overflow buffers to Fjall, enabling time-travel and replay.

### Cursor as Stream

A cursor is a stream that starts at a specific position in an existing stream:

```fmpl
let original = http.get(url) |> parse(tool_grammar) |> handle_result
let cursor = observe(original)  -- Returns a stream (cursor) starting at current position

-- Cursor is itself a stream
cursor |> take(10)           -- Get next 10 values
cursor |> filter(|x| x > 5)  -- Filter from cursor position
*cursor                    -- Get current head value
```

### Observer Semantics

**Observation creates a reference, not a copy:**

```fmpl
let stream = source |> map(|x| x + 1) |> filter(|x| x > 0)

let cursor1 = observe(stream)  -- Cursor at stream's current position
let cursor2 = observe(stream)  -- Independent cursor at same position

*cursor1  -- Consumes first value from cursor1
*cursor2  -- Consumes first value from cursor2 (same value)

-- Both cursors share the underlying stream position
-- Both can fork into independent downstream experiments
```

**Multiple observers don't interfere:**

- Original stream continues flowing to its consumer
- Observers tap into the stream without blocking
- Each cursor maintains its own position
- Fjall persistence ensures values aren't lost

### Forking and Experiments

A key debugging power is forking streams at any cursor position to try alternative transformations. Since `observe()` works on both streams and cursors, forking is simply observing a cursor:

```fmpl
let cursor = observe(agent_stream)

-- Get to a specific point
cursor |> foreach(\x *cursor)  -- Consume until interesting state

-- Fork into parallel experiments by observing the cursor
let branch1 = observe(cursor) |> map(|x| x * 2)
let branch2 = observe(cursor) |> map(|x| x + 10)

-- Both branches start from the same cursor position
-- Original stream continues unaffected
-- Try alternative debugging approaches in parallel
```

**Forking with `observe()`:**
- `observe(cursor)` creates a new cursor at the same position
- Each cursor maintains its own position and downstream operations
- All cursors share the same underlying immutable stream
- Can fork recursively: `observe(observe(cursor))`

### Time-Travel via Rewinding

Persistent stream buffers enable rewinding to earlier positions:

```fmpl
let cursor = observe(stream)

-- Move forward
*cursor        -- Current value
*cursor.next  -- Next value
*cursor.next  -- Following value

-- Rewind to earlier position
cursor.rewind(position_id)  -- Jump back

-- Continue from rewound position
*cursor        -- Value at rewound position
*cursor.next  -- Next value (replays from buffer)
```

`position_id` uniquely identifies a stream position across time, enabling:
- Exact replay of values from that point
- Alternative downstream experiments from historical positions
- Audit trails for compliance debugging

---

## API Design

### observe()

```fmpl
observe(stream_or_cursor) -> Cursor (which is a Stream)
```

Creates a cursor stream starting at the current position.

**Behavior:**
- **On streams**: Returns a cursor at the stream's current position
- **On cursors**: Returns a new cursor at the same position (forking)
- Does not consume values from the original stream/cursor
- Original stream continues to its consumer
- Multiple observers can attach to the same stream or cursor

**Examples:**

```fmpl
let stream = source |> map(|x| x + 1)
let cursor1 = observe(stream)     -- Cursor at stream position
let cursor2 = observe(cursor1)    -- Fork cursor1
let cursor3 = observe(cursor1)    -- Another independent fork
```

### Cursor Operations

Since cursors are streams, they support all stream operations plus cursor-specific methods:

```fmpl
let cursor = observe(stream)

-- Standard stream operations (via |> pipe)
cursor |> take(10)           -- Get next 10 values as list
cursor |> filter(|x| x > 5)  -- Filter from cursor position
cursor |> map(|x| x * 2)     -- Transform values

-- Cursor-specific operations
*cursor                    -- Dereference: get current head value
cursor.next()              -- Advance to next value, return it
cursor.peek()              -- Look at next value without consuming
observe(cursor)            -- Fork: create independent copy at current position
cursor.rewind(pos)        -- Rewind to specific position
cursor.position()          -- Get current position ID
```

### Dereference: *cursor

The `*` operator dereferences a cursor to its current head value:

```fmpl
let cursor = observe(stream)
*cursor                    -- Returns current Value at cursor position

-- If cursor is at position 5 with value 42:
*cursor == 42              -- true

-- Pattern matching works naturally
cursor @ {
  %{data: value} => print("Got: " + value)
  %{ok: result} => print("Done: " + result)
}
```

### rewind(position_id)

Jump to an earlier position in the stream:

```fmpl
let cursor = observe(stream)
let pos1 = cursor.position()  -- Save position
*cursor.next()               -- Advance
*cursor.next()               -- Advance again

cursor.rewind(pos1)         -- Rewind to pos1
*cursor                    -- Value is the same as before advancement
```

**Rewind behavior:**
- Position IDs uniquely identify stream positions across time
- Fjall-backed buffers enable replay from any historical position
- Rewinding doesn't affect the original stream
- Multiple cursors can rewind independently

---

## Everything as a Stream

This design unifies observation across all stream-producing entities:

### Network Connections

```fmpl
let connection = http.get(url)
connection |> observe |> log_events

-- Connection behaves like a stream
connection |> take(1)           -- Get first chunk
connection |> foreach(|chunk process(chunk))
```

### Agent Streams

```fmpl
let agent = spawn(tool_agent, config)
agent_stream = agent.output
agent_stream |> observe |> debug_log

-- Agent output is a stream
agent_stream |> take(5)          -- Get next 5 messages
agent_stream |> foreach(|msg handle(msg))
```

### TUI Panels

```fmpl
-- TUI panels expose their state as streams
let panel = tui.panel("research")
panel_stream = panel.changes
panel_stream |> observe |> update_tui

-- Panel changes are a stream
panel_stream |> foreach(|change apply(change))
```

### Composition

Since cursors are streams, they compose with all stream operations:

```fmpl
let cursor = observe(agent_stream)

-- Complex observation pipeline
cursor
  |> filter(|msg| msg.type == "tool_call")
  |> map(|msg| %{timestamp: now(), msg: msg})
  |> take(10)
  |> foreach(|tool log_tool_call(tool))
```

---

## Pattern Matching

Cons-style structure enables pattern matching on stream state:

```fmpl
cursor @ {
  {head | tail} => {
    -- head is current value (*cursor)
    -- tail is the remainder stream
    print("Current: " + *head)
    tail |> continue_processing
  }
}
```

This enables:
- Extracting current value
- Processing remainder recursively
- Forking at specific patterns
- Implementing custom traversal logic

---

## Implementation Notes

### Value::Cursor

Add a new `Value::Cursor(Arc<Cursor>)` variant to `value.rs`:

```rust
pub struct Cursor {
    stream: Arc<Stream>,
    position: StreamPosition,
}
```

### observe() Instruction

Add `Instruction::Observe { target: InstrIndex }` to compiler.rs:

```rust
Instruction::Observe { target: InstrIndex }
```

**Behavior:**
- If target is a `Value::AsyncStream`: creates cursor at stream's current position
- If target is a `Value::Cursor`: creates new cursor at same position (fork)
- Returns `Value::Cursor(Arc<Cursor>)`

**Implementation sketch:**

```rust
pub fn observe(value: &Value) -> Result<Value> {
    match value {
        Value::AsyncStream(stream) => {
            Ok(Value::Cursor(Arc::new(Cursor {
                stream: stream.clone(),
                position: stream.current_position()?,
            })))
        }
        Value::Cursor(cursor) => {
            // Fork: create new cursor at same position
            Ok(Value::Cursor(Arc::new(Cursor {
                stream: cursor.stream.clone(),
                position: cursor.position.clone(),
            })))
        }
        _ => Err(Error::TypeError("observe requires stream or cursor"))
    }
}
```

### Cursor Methods

Implement cursor methods as built-in functions or VM instructions:

- `*cursor` → Load current head value
- `cursor.next()` → Consume next value, update position
- `cursor.peek()` → Look at next value without consuming
- `cursor.rewind(pos)` → Jump to earlier position
- `cursor.position()` → Get current position ID

**Note**: Forking is done via `observe(cursor)`, not a separate method.

### Position Tracking

Leverage existing `StreamPosition` with Fjall backing:

- Each stream position has a unique ID
- Fjall persists values to enable replay
- Cursors store position IDs for rewinding
- Memoization enables efficient position resumption

---

## Testing Strategy

### Unit Tests

- `cursor_observe_returns_stream` - Verify observe() returns cursor
- `cursor_observe_cursor_forks` - Verify observe(cursor) creates independent cursor
- `cursor_dereference_gets_current_value` - Test `*cursor` behavior
- `cursor_rewind_jumps_to_position` - Time-travel correctness
- `multiple_observers_dont_interfere` - Concurrent observation

### Integration Tests

- `network_stream_observation` - Tap HTTP stream, inspect chunks
- `agent_stream_forking` - Try alternative transformations at same point
- `cursor_rewind_with_fjall` - Rewind across persisted buffer boundaries
- `tui_panel_as_stream` - TUI panel changes observable as stream

### Manual Tests

- Start agent, observe output stream at mid-execution
- Fork cursor with `observe(cursor)`, try different debug transformations
- Rewind cursor to earlier position, replay with different logic
- Verify original agent unaffected by observations

---

## Future Enhancements

### Phase 2: Time-Travel Cursors

Full time-travel with position history:
- `cursor.history()` - List all position IDs reachable from cursor
- `cursor.rewind_to(pos_id)` - Jump to any position in history
- `observe(cursor_at_history)` - Fork from historical position

### Phase 3: TUI Integration

Visual stream inspection in the TUI:
- `Ctrl+O` - Attach observer to focused panel's stream
- Stream browser panel showing all active streams
- Cursor control: pause, resume, fork, rewind
- Pattern matching on stream values for conditional breakpoints

### Phase 4: Observability Registry

Central registry of all observable streams:
- `streams.list()` - Show all active streams
- `streams.info(id)` - Get metadata and position
- `streams.tap(id)` - Attach observer to stream by ID
- Persistence of observation graphs for audit

---

## References

- `fmpl-core/src/stream.rs` - Stream and StreamHandle implementation
- `fmpl-core/src/value.rs` - Value enum and AsyncStream types
- `fmpl-core/src/grammar/stream_input.rs` - StreamPosition with Fjall backing
- `docs/plans/2026-01-20-streaming-grammar-push-model-design.md` - Incremental parsing with push-model
- `docs/design/project-overview-draft.md` - 12-factor agents with streaming grammars
