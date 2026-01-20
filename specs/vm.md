# VM

Stack-based bytecode virtual machine with async support.

**Location**: [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs)

---

## Overview

The VM executes compiled bytecode using:

- **Stack-based execution** — Operands pushed/popped from stack
- **Indexed RPN** — Flat bytecode format (from burakemir.ch)
- **Async runtime** — Tokio-based for `<-` and streams
- **Object integration** — Method dispatch via ObjectDb

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                        VM                               │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Operand Stack                                    │   │
│  │ [Value, Value, Value, ...]                       │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Call Stack (Frames)                              │   │
│  │ [Frame { ip, locals, ... }, ...]                 │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Globals                                          │   │
│  │ { name → Value }                                 │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ ObjectDb                                         │   │
│  │ { id → Object }                                  │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## Bytecode Format

Instructions are flat opcodes with inline operands:

```rust
pub enum Op {
    // Stack operations
    Push(Value),
    Pop,
    Dup,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    Not,
    And,
    Or,

    // Variables
    LoadLocal(usize),
    StoreLocal(usize),
    LoadGlobal(SmolStr),
    StoreGlobal(SmolStr),

    // Control flow
    Jump(usize),
    JumpIfFalse(usize),
    Call(usize),
    Return,

    // Objects
    GetProperty(SmolStr),
    SetProperty(SmolStr),
    MethodCall(SmolStr, usize),

    // Streams
    Pipe,
    Spawn,
    Await,

    // Grammars
    Apply(SmolStr),

    // ...more opcodes
}
```

---

## Execution Model

### Stack Operations

```
Push(42)     : [] → [42]
Push(3)      : [42] → [42, 3]
Add          : [42, 3] → [45]
```

### Function Calls

```
Push(args...)
Push(function)
Call(arity)  : creates new frame, jumps to function body
Return       : pops frame, pushes result
```

### Async Operations

```
Spawn        : creates async task, pushes Promise
Await        : suspends until Promise resolves
Pipe         : connects stream to operator
```

---

## Key Types

### Vm

```rust
pub struct Vm {
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: HashMap<SmolStr, Value>,
    pub objects: ObjectDb,
    grammars: GrammarRegistry,
    // ... async runtime state
}
```

### Frame

```rust
pub struct Frame {
    ip: usize,
    code: Arc<CompiledCode>,
    locals: Vec<Value>,
    base: usize,  // stack base
}
```

### CompiledCode

```rust
pub struct CompiledCode {
    pub ops: Vec<Op>,
    pub constants: Vec<Value>,
    pub local_count: usize,
}
```

---

## Public API

```rust
impl Vm {
    pub fn new() -> Self;

    /// Run compiled code
    pub fn run(&mut self, code: &CompiledCode) -> Result<Value>;

    /// Call a method on an object
    pub fn call_method(
        &mut self,
        obj: ObjectId,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value>;

    /// Evaluate FMPL source
    pub fn eval(&mut self, source: &str) -> Result<Value>;

    /// Get/set globals
    pub fn get_global(&self, name: &str) -> Option<Value>;
    pub fn set_global(&mut self, name: SmolStr, value: Value);
}
```

---

## Async Support

### Promises

```fmpl
let promise = spawn(async_task)
<- promise  -- await result
```

### Streams

```fmpl
let stream = <- http.get(url)
stream |> map(f) |> filter(g) |> collect
```

### Tokio Integration

```rust
impl Vm {
    pub async fn run_async(&mut self, code: &CompiledCode) -> Result<Value> {
        // Uses tokio::spawn for async operations
        // Handles channel-based stream communication
    }
}
```

---

## Exception Handling

Cross-frame exception unwinding:

```fmpl
try {
  risky_operation()
} catch (e) {
  handle(e)
}
```

Implemented via:
- Exception frames pushed on catch
- Unwinding pops frames until handler found
- Error values carry stack trace

---

## Grammar Integration

The VM integrates with the grammar system:

```rust
Op::Apply(rule) => {
    let input = self.pop()?;
    let grammar = self.pop()?;
    let result = self.apply_grammar(grammar, rule, input)?;
    self.push(result);
}
```

---

## Builtins

Built-in functions available in the VM:

| Builtin | Description |
|---------|-------------|
| `print(x)` | Print to stdout |
| `type(x)` | Get type as symbol |
| `len(x)` | Length of list/string |
| `map(f, list)` | Map function over list |
| `filter(f, list)` | Filter list |
| `range(start, end)` | Generate range |
| `http_get(url)` | HTTP GET request |
| `json_parse(s)` | Parse JSON string |

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [object-system.md](./object-system.md) — Object database
- [grammar-system.md](./grammar-system.md) — Grammar integration

---

## References

- [Indexed RPN](https://burakemir.ch/post/indexed-rpn/) — Bytecode format inspiration
