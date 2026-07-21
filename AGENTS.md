# AGENTS.md

Workflow rules and gotchas for working in this repo. Codebase inventory,
key-file locations, and doc pointers live in [`DEV.md`](DEV.md).

## Design Principles (read first)

Durable invariants live in [`docs/design-principles.md`](docs/design-principles.md).
These override iteration scope when in conflict. If you are about to make a
change that would violate a principle, stop — the principle wins, the change
needs to be reframed. Currently captured: metacircular bootstrap (DESIGN-001),
single canonical list-form for structured data (DESIGN-002), symbols for type
names (DESIGN-003), tree-based IR with named temporaries (DESIGN-004), grammar
inheritance deferred (DESIGN-005).

## Orientation

FMPL is a streaming-first DSL for AI agents: prototype objects, OMeta-style
PEG grammars, pattern matching via `@`, and an Indexed RPN bytecode VM.

- **Core flow**: Source → Lexer (logos) → Parser → AST → Compiler → Indexed RPN bytecode → VM
- **Bootstrap pipeline**: `ast::parse(source)` → `ast @ ast_to_ir.expr` → `ir::compile(ir)` → `code::eval(code)` — the FMPL-in-FMPL path being built toward self-hosting
- Current limitations are tracked in [`docs/known-gaps.md`](docs/known-gaps.md), grouped by root cause.

## Build & Test

```bash
just build                       # REQUIRED build: bootstraps the FMPL-generated parser, then builds
just test                        # full suite with the canonical parser active
cargo test -p fmpl-core <name>   # targeted test during development
just repl                        # REPL (dot-prefixed commands: .help, .quit)
just web                         # web server (port 3000)
just tui                         # TUI (Ctrl+L for LLM chat)
```

- **Plain `cargo build` silently uses the Rust *fallback* parser**, not the
  canonical FMPL-generated one. Use `just build` (or run the two bootstrap
  steps in README). `canonical_pipeline_parity` fails loudly if the fallback
  is active.
- Changing parser codegen (`ir_to_rust.rs`) or anything the generated parser
  embeds? Read the bump policy in `fmpl-core/src/parser_epoch.rs` and bump
  `PARSER_EPOCH`.
- Feature flags: `fjall-persistence` (durable storage), `trampolined-grammar`
  (bounded stack), `cross_compile` (dormant — needs the external
  `execution_tape` crate, see Cargo.toml comments).
- Test helpers: `eval(&mut vm, source)` for VM tests, `parse(source)` for
  parser tests; `wiremock` for async HTTP tests (see `tests/async_curl.rs`).

## Quality Gates

- **TDD**: write tests first. Don't fix failing tests by changing the test.
- **Green build is a precondition, not a postcondition.** If tests fail when
  you start, fixing them is your first task — there is no "pre-existing" failure.
- **`just test` must pass before commit**; full suite once before commit,
  targeted tests during development.
- **clippy: zero warnings, workspace-wide** (`cargo clippy --workspace --all-targets`),
  never on individual test files (`--test`). Includes build-script warnings,
  dead code, unused fields. If you need `#[allow(...)]`, put it at file top
  with a comment explaining why.
- **CI runs the latest stable toolchain.** Keep local rust current
  (`rustup update stable`) — a locally-clean clippy on an older toolchain is
  not proof of a green CI.
- DRY, KISS, YAGNI: only implement what's needed now.

## Critical Patterns & Gotchas

- **Indexed RPN, not a stack machine**: each instruction stores its result in
  `values[ip]`; operands are `InstrIndex` references to earlier results, never
  immediate values (`Add { lhs: InstrIndex(5), rhs: InstrIndex(7) }`). See
  `fmpl-core/src/vm.rs`.
- **Grammars in FMPL, not Rust** (DESIGN-001): parsers are written in FMPL via
  the grammar system. Rust builtins are only for low-level I/O, external
  interfaces, and performance-critical primitives.
- **Parser changes go to BOTH parsers**: `fmpl-core/src/parser.rs` (fallback)
  and `lib/core/fmpl_parser.fmpl` (canonical) describe the same language.
- **Strings/memory**: `SmolStr` for identifiers and small strings, `Arc<T>`
  for shared data, `rkyv` for zero-copy serialization, `serde_json` for
  external JSON I/O. Errors are `thiserror` enums returning `Result<T>`.
- **`docs/behavior-scenarios.md` is a build input** — fmpl-core's build.rs
  generates the scenario test suite from it; don't move it without updating
  the path refs.
- When a dependency API doesn't work after 2 attempts, read the actual source
  under the cargo registry instead of guessing.

## Documentation Conventions

- **```` ```fmpl ```` blocks in TUTORIAL.md / DEMO.md / README.md are executed
  in CI** by `fmpl-core/tests/doc_examples.rs`; `-- Returns:` / `-- =>` /
  `// =>` comments are asserted against real results. Mark blocks that can't
  run (network) with `<!-- fmpl-doctest: skip -->` before the fence; use the
  ```` ```fmpl-sketch ```` tag for non-executable design sketches. Full
  conventions in the harness header.
- Discoveries during implementation that need fixing → `specs/`
- Design decisions → `docs/`
- Build/workflow rules and gotchas → `AGENTS.md` (this file); codebase
  inventory and pointers → `DEV.md`. Don't pollute either with design decisions.
- Specs should be clear, concise, < 200 lines; break large specs into a
  directory with subspecs.

## Version Control (jj)

- Describe the current change with `jj describe -m "message"`, not
  `jj new -m "message"` (which creates an empty change).
- After `jj commit`/`jj describe` the working copy shows as modified — normal,
  not an error.
- Check `jj diff` before committing to avoid mixing unrelated changes; use
  `jj split` rather than manual workarounds.
