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
cargo clippy -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:)' | head -30
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
obj @ { %{type: t} => t }       -- Pattern match (fully functional)
stream @ parser.incremental     -- Streaming parse
```

**Note**: Map/list patterns (`%{k: v}`, `[a, b]`) work in both `let` destructuring and `@` blocks. See `specs/pattern-matching.md`.

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

- `docs/STANDARDS.md` — **Documentation standards** for design docs, implementation plans, and specs
- `specs/README.md` — Spec index and crate overview
- `docs/design/language-guide.md` — DSL concepts and examples
- `docs/plans/2026-01-19-unified-grammars-and-agents-design.md` — `@` operator unification
- `specs/grammar-system.md` — PEG grammar implementation details
- `specs/indexed-rpn-conversion.md` — Indexed RPN design rationale
- `specs/persistence.md` — Fjall-backed storage and continuations

## Current Limitations (Jan 2026)

- **Assignment syntax**: `=` for variable mutation is implemented (supports simple variable and object property assignment)
- **Recursive let bindings**: Lambda self-reference requires special handling (e.g., `let rec` in ML or Y combinator pattern)
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

## Using the REPL

The FMPL CLI (`cargo run -p fmpl-cli`) provides an interactive REPL with rustyline history.

### REPL Commands

- `:help`, `:h`, `:?` — Show help
- `:quit`, `:q`, `:exit` — Exit the REPL
- `:clear` — Clear the screen
- `:reset` — Reset VM state (clears all variables)
- `:objects` — List all named objects

### Loading Files

Use `io::load("path/to/file.fmpl")` to load and execute FMPL code from a file:

```fmpl
fmpl> io::load("lib/anthropic.fmpl")
=> :__builtin_io
fmpl> let (response = anthropic::messages.create(...))
```

### Examples

```fmpl
// Basic arithmetic
fmpl> 1 + 2
=> 3

// Let bindings
fmpl> let x = 42
=> 42
fmpl> x + 1
=> 43

// Pattern matching
// Note: Expressions starting with : must be bound first (REPL limitation)
fmpl> let x = :Binary(:+, :Int(1), :Int(2))
=> :Binary(:+, :Int(1), :Int(2))
fmpl> x @ { :Binary(op, a, b) => [op, a, b] }
=> [:+, :Int(1), :Int(2)]

// Metaprogramming pipeline
fmpl> let ast = ast::parse("1 + 2")
=> :Binary(:+, :Int(1), :Int(2))
fmpl> let ir = ast @ { :Binary(:+, :Int(a), :Int(b)) => :Add(:LoadInt(a), :LoadInt(b)) }
=> :Add(:LoadInt(1), :LoadInt(2))
fmpl> let code = ir::compile(ir)
=> <code>
fmpl> code::eval(code)
=> 3

// Async operations (auto-wait in REPL)
fmpl> <- http::get("https://example.com")
=> %{status: 200, body: "..."}
```

### Current Limitations

- Expressions starting with `:` are interpreted as REPL commands — bind to variable first
- No `-e` flag for one-liners
- No direct file execution (`fmpl script.fmpl`)
- No stdin input for piping
- No multiline input (single expressions only)
- Use `io::load()` for loading files within the REPL session
