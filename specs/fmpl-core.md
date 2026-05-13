# fmpl-core

Core runtime library for FMPL.

**Crate**: `fmpl-core`
**Location**: [fmpl-core/](../fmpl-core/)

---

## Overview

The core crate provides the complete language runtime:

- **Lexer** — Tokenization using [logos](https://github.com/maciejhirsz/logos)
- **Parser** — Recursive descent producing AST
- **Compiler** — AST to indexed RPN bytecode
- **VM** — Indexed RPN bytecode execution with async support
- **Object Database** — Prototype-based objects with Goblins patterns
- **Grammar Engine** — OMeta-style extensible PEG grammars

---

## Module Structure

```
fmpl-core/src/
├── lib.rs              # Public API exports
├── lexer.rs            # Token definitions and lexing (logos)
├── parser.rs           # Recursive descent parser (+ generated parser support)
├── ast.rs              # AST node definitions
├── compiler.rs         # AST → Indexed RPN bytecode compilation
├── bytecode/           # Bytecode format definitions
├── vm.rs               # Indexed RPN VM with async runtime
├── vm_internal/        # VM internal implementation details
├── instructions/       # Instruction handlers (arithmetic, control_flow, etc.)
├── value.rs            # Runtime values (primitives, streams, grammars)
├── object.rs           # Object database with facets
├── stream.rs           # Async stream primitives (StreamHandle, SinkHandle)
├── parse_stream.rs     # ParseStream: unified parsing with combinators and memoization
├── pattern/            # Unified pattern type for let bindings and grammars
│   └── mod.rs          # Pattern enum with compilation modes
├── repr.rs             # Source code representation (pretty-print)
├── debug.rs            # Debug utilities
├── error.rs            # Error types (thiserror)
├── tuplespace/         # Linda-style tuple space coordination
├── builtins/           # Built-in functions (17 modules)
│   ├── ast.rs          # AST manipulation
│   ├── curl.rs         # HTTP requests
│   ├── io.rs           # I/O operations
│   ├── rand.rs         # Random number generation
│   ├── sse.rs          # Server-sent events
│   ├── time.rs         # Time operations
│   ├── bytes.rs        # Byte operations
│   ├── bridge.rs       # Rust-FMPL bridges
│   ├── ir.rs           # Intermediate representation
│   ├── codegen/        # Code generation
│   └── ...
└── grammar/
    ├── mod.rs          # Grammar registry and public API
    ├── parser.rs       # Grammar definition parser
    ├── runtime.rs      # PEG runtime with memoization and backtracking
    ├── trampoline.rs   # Stack-safe recursion via trampolining
    ├── input.rs        # Input sources (string, list)
    ├── stream_input.rs # Streaming input with Fjall overflow
    ├── incremental.rs  # ParseState/ParseNext for suspension
    └── driver.rs       # ParseDriver for async pipelines
```

---

## Key Types

### Public API

```rust
// Evaluation
pub fn eval(vm: &mut Vm, source: &str) -> Result<Value>;

// Core types
pub use ast::Expr;
pub use compiler::{CompiledCode, Compiler};
pub use grammar::{Grammar, GrammarRegistry, Pattern, Rule};
pub use lexer::{Lexer, Token};
pub use object::{Object, ObjectDb, ObjectId};
pub use parser::Parser;
pub use value::Value;
pub use vm::Vm;
```

### Value Enum

Runtime values include:

- Primitives: `Null`, `Bool`, `Int`, `Float`, `String`, `Symbol`
- Collections: `List(Arc<Vec<Value>>)`, `Map(Arc<HashMap<SmolStr, Value>>)`
- Objects: `Object(ObjectId)`, `Facet { object, members }`
- Functions: `Lambda(Arc<Lambda>)`, `Partial(Arc<Partial>)`
- Data: `Tagged(SmolStr, Arc<Vec<Value>>)` — Constructor values with tag + children
- Grammars: `Grammar(Arc<Grammar>)` — First-class grammar values
- Async Streams: `Stream(Arc<Stream>)`, `AsyncStream`, `Sink`, `SuspendedStream`, `SuspendedSink`
- Parsing: `ParseStream(Arc<Mutex<ParseStream>>)` — Unified stream for combinator parsing
- Coordination: `TupleSpace`, `TupleSpaceFacet`
- Observation: `Cursor` — RLM-style CoW reference
- Code: `Code(Arc<CompiledCode>)` — Compiled bytecode (opaque)

### Stream Operations

```rust
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse { grammar: Value, rule: SmolStr },      // Blocking parse
    AsyncParse { grammar: Value, rule: SmolStr },  // Incremental parse
}
```

Note: `Collect`, `Take`, `Drop` are not implemented as StreamOp variants.

---

## Features

### Default

No optional features enabled.

### `persistence`

Enables Fjall-backed persistence for:

- Stream position overflow (large buffers spill to disk)
- Memo table persistence (memoization survives suspension)
- ParseState serialization (durable parse suspension)

```toml
[dependencies]
fmpl-core = { path = "../fmpl-core", features = ["persistence"] }
```

---

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `logos` | Lexer generator |
| `thiserror` | Error types |
| `smol_str` | Interned strings |
| `serde`, `rkyv` | Serialization |
| `tokio` | Async runtime |
| `curl` | HTTP client |
| `fjall` | LSM persistence (optional) |

---

## Usage

```rust
use fmpl_core::{eval, Vm, ObjectDb};

fn main() -> fmpl_core::Result<()> {
    let db = ObjectDb::new();
    let mut vm = Vm::new(db);

    let result = eval(&mut vm, "1 + 2 * 3")?;
    println!("{:?}", result);  // Int(7)

    Ok(())
}
```

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — OMeta-style grammars with streaming support
- [parse-stream.md](./parse-stream.md) — ParseStream with combinators and packrat memoization
- [object-system.md](./object-system.md) — Goblins-inspired objects
- [vm.md](./vm.md) — Bytecode VM details
- [pattern-matching.md](./pattern-matching.md) — Pattern matching with `@` operator
