# Pattern Matching

Pattern matching with the `@` operator.

**Location**: [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs)

**Key files**:
- `ast.rs:88-111` — Pattern enum (value patterns)
- `ast.rs:113-119` — MatchCase with guard
- `parser.rs:250-291` — `@` operator parsing
- `parser.rs:1043-1061` — Match case parsing with `when` guards
- `parser.rs:1125-1207` — Pattern parsing
- `compiler.rs:503-556` — Match expression compilation
- `vm.rs:579-612` — Pattern execution (ExtractMapKey, ExtractListIndex)

---

## Overview

The `@` operator provides pattern matching on values and grammar application:

- **Grammar application** — Parse input with grammar rules
- **Value matching** — Destructure and match values
- **Control flow** — Branch based on patterns

---

## Grammar Application

Apply a grammar rule to input:

```fmpl
"take sword" @ mud::parser.command
-- Returns: %{action: :take, target: "sword"}

file_bytes @ png::header.magic
-- Returns: matched value or fails
```

---

## Value Matching

Match and destructure values:

```fmpl
result @ {
  %{tool: t, args: a} => execute_tool(t, a)
  %{text: t}          => emit_text(t)
  %{error: e}         => handle_error(e)
  _                   => default_case()
}
```

✅ **IMPLEMENTED**: Map and list patterns in `@` match expressions are now fully implemented. All pattern types work correctly in `@` expressions.

### List Patterns

```fmpl
-- ✅ These patterns are fully implemented in @ expressions
list @ {
  []              => "empty"
  [x]             => "single: " + x
  [head | tail]   => "head: " + head + ", rest: " + tail
  [a, b, c]       => "three elements"
}
```

### Map Patterns

```fmpl
-- ✅ These patterns are fully implemented in @ expressions
data @ {
  %{type: "user", name: n, age: a} => "User " + n + " is " + a
  %{type: "bot", id: i}            => "Bot #" + i
  %{}                              => "empty map"
}
```

### Literal Patterns

**✅ FULLY IMPLEMENTED**: Literal values in patterns act as guards, matching only when equal.

```fmpl
-- Integer literals
%{code: 404} @ {
  %{code: 404} => "not_found"
  _            => "found"
}

-- String literals
%{type: "user"} @ {
  %{type: "user"} => "user_type"
  _               => "other"
}

-- Boolean literals (note: true/false need proper parsing context)
%{active: true} @ {
  %{active: true} => "enabled"
  _                 => "disabled"
}

-- Mix literals and bindings
%{status: 200, body: msg} @ {
  %{status: 200, body: _:content} => content
}
```

**Key Implementation Details**:
- Literal values are stored in the constant pool as `Value` types (Int, String, Bool, etc.)
- The `MatchMap` instruction uses `MapValuePattern::MatchLiteral(const_idx)` to compare values
- Comparison uses `Value`'s `PartialEq` implementation for type-safe matching
- Supports all primitive types: integers, floats, strings, booleans, symbols, null

**Future Work**:
- Wildcard keys: `%{_: value} @ { %{_: literal} => ... }` (VM ready, parser needs update)
- Complex nested patterns in map values

---

## Guards

Add conditions to patterns using `when` keyword (`parser.rs:1046`):

```fmpl
value @ {
  n when n > 0  => "positive"
  n when n < 0  => "negative"
  _             => "zero"
}
```

Combined with destructuring:

```fmpl
-- ✅ Map patterns with guards are fully implemented in @ expressions
result @ {
  %{status: s, data: d} when s == 200  => process(d)
  %{status: s, error: e} when s >= 400 => handle_error(e)
}
```

**Note**: Map patterns with guards work directly in `@` expressions - no workaround needed.

Note: The `&{ condition }` syntax is for grammar predicates only (`grammar/parser.rs:248`).

---

## Binding in Patterns

Bind matched values to names using `as` (`parser.rs:1199-1204`):

```fmpl
-- ✅ Map and list patterns with 'as' binding are fully implemented
-- Bind entire match
input @ {
  %{nested: inner} as whole => use_both(whole, inner)
}

-- Bind in lists
list @ {
  [first, second | rest] as all => ...
}
```

---

## Nested Patterns

Patterns can nest arbitrarily:

```fmpl
-- ✅ Nested map and list patterns are fully implemented in @ expressions
data @ {
  %{
    user: %{name: n, prefs: %{theme: t}},
    items: [first | _]
  } => "User " + n + " with theme " + t + " has " + first
}
```

---

## With Async Streams

Pattern match on async results:

```fmpl
-- ✅ Map patterns with async streams are fully implemented
<- http.get(url) @ {
  %{status: 200, body: b} => parse_json(b)
  %{status: 404}          => not_found()
  %{error: e}             => handle_error(e)
}
```

---

## In Grammar Rules

Patterns appear in grammar semantic actions:

```fmpl
grammar ToolAgent <: base::tree {
  turn = message:m => <- llm(m) |> tool_output

  tool_output =
    | %{tool: t, args: a} => execute(t, a)  -- map pattern in grammar rule
    | [head | tail]       => process(head)   -- list pattern in grammar rule
    | :done               => finish()        -- symbol pattern
}
```

**Note**: Patterns in grammar semantic actions (after `=>`) use a different code path than `@` match expressions and may have different capabilities.

---

## Exhaustiveness

Patterns are tried in order. Use `_` for catch-all:

```fmpl
-- ✅ Map patterns in @ expressions are fully implemented
value @ {
  %{type: "a"} => ...
  %{type: "b"} => ...
  _            => default()  -- catches everything else
}
```

Without `_`, unmatched values cause runtime errors.

**Current Working Example** (using only `Var` and `Wildcard` patterns):
```fmpl
value @ {
  x when x > 0 => "positive"
  x when x < 0 => "negative"
  _            => "zero"
}
```

---

## Pattern Types Summary

| Pattern | Matches | Example | Status |
|---------|---------|---------|--------|
| `_` | Anything | `_ => default()` | Implemented ✅ |
| Literal | Exact value | `42 => ...` | Parsed, not compiled |
| `:symbol` | Symbol | `:ok => ...` | Parsed, not compiled |
| `name` | Bind to name | `x => use(x)` | Implemented ✅ |
| `%{k: v}` | Map with key | `%{id: i} => ...` | Implemented ✅ |
| `[...]` | List | `[a, b] => ...` | Implemented ✅ |
| `[h \| t]` | Head/tail | `[first \| rest] => ...` | Implemented ✅ |
| `p when g` | Pattern with guard | `n when n > 0 => ...` | Implemented ✅ |
| `p as name` | Bind match to name | `%{...} as whole => ...` | Implemented ✅ |

**✅ All pattern types are now supported in `@` match expressions**, including map patterns, list patterns, head/tail patterns, patterns with guards, and as-patterns.

---

## Implementation Status

| Feature | Parsing | Compilation | Execution |
|---------|---------|-------------|-----------|
| Var patterns | ✓ | ✓ | ✓ |
| Wildcard patterns | ✓ | ✓ | ✓ |
| Guards (`when`) | ✓ | ✓ | ✓ |
| Let map destructure | ✓ | ✓ | ✓ |
| Let list destructure | ✓ | ✓ (fixed-length) | ✓ |
| **Map patterns in @** | ✓ | ✓ | ✓ |
| **List patterns in @** | ✓ | ✓ | ✓ |
| **Head/tail lists in @** | ✓ | ✓ | ✓ |
| **As-patterns in @** | ✓ | ✓ | ✓ |
| **Literal patterns** | ✓ | ✓ | ✓ |
| **Symbol patterns** | ✓ | ✓ | ✓ |
| **Constructor patterns** | ✓ | — | — |

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — Grammar patterns (separate system)
- [vm.md](./vm.md) — VM execution
