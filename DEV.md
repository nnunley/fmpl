# DEV.md — codebase reference

Orientation material for humans and agents: what lives where, and where to
read more. Workflow rules and gotchas live in [`AGENTS.md`](AGENTS.md); this
file is inventory and pointers, safe to skim.

## Project overview

FMPL is a streaming-first DSL for building AI agents with grammars,
capabilities, and durable state:

- **Prototype-based objects** with Goblins-inspired patterns (spawn, facets, bcom)
- **OMeta-style PEG grammars** with memoization for parsing any stream (text, bytes, objects)
- **Indexed RPN bytecode VM** with async support (`<-` operator, streams)
- **Fjall-backed persistence** for live image and streaming overflow
- **Pattern matching via `@` operator** for parsing and data transformation

**Historical note**: this FMPL is Norman Nunley's descendant of the original
FMPL ("of Accardi", UC Berkeley XCF, c. 1992, interpreter by Jon Blow) — seeded
from an EBNF grammar Nunley extracted from the original sources in the late
1990s, with new syntax and semantics beyond it. See [`README.md`](README.md)
and [`project.md`](project.md).

## Workspace layout

Six workspace crates (`benches/` exists but is excluded from the workspace —
it depends on the external `execution_tape` crate; see the root `Cargo.toml`
comments):

- `fmpl-core/` — Lexer, parser, compiler, bytecode VM, object system, grammar engine
  - `builtins/` — 16 builtin modules (ast, ir, io, curl, grammar_to_ir, ir_to_rust, human, …)
  - `grammar/` — OMeta-style PEG engine (parser, runtime, optimizer, trampoline, incremental)
  - `instructions/` — Bytecode instruction definitions (arithmetic, control_flow, functions, objects)
  - `pattern/` — Pattern matching implementation
  - `vm_internal/` — VM internals (frame, parse_state)
- `fmpl-scenario-runner/` — Data-driven behavior-scenario test runner; the corpus
  lives at `docs/behavior-scenarios.md` and is a **build input** (fmpl-core's
  build.rs generates a test suite from it)
- `fmpl-cli/` — REPL with rustyline history
- `fmpl-web/` — Axum server with HTMX frontend, per-user sessions, approval queue, storylet system
- `fmpl-tui/` — Ratatui TUI with DAG-based conversation management (Ctrl+L for chat mode)
- `fmpl-bootstrap/` — Minimal interpreter for build-time parser generation (avoids circular deps)

**FMPL standard library** (`lib/`):

- `lib/core/` — Compiler pipeline modules written in FMPL
  - `prelude.fmpl` — Standard library prelude
  - `fmpl_parser.fmpl` — Metacircular FMPL parser (source of the generated canonical parser)
  - `ast_to_ir.fmpl` — AST→IR tree grammar transformer
  - `parser_generator.fmpl`, `grammar_optimizer.fmpl`, `ast_optimizer.fmpl`
  - `ir_to_rust.fmpl`, `ir_to_execution_tape.fmpl` — Backend code generators
- `lib/anthropic.fmpl` — Claude API client (requires `ANTHROPIC_API_KEY`)
- `lib/ollama.fmpl` — Ollama local LLM client
- `lib/llm-common.fmpl` — Shared LLM utilities
- `lib/json.fmpl`, `lib/yaml.fmpl` — Format parsers in FMPL
- `lib/rlm.fmpl` — Reinforcement learning module

## Key files

- `fmpl-core/src/ast.rs` — AST node definitions (`QualifiedName`, `Expr`, `Pattern`)
- `fmpl-core/src/compiler.rs` — AST → Indexed RPN bytecode compilation
- `fmpl-core/src/vm.rs` — Indexed RPN VM execution with async support
- `fmpl-core/src/value.rs` — Runtime value enum (`Int`, `String`, `Map`, `AsyncStream`, …)
- `fmpl-core/src/grammar/mod.rs` — PEG grammar system entry point
- `fmpl-core/src/grammar/runtime.rs` — Grammar matching engine (interpreted path)
- `fmpl-core/src/grammar/optimizer.rs` — Grammar optimization (first-set computation)
- `fmpl-core/src/builtins/ast.rs` — `ast::parse` builtin (source → AST values)
- `fmpl-core/src/builtins/ir.rs` — `ir::compile` builtin (IR values → bytecode)
- `fmpl-core/src/builtins/ir_to_rust.rs` — grammar-mode codegen (emits the generated parser)
- `fmpl-core/src/parser_epoch.rs` — generator epoch; read its bump policy before changing codegen
- `fmpl-core/src/error.rs` — Unified error types with `thiserror`
- `fmpl-core/src/object.rs` — Prototype-based object system (spawn, facets)
- `fmpl.ebnf` — Language grammar (reference only, not used by the parser; descends from Nunley's late-1990s extraction of the original FMPL grammar)

## Test organization

- **Unit tests**: inline in source files (`#[cfg(test)] mod tests`)
- **Integration tests**: `fmpl-core/tests/` — parser, compiler, VM, grammar,
  streaming, async, objects, patterns, tool calling
- **Parity tests**: `fmpl-core/tests/ast_to_ir_parity.rs` (bootstrap pipeline vs
  Rust compiler) and `fmpl-core/tests/canonical_pipeline_parity.rs` (generated
  parser vs source-tree parser; fails loudly if the fallback parser is active)
- Integration tests use `run(code).expect(...)` / `map_err(|e| e.to_string())`;
  error types are `thiserror` enums in `fmpl-core/src/error.rs`

## Documentation map

- `docs/known-gaps.md` — current limitations, grouped by root cause (the ~185
  `#[ignore]`d tests each carry a reason pointing here)
- `docs/design-principles.md` — durable invariants (DESIGN-001…005)
- `docs/STANDARDS.md` — documentation standards for design docs, plans, specs
- `specs/README.md` — spec index and crate overview
- `specs/grammar-system.md` — PEG grammar implementation details
- `specs/indexed-rpn-conversion.md` — Indexed RPN design rationale
- `specs/persistence.md` — Fjall-backed storage and continuations
- `specs/fmpl-cli.md` — REPL commands, features, keybindings
- `docs/design/language-guide.md` — DSL concepts and examples (partly aspirational)
- `docs/plans/2026-01-19-unified-grammars-and-agents-design.md` — `@` operator unification
- `docs/codebase/` — implementation patterns discovered during development
  (e.g. `fjall-persistence-patterns.md`); check here before deep-diving
- `TUTORIAL.md` / `DEMO.md` — language walkthrough and examples, REPL-verified
