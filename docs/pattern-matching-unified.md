# Unified Pattern Matching in FMPL

FMPL provides a unified pattern matching system that works across multiple contexts with context-aware compilation. Patterns written in a single syntax can be used in let bindings (fast mode) or the `@` operator (full mode).

**Location**: [fmpl-core/src/pattern/mod.rs](../fmpl-core/src/pattern/mod.rs)

---

## Overview

The unified pattern system consolidates two previously separate implementations:
- **Value patterns** for `let` bindings and match expressions
- **Grammar patterns** for PEG-style parsing with backtracking

Both now use a single `Pattern` type with different compilation strategies based on context.

---

## Pattern Syntax Reference

### Basic Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `_` | Wildcard - matches anything, binds nothing | `let _ = value` |
| `x` | Variable binding - matches anything, binds to name | `let x = 42` |
| `42` | Integer literal - matches exact value | `42 => "found"` |
| `"hello"` | String literal - matches exact value | `"ok" => success()` |
| `true`/`false` | Boolean literal | `true => "yes"` |
| `null` | Null literal | `null => default()` |

### Structured Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `%{k: p}` | Map pattern - extracts key values | `%{name: n, age: a}` |
| `%{}` | Empty map pattern | `%{} => "empty"` |
| `[p1, p2]` | List pattern - exact length | `[a, b, c]` |
| `[h \| t]` | Head/tail pattern | `[head \| tail]` |
| `[p*]` | Repeat pattern - zero or more | `[item*]` |
| `[]` | Empty list pattern | `[] => "empty"` |
| `:Tag(p1, p2)` | Tagged/constructor pattern | `:Some(x)` |
| `:Tag` | Tag-only pattern (no children) | `:None => default()` |

### Grammar Patterns (Full Mode Only)

| Pattern | Description | Example |
|---------|-------------|---------|
| `'a'` | Character literal | `'a' 'b' 'c'` |
| `[a-z]` | Character class | `[a-zA-Z]` |
| `[^a-z]` | Negated character class | `[^0-9]` |
| `p1 p2` | Sequence - all must match in order | `digit+ '.' digit+` |
| `p1 \| p2` | Ordered choice - first match wins | `"hello" \| "world"` |
| `p*` | Zero or more repetitions | `letter*` |
| `p+` | One or more repetitions | `digit+` |
| `p?` | Optional - zero or one | `sign?` |
| `&p` | Positive lookahead - match without consuming | `&"end"` |
| `!p` | Negative lookahead - fail if matches | `!"error"` |
| `name: p` | Binding - bind match result to name | `digit+:value` |
| `<rule>` | Rule application | `<expr>` |

### Pattern Modifiers

| Modifier | Description | Example |
|----------|-------------|---------|
| `p when e` | Guard - pattern with predicate | `n when n > 0` |
| `p => e` | Action - pattern with transformation | `%{x: v} => v * 2` |
| `p as name` | Bind entire match to name | `%{...} as whole` |

---

## Let Bindings vs @ Operator

FMPL compiles patterns differently based on context:

### Let Bindings (Fast Mode)

In `let` bindings, patterns use **direct extraction** for optimal performance:

```fmpl
-- Map destructuring
let %{name: n, age: a} = %{name: "Alice", age: 30}
-- n = "Alice", a = 30

-- List destructuring
let [first, second | rest] = [1, 2, 3, 4, 5]
-- first = 1, second = 2, rest = [3, 4, 5]

-- Tagged value destructuring
let :Some(value) = :Some(42)
-- value = 42

-- Nested destructuring
let %{user: %{name: n}} = %{user: %{name: "Bob"}}
-- n = "Bob"
```

**Fast mode characteristics:**
- Direct extraction using `ExtractMapKey`, `ExtractListIndex`, `ExtractTaggedChild` instructions
- No backtracking
- No ordered choice (`|`)
- Guards must be checked manually after binding
- Pattern match failure causes runtime error

### @ Operator (Full Mode)

The `@` operator uses **full PEG matching** with backtracking:

```fmpl
-- Named grammar application
"hello world" @ Parser.command

-- Inline pattern block (anonymous grammar)
result @ {
  %{type: "move", dir: d} => move(d)
  %{type: "attack", target: t} => attack(t)
  %{type: "quit"} => exit()
  _ => continue()
}

-- With guards
value @ {
  n when n > 0 => "positive"
  n when n < 0 => "negative"
  _ => "zero"
}

-- Ordered choice with backtracking
input @ {
  "hello" " " "world" => greeting()
  "hello" name => greet(name)
  _ => unknown()
}
```

**Full mode characteristics:**
- Full PEG semantics with backtracking
- Ordered choice (`|`) tries alternatives in order
- Guards evaluated as predicates
- Actions transform matched values
- Pattern match failure backtracks to next alternative

---

## Compilation Modes Table

| Feature | Fast Mode (`let`) | Full Mode (`@`) |
|---------|-------------------|-----------------|
| Basic patterns (`_`, `x`, literals) | Yes | Yes |
| Map patterns (`%{k: p}`) | Yes | Yes |
| List patterns (`[a, b, c]`) | Yes | Yes |
| Head/tail (`[h \| t]`) | Yes | Yes |
| Tagged patterns (`:Tag(p)`) | Yes | Yes |
| Character patterns (`'a'`, `[a-z]`) | No | Yes |
| Sequence (`p1 p2`) | No | Yes |
| Ordered choice (`p1 \| p2`) | No | Yes |
| Repetition (`p*`, `p+`, `p?`) | No | Yes |
| Lookahead (`&p`, `!p`) | No | Yes |
| Guards (`when`) | Manual check after | Yes |
| Actions (`=>`) | No | Yes |
| Backtracking | No | Yes |
| Performance | Faster | More flexible |

### When to Use Each Mode

**Use `let` bindings (fast mode) when:**
- You know the value's structure matches the pattern
- You need maximum performance
- No alternatives or guards are needed

**Use `@` operator (full mode) when:**
- Multiple patterns need to be tried in order
- Guards filter which pattern applies
- Parsing text or structured data
- Pattern might not match (graceful failure)

---

## Polymorphic Stream Coercion

The `@` operator automatically coerces input values to appropriate stream types:

| Input Type | Stream Mode | Behavior |
|------------|-------------|----------|
| String | Chars | Each character becomes a stream element |
| List | Items | Each element becomes a stream element |
| Map | Once | Map wrapped as single stream element |
| Tagged | Once | Tagged value wrapped as single element |
| Other | Once | Value wrapped as single stream element |

### StreamMode Enum

```rust
pub enum StreamMode {
    Chars,   // String -> character stream
    Items,   // List -> element stream
    Once,    // Any value -> single-element stream
    Auto,    // Detect from input type at runtime
}
```

### Examples

```fmpl
-- String to character stream
"hello" @ {
  'h' 'e' 'l' 'l' 'o' => "matched"
}
-- Characters consumed one at a time

-- List to element stream
[1, 2, 3] @ {
  1 2 3 => "matched sequence"
}
-- Elements consumed one at a time

-- Map to single-element stream
%{x: 1, y: 2} @ {
  %{x: a, y: b} => a + b
}
-- Entire map matched as one element

-- Tagged to single-element stream
:Point(3, 4) @ {
  :Point(x, y) => sqrt(x*x + y*y)
}
-- Entire tagged value matched as one element
```

### Auto Detection

When using `StreamMode::Auto` (the default), the runtime detects input type:

```fmpl
-- All these work with @ operator
"text" @ Parser.rule       -- Auto detects Chars mode
[1, 2, 3] @ Parser.rule    -- Auto detects Items mode
%{k: v} @ Parser.rule      -- Auto detects Once mode
:Tag(x) @ Parser.rule      -- Auto detects Once mode
```

---

## Pattern Type Definition

The unified `Pattern` enum (from `fmpl-core/src/pattern/mod.rs`):

```rust
pub enum Pattern {
    // Basic patterns (fast mode compatible)
    Any,                           // _
    Var(SmolStr),                  // x
    Literal(LiteralValue),         // 42, "hello", true

    // Structured patterns (fast mode compatible)
    Map(Vec<(SmolStr, Pattern)>),  // %{k: p}
    List(ListPattern),             // [p1, p2] or [h | t]
    Tagged { tag: SmolStr, patterns: Vec<Pattern> },  // :Tag(p)

    // Grammar patterns (full mode only)
    Char(CharPattern),             // 'a', [a-z]
    Seq(Vec<Pattern>),             // p1 p2 p3
    Choice(Vec<Pattern>),          // p1 | p2 | p3
    Repeat { pattern: Box<Pattern>, kind: RepeatKind },  // p*, p+
    Optional(Box<Pattern>),        // p?
    Lookahead { pattern: Box<Pattern>, positive: bool }, // &p, !p

    // Modifiers
    Bind { name: SmolStr, pattern: Box<Pattern> },       // name: p
    Guard { pattern: Box<Pattern>, predicate: GuardPredicate },
    Action { pattern: Box<Pattern>, action: SmolStr },   // p => expr

    // Rule application
    ApplyRule(SmolStr),            // <rule>
}
```

### PatternMode

```rust
pub enum PatternMode {
    /// Fast path: direct extraction, no backtracking
    /// Uses ExtractMapKey, ExtractListIndex, ExtractTaggedChild
    Fast,

    /// Full path: grammar matching with backtracking
    /// Uses MatchSeq, MatchChoice, MatchGuard, etc.
    Full,
}
```

### Mode Detection

Patterns can report their recommended compilation mode:

```rust
impl Pattern {
    pub fn requires_full_mode(&self) -> bool {
        match self {
            Seq(_) | Choice(_) | Repeat { .. } => true,
            Lookahead { .. } | Guard { .. } | Action { .. } => true,
            Char(_) => true,
            List(ListPattern::Repeat { .. }) => true,
            _ => false,
        }
    }

    pub fn recommended_mode(&self) -> PatternMode {
        if self.requires_full_mode() {
            PatternMode::Full
        } else {
            PatternMode::Fast
        }
    }
}
```

---

## Examples

### Basic Let Destructuring

```fmpl
-- Map destructuring
let %{status: s, body: b} = response
if s == 200 { process(b) } else { handle_error(s) }

-- List destructuring
let [cmd, arg1, arg2] = args
execute(cmd, arg1, arg2)

-- Nested destructuring
let %{config: %{database: %{host: h, port: p}}} = settings
connect(h, p)
```

### Pattern Matching with @

```fmpl
-- HTTP response handling
response @ {
  %{status: 200, body: b} => parse_json(b)
  %{status: 404} => not_found()
  %{status: s} when s >= 500 => server_error(s)
  _ => unknown_response()
}

-- Option type handling
maybe_value @ {
  :Some(x) => x
  :None => default_value
}

-- Recursive data processing
tree @ {
  :Leaf(v) => v
  :Node(left, right) => tree_sum(left) + tree_sum(right)
}
```

### Grammar-Style Parsing

```fmpl
-- Parse integers
input @ {
  '-':sign digit+:digits => -parse_int(digits)
  digit+:digits => parse_int(digits)
}

-- Parse expressions
expr @ {
  term:left '+' expr:right => :Add(left, right)
  term:left '-' expr:right => :Sub(left, right)
  term
}

-- JSON-like parsing (simplified)
value @ {
  '{' pair*:pairs '}' => make_object(pairs)
  '[' value*:items ']' => make_array(items)
  string | number | 'true' | 'false' | 'null'
}
```

### Combined Let and @ Patterns

```fmpl
-- Process streamed messages
fn process_messages(stream) {
  stream |> each(|msg| {
    -- First destructure known fields
    let %{type: t, payload: p} = msg

    -- Then match on type with full patterns
    p @ {
      %{action: a, data: d} when t == "command" => execute(a, d)
      %{text: txt} when t == "chat" => display(txt)
      _ => log("unknown payload", p)
    }
  })
}
```

---

## Implementation Notes

### Instruction Mapping

**Fast Mode Instructions:**
- `ExtractMapKey { source, key }` - Extract value from map by key
- `ExtractListIndex { source, index }` - Extract list element by index
- `ExtractTaggedChild { source, index }` - Extract tagged value child
- `Bind { name, value }` - Bind value to variable name

**Full Mode Instructions:**
- `CoerceStream { value, mode }` - Convert input to stream
- `MatchSeq { patterns }` - Match sequence in order
- `MatchChoice { patterns }` - Try alternatives with backtracking
- `MatchGuard { pattern, predicate }` - Pattern with boolean guard
- `MatchAction { pattern, action }` - Transform matched value
- `MatchRepeat { pattern, kind }` - Match repetitions

### Error Handling

**Fast mode errors:**
- Pattern mismatch causes immediate runtime error
- No recovery or alternatives

**Full mode errors:**
- Mismatch triggers backtracking to previous choice point
- If all alternatives exhausted, returns match failure
- Can be handled with catch-all `_` pattern

---

## Related Specifications

- [pattern-matching.md](../specs/pattern-matching.md) - Original pattern matching spec (value patterns)
- [grammar-system.md](../specs/grammar-system.md) - Grammar system with PEG patterns
- [indexed-rpn-conversion.md](../specs/indexed-rpn-conversion.md) - VM instruction format
- [language-guide.md](./design/language-guide.md) - FMPL language overview
