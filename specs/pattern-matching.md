# Pattern Matching

Pattern matching with the `@` operator.

**Location**: [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs)

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

### List Patterns

```fmpl
list @ {
  []              => "empty"
  [x]             => "single: " + x
  [head | tail]   => "head: " + head + ", rest: " + tail
  [a, b, c]       => "three elements"
}
```

### Map Patterns

```fmpl
data @ {
  %{type: "user", name: n, age: a} => "User " + n + " is " + a
  %{type: "bot", id: i}            => "Bot #" + i
  %{}                              => "empty map"
}
```

### Literal Patterns

```fmpl
value @ {
  42        => "the answer"
  :ok       => "success"
  "hello"   => "greeting"
  true      => "affirmative"
  null      => "nothing"
}
```

---

## Guards

Add conditions to patterns:

```fmpl
value @ {
  n &{ n > 0 }  => "positive"
  n &{ n < 0 }  => "negative"
  _             => "zero"
}
```

Combined with destructuring:

```fmpl
result @ {
  %{status: s, data: d} &{ s == 200 } => process(d)
  %{status: s, error: e} &{ s >= 400 } => handle_error(e)
}
```

---

## Binding in Patterns

Bind matched values to names:

```fmpl
-- Bind entire match
input @ {
  %{nested: inner}:whole => use_both(whole, inner)
}

-- Bind in lists
list @ {
  [first, second | rest]:all => ...
}
```

---

## Nested Patterns

Patterns can nest arbitrarily:

```fmpl
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
    | %{tool: t, args: a} => execute(t, a)  -- map pattern
    | [head | tail]       => process(head)   -- list pattern
    | :done               => finish()        -- symbol pattern
}
```

---

## Exhaustiveness

Patterns are tried in order. Use `_` for catch-all:

```fmpl
value @ {
  %{type: "a"} => ...
  %{type: "b"} => ...
  _            => default()  -- catches everything else
}
```

Without `_`, unmatched values cause runtime errors.

---

## Pattern Types Summary

| Pattern | Matches | Example |
|---------|---------|---------|
| `_` | Anything | `_ => default()` |
| Literal | Exact value | `42 => ...` |
| `:symbol` | Symbol | `:ok => ...` |
| `name` | Bind to name | `x => use(x)` |
| `%{k: v}` | Map with key | `%{id: i} => ...` |
| `[...]` | List | `[a, b] => ...` |
| `[h \| t]` | Head/tail | `[first \| rest] => ...` |
| `p &{ guard }` | Pattern with guard | `n &{ n > 0 } => ...` |
| `p:name` | Bind match to name | `%{...}:whole => ...` |

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — Grammar patterns
- [language-guide.md](../docs/design/language-guide.md) — Full syntax
- [vm.md](./vm.md) — VM execution
