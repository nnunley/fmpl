# Prolog-Style Backtracking and CSP Solving in FMPL

## Overview

FMPL now supports **Prolog-style backtracking** for constraint satisfaction problems (CSP) and logic programming. The backtracking stack allows exploring all alternatives in ambiguous grammars, similar to how Prolog handles choice points.

---

## Implementation

### BacktrackEntry Enum

```rust
enum BacktrackEntry {
    Single { position: usize, value: Value },  // Explicit continuation points
    Choice { position: usize, alternatives: Vec<(Value, usize)>, next_index: usize },
}
```

### Public API

| Method | Purpose |
|--------|---------|
| `has_more_choices()` | Check if unexplored alternatives exist |
| `backtrack()` | Get next alternative (depth-first) |
| `get_all_alternatives()` | Get all remaining (for parallel) |
| `push_target()` | Manually push continuation points |

---

## Example: SEND + MORE = MONEY

### The Puzzle

```
   SEND
+  MORE
-------
  MONEY
```

Each letter represents a unique digit 0-9:
- S, M ≠ 0 (leading digits)
- All letters represent distinct digits
- Arithmetic must be correct

### Solution

```
S=9 E=5 N=6 D=7 M=1 O=0 R=8 Y=2

  9567
+ 1085
-------
 10652
```

### Verification Code

```rust
let solution = Assignment {
    s: 9, e: 5, n: 6, d: 7,
    m: 1, o: 0, r: 8, y: 2,
};

assert!(solution.verify()); // Checks uniqueness, leading digits, arithmetic
```

---

## Usage Patterns

### 1. Grammar-Based Backtracking

When multiple patterns can match the same input, the runtime automatically creates choice points:

```rust
grammar.add_rule(
    "ambiguous".into(),
    Rule::new(Pattern::Choice(vec![
        Pattern::Literal("a".into()),        // Matches "a"
        Pattern::CharClass(vec![Range('a','z')]),  // Also matches "a"
    ])),
);

let mut runtime = PegRuntime::new(input, &registry, grammar)
    .with_backtracking_mode();

runtime.parse("ambiguous")?;

// Explore all alternatives
while runtime.has_more_choices() {
    let (value, pos) = runtime.backtrack().unwrap();
    // Process this alternative
}
```

### 2. Manual Search Control

For explicit CSP solving, use `push_target`:

```rust
let mut runtime = PegRuntime::new(...);

// Push intermediate states to explore later
runtime.push_target(Value::Int(state), position);

// Backtrack through all states
while let Some((value, pos)) = runtime.backtrack() {
    // Continue from this state
}
```

### 3. Parallel Iteration

Get all remaining alternatives for parallel processing:

```rust
let all = runtime.get_all_alternatives();

// Process in parallel
for (value, pos) in all {
    // Spawn new runtime from this position
}
```

---

## Key Insight

**Backtracking only occurs when multiple patterns CAN match the same input.**

For digit assignment:
- `Pattern::Literal("1")` only matches "1"
- `Pattern::CharClass(['0'-'9'])` matches any single digit
- Both match "1" → creates choice point ✅
- `Pattern::Literal("0")` doesn't match "1" → no choice ❌

---

## Test Files

| File | Purpose |
|------|---------|
| `test_prolog_solver.rs` | Backtracking API demonstrations |
| `test_send_more_money_solver.rs` | CSP structure and verification |
| `test_csp_solver.rs` | Solution verification and patterns |

---

## Memoization Sharing

All alternatives automatically share the same memoization table:

- **TextInput, BinaryInput, ValueInput**: Single shared `HashMap<(pos, rule), MemoEntry>`
- **StreamInput**: Per-position `HashMap<rule, MemoEntry>` with immutable `Rc<StreamPosition>`

This ensures packrat parsing efficiency even with backtracking.

---

## Future Work

1. **FMPL-Language Integration**: Expose backtracking API to FMPL code
2. **Constraint Predicates**: Add `when` guards for filtering alternatives
3. **Lazy Evaluation**: Defer alternative expansion until needed
4. **Parallel Backtracking**: Fork runtimes at choice points for parallel search

---

## References

- `src/grammar/runtime.rs` - Backtracking implementation
- `specs/pattern-matching.md` - Pattern matching details
- Prolog choice points and cut operator
