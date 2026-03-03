# VM

**Indexed RPN bytecode virtual machine** with async support.

**Location**: [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs:89)

**Implementation**: See [Indexed RPN Conversion Spec](./indexed-rpn-conversion.md)

---

## Overview

The VM executes compiled bytecode using:

- **Indexed RPN execution** — Each instruction stores result in `values[ip]`, operands referenced by index
- **No operand stack** — Direct index access instead of push/pop
- **Compile-time name resolution** — `resolve_names` pass wires variable references to bindings
- **Async runtime** — Tokio-based for `<-` and streams
- **Object integration** — Method dispatch via ObjectDb
- **Grammar integration** — PEG grammar application with semantic actions

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                        VM                               │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Values Array (per Frame)                         │   │
│  │ [Value, Value, Value, ...]                       │   │
│  │ Indexed by instruction position (ip)             │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │ Call Stack (Frames)                              │   │
│  │ [Frame { ip, code, values, locals, this, caller }]  │
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

Instructions are defined in [compiler.rs:37](../fmpl-core/src/compiler.rs:37):

```rust
pub enum Instruction {
    // Literals (produce values, no operand references)
    LoadNull,
    LoadBool(bool),
    LoadInt(i64),
    LoadFloat(f64),
    LoadString(SmolStr),
    LoadSymbol(SmolStr),

    // Variable access
    LoadVar(SmolStr),                              // Legacy: for runtime lookups
    StoreVar { name: SmolStr, value: InstrIndex },

    // Special references (produce values)
    LoadSelf,
    LoadParent,
    LoadCaller,
    LoadUser,
    LoadArgs,

    // Binary arithmetic (explicit operand indices)
    Add { lhs: InstrIndex, rhs: InstrIndex },
    Sub { lhs: InstrIndex, rhs: InstrIndex },
    Mul { lhs: InstrIndex, rhs: InstrIndex },
    Div { lhs: InstrIndex, rhs: InstrIndex },
    Mod { lhs: InstrIndex, rhs: InstrIndex },

    // Unary (explicit operand index)
    Neg { operand: InstrIndex },
    Not { operand: InstrIndex },

    // Comparison (explicit operand indices)
    Eq { lhs: InstrIndex, rhs: InstrIndex },
    NotEq { lhs: InstrIndex, rhs: InstrIndex },
    Lt { lhs: InstrIndex, rhs: InstrIndex },
    Gt { lhs: InstrIndex, rhs: InstrIndex },
    LtEq { lhs: InstrIndex, rhs: InstrIndex },
    GtEq { lhs: InstrIndex, rhs: InstrIndex },

    // Control flow (explicit condition indices)
    Jump { target: InstrIndex },
    JumpIfFalse { cond: InstrIndex, target: InstrIndex },
    JumpIfTrue { cond: InstrIndex, target: InstrIndex },

    // Functions and calls (explicit operand indices)
    Call { func: InstrIndex, args: Vec<InstrIndex> },
    TailCall { func: InstrIndex, args: Vec<InstrIndex> },
    MethodCall { receiver: InstrIndex, method: SmolStr, args: Vec<InstrIndex> },
    Return { value: InstrIndex },

    // Objects (explicit operand indices)
    GetProp { object: InstrIndex, name: SmolStr },
    SetProp { object: InstrIndex, name: SmolStr, value: InstrIndex },
    Spawn { object: InstrIndex, args: Vec<InstrIndex> },
    GetFacet { object: InstrIndex, name: SmolStr },

    // Sync/Async (explicit operand indices)
    SyncCall { target: InstrIndex },
    AsyncCall { target: InstrIndex },

    // Data structures (explicit operand indices)
    MakeList { elements: Vec<InstrIndex> },
    MakeMap { pairs: Vec<(InstrIndex, InstrIndex)> },
    Index { collection: InstrIndex, key: InstrIndex },
    Slice { collection: InstrIndex, start: Option<InstrIndex>, end: Option<InstrIndex> },

    // Binding & Scope (BlockStart/BlockEnd replace PushScope/PopScope)
    BlockStart,                                  // Scope boundary: begin
    BlockEnd,                                    // Scope boundary: end
    Bind { name: SmolStr, value: InstrIndex },   // Introducer: name → value index
    NameRef { bind: InstrIndex },                // Reference to Bind instruction (resolved at compile time)

    // Legacy scope instructions (deprecated)
    PushScope,
    PopScope,

    // Lambda (explicit capture indices)
    MakeLambda { params: Vec<SmolStr>, body: usize, captures: Vec<InstrIndex> },

    // Pipe (explicit operand indices)
    Pipe { arg: InstrIndex, func: InstrIndex },

    // Streams (explicit operand indices)
    MakeStream { source: InstrIndex },
    StreamMap { source: InstrIndex, func: InstrIndex },
    StreamFilter { source: InstrIndex, pred: InstrIndex },
    StreamFlatMap { source: InstrIndex, func: InstrIndex },
    StreamReduce { source: InstrIndex, init: InstrIndex, func: InstrIndex },
    StreamParse { source: InstrIndex, grammar: InstrIndex, rule: SmolStr },

    // Pattern matching (explicit operand indices)
    MatchPattern { value: InstrIndex, fail_target: InstrIndex },
    ExtractMapKey { source: InstrIndex, key: SmolStr },
    ExtractListIndex { source: InstrIndex, index: usize },

    // Object definition (creates object in DB)
    DefineObject(SmolStr),
    DefineMethod { object: InstrIndex, name: SmolStr, body: usize },
    DefineProp { object: InstrIndex, name: SmolStr, value: InstrIndex },
    DefineFacet { object: InstrIndex, name: SmolStr, members: Vec<InstrIndex>, terminal: bool },

    // Grammar application (explicit operand indices)
    GrammarApply { input: InstrIndex, grammar: InstrIndex, rule: SmolStr },
    LoadGrammar(Arc<Grammar>),
    ExtendGrammar { base: InstrIndex, extension: Grammar },

    // Exception handling
    PushHandler { catch_target: InstrIndex },
    PopHandler,
    Throw { value: InstrIndex },

    // Copy (for control flow convergence)
    Copy { source: InstrIndex },

    // No-op (placeholder)
    Nop,
}
```

---

## Execution Model

### Indexed RPN Operations

```rust
// (3 + 4) * 5 in Indexed RPN
// Index 0: LoadInt(3)           → values[0] = 3
// Index 1: LoadInt(4)           → values[1] = 4
// Index 2: Add(lhs: 0, rhs: 1)  → values[2] = values[0] + values[1] = 7
// Index 3: LoadInt(5)           → values[3] = 5
// Index 4: Mul(lhs: 2, rhs: 3)  → values[4] = values[2] * values[3] = 35
```

### Name Resolution

Variables are resolved at **compile time** by the `resolve_names` pass:

```rust
// Before resolve_names:
// Index 0: LoadInt(10)
// Index 1: Bind("x", 0)
// Index 2: LoadVar("x")          ← runtime lookup
// Index 3: LoadInt(5)
// Index 4: Add(lhs: 2, rhs: 3)

// After resolve_names:
// Index 0: LoadInt(10)
// Index 1: Bind("x", 0)
// Index 2: NameRef(bind: 1)      ← direct reference to Bind instruction
// Index 3: LoadInt(5)
// Index 4: Add(lhs: 2, rhs: 3)
```

### Function Calls

```
// add(3, 4) where add = lambda(a, b) a + b
// Index 0: LoadVar("add")         → values[0] = Lambda
// Index 1: LoadInt(3)             → values[1] = 3
// Index 2: LoadInt(4)             → values[2] = 4
// Index 3: Call { func: 0, args: [1, 2] } → creates new frame, executes lambda body
```

### Method Calls and Magical Variables

When a method is called on an object, the new frame's environment is **pre-bound** with magical variables that provide context about the call:

| Variable | Type | Description |
|----------|------|-------------|
| `self` | `ObjectId` | The object receiving the method call |
| `parent` | `ObjectId` or `null` | The object's prototype parent (for prototype chain lookup) |
| `caller` | `ObjectId` or `null` | The object that initiated this method call |
| `user` | `ObjectId` or `null` | The current user context (from `VM.current_user`) |
| `args` | `List` | The list of all arguments passed to the method |

These are bound as **local variables** in the method's execution environment, accessible by name just like normal parameters:

```fmpl
object counter {
  value: 0

  increment(): self.value + 1   -- 'self' is pre-bound
  get_parent(): parent          -- 'parent' is pre-bound
}
```

**Implementation** ([vm.rs:1352](../fmpl-core/src/vm.rs:1352)):
- When `MethodCall` executes, a new `Frame` is created
- `frame.this` is set to the receiver object ID
- `frame.caller` is set to the previous frame's `this` (if any)
- The magical variables are implicitly available through special `LoadSelf`, `LoadParent`, `LoadCaller`, `LoadUser`, `LoadArgs` instructions

**Note**: The current implementation uses dedicated bytecode instructions (`LoadSelf`, etc.) rather than explicit environment binding. Future implementations may bind these as actual local variables for consistency with parameter passing.

### Async Operations

```
AsyncCall    : wraps value in AsyncStream (requires runtime)
MakeStream   : creates Stream from source value
Pipe         : applies function to argument (x |> f → f(x))
```

---

## Key Types

### Vm

[vm.rs:89](../fmpl-core/src/vm.rs:89):

```rust
pub struct Vm {
    pub objects: ObjectDb,
    pub grammars: GrammarRegistry,
    frames: Vec<Frame>,
    scopes: Vec<Scope>,
    pub current_user: Option<ObjectId>,
    exception_handlers: Vec<(InstrIndex, usize)>,
    runtime: Option<tokio::runtime::Handle>,
}
```

### Frame

[vm.rs:22](../fmpl-core/src/vm.rs:22):

```rust
struct Frame {
    code: Arc<CompiledCode>,
    ip: usize,                    // Instruction pointer
    values: Vec<Value>,           // Indexed by instruction position
    locals: HashMap<SmolStr, Value>,
    this: Option<ObjectId>,
    caller: Option<ObjectId>,
}
```

**Key change**: `values: Vec<Value>` replaces operand stack — each instruction stores its result at `values[ip]`.

### CompiledCode

[compiler.rs:159](../fmpl-core/src/compiler.rs:159):

```rust
pub struct CompiledCode {
    pub instructions: Vec<Instruction>,
    pub nested: Vec<CompiledCode>,  // lambdas, methods
    pub source: Option<SmolStr>,
}
```

### InstrIndex

[compiler.rs:18](../fmpl-core/src/compiler.rs:18):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrIndex(pub usize);
```

Index into the instructions/values array for operand references.

---

## Compiler API

### Name Resolution Pass

[compiler.rs:235](../fmpl-core/src/compiler.rs:235):

```rust
pub fn resolve_names(code: &mut CompiledCode) {
    // Converts LoadVar("x") → NameRef { bind: InstrIndex }
    // Single pass O(n) traversal
    // Eliminates runtime scope lookup
}
```

### Backpatching

[compiler.rs:186](../fmpl-core/src/compiler.rs:186):

```rust
impl CompiledCode {
    fn emit(&mut self, instr: Instruction) -> InstrIndex;
    fn next_index(&self) -> InstrIndex;
    fn patch_jump_target(&mut self, idx: InstrIndex, target: InstrIndex);
}
```

---

## Public API

[vm.rs:102](../fmpl-core/src/vm.rs:102):

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

The VM integrates with the grammar system via the `GrammarApply` instruction:

```rust
Instruction::GrammarApply { input, grammar, rule } => {
    let input_val = frame.get(input);
    let grammar_val = frame.get(grammar);
    let result = self.apply_grammar(input_val, grammar_arc, &rule)?;
    frame.set_current(result.unwrap());
}
```

Grammar application supports string, list, and AsyncStream inputs.

---

## Builtins

Built-in methods available via special symbols:

| Builtin | Description |
|---------|-------------|
| `curl.get(url)` | HTTP GET, returns `%{source: stream, sink: null}` |
| `curl.post(url, body)` | HTTP POST, returns `%{source: stream, sink: null}` |
| `json::parse(str)` | Parse JSON string to FMPL values |
| `json::stringify(val)` | Serialize FMPL values to JSON string |
| `rand::int(min, max)` | Random integer in range |
| `rand::float()` | Random float [0, 1) |
| `io::load(path)` | Load and execute FMPL file |

### ParseStream builtins (`stream::*`)

| Builtin | Description |
|---------|-------------|
| `stream::new(input)` | Create ParseStream from string or list |
| `stream::match_char(s, ch)` | Match exact character |
| `stream::match_class(s, cls)` | Match character class (e.g., `"a-z"`, `"0-9"`) |
| `stream::fail(msg)` | Raise parse failure |
| `stream::choice(s, [alts])` | Try alternatives with backtracking |
| `stream::star(s, rule)` | Zero-or-more matches |
| `stream::plus(s, rule)` | One-or-more matches |
| `stream::seq(s, [rules])` | Sequence all rules |
| `stream::not(s, rule)` | Negative lookahead |
| `stream::lookahead(s, rule)` | Positive lookahead |
| `stream::optional(s, rule)` | Zero-or-one match |

See [parse-stream.md](./parse-stream.md) for full combinator documentation.

### Collection methods

List methods (built-in):
- `.len()`, `.first()`, `.last()`, `.push(item)`

String methods (built-in):
- `.len()`, `.upper()`, `.lower()`

### ParseStream methods

- `.head()`, `.position()`, `.advance(n)`, `.checkpoint()`, `.restore(cp)`, `.apply(rule)`

See [builtins/curl.rs](../fmpl-core/src/builtins/curl.rs) for HTTP implementation.

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [object-system.md](./object-system.md) — Object database
- [grammar-system.md](./grammar-system.md) — Grammar integration
- [parse-stream.md](./parse-stream.md) — ParseStream combinators and memoization
- [pattern-matching.md](./pattern-matching.md) — Pattern matching with `@` operator

---

## References

- [Indexed RPN](https://burakemir.ch/post/indexed-rpn/) — Bytecode format inspiration
