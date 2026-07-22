# FMPL

**FMPL** is a prototype-based, image-based programming language and live
multi-user/multi-agent environment, implemented in Rust. The persistent object
image is the source of truth; source files are a bootstrapping convenience.

New here? Read the **[engineering tour](https://mparrett.github.io/fmpl/fmpl-tour.html)** —
architecture, verified capabilities, and the honest gap ledger, in one page —
or try the **[browser REPL](https://mparrett.github.io/fmpl/repl.html)**
(fmpl-core compiled to WebAssembly, no install).

> **Status: experimental.** FMPL is a working prototype under active
> development, not a finished language. The core pipeline (lexer → parser →
> compiler → VM), object system, PEG grammar engine, and REPL/TUI/web front-ends
> all run today; the metacircular self-hosting path and persistence layer are
> in progress. Expect sharp edges.

## Lineage

FMPL ("of Accardi") was created in 1992 by Jon Blow at UC Berkeley's
Experimental Computing Facility — a MUD server language in the LambdaMOO /
ColdMUD tradition. This is a 2025-present revival as a Rust implementation,
combining that MUD heritage with modern streaming, first-class PEG grammars, and
capability security. See [`project.md`](project.md) for the full north star.

## A taste

```fmpl
let nums = [1, 2, 3, 4, 5]
nums.map(\x x * 2)              -- => [2, 4, 6, 8, 10]
nums.fold(0, \acc, x acc + x)   -- => 15

%{name: "Alice", age: 30}.name  -- => "Alice"

-- Guarded pattern match via the `@` operator (`when` and `if` are equivalent):
42 @ { n when n > 0 => n * 2, _ => 0 }   -- => 84

-- Prototype objects with capability-scoped access:
object counter {
  init(n): self.count = n
  get(): self.count
  count: 0
}
let c = spawn counter(7)
c.get()                         -- => 7
```

## Build & run

FMPL is a Cargo workspace (Rust, edition 2024). It is **metacircular**: the
canonical parser is generated at build time from FMPL source
(`lib/core/fmpl_parser.fmpl`) via a two-step bootstrap. The convenience target
runs both steps for you:

```sh
just build      # bootstrap the FMPL-generated parser, then build the workspace
just test       # run the test suite
just repl       # launch the REPL (fmpl-cli)
```

If you don't have [`just`](https://github.com/casey/just), the raw commands are:

```sh
# 1. Build the bootstrap binary (skips parser generation on this pass).
FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap
# 2. Rebuild fmpl-core so it embeds the FMPL-generated parser, then build all.
touch fmpl-core/build.rs
cargo build --workspace

cargo run -p fmpl-cli    # REPL   (commands are dot-prefixed: .help, .quit)
cargo run -p fmpl-web    # web REPL on http://localhost:3000
cargo run -p fmpl-tui    # TUI (Ctrl+L for LLM chat)
```

> **Why the two steps?** A plain `cargo build` works, but silently uses the Rust
> *fallback* parser rather than the canonical FMPL-generated one. Building
> `fmpl-bootstrap` first (step 1) lets `fmpl-core`'s build script generate the
> real parser on the next build (step 2). The `canonical_pipeline_parity` test
> enforces that the generated parser is active.

## Architecture

```
Source → Lexer (logos) → Parser → AST → Compiler → Indexed RPN bytecode → VM
                                                     ObjectDb (image) · TupleSpace · Fjall (persist)
```

| Crate | Purpose |
|-------|---------|
| `fmpl-core` | Lexer, parser, compiler, VM, object system, PEG grammar engine, tuple space, persistence |
| `fmpl-cli` | REPL (rustyline) |
| `fmpl-web` | Axum + HTMX web REPL |
| `fmpl-tui` | Ratatui TUI for agentic LLM interaction |
| `fmpl-scenario-runner` | Data-driven behavior-scenario test runner |
| `fmpl-bootstrap` | Stage-0 Rust-compiler fallback for the bootstrap |
| `fmpl-wasm` | wasm-bindgen bindings for the browser REPL |

## Documentation

- [Engineering tour](https://mparrett.github.io/fmpl/fmpl-tour.html) — one-page overview: architecture, what works, what doesn't
- [Browser REPL](https://mparrett.github.io/fmpl/repl.html) — fmpl-core as WebAssembly, live on GitHub Pages
- Field logs — engineering retrospectives: [revitalizing the agent-written codebase](https://mparrett.github.io/fmpl/fmpl-rehab-log.html) and [closing the metacircular-parser gap](https://mparrett.github.io/fmpl/fmpl-field-log.html)
- [`project.md`](project.md) — north star, principles, design lineage
- [`docs/design-principles.md`](docs/design-principles.md) — durable design invariants
- [`AGENTS.md`](AGENTS.md) — workflow rules and gotchas for agents and humans
- [`DEV.md`](DEV.md) — codebase inventory: workspace layout, key files, documentation map
- [`TUTORIAL.md`](TUTORIAL.md) / [`DEMO.md`](DEMO.md) — language walkthrough and examples
- [`specs/`](specs/) — implementation specs (VM, object system, grammars, tuplespace, persistence, pattern matching)

## License

MIT — see [`LICENSE`](LICENSE).
