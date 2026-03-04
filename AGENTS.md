# AGENTS.md

## Project Overview

FMPL is a streaming-first DSL for building AI agents with grammars, capabilities, and durable state. It features:

- **Prototype-based objects** with Goblins-inspired patterns (spawn, facets, bcom)
- **OMeta-style PEG grammars** with inheritance and memoization for parsing any stream (text, bytes, objects)
- **Indexed RPN bytecode VM** with async support (`<-` operator, streams)
- **Fjall-backed persistence** for live image and streaming overflow
- **Pattern matching via `@` operator** for parsing and data transformation

**Historical note**: FMPL ("of Accardi") originated as a prototype-based OOP language developed at the Experimental Computing Facility (XCF) of UC Berkeley in 1992. This repository builds on that foundation with modern streaming and agent capabilities.

## Architecture

**Rust workspace** with 6 crates:

- `fmpl-core/` — Lexer, parser, compiler, bytecode VM, object system, grammar engine
  - `builtins/` — 16 builtin modules (ast, ir, io, curl, grammar_to_ir, ir_to_rust, human, etc.)
  - `grammar/` — OMeta-style PEG engine (parser, runtime, optimizer, trampoline, incremental)
  - `instructions/` — Bytecode instruction definitions (arithmetic, control_flow, functions, objects)
  - `pattern/` — Pattern matching implementation
  - `vm_internal/` — VM internals (frame, parse_state)
- `fmpl-cli/` — REPL with rustyline history
- `fmpl-web/` — Axum server with HTMX frontend, per-user sessions, approval queue, storylet system
- `fmpl-tui/` — Ratatui TUI with DAG-based conversation management (Ctrl+L for chat mode)
- `fmpl-bootstrap/` — Minimal interpreter for build-time parser generation (avoids circular deps)
- `benches/` — Performance benchmarks (pattern matching, VM comparison)

**FMPL standard library** (`lib/`):

- `lib/core/` — Compiler pipeline modules written in FMPL
  - `prelude.fmpl` — Standard library prelude
  - `ast_to_ir.fmpl` — AST→IR tree grammar transformer
  - `fmpl_parser.fmpl` — Metacircular FMPL parser
  - `parser_generator.fmpl`, `grammar_optimizer.fmpl`, `ast_optimizer.fmpl`
  - `ir_to_rust.fmpl`, `ir_to_execution_tape.fmpl` — Backend code generators
- `lib/anthropic.fmpl` — Claude API client (requires `ANTHROPIC_API_KEY`)
- `lib/ollama.fmpl` — Ollama local LLM client
- `lib/llm-common.fmpl` — Shared LLM utilities
- `lib/rlm.fmpl` — Reinforcement learning module
- `lib/json.fmpl`, `lib/yaml.fmpl` — Format parsers in FMPL

**Core flow**: Source → Lexer (logos) → Parser (recursive descent) → AST → Compiler → Indexed RPN bytecode → VM execution

**Bootstrap pipeline**: `ast::parse(source)` → `ast @ ast_to_ir.expr` → `ir::compile(ir)` → `code::eval(code)` — the FMPL-in-FMPL compilation pipeline being built toward self-hosting

## Development Conventions

### Quality Gates

- **TDD**: Write tests first, then implementation. In green mode, don't fix failing tests by changing the test.
- **DRY, KISS, YAGNI**: Don't over-engineer. Only implement what's needed now.
- **Green build is a precondition, not a postcondition.** If tests are failing when you start, fixing them is your first task. There is no such thing as a "pre-existing" failure — if it's failing, it's your problem.
- **cargo test must pass before commit**. Run full suite once before commit; targeted tests during development.
- **cargo clippy must pass before commit with zero warnings**. Run clippy workspace-wide (`cargo clippy`), never on individual test files (`--test`). Apply all suggestions. Zero warnings required — including build-script warnings, dead code, unused fields. If you need `#[allow(...)]`, add it at the file top with a comment explaining why.
- **Zero warnings**: There MUST be no warnings while building.
- **3-strike rule**: If you hit the same error 3 times, write a spec with what you tried and what failed, comment the spec path on the issue (`jj issue comment <id> "Blocked: see specs/<path>"`), then stop.

### Documentation Conventions

- Discoveries during implementation that need fixing → document in `specs/` directory.
- Design decisions → `docs/` directory.
- Build/implementation instructions → `AGENTS.md` (this file). Don't pollute with design decisions.
- Specs should be clear, concise, < 200 lines. Break large specs into a directory with subspecs.

## Operating Instructions (Automated Loops)

### Issue Descriptions Are Pre-Digested Research

`jj issue show` descriptions contain key files, code snippets, and context needed for implementation. Treat the description as your research phase — go directly to implementation after reading it. Only read additional files if you need surrounding context for an edit that the issue doesn't cover.

### Cargo Output Filtering

Always filter cargo output. Unfiltered cargo output wastes context.

```bash
# Tests
cargo test -p fmpl-core --test <name> <filter> 2>&1 | grep -E '^(test |test result:|error\[|thread.*panicked|assertion)'

# Build / check / clippy
cargo build -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:.*fmpl|Compiling fmpl)' | head -30
cargo clippy 2>&1 | grep -v objfs | grep -E '^(error|warning:)' | grep -v 'generated.*warnings' | head -30
```

### Avoid Re-reading What You Already Have

- Don't `jj issue show` parent issues just to pick a subtask — `jj issue ready` gives you what you need
- Don't `jj diff` to verify edits — run the failing test instead
- After editing, run only the specific failing test, not the full suite. Full suite once before commit.
- Don't re-read files you just wrote — you know what's in them
- Read files once, generously. Don't re-read narrow windows of the same file.

### How To: Understand an External Crate API

Use context7 (`resolve-library-id` then `query-docs`) or fetch docs.rs. Do not grep through `~/.cargo/registry/src/`.

### How To: Fix Axum Handler Trait Errors

The issue is extractor ordering or missing middleware. `Session` requires `SessionManagerLayer` on the router. Use `State<T>` instead of `Extension<T>` when extractors need to work with middleware layers. Check tower-sessions docs for correct extractor signatures.

### How To: Debug Cargo Build Failures

```bash
cargo build -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:.*fmpl)' | head -30
```

If errors reference external crate types, check API docs first (see above).

## Build & Test

```bash
cargo build                      # Build all crates
cargo test                       # Run all tests
cargo test -p fmpl-core <name>   # Run specific test (e.g., tool_calling, apply_operator)
cargo run -p fmpl-cli            # Launch REPL
cargo run -p fmpl-web            # Launch web server (port 3000)
cargo run -p fmpl-tui            # Launch TUI (Ctrl+L for LLM chat)
```

### Test Organization

- **Unit tests**: Inline in source files (`#[cfg(test)] mod tests`)
- **Integration tests**: `fmpl-core/tests/` — 60+ test files covering parser, compiler, VM, grammar, streaming, async, objects, patterns, tool calling
- **Parity tests**: `fmpl-core/tests/ast_to_ir_parity.rs` — Verifies FMPL bootstrap pipeline produces identical results to Rust compiler
- **Test helpers**: Use `eval(&mut vm, source)` for VM tests, `parse(source)` for parser tests
- **Mock HTTP**: Use `wiremock` for async HTTP tests (see `fmpl-core/tests/async_curl.rs`)
- **Always run tests after changes**: `cargo test -p fmpl-core`

### Feature Flags

- `fjall-persistence` — Enable Fjall-backed durable storage (optional)
- `trampolined-grammar` — Bounded stack usage for grammar evaluation
- `cross_compile` — Cross-compilation to execution_tape (disabled by default)

## Critical Patterns

### 1. Indexed RPN Execution (NOT stack-based)

The VM uses **Indexed RPN**: each instruction stores its result in `values[ip]`, operands reference results by instruction index. See `fmpl-core/src/vm.rs`.

```rust
// WRONG: Traditional stack-based thinking
// RIGHT: Operands are InstrIndex, results stored at IP
Frame { values: Vec<Value>, ip: usize }

// Instructions reference operands by index:
Add { lhs: InstrIndex(5), rhs: InstrIndex(7) }  // Add results from instructions 5 and 7
```

When adding instructions, operands MUST be `InstrIndex` references to previous results, not immediate values.

### 2. Grammars in FMPL, Not Rust

Parsers should be written in FMPL using the grammar system, not hardcoded in Rust. Use Rust builtins only for low-level I/O, external system interfaces, and performance-critical primitives. See `docs/design/language-guide.md` for language features and `specs/grammar-system.md` for grammar implementation details.

### 3. String and Memory Management

- **Use `SmolStr`** for identifiers and small strings (< 23 bytes, stack-allocated)
- **Use `Arc<T>`** for shared data (lists, maps, compiled code)
- **Use `rkyv`** for zero-copy serialization (bytecode, persistence)
- **Use `serde_json`** for JSON I/O with external systems

### 4. Error Handling Patterns

Use `thiserror` for error types, `Result<T>` returns:

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("Parse failed at {position}: {message}")]
    ParseFailed { position: usize, message: String },
}
```

All integration tests use `run(code).expect("runtime error")` or `map_err(|e| e.to_string())`.

## Key Files Reference

- `fmpl.ebnf` — Language grammar (reference only, not used by parser)
- `fmpl-core/src/ast.rs` — AST node definitions (`QualifiedName`, `Expr`, `Pattern`)
- `fmpl-core/src/compiler.rs` — AST → Indexed RPN bytecode compilation
- `fmpl-core/src/vm.rs` — Indexed RPN VM execution with async support
- `fmpl-core/src/value.rs` — Runtime value enum (`Int`, `String`, `Map`, `AsyncStream`, etc.)
- `fmpl-core/src/grammar/mod.rs` — OMeta-style PEG grammar system entry point
- `fmpl-core/src/grammar/runtime.rs` — Grammar pattern matching engine (TagMatch, ListMatch, Repeat)
- `fmpl-core/src/grammar/optimizer.rs` — Grammar optimization (first-set computation)
- `fmpl-core/src/builtins/ir.rs` — `ir::compile` builtin (IR tagged values → bytecode)
- `fmpl-core/src/builtins/ast.rs` — `ast::parse` builtin (source → AST tagged values)
- `fmpl-core/src/ir_builder.rs` — IR construction utilities
- `fmpl-core/src/error.rs` — Unified error types with `thiserror`
- `fmpl-core/src/object.rs` — Prototype-based object system (spawn, facets)
- `lib/core/prelude.fmpl` — Standard library prelude
- `lib/core/ast_to_ir.fmpl` — FMPL-in-FMPL AST→IR tree grammar
- `lib/core/fmpl_parser.fmpl` — Metacircular FMPL parser
- `lib/anthropic.fmpl` — Claude API client (requires `ANTHROPIC_API_KEY`)
- `lib/ollama.fmpl` — Ollama local LLM client

## Design Documentation

- `docs/STANDARDS.md` — **Documentation standards** for design docs, implementation plans, and specs
- `specs/README.md` — Spec index and crate overview
- `docs/design/language-guide.md` — DSL concepts and examples
- `docs/plans/2026-01-19-unified-grammars-and-agents-design.md` — `@` operator unification
- `specs/grammar-system.md` — PEG grammar implementation details
- `specs/indexed-rpn-conversion.md` — Indexed RPN design rationale
- `specs/persistence.md` — Fjall-backed storage and continuations

## Codebase Discovery Docs

`docs/codebase/` contains consolidated implementation patterns discovered during development.
**Read these before exploring the codebase** — they save significant research time.

- `docs/codebase/fjall-persistence-patterns.md` — Save/load patterns, serde serialization, keyspace layout, test setup

## Current Limitations (Mar 2026)

- **Bootstrap pipeline**: `ast_to_ir.fmpl` handles core expressions but several AST node types still produce incorrect IR (lists, lambdas, maps, sequences, match, for, while, try/catch, pipe, slice, block). 21 parity tests track progress.
- **Recursive let bindings**: Lambda self-reference requires special handling (e.g., `let rec` in ML or Y combinator pattern)
- Object system persistence not fully integrated with Fjall backend

## Language & REPL Reference

- `docs/design/language-guide.md` — Language features, syntax, examples
- `fmpl.ebnf` — Formal grammar (reference only, not used by parser)
- `specs/fmpl-cli.md` — REPL commands, features, keybindings
