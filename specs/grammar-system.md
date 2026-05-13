# Grammar System

OMeta-style extensible PEG grammars for FMPL.

**Location**: [`fmpl-core/src/grammar/`](../fmpl-core/src/grammar/)

---

## Overview

PEG-based parsing with grammar inheritance, packrat memoization, and semantic actions. Unlike traditional PEG parsers, this system can parse any stream of objects:

- **Text** — Character-by-character parsing
- **Binary** — Byte streams for protocols/file formats
- **Objects** — Lists/trees of values for AST transformation

## Streaming and Incremental Parsing

The grammar system supports push-based incremental parsing for async streams (LLM output, HTTP chunks) with:

- **Push-based parsing** — Values arrive asynchronously, grammar emits matches
- **Unlimited backtracking** — OMeta-style cons-cell positions with Fjall overflow
- **Packrat memoization** — Per-position memo tables with optional Fjall backing
- **Incremental API** — `start()`/`resume()` for durable parse states

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    ParseDriver                          │
│  driver.rs:24                                           │
│  - Collects values from async stream                    │
│  - Runs grammar against each value                      │
│  - Emits matched values downstream                      │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│                   PegRuntime                            │
│  runtime.rs:900                                         │
│  - start(rule) → ParseState                             │
│  - resume(state) → ParseNext                            │
│  - Per-position packrat memoization                     │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│               StreamPosition                            │
│  stream_input.rs:42                                     │
│  - Immutable cons-cell with lazy tail                   │
│  - Per-position memo table                              │
│  - Fjall overflow in StreamSource::Async                │
└─────────────────────────────────────────────────────────┘
```

### ParseState (`incremental.rs:15`)

Represents suspended parse state:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Current position index in input
    pub position_index: usize,
    /// Rule call stack: (rule_name, entry_position_index)
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Current variable bindings
    pub bindings: HashMap<SmolStr, Value>,
}
```

Serialization methods (`incremental.rs:65-97`, feature-gated):
- `to_bytes()` / `from_bytes()` — rkyv serialization
- `save_to_fjall()` / `load_from_fjall()` — durable persistence

### ParseNext (`incremental.rs:26`)

Result of incremental parse step:

```rust
pub enum ParseNext {
    /// Rule matched, here's the result value
    Match(Value),
    /// Need more input - here's state to resume from
    NeedInput(ParseState),
    /// Input stream ended
    End,
}
```

### StreamPosition (`stream_input.rs:42`)

OMeta-style immutable cons-cell for streaming input:

```rust
pub struct StreamPosition {
    /// The value at this position (None = end of stream)
    head: Option<Value>,
    /// The next position (lazily computed)
    tail: RefCell<Option<Rc<StreamPosition>>>,
    /// Position index (for memoization keys)
    index: usize,
    /// Per-position memoization table
    memo: RefCell<HashMap<SmolStr, MemoEntry>>,
    /// Source reference for pulling more data
    source: Rc<StreamSource>,
    /// Optional Fjall partition for memo persistence
    #[cfg(feature = "persistence")]
    memo_fjall: Option<Arc<Mutex<MemoFjall>>>,
}
```

### Pipeline Syntax

```fmpl
-- LLM stream → parser → handler
llm_stream |> parser.tool_call |> execute_tool

-- With async parse operator
llm_stream |> AsyncParse { grammar: ToolParser, rule: "output" } |> handler
```

### Incremental API (`runtime.rs:900-945`)

```rust
// Start parsing
let input = StreamingInput::from_values(values);
let mut runtime = PegRuntime::new(input, &registry, grammar);
let state = runtime.start("rule_name");

// Resume and get result
match runtime.resume(state)? {
    ParseNext::Match(value) => {
        // Successfully matched - use value
    }
    ParseNext::NeedInput(state) => {
        // Need more input - state can be saved for later
    }
    ParseNext::End => {
        // Input stream ended
    }
}
```

### Fjall Backing

For async streams, positions can spill to Fjall when memory is limited:

```rust
enum StreamSource {
    Async {
        handle: Mutex<StreamHandle>,
        timeout: Option<Duration>,
        /// Cached positions for index lookup
        positions: Mutex<Vec<Rc<StreamPosition>>>,
        /// Fjall overflow for spilled positions
        #[cfg(feature = "persistence")]
        fjall: Option<FjallOverflow>,
        /// Memory limit before spilling
        #[cfg(feature = "persistence")]
        memory_limit: Option<usize>,
    },
    Static(Vec<Value>),
    Empty,
}
```

### StreamOp Variants (`value.rs:87`)

```rust
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse { grammar: Value, rule: SmolStr },      // Blocking parse
    AsyncParse { grammar: Value, rule: SmolStr }, // Incremental parse
}
```

### ParseDriver (`driver.rs:24`)

Async driver connecting streams to grammars:

```rust
pub struct ParseDriver {
    input_handle: StreamHandle,
    grammar: Arc<Grammar>,
    rule: String,
    registry: GrammarRegistry,
    output: mpsc::Sender<Value>,
    timeout: Option<Duration>,
}
```

---

---

## Grammar Definition

```fmpl
grammar mud::commands <: base::parser {
    verb = word:v &{ valid_verb(v) } => v;
    command = "take" spaces noun:obj => %{action: :take, target: obj};
}

-- Apply grammar to input
"take sword" @ mud::commands.command
```

### Syntax Conventions

**Rule separators**: Named rules within a grammar are separated by `;` or `,`:

```fmpl
grammar example {
    rule1 = pattern1;           -- semicolon separator
    rule2 = pattern2,           -- comma also works
    rule3 = pattern3;           -- last rule optionally terminated
}
```

**Alternative separator**: Within a rule, alternatives use `|`:

```fmpl
grammar example {
    -- | separates alternatives within a rule
    value = string | number | boolean;
}
```

**Binding syntax**: Bindings use `pattern:name` syntax (pattern first, then colon, then variable name):

```fmpl
grammar example {
    int = digit+:value => %{type: :int, value: value};
    pair = ident:key "=" expr:val => %{k: key, v: val};
}
```

### Inheritance

```fmpl
grammar child <: parent { ... }
```

- Child inherits all parent rules
- Child rules override same-named parent rules
- `<super.rule>` calls parent rule explicitly

### Anonymous Grammars

```fmpl
grammar { rule = pattern }           -- anonymous literal
base <: { rule = pattern }           -- extend base (no mutation)
```

---

## Pattern Types

### Text Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `.` | Any character | `any` |
| `'c'` | Specific character | `Char('a')` |
| `"str"` | Literal string | `Literal("hello")` |
| `[a-z]` | Character class | `CharClass([Range('a','z')])` |
| `[^a-z]` | Negated class | `NegCharClass([...])` |

### Combinators

| Pattern | Description | Example |
|---------|-------------|---------|
| `a b` | Sequence | `Seq([a, b])` |
| `a / b` | Ordered choice | `Choice([a, b])` |
| `a*` | Zero or more | `Star(a)` |
| `a+` | One or more | `Plus(a)` |
| `a?` | Optional | `Optional(a)` |
| `&a` | Positive lookahead | `Lookahead(a)` |
| `!a` | Negative lookahead | `Not(a)` |

### Rule References

| Pattern | Rust Variant | Description |
|---------|--------------|-------------|
| `rulename` | `Rule(SmolStr)` | Apply a named rule |
| `<rulename>` | `Super(SmolStr)` | Apply parent's rule (super call) |

### Bindings and Actions

| Pattern | Rust Variant | Description |
|---------|--------------|-------------|
| `pattern:name` | `Bind(Box<Pattern>, SmolStr)` | Bind match result to variable |
| `&{ expr }` | `Predicate(Expr)` | Semantic predicate (succeed if truthy) |
| `p => expr` | `Action(Box<Pattern>, Expr)` | Transform matched value |

### Binary Patterns

| Pattern | Rust Variant | Description |
|---------|--------------|-------------|
| `byte(0x42)` | `Byte(u8)` | Match specific byte value |
| `uint8` | `UInt8` | Unsigned 8-bit integer |
| `uint16be`, `uint16le` | `UInt16BE`, `UInt16LE` | Unsigned 16-bit integers |
| `uint32be`, `uint32le` | `UInt32BE`, `UInt32LE` | Unsigned 32-bit integers |
| `int8` | `Int8` | Signed 8-bit integer |
| `int16be`, `int16le` | `Int16BE`, `Int16LE` | Signed 16-bit integers |
| `int32be`, `int32le` | `Int32BE`, `Int32LE` | Signed 32-bit integers |
| `bytes(n)` | `Bytes(usize)` | Consume exactly n bytes |
| `byte(lo..hi)` | `ByteRange(u8, u8)` | Match byte in range (inclusive) |

### Object/Tree Patterns

| Pattern | Description |
|---------|-------------|
| `MatchValue(v)` | Match specific value |
| `MatchType(t)` | Match type (null, bool, int, etc.) |
| `ListMatch([...])` | Match list structure |
| `MapMatch([...])` | Match map keys |
| `SymbolMatch(s)` | Match specific symbol |
| `Apply(p)` | Descend into value |

---

## Compilation Architecture

Grammar patterns are **lowered to base IR** (Indexed RPN bytecode) rather than using specialized VM instructions. This follows the OMeta approach where grammars compile to the target language.

### Lowering Strategy

**Star/Plus/Choice patterns** are lowered to loops and conditional jumps:

```
# Star(pattern) lowers to:
results = []
loop_start:
  checkpoint = ParseCheckpoint
  result = <compile pattern>
  JumpIfNull result, loop_end
  results = ListAppend(results, result)
  JumpIfEqual position, checkpoint_pos, loop_end  # zero-length guard
  Jump loop_start
loop_end:
  ParseRestore checkpoint
  Return results

# Choice([p1, p2, p3]) lowers to:
  checkpoint = ParseCheckpoint
  r1 = <compile p1>
  JumpIfNotNull r1, done
  ParseRestore checkpoint
  r2 = <compile p2>
  JumpIfNotNull r2, done
  ParseRestore checkpoint
  r3 = <compile p3>
  JumpIfNotNull r3, done
  ParseRestore checkpoint
  Return Null
done:
  Return <result>
```

**Specialized instructions** are kept for optimization of common patterns:
- `MatchStarCharClass`, `MatchPlusCharClass` — Character class repetition (joins result strings)
- `MatchStarChar`, `MatchPlusChar` — Single character repetition
- `MatchStarLiteral`, `MatchPlusLiteral` — Literal string repetition
- `MatchStarRule`, `MatchPlusRule` — Rule application repetition

### Input Stack Model

For **OMeta-style tree matching**, the VM uses an input stack to enable descent into nested structures:

```rust
struct ParseState {
    input_stack: Vec<InputFrame>,  // Stack of (value, position) for tree descent
    memo: HashMap<MemoKey, MemoEntry>,  // Memoization table
}

struct InputFrame {
    value: Value,      // Current input (string, list, or single value)
    position: usize,   // Current position within this value
    identity: u64,     // Identity hash for memoization
}
```

**Instructions for tree descent:**
- `ParsePush { value }` — Push value as new input stream (descend into tree)
- `ParsePop` — Pop to previous input stream (ascend from tree)
- `ParsePosition` — Get current position (for zero-length guards)
- `ParseCheckpoint` — Save (stack_depth, position) for backtracking
- `ParseRestore { checkpoint }` — Restore to checkpoint

**Type checking for tree matching:**
- `IsList { value }` — Check if value is a list
- `IsMap { value }` — Check if value is a map
- `IsString { value }` — Check if value is a string

### Benefits

1. **Simpler VM** — Complex pattern logic moved to compiler
2. **Unified model** — Same primitives work for text and tree parsing
3. **Extensibility** — New patterns can be added by compiler lowering
4. **Debuggability** — Generated bytecode is inspectable

---

## Built-in Grammars

### base::parser

Text parsing primitives:

```fmpl
any     = .              -- any character
digit   = [0-9]          -- digit
letter  = [a-zA-Z]       -- letter
space   = [ \t\n\r]      -- whitespace char
spaces  = space*         -- whitespace
word    = letter+        -- word
integer = digit+         -- integer
eof     = !.             -- end of input
end     = <end>          -- end of input
```

### base::binary

Binary parsing primitives:

```fmpl
any      = .
byte     = uint8
uint8    = <uint8>
uint16be = <uint16be>
uint16le = <uint16le>
uint32be = <uint32be>
uint32le = <uint32le>
end      = <end>
```

### base::tree

Object/tree parsing primitives:

```fmpl
any    = .               -- any value
null   = <null>          -- null value
bool   = <bool>          -- any boolean
int    = <int>           -- any integer
float  = <float>         -- any float
string = <string>        -- any string
symbol = <symbol>        -- any symbol
list   = <list>          -- any list
map    = <map>           -- any map
end    = <end>           -- end of input
```

---

## Key Types

All types defined in [`fmpl-core/src/grammar/mod.rs`](../fmpl-core/src/grammar/mod.rs).

### Grammar (mod.rs:97)

```rust
pub struct Grammar {
    pub name: SmolStr,              // e.g., "mud::commands"
    pub parent: Option<SmolStr>,    // For registry lookup
    pub parent_grammar: Option<Arc<Grammar>>,  // Direct parent ref
    pub rules: HashMap<SmolStr, Rule>,
}
```

### Rule (mod.rs:145)

```rust
pub struct Rule {
    pub pattern: Pattern,
    pub action: Option<Expr>,
}
```

### Pattern (mod.rs:170)

30+ variants covering all pattern types. Key variants:

```rust
pub enum Pattern {
    Empty, Any, End,
    // Text
    Char(char), Literal(SmolStr), CharClass(Vec<CharRange>), NegCharClass(Vec<CharRange>),
    // Rule calls
    Rule(SmolStr), Super(SmolStr),
    // Combinators
    Seq(Vec<Pattern>), Choice(Vec<Pattern>), Star(Box<Pattern>), Plus(Box<Pattern>), Optional(Box<Pattern>),
    // Lookahead
    Lookahead(Box<Pattern>), Not(Box<Pattern>),
    // Bindings/Actions
    Bind(Box<Pattern>, SmolStr), Predicate(Expr), Action(Box<Pattern>, Expr),
    // Binary
    Byte(u8), ByteRange(u8, u8), Bytes(usize),
    UInt8, UInt16BE, UInt16LE, UInt32BE, UInt32LE,
    Int8, Int16BE, Int16LE, Int32BE, Int32LE,
    // Object/Value
    MatchValue(Value), MatchType(SmolStr), ListMatch(..), MapMatch(..), SymbolMatch(SmolStr), Apply(Box<Pattern>),
}
```

### GrammarRegistry (mod.rs:380)

```rust
pub struct GrammarRegistry {
    grammars: HashMap<SmolStr, Arc<Grammar>>,
}
```

Methods: `register()`, `get()`. Automatically registers built-in grammars (`base::parser`, `base::binary`, `base::tree`).

### GrammarParser (parser.rs:18)

Parses grammar definition syntax into `Grammar` structs.

```rust
let mut parser = GrammarParser::new(source);
let grammar = parser.parse()?;       // Named grammar
let grammar = parser.parse_anonymous()?;  // Anonymous: { rules }
```

---

## Examples

### Text Parsing

```fmpl
grammar json <: base::parser {
    value = string | number | object | array | true | false | null
    number = integer ("." integer)?
    string = '"' (!'"' any)* '"'
}
```

### Binary Parsing

```fmpl
grammar png::header <: base::binary {
    magic = byte(0x89) byte(0x50) byte(0x4E) byte(0x47);
    chunk = uint32be:len uint32be:type bytes(len):data uint32be:crc;
}
```

### Tree Transformation

```fmpl
grammar ast::optimizer <: base::tree {
    -- Constant folding: (+ 1 2) => 3
    add = [:add const:a const:b] => a + b;
    const = :int(n) => n;
}
```

---

## Runtime API

See [`runtime.rs`](../fmpl-core/src/grammar/runtime.rs) for full API.

### Convenience Functions

```rust
// Parse text with named grammar
parse("123", &registry, "base::parser", "integer")?;

// Parse ensuring full input consumed
parse_full("123", &registry, "base::parser", "integer")?;

// Parse any value (polymorphic)
apply_grammar_to_value(Value::String("hello".into()), &grammar, &registry, "word")?;
```

### PegRuntime (runtime.rs:32)

Generic runtime over input types:

```rust
let mut runtime = text_runtime(text, &registry, grammar);
let result = runtime.parse("rule_name")?;

// With action evaluator
let mut runtime = runtime.with_action_evaluator(evaluator);
```

---

## Module Structure

```
fmpl-core/src/grammar/
├── mod.rs          # Grammar, Rule, Pattern types, GrammarRegistry
├── parser.rs       # Grammar definition syntax parser
├── runtime.rs      # PEG runtime with memoization and backtracking
├── trampoline.rs   # Stack-safe recursion via trampolining (2.4K lines)
├── input.rs        # Input sources (string, list)
├── stream_input.rs # Streaming input with Fjall overflow
├── incremental.rs  # ParseState/ParseNext for suspension
└── driver.rs       # ParseDriver for async pipelines
```

### Trampoline (`trampoline.rs`)

The trampoline module provides stack-safe execution for deeply recursive grammar rules. Instead of using Rust's call stack (which overflows on deep recursion), pattern matching is converted to a continuation-passing style that loops in the trampoline.

Key features:
- **BacktrackEntry** — Choice points for Prolog-style backtracking on ambiguous matches
- **Stack-safe recursion** — Arbitrary grammar depth without stack overflow
- **Backtracking modes** — Depth-first search through alternatives

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [parse-stream.md](./parse-stream.md) — Combinator-based parsing alternative
- [async-streams.md](./async-streams.md) — Stream types for grammar pipelines
- [persistence.md](./persistence.md) — Fjall storage for durable state
- [backtracking-csp.md](./backtracking-csp.md) — Backtracking and CSP solving

---

## References

- [OMeta](https://tinlizzie.org/ometa/) — Original OMeta paper
- [Extensible Parsing for DSLs](http://www.tinlizzie.org/~awarth/papers/dls07.pdf) — Grammar inheritance
