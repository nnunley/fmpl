# VM

Stack-based bytecode virtual machine with async support.

**Location**: [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs:46)

---

## Overview

The VM executes compiled bytecode using:

- **Stack-based execution** — Operands pushed/popped from stack
- **Indexed RPN** — Flat bytecode format (from burakemir.ch)
- **Async runtime** — Tokio-based for `<-` and streams
- **Object integration** — Method dispatch via ObjectDb
- **Grammar integration** — PEG grammar application with semantic actions

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
│  │ [Frame { ip, code, locals, this, caller }, ...]  │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Scopes (let bindings)                            │   │
│  │ [{ name → Value }, ...]                          │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ ObjectDb + GrammarRegistry                       │   │
│  │ { id → Object } + { name → Grammar }             │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## Bytecode Format

Instructions are defined in [compiler.rs:12](../fmpl-core/src/compiler.rs:12):

```rust
pub enum Instruction {
    // Literals
    LoadNull,
    LoadBool(bool),
    LoadInt(i64),
    LoadFloat(f64),
    LoadString(SmolStr),
    LoadSymbol(SmolStr),

    // Variable access
    LoadVar(SmolStr),
    StoreVar(SmolStr),

    // Special references
    LoadSelf,
    LoadParent,
    LoadCaller,
    LoadUser,
    LoadArgs,

    // Arithmetic
    Add, Sub, Mul, Div, Mod, Neg,

    // Comparison
    Eq, NotEq, Lt, Gt, LtEq, GtEq,

    // Logical
    Not, And, Or,

    // Control flow
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),

    // Functions and calls
    Call(usize),               // arg count
    TailCall(usize),
    MethodCall(SmolStr, usize),
    Return,

    // Objects
    GetProp(SmolStr),
    SetProp(SmolStr),
    Spawn(usize),
    GetFacet(SmolStr),

    // Sync/Async
    SyncCall,
    AsyncCall,

    // Data structures
    MakeList(usize),
    MakeMap(usize),
    Index,
    Slice,

    // Binding
    PushScope,
    PopScope,
    Bind(SmolStr),

    // Lambda
    MakeLambda(Vec<SmolStr>, usize),

    // Stack
    Pop,
    Dup,
    Pipe,

    // Streams
    MakeStream,
    StreamMap,
    StreamFilter,
    StreamFlatMap,
    StreamReduce,
    StreamParse(SmolStr),

    // Pattern matching
    MatchPattern(usize),
    ExtractMapKey(SmolStr),
    ExtractListIndex(usize),

    // Object definition
    DefineObject(SmolStr),
    DefineMethod(SmolStr, usize),
    DefineProp(SmolStr),
    DefineFacet(SmolStr, usize, bool),

    // Grammar
    GrammarApply(SmolStr),
    LoadGrammar(Arc<Grammar>),
    ExtendGrammar(Grammar),

    // Exception handling
    PushHandler(usize),
    PopHandler,
    Throw,
}
```

---

## Execution Model

### Stack Operations

```
LoadInt(42)  : [] → [42]
LoadInt(3)   : [42] → [42, 3]
Add          : [42, 3] → [45]
```

### Function Calls

```
LoadInt(args...)
LoadVar(function)
Call(arity)  : creates new frame, jumps to function body
Return       : pops frame, pushes result
```

### Async Operations

```
AsyncCall    : wraps value in AsyncStream (requires runtime)
MakeStream   : creates Stream from source value
Pipe         : applies function to argument (x |> f → f(x))
```

---

## Key Types

### Vm

[vm.rs:46](../fmpl-core/src/vm.rs:46):

```rust
pub struct Vm {
    pub objects: ObjectDb,
    pub grammars: GrammarRegistry,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    scopes: Vec<Scope>,
    pub current_user: Option<ObjectId>,
    exception_handlers: Vec<(usize, usize, usize)>,
    runtime: Option<tokio::runtime::Handle>,
}
```

### Frame

[vm.rs:13](../fmpl-core/src/vm.rs:13):

```rust
struct Frame {
    code: Arc<CompiledCode>,
    ip: usize,
    base: usize,
    locals: HashMap<SmolStr, Value>,
    this: Option<ObjectId>,
    caller: Option<ObjectId>,
    next_nested: usize,
}
```

### CompiledCode

[compiler.rs:128](../fmpl-core/src/compiler.rs:128):

```rust
pub struct CompiledCode {
    pub instructions: Vec<Instruction>,
    pub nested: Vec<CompiledCode>,  // lambdas, methods
    pub source: Option<SmolStr>,
}
```

---

## Public API

[vm.rs:60](../fmpl-core/src/vm.rs:60):

```rust
impl Vm {
    pub fn new() -> Self;

    /// Create VM with tokio runtime handle (required for async)
    pub fn with_runtime(handle: tokio::runtime::Handle) -> Self;

    /// Set runtime handle after construction
    pub fn set_runtime(&mut self, handle: tokio::runtime::Handle);

    /// Run compiled code
    pub fn run(&mut self, code: &CompiledCode) -> Result<Value>;

    /// Evaluate expression with bindings (for semantic actions)
    pub fn eval_with_bindings(
        &mut self,
        expr: &Expr,
        bindings: &HashMap<SmolStr, Value>,
    ) -> Result<Value>;

    /// Apply grammar to input with semantic action evaluation
    pub fn apply_grammar(
        &mut self,
        input: Value,
        grammar: Arc<Grammar>,
        rule_name: &str,
    ) -> Result<Option<Value>>;
}
```

---

## Async Support

### Async Calls

```fmpl
<- expr  -- wraps expr in AsyncStream (requires runtime)
```

### Streams

```fmpl
let stream = <- curl.get(url)
stream |> map(\x x.body) |> filter(\x x != null)
```

### Runtime Initialization

```rust
// Async operations require runtime handle
let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

// Or set later
vm.set_runtime(handle);
```

---

## Exception Handling

Cross-frame exception unwinding ([vm.rs:784](../fmpl-core/src/vm.rs:784)):

```fmpl
try {
  1 / 0
} catch (e) {
  99  -- returns 99
}
```

Implemented via:
- `PushHandler(catch_ip)` — register handler with stack/frame depth
- On error: unwind to handler depth, push error value, jump to catch
- `PopHandler` — remove handler on normal exit

---

## Grammar Integration

The VM integrates with the grammar system ([vm.rs:657](../fmpl-core/src/vm.rs:657)):

```rust
Instruction::GrammarApply(rule_name) => {
    let grammar_val = self.pop()?;
    let input = self.pop()?;
    let result = self.apply_grammar(input, grammar, &rule_name)?;
    self.stack.push(result.unwrap());
}
```

Grammar application supports both string and AsyncStream inputs.

---

## Builtins

Built-in methods available via special symbols:

| Builtin | Description |
|---------|-------------|
| `curl.get(url)` | HTTP GET, returns `%{source: stream, sink: null}` |
| `curl.post(url, body)` | HTTP POST, returns `%{source: stream, sink: null}` |

List methods (built-in):
- `.len()`, `.first()`, `.last()`, `.push(item)`

String methods (built-in):
- `.len()`, `.upper()`, `.lower()`

See [builtins/curl.rs](../fmpl-core/src/builtins/curl.rs:17) for HTTP implementation.

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [object-system.md](./object-system.md) — Object database
- [grammar-system.md](./grammar-system.md) — Grammar integration

---

## References

- [Indexed RPN](https://burakemir.ch/post/indexed-rpn/) — Bytecode format inspiration
