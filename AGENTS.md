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

**Rust workspace** with 4 crates:

- `fmpl-core/` — Lexer, parser, compiler, bytecode VM, object system, grammar engine
- `fmpl-cli/` — REPL with rustyline history
- `fmpl-web/` — Axum server with HTMX frontend
- `fmpl-tui/` — Ratatui TUI for agentic LLM interaction (Ctrl+L for chat mode)

**Core flow**: Source → Lexer (logos) → Parser (recursive descent) → AST → Compiler → Indexed RPN bytecode → VM execution

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
- **Integration tests**: `fmpl-core/tests/` — `tool_calling.rs`, `async_curl.rs`, `exceptions.rs`, `streaming_parse.rs`, `apply_operator.rs`
- **Test helpers**: Use `eval(&mut vm, source)` for VM tests, `parse(source)` for parser tests
- **Mock HTTP**: Use `wiremock` for async HTTP tests (see `fmpl-core/tests/async_curl.rs`)
- **Always run tests after changes**: `cargo test -p fmpl-core`

### Feature Flags

- `fjall-persistence` — Enable Fjall-backed durable storage (optional)

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

### 2. Async Operations Return Streams

Async calls (`<- expr`) return `Value::AsyncStream` that must be consumed:

- In REPL: `wait_for_async()` blocks until stream completes
- In code: Use `@` operator to pattern-match stream events
- Example: `@ http_response { %{ok: data} => process(data), %{error: e} => handle(e) }`

### 3. Grammar Application with `@`

The `@` operator unifies parsing, pattern matching, and tree transformation:

```fmpl
"hello world" @ grammar.rule    -- Parse text
obj @ { %{type: t} => t }       -- Pattern match (limited)
stream @ parser.incremental     -- Streaming parse
```

**Current limitation**: Map/list patterns (`%{k: v}`, `[a, b]`) work in `let` destructuring but NOT in `@` blocks. See `specs/pattern-matching.md`.

### 4. Grammars in FMPL, Not Rust

**CRITICAL**: Parsers should be written in FMPL using the grammar system, not hardcoded in Rust. This includes:

- JSON parsing (currently `json::parse` is a Rust builtin, should migrate to FMPL grammar)
- Protocol parsers (HTTP, SSE, etc.)
- File format parsers (PNG, etc.)
- DSL parsers and transformers

See `specs/grammar-system.md` for JSON grammar example and `fmpl-core/tests/fmpl/fmpl_grammar.fmpl` for metacircular FMPL parser.

**When to use Rust builtins**:

- Low-level I/O (curl, file operations, environment variables)
- External system interfaces (LLM APIs, databases)
- Performance-critical primitives (hashing, crypto)

**When to use FMPL grammars**:

- Any parsing task (text, binary, or tree structures)
- Data transformation and validation
- Protocol implementation
- DSL embedding

### 5. String and Memory Management

- **Use `SmolStr`** for identifiers and small strings (< 23 bytes, stack-allocated)
- **Use `Arc<T>`** for shared data (lists, maps, compiled code)
- **Use `rkyv`** for zero-copy serialization (bytecode, persistence)
- **Use `serde_json`** for JSON I/O with external systems

### 6. Error Handling Patterns

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
- `fmpl-core/src/grammar/mod.rs` — OMeta-style PEG grammar system
- `fmpl-core/src/error.rs` — Unified error types with `thiserror`
- `fmpl-core/src/object.rs` — Prototype-based object system (spawn, facets)
- `lib/anthropic.fmpl` — Claude API client (requires `ANTHROPIC_API_KEY`)
- `lib/ollama.fmpl` — Ollama local LLM client

## Design Documentation

- `specs/README.md` — Spec index and crate overview
- `docs/design/language-guide.md` — DSL concepts and examples
- `docs/plans/2026-01-19-unified-grammars-and-agents-design.md` — `@` operator unification
- `specs/grammar-system.md` — PEG grammar implementation details
- `specs/indexed-rpn-conversion.md` — Indexed RPN design rationale
- `specs/persistence.md` — Fjall-backed storage and continuations

## Current Limitations (Jan 2026)

- Map/list patterns in `@` blocks not yet implemented (use `let` destructuring)
- Lambdas have parameter binding issues after Indexed RPN transition
- Some operators partially implemented (`&&`, `||`, `!=`)
- Object system persistence not fully integrated with Fjall backend

## Grammar Structure (Historical Reference)

The original grammar defines a language with:

- **Expressions** (`<exp>`): Core construct supporting arithmetic, logical, comparison, and composition operators
- **Control flow**: if/then/else, while/do, do/while, return
- **Functions**: Named functions, lambdas (`\x expr`), function calls with parameter lists
- **Data structures**: Lists `[]`, hash tables `htable()`, objects with tagged properties
- **Bindings**: let-bindings, object property bindings with public/private modifiers
- **Object system**: Object definitions with inheritance (`<olist>`) and sparse structures

## Grammar Conventions

- Optional elements use `[ ]` brackets
- Alternatives separated by newlines (not `|`)
- `<error>` productions handle malformed input
- Optional separators: commas between list items are often optional
- Optional semicolons between statements (`<optsemi>`)
