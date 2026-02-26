# Stream-Based Grammar Compilation Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace 37 specialized grammar VM instructions with 8 stream primitives. Grammar rules compile to functions that call stream methods. Combinators become control flow.

**Architecture:** A unified Stream value type provides `head()`, `advance()`, `checkpoint()`, `restore()`, `position()`, `apply()`, `push()`, and `pop()`. Grammar rules compile to lambdas taking a stream and returning a value. `apply()` wraps each rule call with packrat memoization. Combinators like `seq`, `choice`, `star`, and `plus` compile to loops and branches over primitives — no special instructions needed.

**Key constraint:** The stream must lower cleanly to any VM target (interpreter, ORC JIT, execution tape). Each primitive maps to one or two machine-level operations.

---

## Core Model

A stream is a stream. There is no separate "parse stream" — the same type handles text parsing, list processing, pattern matching, and async event streams.

The stream works over any iterable value type:

| Input | `head()` returns |
|-------|-----------------|
| String | Next character (as single-char String) |
| List | Next element |
| Tagged value | The value itself (single-element) |
| AsyncStream | Next event as it arrives |

### Primitives (8 total, compiler intrinsics)

| Method | VM Instruction | JIT Lowering |
|--------|---------------|--------------|
| `head()` | `StreamHead` | load from buffer[position] |
| `advance(n)` | `StreamAdvance` | position += n |
| `checkpoint()` | `StreamCheckpoint` | save position to local |
| `restore(cp)` | `StreamRestore` | position = saved |
| `position()` | `StreamPosition` | read position field |
| `apply(rule)` | `StreamApply` | memo_check -> call -> memo_store |
| `push(value)` | `StreamPush` | push input frame (OMeta tree descent) |
| `pop()` | `StreamPop` | pop input frame |

All names — rule names, property names, method names — are symbols (`SmolStr`, interned). Memo keys and method dispatch use symbol comparison.

### What disappears

The 37 grammar instruction variants (`MatchChar`, `MatchSeq`, `MatchStarCharClass`, `ApplyRule`, etc.) are replaced by control flow that uses these 8 primitives. For example:

`match_class("0-9")` compiles to:
```
let c = s.head()
if c >= '0' && c <= '9' { s.advance(1); c }
else { fail }
```

`star(rule)` compiles to:
```
let results = []
loop {
  let cp = s.checkpoint()
  match s.apply(rule) {
    :ok(v) => results.push(v)
    :fail => { s.restore(cp); break }
  }
}
results
```

---

## Grammar Compilation

Grammar definitions compile their rules to lambdas. The grammar becomes a map of symbols to functions.

```fmpl
grammar G {
  digit = [0-9]:d => int(d)
  number = digit+:ds => ds.fold(0, \acc \d acc * 10 + d)
}
```

Compiles to:

```fmpl
let G = %{
  :digit: \s {
    let d = s.head()
    if d >= '0' && d <= '9' { s.advance(1); int(d) }
    else { fail }
  },
  :number: \s {
    let ds = []
    loop {
      let cp = s.checkpoint()
      match s.apply(G[:digit]) {
        :ok(v) => ds.push(v)
        :fail => { s.restore(cp); break }
      }
    }
    if ds.len() == 0 { fail }
    ds.fold(0, \acc \d acc * 10 + d)
  }
}
```

The `@` operator wires it:

```fmpl
"123" @ G.number
// becomes:
let s = stream("123")
s.apply(G[:number])
```

Inheritance (`grammar Child <: Parent`) merges maps — child rules shadow parent rules. `<super.rule>` calls the parent's version directly.

---

## Memoization via `apply()`

`s.apply(rule)` is the memoization boundary. Direct calls bypass it. This is the standard packrat contract:

1. Compute memo key: `(s.position(), rule_identity)`
2. Check memo — hit returns cached result, advances to cached end position
3. `InProgress` entry means left recursion — fail
4. Mark `InProgress`, call `rule(s)`, store result + end position
5. On failure, store failure in memo, propagate

External calls within semantic actions get the same treatment. A rule containing `<- curl.post(url, body)` executes once; backtracking that re-enters the rule returns the memoized result.

---

## Backtracking

Full backtracking, always. No cut operators in this iteration.

Buffer management: the stream buffers input for backtracking. Large buffers spill to Fjall (future work). Memo tables also spill to Fjall. A parse can suspend mid-stream, persist to Fjall, and resume later with full state.

Pipe model: `stream |> grammar.rule |> handler`. Each match flows downstream. Repetition is implicit in the pipe. Backtracking rewinds the pipeline speculatively.

Cut/commit may be added later as memory management optimization — they prune checkpoints and buffers but don't change correctness.

---

## Failure Model

Grammar rule failure is not an exception. It is a structured return — a rule either produces a value or produces a parse failure with position. The `choice` combinator catches failures and tries alternatives. Only when all alternatives fail does failure propagate.

---

## TDD Scope (Phase 1)

Build a minimal working stream that proves the architecture:

**Build:**
- Stream value type with 8 primitive methods
- `apply()` with packrat memoization
- Combinators as Rust builtins: `seq`, `choice`, `star`, `plus`, `optional`, `not`, `lookahead`, `match_char`, `match_class`
- Enough to parse `digit+ => number`

**Don't build yet:**
- Grammar syntax compilation to stream form
- Fjall buffer spilling
- OMeta tree descent (`push`/`pop`)
- Streaming/async pipes
- Replacing the existing 37 instructions

**Test progression:**
1. Create stream from string — `head()` returns first char
2. `advance(1)` — `head()` returns second char
3. `checkpoint()` / `restore()` round-trip
4. `apply(rule)` memoization — same `(position, rule)` returns cached result
5. Create stream from list — `head()` returns first element
6. `match_char` combinator — matches one char, fails on mismatch
7. `match_class` combinator — matches char range
8. `choice` — tries alternatives, backtracks on failure
9. `star` / `plus` — repetition with backtracking
10. `seq` — sequence of patterns
11. Full parse: `digit+ => int` parses `"123"` to `123`

---

## Files

| File | Purpose |
|------|---------|
| `fmpl-core/src/stream/parse_stream.rs` | Stream type with 8 primitives |
| `fmpl-core/src/stream/memo.rs` | Packrat memo table |
| `fmpl-core/src/stream/combinators.rs` | `seq`, `choice`, `star`, `plus`, etc. |
| `fmpl-core/tests/stream_parsing.rs` | Integration tests for TDD |
