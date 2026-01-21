# Grammar System

OMeta-style extensible PEG grammars for FMPL.

**Location**: [`fmpl-core/src/grammar/`](../fmpl-core/src/grammar/)

---

## Overview

PEG-based parsing with grammar inheritance, packrat memoization, and semantic actions. Unlike traditional PEG parsers, this system can parse any stream of objects:

- **Text** — Character-by-character parsing
- **Binary** — Byte streams for protocols/file formats
- **Objects** — Lists/trees of values for AST transformation

For incremental/streaming parsing, see [streaming-grammar.md](./streaming-grammar.md).

---

## Grammar Definition

```fmpl
grammar mud::commands <: base::parser {
    verb = word:v &{ valid_verb(v) } => v
    command = "take" spaces noun:obj => %{action: :take, target: obj}
}

-- Apply grammar to input
"take sword" @ mud::commands.command
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
| `p:name` | `Bind(Box<Pattern>, SmolStr)` | Bind match to variable |
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
    value = string / number / object / array / true / false / null
    number = integer ("." integer)?
    string = '"' (!'"' any)* '"'
}
```

### Binary Parsing

```fmpl
grammar png::header <: base::binary {
    magic = byte(0x89) byte(0x50) byte(0x4E) byte(0x47)
    chunk = uint32be:len uint32be:type bytes(len):data uint32be:crc
}
```

### Tree Transformation

```fmpl
grammar ast::optimizer <: base::tree {
    -- Constant folding: (+ 1 2) => 3
    add = [:add const:a const:b] => a + b
    const = :int(n) => n
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

## Related Specs

- [streaming-grammar.md](./streaming-grammar.md) — Incremental parsing and durable suspension
- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [async-streams.md](./async-streams.md) — Stream types for grammar pipelines

---

## References

- [OMeta](https://tinlizzie.org/ometa/) — Original OMeta paper
- [Extensible Parsing for DSLs](http://www.tinlizzie.org/~awarth/papers/dls07.pdf) — Grammar inheritance
