# fmpl-web

Web-based REPL and storylet server.

**Crate**: `fmpl-web`
**Binary**: `fmpl-web`
**Location**: [fmpl-web/](../fmpl-web/)

---

## Overview

A web server providing:

- **REPL Interface** — Browser-based code evaluation with HTMX
- **Storylet System** — Fallen London-inspired narrative engine
- **Image Store** — Fjall-backed object persistence
- **Continuation Sessions** — Seaside-style stateful web interactions

Built with [Axum](https://github.com/tokio-rs/axum) and [HTMX](https://htmx.org/).

---

## Usage

```bash
# Start the server
cargo run --bin fmpl-web

# Access at
open http://localhost:3000
```

---

## Routes

| Route | Method | Description | Location |
|-------|--------|-------------|----------|
| `/` | GET | REPL interface | `main.rs:24` |
| `/eval` | POST | Evaluate FMPL code | `main.rs:25` |
| `/reset` | POST | Reset VM state | `main.rs:26` |
| `/static/*` | GET | Static assets | `main.rs:27` |
| `/play` | GET | Start new storylet session | `storylet.rs:41` |
| `/play/{token}` | GET | Resume storylet session | `storylet.rs:42` |
| `/play/{token}/choice` | POST | Submit player choice | `storylet.rs:43` |

---

## Module Structure

```
fmpl-web/src/
├── main.rs           # Server setup, REPL routes
├── lib.rs            # Public exports (continuations, image_store, storylet)
├── storylet.rs       # Storylet app builder and handlers
├── continuations.rs  # Continuation persistence via Fjall
├── image_store.rs    # Object image persistence

fmpl-web/
├── seed/             # Initial storylet data (seed.fmpl)
├── static/           # Static assets (CSS, JS)
└── tests/            # Integration tests
```

Note: `data/` directory is created at runtime for Fjall persistence.

---

## Architecture

### REPL Flow

```
Browser                    Server
   │                          │
   │  POST /eval {code: "1+2"}│
   │─────────────────────────►│
   │                          │  Vm::eval(code)
   │  <div class="entry">...  │
   │◄─────────────────────────│
   │                          │
   │  HTMX appends to #output │
   │                          │
```

### Shared State

```rust
type SharedVm = Arc<Mutex<Vm>>;  // main.rs:15
```

The VM is wrapped in `Arc<Mutex<>>` for thread-safe access across requests. Each request acquires the lock, evaluates, and releases.

Storylet routes use `AppState` (`storylet.rs:15-20`) which holds:
- `vm: Arc<Mutex<Vm>>` — Shared VM instance
- `continuations: Arc<ContinuationStore>` — Session persistence
- `image: Arc<ImageStore>` — Object storage

---

## HTMX Integration

The frontend uses HTMX for dynamic updates without full page reloads:

```html
<form hx-post="/eval"
      hx-target="#output"
      hx-swap="beforeend">
  <input name="code" type="text">
  <button type="submit">Eval</button>
</form>
```

- `hx-post="/eval"` — Submit to eval endpoint
- `hx-target="#output"` — Append result to output div
- `hx-swap="beforeend"` — Insert at end (not replace)

---

## Storylet System

Fallen London-inspired narrative engine for text-based games:

```fmpl
object ^market_encounter (storylet) {
  location: @market_square
  prose: "The merchant beckons you closer..."
  choices: [
    %{label: "Listen", target: @secrets, cost: %{Focus: 1}},
    %{label: "Leave", target: @market_square}
  ]
}
```

### Features

- **Storylets** — Narrative chunks with prose and choices
- **Energy Pools** — Focus, Stamina, Social with adaptive regeneration
- **Qualities** — Universal stat/inventory/progress tracking
- **Scene Rendering** — Background images, character portraits

---

## Dependencies

See `fmpl-web/Cargo.toml:13-23`.

| Dependency | Purpose |
|------------|---------|
| `fmpl-core` | Language runtime |
| `axum` | HTTP framework |
| `tokio` | Async runtime |
| `tower-http` | Static file serving |
| `tower` | Service abstraction |
| `serde`, `serde_json` | JSON serialization |
| `fjall` | LSM persistence |
| `blake3`, `base64` | Token generation |

---

## Configuration

Currently hardcoded (no environment variables):

| Setting | Value | Location |
|---------|-------|----------|
| Server bind | `0.0.0.0:3000` | `main.rs:32` |
| Data directory | `"data"` | `storylet.rs:21` |
| Seed path | `fmpl-web/seed/seed.fmpl` | `storylet.rs:24-27` |
| Payload limit | 4096 bytes | `continuations.rs:8` |

---

## Key Types

### ContinuationStore (`continuations.rs:50-52`)

Persists session state to Fjall with linked-list overflow for large payloads.

```rust
pub struct ContinuationStore {
    partition: PartitionHandle,
}
```

Key methods:
- `save()` — Generate token, store envelope (`continuations.rs:64-70`)
- `load()` — Retrieve by token (`continuations.rs:72-80`)
- `update_last_action()` — Append choice, handle overflow (`continuations.rs:82-151`)

### SnapshotEnvelope (`continuations.rs:12-20`)

Versioned container for continuation payloads.

```rust
pub struct SnapshotEnvelope {
    pub schema_version: u16,
    pub bytecode_version: u16,
    pub engine_version: u32,
    pub created_at: u64,
    pub payload_format: String,
    pub payload: Vec<u8>,
}
```

### ImageStore (`image_store.rs:7-9`)

Object persistence with bootstrap from seed file.

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [fmpl-cli.md](./fmpl-cli.md) — CLI alternative
- [persistence.md](./persistence.md) — Fjall storage
