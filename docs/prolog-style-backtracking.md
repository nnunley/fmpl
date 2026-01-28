# Prolog-Style Backtracking for FMPL

## Overview

FMPL already has ALL the building blocks for Prolog-style backtracking through grammars and streams! We just need to connect the pieces.

## The Key Insight

**Pattern matching with `@` can send results to a sink**, and grammars can recursively explore a search space. The missing piece is making `@` send ALL successful matches to a sink, not just the first one.

## What We Already Have

### ✅ 1. Tree Grammars with Recursion

Named rules, `*` and `+` patterns, and recursive dependencies:

```fmpl
let g = grammar {
  perm = [x] => [[x]]
       | [x, ...rest] => {
           let subperms = g.perm(rest)  -- Recursive call
           subperms @ { [p] => [x, ...p] }
         }
}
```

### ✅ 2. `?{action}` Predicates

Grammar actions can fail, triggering backtracking:

```fmpl
let g = grammar {
  positive = ?{ [x] when x > 0 } => x  -- Fails if x <= 0
}
```

### ✅ 3. Ordered Choice (`|`)

Tries alternatives left-to-right, backtracks on failure:

```fmpl
let g = grammar {
  expr = "a" => 1 | "b" => 2 | "c" => 3
}
```

### ✅ 4. `when` Guards

Conditional pattern matching:

```fmpl
%{status: 200} @ {
  %{status: s} when s == 200 => "success"
  %{status: s} => "other"
}
```

### ✅ 5. Sinks for Stream Results

`stream.sink()` creates a sink that can receive stream values:

```fmpl
let sink = stream.sink()
-- sink can receive values asynchronously
```

### ✅ 6. Stream Operations

`StreamCollect` gathers stream results:

```fmpl
stream |> collect  -- Collects all values from stream into a list
```

## The Missing Piece

### ❌ `@` Only Returns First Match

Currently `value @ { pattern => result }` returns the FIRST successful match, not all matches.

**The fix**: Grammar apply should **always** return a STREAM of all matches.

## How It Works

### Current (First Match Only)

```fmpl
[1, 2, 3] @ g.perm
-- Returns: [[1,2,3]] (first permutation only)
```

### Proposed (Stream of All Matches)

```fmpl
-- ALL grammar applies return a stream of matches
let results = [1, 2, 3] @ g.perm
-- results is a stream: [[1,2,3], [1,3,2], [2,1,3], [2,3,1], [3,1,2], [3,2,1]]

-- Collect into list (or take first, filter, etc.)
let first = results |> take(1)  -- Take just first match
let all = results |> collect    -- Collect all matches
```

### Semantics

1. **Grammar apply always returns a stream** of all successful matches
2. **Each match automatically yields its result** to the stream
3. **`yield` allows multiple outputs** from a single match action
4. **Backtracking continues** until search space exhausted
5. **Stream ends** when no more matches possible

### `yield` Meaning

`yield` allows explicit injection of values into the output stream:

```fmpl
-- Without yield: result automatically sent to stream
[x, y] => [x, y]  -- One value per match

-- With yield: can send multiple values
[x, y] => {
    yield(x)
    yield(y)
    yield([x, y])
}  -- Three values per match
```

## Implementation: Grammar Apply Returns Stream

### Change 1: Grammar Apply Always Returns Stream

```rust
// AST - no sink parameter needed
GrammarApply {
    input: Box<Expr>,
    grammar: Box<Expr>,
    rule: SmolStr,
}

// VM - creates a stream and runs grammar backtracking
Instruction::GrammarApply {
    input: InstrIndex,
    grammar: InstrIndex,
    rule: SmolStr,
}
// Returns: Value::Stream (always)
```

### Change 2: ParseState Has Output Channel

```rust
pub struct ParseState {
    input_stack: Vec<InputFrame>,
    grammar: Option<Arc<Grammar>>,
    memo: HashMap<MemoKey, MemoEntry>,
    output_tx: Option<mpsc::Sender<Value>>,  // Sends to output stream
}
```

### Change 3: Match Results Auto-Yield

Each successful match automatically sends its result to the output stream:

```rust
// At end of match action, if no explicit yield:
//   Send the action's result to output_tx
// On explicit yield:
//   Send the yielded value to output_tx
```

### Change 4: Stream Driven Backtracking

```fmpl
let results = [1, 2, 3] @ g.perm
-- Creates stream, spawns backtracking task

results |> collect  -- Consumes stream, drives backtracking
```

The VM spawns a task that:
1. Executes grammar with full backtracking
2. Sends each match result to output channel
3. Closes channel when search exhausted

With memoization, failing naturally prunes branches (no need for explicit `cut`).

### Stream Control Primitives

Stream consumers can control backtracking:

```fmpl
let results = [1, 2, 3] @ g.perm

-- Take first n matches, then stop backtracking
let first_three = results |> take(3)

-- Take just first match, then stop
let first = results |> take(1)

-- Explicitly close/stop the stream
results |> close()
```

`take(n)`:
1. Consumes n elements from the stream
2. Then closes the stream, stopping further backtracking
3. The grammar search is abandoned early

`close()`:
1. Immediately closes the stream
2. Stops any ongoing backtracking
3. Useful for "find first" patterns

This is more efficient than collecting all matches when you only need a few.

## Complete Example

```fmpl
// Permutation grammar
let g = grammar {
  perm = [x] => [[x]]
       | [x, ...rest] => {
           let subperms = g.perm(rest)
           -- Each subperm is yielded, then we prepend x
           subperms @ {
               [p] => [x, ...p]
           }
         }
}

-- Grammar apply returns a STREAM of all matches
let results = [1, 2, 3] @ g.perm
-- results is a lazy stream of all 6 permutations

-- Collect all matches
let all_perms = results |> collect
-- all_perms = [[1,2,3], [1,3,2], [2,1,3], [2,3,1], [3,1,2], [3,2,1]]

-- Take first 3 matches, then stop backtracking
let first_three = [1, 2, 3] @ g.perm |> take(3) |> collect
-- first_three = [[1,2,3], [1,3,2], [2,1,3]]

-- Take just first match (stops after first)
let first = [1, 2, 3] @ g.perm |> take(1) |> collect
-- first = [[1,2,3]]

-- Find first match with specific property
let g = grammar {
  find_small = [x, ...rest] when x < 3 => [x, ...rest]
              | [_x, ...rest] => g.find_small(rest)
}
let result = [5, 1, 4, 2, 3] @ g.find_small |> take(1) |> collect
-- result = [[1, 4, 2, 3]]  -- Stops after finding first with x < 3
```

## Relation to Proto

| Proto | FMPL | Status |
|-------|------|--------|
| `?` (choice) | `\|` in grammars | ✅ |
| `commit` | Match success | ✅ |
| `fail` | `?{action}` or guard failure | ✅ |
| `spawn` | Streams (grammar apply returns stream) | ✅ |
| `yield` | `yield` expression (for multiple values per match) | ✅ |
| All solutions | Stream of all matches + memoization | ⏳ In Progress |

## Implementation Status

### Completed ✅
1. **`yield` expression** - Compiles and runs
2. **`YieldToSink` VM instruction** - Sends value to current channel
3. **`GrammarApply` AST** - Has `sink` field (will be removed for stream approach)
4. **`ParseState`** - Has `sink` field (will become `output_tx`)
5. **Parser** - `yield` keyword and `(sink)` syntax parsing

### Redesigning - Stream Approach ⏳
The user clarified that ALL grammar applies should return streams:

1. **Remove sink parameter** - GrammarApply doesn't need explicit sink
2. **Always return stream** - `@` always returns `Value::Stream`
3. **Auto-yield results** - Each match automatically sends to output
4. **`yield` for multiple values** - Allows multiple outputs per match
5. **Memoization** handles tabling/pruning (no need for explicit `cut`)

### New Implementation Plan
1. Change `GrammarApply` to always create and return a `Value::Stream`
2. Rename ParseState.sink to ParseState.output_tx
3. Each match action auto-sends result to output_tx
4. `yield` sends additional values to output_tx
5. Add `*` and `+` patterns for tree grammars
6. Memoization (already exists) handles tabling/pruning
7. Stream primitives: `take(n)` and `close()` for controlling search

## Minimal Implementation

The core pieces are in place:
- ✅ Backtracking (`|`, `?{action}`)
- ✅ Guards (`when`)
- ✅ Recursion in grammars
- ✅ Sinks (`stream.sink()`)
- ✅ Stream collection (`|> collect`)
- ✅ `yield` expression and `YieldToSink` instruction

What's needed:
- ⏳ Wire sink from `GrammarApply` to `parse_state`
- ⏳ Parser support for `@ grammar.rule(sink)` syntax

## What About Filtering?

Filtering is just failing with guards:

```fmpl
let g = grammar {
  positive_only = [x, ...rest] when x > 0 => yield(x)
                | [_, ...rest] => g.positive_only(rest)
                | [] => fail()
}

[1, -2, 3, -4] @ g.positive_only(sink)
-- Sends [1, 3] to sink
```

This is exactly like Prolog's:
```prolog
filter([X|X], [X|Rest]).
```