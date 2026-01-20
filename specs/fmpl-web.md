# fmpl-web

Web-based REPL and storylet server for [Project Name TBD].

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

| Route | Method | Description |
|-------|--------|-------------|
| `/` | GET | REPL interface |
| `/eval` | POST | Evaluate FMPL code |
| `/reset` | POST | Reset VM state |
| `/static/*` | GET | Static assets |
| `/play` | GET | Storylet player |
| `/storylet/*` | — | Storylet API routes |

---

## Module Structure

```
fmpl-web/src/
├── main.rs           # Server setup, REPL routes
├── lib.rs            # Public exports
├── storylet.rs       # Storylet engine (narrative system)
├── continuations.rs  # Seaside-style continuation sessions
├── image_store.rs    # Fjall persistence adapter

fmpl-web/
├── seed/             # Initial storylet data
├── static/           # Static assets (CSS, JS)
├── tests/            # Integration tests
└── data/             # Runtime data (Fjall DB)
```

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
type SharedVm = Arc<Mutex<Vm>>;
```

The VM is wrapped in `Arc<Mutex<>>` for thread-safe access across requests. Each request acquires the lock, evaluates, and releases.

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

| Dependency | Purpose |
|------------|---------|
| `fmpl-core` | Language runtime |
| `axum` | HTTP framework |
| `tokio` | Async runtime |
| `tower-http` | Static file serving |
| `serde`, `serde_json` | JSON serialization |
| `fjall` | LSM persistence |
| `blake3`, `base64` | Content hashing |

---

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| — | `0.0.0.0:3000` | Server bind address |
| — | `data` | Storylet data directory |

---

## UI Features

### REPL Interface

- Syntax highlighting (planned)
- Command history (in-browser)
- Output stream with color-coded results
- Command log panel
- Tips and quick reference

### Storylet Player

- Scene backgrounds and character portraits
- Choice buttons with energy costs
- Quality display
- Continuation-based session state

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [fmpl-cli.md](./fmpl-cli.md) — CLI alternative
- [persistence.md](./persistence.md) — Fjall storage
