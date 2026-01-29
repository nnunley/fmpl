# CSP Solving and FMPL Backtracking

## What We Actually Built

### FMPL Grammar Backtracking (Rust Level)

We implemented **Prolog-style backtracking** in the grammar runtime (`src/grammar/runtime.rs`):

```rust
enum BacktrackEntry {
    Single { position: usize, value: Value },     // Manual continuation points
    Choice { position: usize, alternatives: Vec<(Value, usize)>, next_index: usize },
}
```

**API:**
- `has_more_choices()` - Check for unexplored alternatives
- `backtrack()` - Get next alternative (depth-first)
- `get_all_alternatives()` - Get all remaining (for parallel)
- `push_target()` - Manual search control

**When backtracking occurs:**
When multiple grammar patterns CAN match the SAME input, the runtime creates choice points:

```rust
// Both match "a":
grammar.add_rule("ambig", Pattern::Choice(vec![
    Pattern::Literal("a"),        // Matches "a"
    Pattern::CharClass(['a'-'z']), // Also matches "a"
]));
```

### SEND + MORE = MONEY Solution

**Solution:** `S=9 E=5 N=6 D=7 M=1 O=0 R=8 Y=2`

```
  9567
+ 1085
-------
  10652
```

### Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Backtracking stack | ✅ Complete | Rust-level API |
| Choice pattern alternatives | ✅ Complete | Automatic on ambiguous matches |
| Shared memoization | ✅ Complete | All branches share cache |
| CSP solver | ✅ Working | Rust implementation (`test_send_more_money_full.rs`) |
| **FMPL-language CSP** | ⚠️ Partial | Needs builtins to expose API to FMPL code |

### Why Not Pure FMPL?

The backtracking system is designed for **ambiguous grammars**, not general CSP solving:

1. **Grammar ambiguities** create choice points automatically
2. **CSP constraints** require semantic actions (when guards)
3. **Search control** needs FMPL builtins for `backtrack()`, `push_target()`

### Example: Using Backtracking

```rust
let input = TextInput::new("a");
let mut runtime = PegRuntime::new(input, &registry, grammar)
    .with_backtracking_mode();

runtime.parse("ambiguous_rule")?;

while runtime.has_more_choices() {
    if let Some((value, pos)) = runtime.backtrack() {
        // Process this alternative
    }
}
```

### Test Files

- `test_prolog_solver.rs` (6 tests) - Backtracking patterns
- `test_send_more_money_full.rs` (3 tests) - CSP solver implementation
- `send_more_money.fmpl` - FMPL demo (needs builtins for full solving)

### Next Steps for Full FMPL CSP

To solve CSPs entirely in FMPL:

1. **Expose backtracking API as builtins:**
   ```fmpl
   let results = grammar.parse_with_backtracking(input, rule)
   while results.has_next() {
       let (value, pos) = results.backtrack()
       # Process alternative
   }
   ```

2. **Add constraint predicates:**
   ```fmpl
   grammar CSP {
       solution = assign_s:s when s != 0
                assign_m:m when m != 0 && m != s
                ...
   }
   ```

3. **Stream-based iteration:**
   ```fmpl
   input @ grammar.rule |> stream_alternatives
       |> filter constraints
       |> take 1
   ```

The **backtracking infrastructure is complete** at the Rust level. Full FMPL-language CSP solving requires exposing this through builtins and language features.
