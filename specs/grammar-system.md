# Grammar System

OMeta-style extensible PEG grammars for [Project Name TBD].

**Location**: [fmpl-core/src/grammar/](../fmpl-core/src/grammar/)

---

## Overview

PEG-based parsing with grammar inheritance, packrat memoization, and semantic actions. Unlike traditional PEG parsers, this system can parse any stream of objects:

- **Text** — Character-by-character parsing
- **Binary** — Byte streams for protocols/file formats
- **Objects** — Lists/trees of values for AST transformation

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

### Bindings and Actions

| Pattern | Description | Example |
|---------|-------------|---------|
| `p:name` | Bind to variable | `Bind(p, "name")` |
| `&{ expr }` | Semantic predicate | `Predicate(expr)` |
| `=> expr` | Semantic action | `Action(p, expr)` |

### Binary Patterns

| Pattern | Description |
|---------|-------------|
| `uint8`, `int8` | 8-bit integers |
| `uint16be`, `uint16le` | 16-bit integers (big/little endian) |
| `uint32be`, `uint32le` | 32-bit integers (big/little endian) |
| `bytes(n)` | Consume n bytes |

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

### Grammar

```rust
pub struct Grammar {
    pub name: SmolStr,
    pub parent: Option<SmolStr>,
    pub parent_grammar: Option<Arc<Grammar>>,
    pub rules: HashMap<SmolStr, Rule>,
}
```

### Rule

```rust
pub struct Rule {
    pub pattern: Pattern,
    pub action: Option<Expr>,
}
```

### Pattern

```rust
pub enum Pattern {
    Empty,
    Any,
    Char(char),
    Literal(SmolStr),
    CharClass(Vec<CharRange>),
    // ... 30+ variants for all pattern types
}
```

### GrammarRegistry

```rust
pub struct GrammarRegistry {
    grammars: HashMap<SmolStr, Arc<Grammar>>,
}
```

Provides `register()`, `get()`, and automatically registers built-in grammars.

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

## Related Specs

- [streaming-grammar.md](./streaming-grammar.md) — Incremental parsing for streams
- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [language-guide.md](../docs/design/language-guide.md) — DSL syntax

---

## References

- [OMeta](https://tinlizzie.org/ometa/) — Original OMeta paper
- [Extensible Parsing for DSLs](http://www.tinlizzie.org/~awarth/papers/dls07.pdf) — Grammar inheritance
