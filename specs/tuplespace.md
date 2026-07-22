# Tuple Space Specification

## Overview

FMPL provides a Linda-style tuple space for pattern-based coordination between agents. The tuple space decouples producers and consumers in both time and space, enabling backpressure through blocking operations and serving as the coordination medium for multi-VAT distributed agent systems.

## Motivation

Direct message passing between agents requires explicit addressing and temporal coupling - the sender must know who to send to, and the receiver must be listening. Tuple spaces provide an alternative coordination model:

- **Pattern-based coordination**: Match tuples by pattern, not address
- **Time decoupling**: Producers can write before consumers exist
- **Space decoupling**: Producers don't need to know who reads
- **Backpressure**: Blocking operations provide natural flow control

This model is particularly well-suited for agent systems where:
- Agents may come and go dynamically
- Multiple agents may be interested in the same events
- Coordination needs to persist across failures

## Syntax

### Creating a Tuple Space

```fmpl
let space = tuplespace()
```

### Writing Tuples

```fmpl
-- Write a tuple with type and data
space.out(%{type: :event, data: "hello"})

-- Write a tuple with explicit namespace
space.out(%{namespace: :user_123, type: :click, data: %{x: 100, y: 200}})
```

### Reading Tuples

```fmpl
-- Non-blocking read (returns nil if no match)
let result = space.rdp(%{type: :event})

-- Blocking read (waits for match)
let result = space.rd(%{type: :event})

-- Non-blocking consume (removes tuple)
let result = space.inp(%{type: :event})

-- Blocking consume
let result = space.in(%{type: :event})
```

### Streaming Tuples

```fmpl
-- Create a stream of matching tuples
let events = stream { space.match(%{type: :log}) }

-- Pipe through operations
events |> filter(|e| e.level == :error) |> handle
```

### Namespace Isolation

```fmpl
let system = tuplespace()
let user_space = system.as(:user_123)

-- This works
user_space.out(%{type: :action, data: "click"})

-- This is denied (different namespace)
user_space.in(%{namespace: :other, ...})
```

## Semantics

### Tuple Structure

A tuple consists of:
- **type**: A symbol identifying the tuple type (required)
- **namespace**: An optional namespace for isolation
- **timestamp**: Ordering timestamp (auto-generated)
- **seq**: Sequence number for deterministic ordering (auto-generated)
- **data**: The payload value

### Pattern Matching

Tuple patterns support:
- **Exact type match**: `type` must match exactly
- **Wildcard patterns**: `%{type: :any}` matches any type
- **Nested patterns**: Pattern match on the `data` field
- **Namespace filtering**: Match only within a namespace

### Blocking Semantics

- **`out`**: Always non-blocking, writes immediately
- **in/rd**: Block until a matching tuple arrives
- **inp/rdp**: Return immediately with `nil` if no match

### Ordering

When multiple tuples match a pattern:
- FIFO order by sequence number
- First `out` wins

### Persistence

By default, tuples are in-memory only. The `durable: true` flag enables Fjall persistence:

```fmpl
space.out(%{type: :config, durable: true, data: %{...}})
```

## API

### Built-in Functions

```fmpl
tuplespace() -> TupleSpace
```

Creates a new tuple space.

### TupleSpace Methods

```fmpl
space.out(tuple: Map) -> Nil
space.in(pattern: Map) -> Map
space.rd(pattern: Map) -> Map
space.inp(pattern: Map) -> Map | Nil
space.rdp(pattern: Map) -> Map | Nil
space.as(namespace: Symbol) -> TupleSpaceFacet
```

### Stream Integration

```fmpl
stream { space.match(pattern: Map) } -> Stream
```

Creates a stream of tuples matching the pattern.

## Value Representation

Tuple spaces are represented as `Value::TupleSpace` containing a handle to the Rust `TupleSpace` implementation.

Tuple streams are `Value::AsyncStream` backed by tokio channels.

## Examples

### Basic Producer-Consumer

```fmpl
-- Producer
let space = tuplespace()
space.out(%{type: :task, data: %{id: 1, work: "compute"}})

-- Consumer
let task = space.in(%{type: :task})
print(task.data.id)
```

### Multiple Subscribers

```fmpl
let space = tuplespace()

-- Multiple agents can listen to the same event type
let logger = stream { space.match(%{type: :log}) }
let metrics = stream { space.match(%{type: :log}) }

logger |> map(|e| format_log(e)) |> write_file
metrics |> count |> update_dashboard
```

### Namespace Isolation

```fmpl
let system = tuplespace()
let alice = system.as(:alice)
let bob = system.as(:bob)

-- Alice's tuples
alice.out(%{type: :message, text: "hello"})

-- Bob doesn't see Alice's tuples
let msg = bob.rdp(%{type: :message})  -- nil
```

### Backpressure with Blocking

```fmpl
let space = tuplespace()

-- Producer (slows down if space is full)
for item in data_source {
  space.out(%{type: :item, data: item})
}

-- Consumer (blocks if no items)
loop {
  let item = space.in(%{type: :item})
  process(item)
}
```

## Edge Cases

### Empty Tuple Space

Reading from an empty tuple space:
- `in/rd` block until a tuple is written
- `inp/rdp` return `nil` immediately

### No Matching Tuple

When no tuple matches the pattern:
- `in/rd` block until a matching tuple is written
- `inp/rdp` return `nil` immediately

### Multiple Matching Tuples

When multiple tuples match:
- FIFO order by sequence number
- Only one tuple is returned per `in` call

### Concurrent Access

Multiple concurrent operations:
- `out` operations are atomic
- `in/rd` operations receive tuples in FIFO order
- No tuple is delivered to multiple consumers (destructive read)

## Implementation Notes

- Uses tokio channels for blocking operation notifications
- Fjall-backed persistence for durable tuples
- BTreeMap for indexed tuple storage
- AtomicU64 for sequence number generation

## Related Specifications

- [Async Streams](async-streams.md) - Stream integration
- [Persistence](persistence.md) - Fjall storage backend
- [Pattern Matching](pattern-matching.md) - Pattern syntax

## References

- [Linda coordination language](https://en.wikipedia.org/wiki/Linda_(coordination_language))
- [Tuple space VAT actor conversion research](../docs/research/2025-12-27-tuplespace-vat-actor-conversion.md)
