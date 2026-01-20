# fmpl-core

Core runtime library for [Project Name TBD].

**Crate**: `fmpl-core`
**Location**: [fmpl-core/](../fmpl-core/)

---

## Overview

The core crate provides the complete language runtime:

- **Lexer** — Tokenization using [logos](https://github.com/maciejhirsz/logos)
- **Parser** — Recursive descent producing AST
- **Compiler** — AST to indexed RPN bytecode
- **VM** — Stack-based bytecode execution with async support
- **Object Database** — Prototype-based objects with Goblins patterns
- **Grammar Engine** — OMeta-style extensible PEG grammars

---

## Module Structure

```
fmpl-core/src/
├── lib.rs           # Public API exports
├── lexer.rs         # Token definitions and lexing
├── parser.rs        # Recursive descent parser
├── ast.rs           # AST node definitions
├── compiler.rs      # Bytecode compilation
├── bytecode/        # Bytecode format definitions
├── vm.rs            # Bytecode VM with async runtime
├── value.rs         # Runtime values (primitives, streams, grammars)
├── object.rs        # Object database with facets
├── stream.rs        # Async stream primitives
├── repr.rs          # Source code representation (pretty-print)
├── error.rs         # Error types
├── builtins/        # Built-in functions
└── grammar/
    ├── mod.rs       # Grammar registry and public API
    ├── parser.rs    # Grammar definition parser
    ├── runtime.rs   # PEG runtime with memoization
    ├── input.rs     # Input sources (string, list)
    ├── stream_input.rs  # Streaming input with Fjall overflow
    ├── incremental.rs   # ParseState/ParseNext for suspension
    └── driver.rs    # ParseDriver for async pipelines
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
- Collections: `List`, `Map`
- Objects: `Object(ObjectId)`, `Facet { object, members }`
- Functions: `Closure`, `BuiltinFn`, `Constructor`
- Async: `Promise`, `Stream`, `StreamOp`
- Grammars: `Grammar`, `GrammarRef`

### Stream Operations

```rust
pub enum StreamOp {
    Map { f: Value },
    Filter { f: Value },
    Parse { grammar: Value, rule: SmolStr },
    AsyncParse { grammar: Value, rule: SmolStr },  // Incremental
    Collect,
    // ...
}
```

---

## Features

### Default

No optional features enabled.

### `fjall-persistence`

Enables Fjall-backed persistence for:

- Stream position overflow (large buffers spill to disk)
- Memo table persistence (memoization survives suspension)
- ParseState serialization (durable parse suspension)

```toml
[dependencies]
fmpl-core = { path = "../fmpl-core", features = ["fjall-persistence"] }
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

- [grammar-system.md](./grammar-system.md) — OMeta-style grammars
- [streaming-grammar.md](./streaming-grammar.md) — Incremental parsing
- [object-system.md](./object-system.md) — Goblins-inspired objects
- [vm.md](./vm.md) — Bytecode VM details
