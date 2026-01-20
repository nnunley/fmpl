# Persistence

Fjall-backed durable storage for live image and streaming.

**Location**: [fmpl-core/](../fmpl-core/), [fmpl-web/](../fmpl-web/)

---

## Overview

Transparent persistence using [Fjall](https://github.com/fjall-rs/fjall) LSM store:

- **Live image** — Object graph persists across restarts
- **Stream overflow** — Large buffers spill to disk
- **Memo persistence** — Memoization survives suspension
- **ParseState serialization** — Durable parse states

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Application                        │
│  ┌─────────────────────────────────────────────────┐   │
│  │ ObjectDb        │ GrammarRegistry │ Streams      │   │
│  └─────────────────────────────────────────────────┘   │
│                          │                              │
│                          ▼                              │
│  ┌─────────────────────────────────────────────────┐   │
│  │                 Fjall Store                      │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐           │   │
│  │  │ objects │ │ streams │ │  memos  │           │   │
│  │  │partition│ │partition│ │partition│           │   │
│  │  └─────────┘ └─────────┘ └─────────┘           │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
                   ┌──────────────┐
                   │   Disk       │
                   │  (LSM SSTs)  │
                   └──────────────┘
```

---

## Feature Flag

Fjall persistence is optional:

```toml
[dependencies]
fmpl-core = { path = "../fmpl-core", features = ["fjall-persistence"] }
```

Without the flag, all storage is in-memory only.

---

## Live Image

Objects persist transparently:

```fmpl
@merchant.mood = "happy"  -- automatically persisted
```

### Storage Format

```
Partition: objects
Key:   object_id (u64)
Value: Object (serialized via rkyv)
```

### Commit Model

- Changes tracked in current transaction
- Commit at end of task/tick
- Fjall handles durability
- Crash recovery from journal

---

## Stream Position Overflow

For long-running streams (e.g., LLM output), positions spill to disk:

```rust
pub struct StreamPosition {
    buffer: Vec<Value>,        // Hot data (in-memory)
    start_offset: usize,       // Buffer start position
    position: usize,           // Current position

    #[cfg(feature = "fjall-persistence")]
    overflow: Option<FjallOverflow>,  // Cold data (on disk)
}
```

### Overflow Behavior

```rust
const BUFFER_THRESHOLD: usize = 1000;

impl StreamPosition {
    fn push(&mut self, value: Value) {
        self.buffer.push(value);
        if self.buffer.len() > BUFFER_THRESHOLD {
            self.spill_oldest_to_fjall();
        }
    }

    fn get(&self, pos: usize) -> Option<Value> {
        if pos < self.start_offset {
            // Read from Fjall
            self.overflow.as_ref()?.get(pos)
        } else {
            // Read from buffer
            self.buffer.get(pos - self.start_offset).cloned()
        }
    }
}
```

### Storage Format

```
Partition: stream_positions
Key:   (stream_id, position) as bytes
Value: Value (serialized)
```

---

## Memo Table Persistence

Packrat memoization results survive suspension:

```rust
pub struct MemoTable {
    hot: HashMap<(usize, SmolStr), ParseResult>,  // In-memory

    #[cfg(feature = "fjall-persistence")]
    cold: FjallPartition,  // Persisted
}
```

### Lookup Order

1. Check in-memory `hot` cache
2. Fall back to Fjall `cold` storage
3. If miss, compute and store

### Storage Format

```
Partition: memo_tables
Key:   (grammar_name, position, rule_name) as bytes
Value: ParseResult (serialized)
```

---

## ParseState Serialization

Durable suspension of parse states (in progress):

```rust
#[derive(Serialize, Deserialize)]
pub struct ParseState {
    pub position_index: usize,
    pub rule_stack: Vec<(SmolStr, usize)>,
    pub bindings: HashMap<SmolStr, Value>,
}
```

### Storage Format

```
Partition: parse_states
Key:   state_id (u64)
Value: ParseState (serialized via rkyv)
```

### Use Case

```fmpl
-- Agent pauses for human approval
let decision = <- human.approve(request)

-- ParseState serialized to Fjall
-- System can restart
-- Later: resume from saved state
```

---

## Fjall Configuration

Default configuration:

```rust
use fjall::{Config, PartitionCreateOptions};

let keyspace = Config::new(path).open()?;

// Partitions
let objects = keyspace.open_partition("objects", PartitionCreateOptions::default())?;
let streams = keyspace.open_partition("streams", PartitionCreateOptions::default())?;
let memos = keyspace.open_partition("memos", PartitionCreateOptions::default())?;
```

### Data Directory

- CLI: Not persisted (in-memory only)
- Web: `data/` directory (configurable)

---

## Web Integration

The web server uses Fjall for session state:

```rust
// fmpl-web/src/image_store.rs

pub struct ImageStore {
    keyspace: fjall::Keyspace,
    objects: fjall::Partition,
}

impl ImageStore {
    pub fn open(path: &str) -> Result<Self> { ... }
    pub fn save_object(&self, id: ObjectId, obj: &Object) -> Result<()> { ... }
    pub fn load_object(&self, id: ObjectId) -> Result<Option<Object>> { ... }
}
```

---

## Serialization

Two serialization formats:

| Format | Use Case |
|--------|----------|
| `rkyv` | High-performance zero-copy for hot path |
| `serde_json` | Human-readable for debugging |

```rust
// rkyv for performance
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Object { ... }

// serde for compatibility
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Object { ... }
```

---

## Related Specs

- [streaming-grammar.md](./streaming-grammar.md) — Uses overflow storage
- [object-system.md](./object-system.md) — Object persistence
- [fmpl-web.md](./fmpl-web.md) — Web server integration

---

## References

- [Fjall](https://github.com/fjall-rs/fjall) — Rust LSM key-value store
- [rkyv](https://rkyv.org/) — Zero-copy deserialization
