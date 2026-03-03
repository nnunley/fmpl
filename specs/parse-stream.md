# ParseStream

Unified stream type for grammar-style parsing with combinators and packrat memoization.

**Status**: Complete
**Location**: [fmpl-core/src/parse_stream.rs](../fmpl-core/src/parse_stream.rs), [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs)

---

## Overview

ParseStream provides a lightweight, imperative parsing API that wraps any iterable `Value` (String, List, Tagged) with:

- **Position tracking** — Current offset into the input
- **Checkpoint/restore** — Backtracking support for choice and lookahead
- **Packrat memoization** — Per-position memo table for `apply()`, with left recursion detection
- **Stream combinators** — `choice`, `star`, `plus`, `seq`, `not`, `lookahead`, `optional` as VM builtins

This complements the OMeta-style grammar system (`grammar/`) by providing a simpler, function-based parsing model where parse rules are ordinary FMPL lambdas.

---

## Creating a ParseStream

```fmpl
let s = stream::new("hello world")   -- From string
let s = stream::new([1, 2, 3])       -- From list
```

The parser compiles `stream::new(...)` to `__builtin_stream.new(...)`.

---

## ParseStream Methods

Methods called directly on a ParseStream value:

| Method | Returns | Description |
|--------|---------|-------------|
| `s.head()` | Value | Current element without advancing. Returns `null` at end. |
| `s.position()` | Int | Current byte/index position |
| `s.advance(n)` | Null | Move forward by `n` items (characters for strings, elements for lists) |
| `s.checkpoint()` | Int | Save current position as an integer |
| `s.restore(cp)` | Null | Restore to a previously saved checkpoint position |
| `s.apply(rule)` | Value | Call `rule(stream)` with packrat memoization |

### String advancement

For strings, `advance(n)` advances by `n` **characters** (not bytes), correctly handling multi-byte UTF-8.

### End of input

`head()` returns `null` when at the end of input. For strings, end-of-input is when `position >= len`. For lists, when `position >= items.len()`.

---

## Stream Combinators

Combinators are module-level functions compiled from `stream::name(...)` to `__builtin_stream.name(...)`. All combinators that accept a `rule` expect a lambda `\s -> result` that takes a ParseStream and returns a value on success or raises a parse failure on error.

### `stream::match_char(stream, char)`

Match a specific single character. Advances on success, raises `ParseFailed` on mismatch.

```fmpl
let s = stream::new("abc")
stream::match_char(s, "a")   -- => "a", position advances
stream::match_char(s, "x")   -- raises ParseFailed
```

### `stream::match_class(stream, class)`

Match a character against a character class specification. Supports ranges (`a-z`) and single characters.

```fmpl
let s = stream::new("3x")
stream::match_class(s, "0-9")     -- => "3"
stream::match_class(s, "a-zA-Z")  -- => "x"
```

Class syntax: `"a-z"`, `"0-9"`, `"a-zA-Z0-9_"` — ranges and individual characters can be combined.

### `stream::fail(message)`

Explicitly fail the parse with a message. Does not require a stream argument.

```fmpl
stream::fail("unexpected token")  -- raises ParseFailed
```

### `stream::choice(stream, [rule1, rule2, ...])`

Try each alternative in order. Automatically restores position before each attempt. Returns the first successful result.

```fmpl
let digit = \s stream::match_class(s, "0-9")
let letter = \s stream::match_class(s, "a-zA-Z")
let s = stream::new("x")
stream::choice(s, [digit, letter])  -- => "x" (letter matched)
```

If all alternatives fail, position is restored to the original and `ParseFailed` is raised.

### `stream::star(stream, rule)`

Zero-or-more repetition. Returns a list of matched results. Stops on first failure (restoring position for the failed attempt). Protected against infinite loops from zero-length matches.

```fmpl
let digit = \s stream::match_class(s, "0-9")
let s = stream::new("123abc")
stream::star(s, digit)  -- => ["1", "2", "3"]
```

### `stream::plus(stream, rule)`

One-or-more repetition. Like `star` but raises `ParseFailed` if no matches occur.

```fmpl
let digit = \s stream::match_class(s, "0-9")
let s = stream::new("abc")
stream::plus(s, digit)  -- raises ParseFailed: "expected at least one match"
```

### `stream::seq(stream, [rule1, rule2, ...])`

Run all rules in sequence. Returns a list of results. If any rule fails, restores position to before the sequence started.

```fmpl
let a = \s stream::match_char(s, "a")
let b = \s stream::match_char(s, "b")
let s = stream::new("abc")
stream::seq(s, [a, b])  -- => ["a", "b"]
```

### `stream::not(stream, rule)`

Negative lookahead. Succeeds (returning `null`) if the rule **fails**. Fails if the rule **succeeds**. Does not consume input in either case.

```fmpl
let digit = \s stream::match_class(s, "0-9")
let s = stream::new("abc")
stream::not(s, digit)  -- => null (digit failed, so not succeeds)
```

### `stream::lookahead(stream, rule)`

Positive lookahead. Runs the rule and returns its result on success, but **restores position** afterward. Does not consume input.

```fmpl
let a = \s stream::match_char(s, "a")
let s = stream::new("abc")
stream::lookahead(s, a)  -- => "a" (but position stays at 0)
```

### `stream::optional(stream, rule)`

Zero-or-one match. Returns the matched value on success, or `null` on failure. Restores position on failure.

```fmpl
let sign = \s stream::match_char(s, "-")
let s = stream::new("42")
stream::optional(s, sign)  -- => null (no minus sign)
```

---

## Packrat Memoization via `apply()`

The `apply()` method provides packrat parsing — it caches the result of calling a rule at a given position. Subsequent calls to the same rule at the same position return the cached result without re-executing.

```fmpl
let digit = \s stream::match_class(s, "0-9")
let s = stream::new("123")
s.apply(digit)  -- Calls digit(s), caches result at position 0
s.apply(digit)  -- Returns cached result from position 1 (or calls if new position)
```

### Memo key

Each memo entry is keyed by `(position, rule_id)`:
- **position**: Current byte/index offset in the stream
- **rule_id**: Identity hash of the rule value — uses pointer identity for `Lambda` and `Partial` values (fast, stable), falls back to `Debug`-format hashing for other types

### Left recursion detection

If `apply()` encounters a rule that is already `InProgress` at the current position, it immediately raises `ParseFailed` with "left recursion detected" rather than looping forever.

### Memo entry states

```rust
pub enum MemoEntry {
    InProgress,                       // Left recursion guard
    Done(Option<Value>, usize),       // Result + end position
}
```

---

## Key Types

### ParseStream (`parse_stream.rs`)

```rust
pub struct ParseStream {
    source: Value,                            // Input being parsed
    position: usize,                          // Current position
    memo: HashMap<MemoKey, MemoEntry>,        // Packrat memo table
}
```

### Checkpoint

```rust
pub struct Checkpoint {
    pub position: usize,
}
```

### Value representation

```rust
pub enum Value {
    // ...
    ParseStream(Arc<Mutex<ParseStream>>),     // Shared, mutable parse state
    // ...
}
```

---

## Error Handling

All combinators use `Error::ParseFailed { position, message }` for parse failures. Non-parse errors (type errors, arity errors) use `Error::Runtime` and propagate without being caught by combinators like `choice` or `star`.

This distinction is critical: `choice` catches `ParseFailed` to try the next alternative, but lets runtime errors propagate immediately.

---

## Examples

### Integer parser

```fmpl
let digit = \s stream::match_class(s, "0-9")
let integer = \s {
  let digits = stream::plus(s, digit)
  digits |> fold("", \acc, d acc + d)
}

let s = stream::new("42 rest")
integer(s)  -- => "42"
```

### Identifier parser

```fmpl
let letter = \s stream::match_class(s, "a-zA-Z_")
let alnum = \s stream::match_class(s, "a-zA-Z0-9_")
let ident = \s {
  let first = letter(s)
  let rest = stream::star(s, alnum)
  first + (rest |> fold("", \acc, c acc + c))
}
```

### Choice with backtracking

```fmpl
let keyword = \s {
  stream::seq(s, [
    \s stream::match_char(s, "i"),
    \s stream::match_char(s, "f")
  ])
}
let ident = \s stream::plus(s, \s stream::match_class(s, "a-z"))

-- keyword tried first, backtracks on failure
let token = \s stream::choice(s, [keyword, ident])
```

---

## Relationship to Grammar System

| Feature | ParseStream | Grammar System (`grammar/`) |
|---------|------------|----------------------------|
| Parse rules | FMPL lambdas | Named grammar rules with pattern syntax |
| Memoization | `apply()` method | Built-in packrat via `PegRuntime` |
| Backtracking | `checkpoint()`/`restore()` via combinators | Automatic via `StreamPosition` cons-cells |
| Inheritance | Manual (compose lambdas) | Grammar inheritance with `<:` |
| Semantic actions | Return values directly | `=> expr` syntax in rules |
| Input types | String, List, Tagged, any Value | Text, Binary, Object streams |

ParseStream is best for:
- **Quick parsers** that don't need grammar inheritance
- **Programmatic construction** of parsers at runtime
- **Testing** parse logic with simple lambdas

The grammar system is best for:
- **Complex grammars** with inheritance and overriding
- **Binary/protocol parsing** with typed patterns
- **Incremental/streaming** parsing with durable state

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — OMeta-style PEG grammars
- [async-streams.md](./async-streams.md) — Async streams with pipe operator
- [vm.md](./vm.md) — VM execution and builtins
