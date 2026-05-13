# Persistence

Fjall-backed durable storage for live image and streaming.

**Location**: [fmpl-core/](../fmpl-core/), [fmpl-web/](../fmpl-web/)

**Key files**:
- `fmpl-core/Cargo.toml:29` — `persistence` feature
- `fmpl-core/src/grammar/incremental.rs:14` — ParseState serialization
- `fmpl-core/src/grammar/stream_input.rs:42` — StreamPosition with memo
- `fmpl-core/src/stream.rs:32` — StreamBuffer/StreamSource with rkyv
- `fmpl-web/src/image_store.rs:7` — ImageStore

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
fmpl-core = { path = "../fmpl-core", features = ["persistence"] }
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

## Stream Position and Fjall Backing

StreamPosition uses OMeta-style cons-cell design with per-position memoization.
Fjall backing is in StreamSource, not StreamPosition directly.

### StreamPosition (`stream_input.rs:42-56`)

```rust
pub struct StreamPosition {
    head: Option<Value>,
    tail: RefCell<Option<Rc<StreamPosition>>>,
    index: usize,
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,  // Per-position memo
    source: Rc<StreamSource>,

    #[cfg(feature = "persistence")]
    memo_fjall: Option<Arc<Mutex<MemoFjall>>>,   // Persistent memo
}
```

### StreamSource with Fjall (`stream_input.rs:97-116`)

```rust
enum StreamSource {
    Async {
        handle: Mutex<StreamHandle>,
        timeout: Option<Duration>,
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        #[cfg(feature = "persistence")]
        fjall: Option<FjallOverflow>,        // Overflow storage here
        #[cfg(feature = "persistence")]
        memory_limit: Option<usize>,
    },
    Static(Vec<Value>),
    Empty,
}
```

### Storage Format

```
Partition: stream_positions
Key:   (stream_id, position) as bytes
Value: Value (serialized)
```

---

## Per-Position Memoization

Memoization is per-position (not centralized MemoTable). Each StreamPosition
has its own memo cache with optional Fjall backing.

### Per-Position Memo (`stream_input.rs:49-50`)

```rust
pub struct StreamPosition {
    // ...
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,  // In-memory

    #[cfg(feature = "persistence")]
    memo_fjall: Option<Arc<Mutex<MemoFjall>>>,   // Persisted
}
```

### Lookup Order

1. Check position's in-memory `memo` cache
2. Fall back to position's `memo_fjall` if enabled
3. If miss, compute and store

### Storage Format

```
Partition: memos
Key:   (position_index, rule_name) as bytes
Value: MemoEntry (serialized)
```

---

## ParseState Serialization

Durable suspension of parse states (`incremental.rs:14-22`):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    pub position_index: usize,
    pub rule_stack: Vec<(SmolStr, usize)>,
    pub bindings: HashMap<SmolStr, Value>,
}
```

### Serialization Methods (`incremental.rs:63-97`)

```rust
#[cfg(feature = "persistence")]
impl ParseState {
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)  // Uses serde_json, not rkyv
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    pub fn save_to_fjall(&self, partition: &PartitionHandle, key: &[u8]) -> Result<()>;
    pub fn load_from_fjall(partition: &PartitionHandle, key: &[u8]) -> Result<Option<Self>>;
}
```

### Storage Format

```
Partition: parse_states
Key:   state_id (u64)
Value: ParseState (serialized via serde_json)
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

The web server uses Fjall for session state (`image_store.rs:7-35`):

```rust
pub struct ImageStore {
    partition: PartitionHandle,
}

impl ImageStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let keyspace = Config::new(path).open()?;
        let partition = keyspace.open_partition("image", Default::default())?;
        Ok(Self { partition })
    }

    pub fn bootstrap_if_empty(&self, seed_file: &str, vm: &mut Vm) -> Result<()> {
        // Loads seed file, stores named objects with key "obj:{name}"
    }

    pub fn has_object(&self, name: &str) -> Result<bool> {
        let key = format!("obj:{}", name);
        Ok(self.partition.get(key)?.is_some())
    }
}
```

---

## Serialization

Two serialization formats used for different types:

| Type | Format | Location |
|------|--------|----------|
| `StreamBuffer` | rkyv + serde | `stream.rs:32-42` |
| `StreamSource` | rkyv + serde | `stream.rs:50-53` |
| `SinkSource` | rkyv + serde | `stream.rs:201-217` |
| `ParseState` | serde_json only | `incremental.rs:63-97` |

```rust
// Stream types use both rkyv and serde (stream.rs:32)
#[derive(Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct StreamBuffer { ... }

// ParseState uses serde_json only (incremental.rs:14)
#[derive(Serialize, Deserialize)]
pub struct ParseState { ... }
```

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — Streaming grammar uses overflow storage
- [object-system.md](./object-system.md) — Object persistence
- [fmpl-web.md](./fmpl-web.md) — Web server integration

---

## References

- [Fjall](https://github.com/fjall-rs/fjall) — Rust LSM key-value store
- [rkyv](https://rkyv.org/) — Zero-copy deserialization
