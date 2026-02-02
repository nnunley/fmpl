# Grammar Optimizer

Optimizations for the FMPL grammar system inspired by Raku grammars and parser generator techniques.

**Location**: `fmpl-core/src/grammar/optimizer.rs` (to be created)

**Key Design Docs**:
- [grammar-system.md](./grammar-system.md) — Current PEG grammar implementation
- [indexed-rpn-conversion.md](./indexed-rpn-conversion.md) — Bytecode compilation target

---

## Overview

The current grammar implementation compiles patterns directly to bytecode without optimization. This spec describes an optimization pass that transforms Pattern AST before compilation, enabling:

1. **O(match length)** instead of O(alternatives × length) for literal choices
2. **Substring search** instead of character-by-character backtracking for `.*"literal"`
3. **Direct dispatch** instead of sequential try/fail for disjoint alternatives

These optimizations preserve PEG semantics while dramatically improving performance for common patterns.

---

## 1. Generalized Prefix Trie

### Problem

Choice patterns compile to sequential checkpoint/try/restore:

```fmpl
keyword = "function" | "for" | "finally" | "false" | "class" | "const" | "continue"
```

Current compilation: Try "function", fail, restore, try "for", fail, restore... O(7 × average_length).

### Solution

Build a prefix trie at compile time, walk it at runtime:

```
f → u → n → c → t → i → o → n → ACCEPT("function")
  → o → r → ACCEPT("for")
  → i → n → a → l → l → y → ACCEPT("finally")
  → a → l → s → e → ACCEPT("false")
c → l → a → s → s → ACCEPT("class")
  → o → n → s → t → ACCEPT("const")
       → t → i → n → u → e → ACCEPT("continue")
```

Runtime: Walk trie character-by-character. O(match_length).

### Generalization to Patterns

The trie isn't limited to characters. Each node can match any deterministic pattern:

```fmpl
statement = "if" expr "then" block
          | "if" expr "then" block "else" block
          | "while" expr "do" block
          | "for" ident "in" expr "do" block
          | ident "=" expr
          | ident "(" args ")"
```

Generalized trie:

```
"if" → expr → "then" → block → [ACCEPT(if-then), "else" → block → ACCEPT(if-then-else)]
"while" → expr → "do" → block → ACCEPT(while)
"for" → ident → "in" → expr → "do" → block → ACCEPT(for)
ident → "=" → expr → ACCEPT(assign)
      → "(" → args → ")" → ACCEPT(call)
```

### Data Structure

```rust
/// A node in the generalized prefix trie
pub enum TrieNode {
    /// Match a literal string, then continue to child
    Literal {
        value: SmolStr,
        child: Box<TrieNode>,
    },
    /// Match a character class, then continue to child
    CharClass {
        ranges: Vec<CharRange>,
        child: Box<TrieNode>,
    },
    /// Match a rule, then continue to child
    Rule {
        name: SmolStr,
        child: Box<TrieNode>,
    },
    /// Match any pattern, then continue to child
    Pattern {
        pattern: Pattern,
        child: Box<TrieNode>,
    },
    /// Branch point: try children based on first-set
    Branch {
        /// Children indexed by first-set for O(1) dispatch when disjoint
        children: Vec<(FirstSet, TrieNode)>,
        /// True if first-sets are disjoint (no backtracking needed)
        disjoint: bool,
    },
    /// Successful match with semantic action
    Accept {
        action: Option<Box<Expr>>,
        /// For longest-match: continue checking for longer alternatives
        continue_node: Option<Box<TrieNode>>,
    },
    /// No match possible from this node
    Reject,
}

/// What a pattern can start with
pub enum FirstSet {
    Chars(HashSet<char>),      // Specific characters
    CharClass(Vec<CharRange>), // Character ranges
    AnyChar,                   // `.` - matches any
    Rule(SmolStr),             // Named rule
    Epsilon,                   // Can match empty string
    // Combinations handled by HashSet<FirstSet> at branch points
}
```

### Trie Construction Algorithm

```rust
fn build_trie(alternatives: Vec<(Vec<Pattern>, Option<Expr>)>) -> TrieNode {
    if alternatives.is_empty() {
        return TrieNode::Reject;
    }

    // Group by first pattern element
    let mut groups: HashMap<PatternKey, Vec<_>> = HashMap::new();
    let mut accepting: Vec<Option<Expr>> = Vec::new();

    for (patterns, action) in alternatives {
        if patterns.is_empty() {
            accepting.push(action);
        } else {
            let (first, rest) = patterns.split_first();
            let key = pattern_key(first);
            groups.entry(key).or_default().push((first.clone(), rest.to_vec(), action));
        }
    }

    // Build branch node
    let children: Vec<_> = groups.into_iter().map(|(key, group)| {
        let first_set = compute_first_set(&key);
        let child = if group.len() == 1 && is_simple_pattern(&group[0].0) {
            // Single simple pattern: emit directly
            build_chain(group[0].0.clone(), build_trie(vec![(group[0].1.clone(), group[0].2.clone())]))
        } else {
            // Multiple alternatives or complex pattern: recurse
            let sub_alts: Vec<_> = group.into_iter()
                .map(|(_, rest, action)| (rest, action))
                .collect();
            build_trie(sub_alts)
        };
        (first_set, child)
    }).collect();

    let disjoint = are_first_sets_disjoint(&children);

    let branch = TrieNode::Branch { children, disjoint };

    // If some alternatives accept here, wrap with Accept
    if !accepting.is_empty() {
        TrieNode::Accept {
            action: accepting.pop(),
            continue_node: Some(Box::new(branch)),
        }
    } else {
        branch
    }
}
```

### Runtime Execution

```rust
fn match_trie(node: &TrieNode, input: &mut Input) -> Option<(Value, usize)> {
    match node {
        TrieNode::Literal { value, child } => {
            if input.match_literal(value) {
                match_trie(child, input)
            } else {
                None
            }
        }
        TrieNode::Branch { children, disjoint } => {
            if *disjoint {
                // O(1) dispatch: check first char/token against first-sets
                let first = input.peek();
                for (first_set, child) in children {
                    if first_set.contains(first) {
                        return match_trie(child, input);
                    }
                }
                None
            } else {
                // Overlapping first-sets: need backtracking
                let checkpoint = input.checkpoint();
                for (_, child) in children {
                    if let Some(result) = match_trie(child, input) {
                        return Some(result);
                    }
                    input.restore(checkpoint);
                }
                None
            }
        }
        TrieNode::Accept { action, continue_node } => {
            let pos = input.position();
            // Try for longer match if possible
            if let Some(cont) = continue_node {
                let checkpoint = input.checkpoint();
                if let Some(longer) = match_trie(cont, input) {
                    return Some(longer);
                }
                input.restore(checkpoint);
            }
            // Accept current match
            let value = action.as_ref()
                .map(|a| eval_action(a))
                .unwrap_or(Value::Null);
            Some((value, pos))
        }
        TrieNode::Reject => None,
        // ... other node types
    }
}
```

---

## 2. Skip-to-Literal Fusion

### Problem

The pattern `.*"hello"` doesn't work with greedy PEG semantics:

```fmpl
grammar Test { rule = .* "hello" }
"foo hello bar" @ Test.rule  -- FAILS: .* consumes everything
```

Backtracking-capable `.*` would be expensive and breaks memoization.

### Solution

Recognize `Seq([Star(Any), Literal(s)])` and fuse into substring search:

```rust
// Before optimization
Pattern::Seq(vec![
    Pattern::Star(Box::new(Pattern::Any)),
    Pattern::Literal("hello".into()),
])

// After optimization
Pattern::SkipToLiteral {
    literal: "hello".into(),
    include_literal: true,
}
```

### Runtime

```rust
Pattern::SkipToLiteral { literal, include_literal } => {
    // Use efficient substring search (Rust's str::find uses SIMD)
    if let Some(pos) = input.remaining().find(literal.as_str()) {
        let skipped = &input.remaining()[..pos];
        input.advance(pos);
        if *include_literal {
            input.advance(literal.len());
            Ok(ParseResult::Success(
                Value::String(format!("{}{}", skipped, literal).into()),
                input.position()
            ))
        } else {
            Ok(ParseResult::Success(
                Value::String(skipped.into()),
                input.position()
            ))
        }
    } else {
        Ok(ParseResult::Failure)
    }
}
```

### Extended Patterns

The fusion extends to related patterns:

| Pattern | Fused Form | Runtime |
|---------|------------|---------|
| `.*"lit"` | `SkipToLiteral` | `str::find` |
| `.*"lit".*` | `SkipToLiteral` + consume rest | `str::find` |
| `.+"lit"` | `SkipToLiteral` with min=1 | `str::find` + check |
| `[^x]*"x"` | `SkipToChar` | `str::find(char)` |
| `.*rule` | `SkipToRule` | scan + try rule |

### Memoization

`SkipToLiteral` is memoizable because at any position P, it deterministically finds the *first* occurrence of the literal. The result is a pure function of position.

---

## 3. Multi-Pattern Search (Aho-Corasick)

### Problem

Multiple skip-to-literal patterns:

```fmpl
detect = .*"error" | .*"warning" | .*"fatal"
```

Sequential substring search is O(n × patterns).

### Solution

Build Aho-Corasick automaton for simultaneous multi-pattern search:

```rust
Pattern::SkipToAnyLiteral {
    literals: vec!["error", "warning", "fatal"],
    automaton: AhoCorasick::new(&["error", "warning", "fatal"]),
}
```

Runtime finds first occurrence of any literal in single pass: O(n + m) where m is total pattern length.

### Integration

Use the `aho-corasick` crate:

```rust
use aho_corasick::AhoCorasick;

Pattern::SkipToAnyLiteral { literals, automaton } => {
    if let Some(mat) = automaton.find(input.remaining()) {
        let skipped = &input.remaining()[..mat.start()];
        let matched = &input.remaining()[mat.start()..mat.end()];
        let which = mat.pattern().as_usize();
        input.advance(mat.end());
        Ok(ParseResult::Success(
            Value::Tagged(
                literals[which].into(),
                Arc::new(vec![Value::String(skipped.into())])
            ),
            input.position()
        ))
    } else {
        Ok(ParseResult::Failure)
    }
}
```

---

## 4. Jump Table for Character Choice

### Problem

Single-character choices:

```fmpl
operator = '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!'
```

Sequential comparison is O(alternatives).

### Solution

Build jump table indexed by character:

```rust
Pattern::CharJumpTable {
    table: HashMap<char, usize>,  // char -> action index
    actions: Vec<Option<Expr>>,
    default: Option<Box<Pattern>>, // fallback for non-table chars
}
```

Runtime: O(1) lookup.

```rust
Pattern::CharJumpTable { table, actions, default } => {
    if let Some(c) = input.peek_char() {
        if let Some(&idx) = table.get(&c) {
            input.advance(1);
            let value = actions[idx].as_ref()
                .map(|a| eval_action(a))
                .unwrap_or(Value::String(c.to_string().into()));
            Ok(ParseResult::Success(value, input.position()))
        } else if let Some(fallback) = default {
            self.match_pattern(fallback, pos)
        } else {
            Ok(ParseResult::Failure)
        }
    } else {
        Ok(ParseResult::Failure)
    }
}
```

### Density Threshold

Only use jump table when alternatives are dense enough:

```rust
fn should_use_jump_table(chars: &[char]) -> bool {
    if chars.len() < 4 {
        return false; // Sequential is fine for few alternatives
    }
    let min = *chars.iter().min().unwrap() as usize;
    let max = *chars.iter().max().unwrap() as usize;
    let range = max - min + 1;
    let density = chars.len() as f64 / range as f64;
    density > 0.25 // At least 25% density
}
```

For sparse character sets, use sorted array + binary search instead.

---

## 5. First-Set Computation

### Algorithm

```rust
fn compute_first_set(pattern: &Pattern) -> FirstSet {
    match pattern {
        Pattern::Empty => FirstSet::Epsilon,
        Pattern::Any => FirstSet::AnyChar,
        Pattern::Char(c) => FirstSet::Chars(hashset![*c]),
        Pattern::Literal(s) => {
            if let Some(c) = s.chars().next() {
                FirstSet::Chars(hashset![c])
            } else {
                FirstSet::Epsilon
            }
        }
        Pattern::CharClass(ranges) => FirstSet::CharClass(ranges.clone()),
        Pattern::Rule(name) => FirstSet::Rule(name.clone()),
        Pattern::Seq(parts) => {
            // First non-epsilon element determines first-set
            for part in parts {
                let fs = compute_first_set(part);
                if !fs.contains_epsilon() {
                    return fs;
                }
                // If can be epsilon, include this first-set and continue
            }
            FirstSet::Epsilon
        }
        Pattern::Choice(alts) => {
            // Union of all alternatives' first-sets
            let mut combined = FirstSet::empty();
            for (alt, _) in alts {
                combined = combined.union(&compute_first_set(alt));
            }
            combined
        }
        Pattern::Star(_) | Pattern::Optional(_) => {
            // Can always match epsilon
            let inner = match pattern {
                Pattern::Star(p) | Pattern::Optional(p) => compute_first_set(p),
                _ => unreachable!(),
            };
            inner.with_epsilon()
        }
        Pattern::Plus(inner) => compute_first_set(inner),
        Pattern::Not(_) | Pattern::And(_) => {
            // Lookahead doesn't consume, check what follows
            FirstSet::AnyChar // Conservative: could be anything
        }
        // ... other patterns
    }
}
```

### Disjointness Check

```rust
fn are_disjoint(a: &FirstSet, b: &FirstSet) -> bool {
    match (a, b) {
        (FirstSet::Chars(ca), FirstSet::Chars(cb)) => ca.is_disjoint(cb),
        (FirstSet::Chars(c), FirstSet::CharClass(ranges)) |
        (FirstSet::CharClass(ranges), FirstSet::Chars(c)) => {
            !c.iter().any(|ch| ranges.iter().any(|r| r.contains(*ch)))
        }
        (FirstSet::CharClass(ra), FirstSet::CharClass(rb)) => {
            !ranges_overlap(ra, rb)
        }
        (FirstSet::AnyChar, _) | (_, FirstSet::AnyChar) => false,
        (FirstSet::Epsilon, FirstSet::Epsilon) => false,
        (FirstSet::Epsilon, _) | (_, FirstSet::Epsilon) => true,
        (FirstSet::Rule(a), FirstSet::Rule(b)) => a != b, // Conservative
        _ => false, // Conservative: assume overlap
    }
}
```

---

## 6. Optimizer Pipeline

### Phase 1: Pattern Analysis

```rust
pub struct PatternAnalysis {
    first_set: FirstSet,
    nullable: bool,          // Can match empty string
    is_literal: bool,        // Pure literal sequence
    is_char_choice: bool,    // Choice of single chars
    has_star_any: bool,      // Contains .*
    estimated_cost: usize,   // Heuristic for optimization priority
}

fn analyze_pattern(pattern: &Pattern) -> PatternAnalysis { ... }
```

### Phase 2: Optimization Transforms

```rust
pub fn optimize_pattern(pattern: Pattern, ctx: &mut OptContext) -> Pattern {
    // Bottom-up: optimize children first
    let pattern = pattern.map_children(|c| optimize_pattern(c, ctx));

    match &pattern {
        // Fuse .*"literal"
        Pattern::Seq(parts) if is_skip_to_literal(parts) => {
            let literal = extract_trailing_literal(parts);
            Pattern::SkipToLiteral { literal, include_literal: true }
        }

        // Build trie for literal choices
        Pattern::Choice(alts) if all_literals(alts) && alts.len() >= 4 => {
            let trie = build_literal_trie(alts);
            Pattern::Trie(Box::new(trie))
        }

        // Jump table for char choices
        Pattern::Choice(alts) if all_single_chars(alts) && alts.len() >= 4 => {
            let table = build_char_table(alts);
            Pattern::CharJumpTable(table)
        }

        // Generalized trie for pattern choices
        Pattern::Choice(alts) if should_build_pattern_trie(alts) => {
            let trie = build_pattern_trie(alts);
            Pattern::PatternTrie(Box::new(trie))
        }

        // Multi-pattern Aho-Corasick
        Pattern::Choice(alts) if all_skip_to_literal(alts) => {
            let literals: Vec<_> = alts.iter().map(extract_literal).collect();
            Pattern::SkipToAnyLiteral {
                literals: literals.clone(),
                automaton: AhoCorasick::new(&literals),
            }
        }

        _ => pattern,
    }
}
```

### Phase 3: Code Generation

New bytecode instructions:

```rust
pub enum Instruction {
    // ... existing instructions ...

    /// Walk a compiled trie
    MatchTrie { trie: ConstIndex },

    /// Skip to first occurrence of literal
    MatchSkipToLiteral { literal: ConstIndex },

    /// Skip to first occurrence of any literal (Aho-Corasick)
    MatchSkipToAnyLiteral { automaton: ConstIndex, literals: ConstIndex },

    /// O(1) character dispatch
    MatchCharJumpTable { table: ConstIndex, default: Option<InstrIndex> },
}
```

---

## 7. Interaction with Memoization

### Safe Optimizations

These preserve memoization:

| Optimization | Why Safe |
|-------------|----------|
| Literal trie | Same result at same position |
| Char jump table | Same result at same position |
| `SkipToLiteral` | Finds first occurrence (deterministic) |
| Aho-Corasick | Finds first occurrence (deterministic) |
| Pattern trie (disjoint) | Same result at same position |

### Requires Care

| Optimization | Issue | Solution |
|-------------|-------|----------|
| Pattern trie (overlapping) | Backtracking changes result | Cache per-branch, not per-pattern |
| Longest match in trie | Different from first-match | Separate optimization flag |

---

## 8. Implementation Plan

### Phase 1: Infrastructure
- [ ] Add `PatternAnalysis` and `FirstSet` types
- [ ] Implement `compute_first_set()` for all patterns
- [ ] Add optimizer pass hook in compilation pipeline

### Phase 2: Core Optimizations
- [ ] Implement `SkipToLiteral` pattern and instruction
- [ ] Implement literal trie for `Choice`
- [ ] Implement char jump table

### Phase 3: Advanced Optimizations
- [ ] Add `aho-corasick` dependency
- [ ] Implement multi-pattern search
- [ ] Implement generalized pattern trie

### Phase 4: Integration
- [ ] Benchmark suite for grammar performance
- [ ] Optimization level flags (`-O0`, `-O1`, `-O2`)
- [ ] Debug output for optimization decisions

---

## 9. Examples

### Before/After: Keyword Matching

```fmpl
-- Before: O(n × keywords)
keyword = "if" | "then" | "else" | "while" | "do" | "for" | "in" | "let" | "fn"

-- After: O(match_length) via trie
-- Automatically optimized, same syntax
```

### Before/After: Substring Search

```fmpl
-- Before: FAILS (greedy .* consumes all)
find_error = .* "error"

-- After: Works via SkipToLiteral fusion
-- Same syntax, optimizer recognizes pattern
```

### Before/After: Multi-Pattern Detection

```fmpl
-- Before: O(n × 3) sequential searches
detect = .*"error" | .*"warning" | .*"fatal"

-- After: O(n) via Aho-Corasick
-- Automatically optimized
```

---

## 10. FMPL-Native Implementation

The grammar optimizer should be written in FMPL itself, using grammars to transform grammars.

### Optimizer as Grammar

```fmpl
grammar PatternOptimizer <: null_opt {
  -- Skip-to-literal fusion: .* followed by literal
  skip_to_literal = :Seq([:Star(:Any), :Literal(s):lit]) =>
    :SkipToLiteral(%{literal: lit, include: true});

  -- Char jump table: choice of 4+ single chars
  char_jump_table = :Choice(alts):a &{ is_char_choice(a) && a.len() >= 4 } =>
    :CharJumpTable(build_char_table(alts));

  -- Literal trie: choice of 4+ literals
  literal_trie = :Choice(alts):a &{ is_literal_choice(a) && a.len() >= 4 } =>
    :Trie(build_literal_trie(alts));

  -- Multi-pattern Aho-Corasick
  multi_skip = :Choice(alts):a &{ all_skip_to_literal(a) } =>
    :SkipToAnyLiteral(build_aho_corasick(alts));
}
```

The grammar inherits from `null_opt` which passes through unmatched patterns unchanged. Child patterns are walked first (bottom-up), so rules only need to match the patterns they optimize.

### Aho-Corasick in FMPL

The automaton is pure data:

```fmpl
-- Build AC automaton from patterns
let build_aho_corasick = \patterns
  let trie = build_ac_trie(patterns)
  let with_failures = add_failure_links(trie)
  %{states: with_failures, start: 0, patterns: patterns}

-- AC trie construction
let build_ac_trie = \patterns
  let root = %{transitions: %{}, fail: 0, output: null, id: 0}
  let states = [root]
  for pattern in patterns do
    let state = 0
    for c in pattern do
      if c in states[state].transitions then
        state = states[state].transitions[c]
      else
        let new_id = states.len()
        states = states.push(%{transitions: %{}, fail: 0, output: null, id: new_id})
        states[state].transitions[c] = new_id
        state = new_id
    states[state].output = pattern
  states

-- Add failure links (BFS)
let add_failure_links = \states
  let queue = [states[0].transitions[c] for c in states[0].transitions]
  while queue.len() > 0 do
    let state_id = queue[0]
    queue = queue[1..]
    let state = states[state_id]
    for (c, next_id) in state.transitions do
      queue = queue.push(next_id)
      let fail = state.fail
      while fail > 0 && !(c in states[fail].transitions) do
        fail = states[fail].fail
      states[next_id].fail = if c in states[fail].transitions then states[fail].transitions[c] else 0
  states
```

The runner is a simple loop:

```fmpl
let run_aho_corasick = \automaton \input
  let state = automaton.start
  let pos = 0
  while pos < input.len() do
    let c = input[pos]
    let st = automaton.states[state]
    -- Follow failure links until transition found or root
    while state > 0 && !(c in st.transitions) do
      state = st.fail
      st = automaton.states[state]
    state = st.transitions[c] ?? 0
    if automaton.states[state].output then
      return %{found: true, pos: pos, pattern: automaton.states[state].output}
    pos = pos + 1
  %{found: false}
```

---

## 11. IR Compiler State Machine Recognition

The IR compiler should recognize state machine patterns and emit optimized bytecode.

### Pattern Recognition

The compiler detects this shape:

```fmpl
let state = <constant>
while <condition> do
  let c = input[pos]
  state = states[state].transitions[c] ?? states[state].fail
  if states[state].output then <return>
  pos = pos + 1
```

Key indicators:
1. `state` variable used as index into constant array
2. Loop body does indexed lookup `array[state]`
3. Transition is map/table lookup by character
4. State variable updated each iteration

### Compilation Strategies

**Threshold heuristics:**

| States | Transitions | Strategy |
|--------|-------------|----------|
| < 4 | any | Inline if/else chain |
| 4-16 | sparse | Switch/case dispatch |
| 4-16 | dense | Small jump table |
| 16+ | dense | Full jump table array |
| 16+ | sparse | Hybrid: jump table + default |

**Dense vs sparse:**
- Dense: > 25% of char range has transitions
- Sparse: < 25% coverage

### Jump Table Compilation

For dense automata, emit jump table per state:

```
; Constant data section
state_0_table: [0, 0, 0, ..., 1, ..., 0]  ; 256 entries, 'h'->1
state_1_table: [0, 0, 0, ..., 2, ..., 0]  ; 'e'->2, else->fail
state_2_table: [0, 0, 0, ..., 3, ..., 0]  ; 'l'->3
; ... etc

outputs: [null, null, null, null, null, "hello"]  ; state 5 outputs

; Code section
run_ac:
  load_const state, 0
  load_const pos, 0
loop:
  ; bounds check
  len tmp, input
  jge pos, tmp, not_found

  ; c = input[pos]
  index c, input, pos

  ; state = jump_tables[state][c]
  index_2d state, jump_tables, state, c    ; specialized instruction

  ; if outputs[state] goto found
  index tmp, outputs, state
  jne tmp, null, found

  ; pos++
  inc pos
  jump loop

found:
  ; return match info
  ...

not_found:
  ; return null
  ...
```

### Specialized Instructions

New bytecode instructions for state machines:

```rust
pub enum Instruction {
    // ... existing ...

    /// 2D indexed lookup: result = table[idx1][idx2]
    /// For jump tables: state = transitions[state][char]
    Index2D {
        result: InstrIndex,
        table: ConstIndex,
        idx1: InstrIndex,
        idx2: InstrIndex,
    },

    /// Fused state transition with output check
    /// new_state = transitions[state][char]; has_output = outputs[state] != null
    StateTransition {
        new_state: InstrIndex,
        has_output: InstrIndex,
        transitions: ConstIndex,
        outputs: ConstIndex,
        state: InstrIndex,
        char: InstrIndex,
    },
}
```

### Hybrid Approach for Sparse Automata

When transitions are sparse, use compressed representation:

```fmpl
-- Sparse state: list of (char, next_state) pairs
let sparse_state = %{
  transitions: [('h', 1), ('w', 7)],  -- sorted by char
  default: 0,  -- failure link
  output: null
}
```

Compiles to binary search or small linear scan:

```
state_0_sparse:
  ; binary search in transitions
  load_const lo, 0
  load_const hi, 2        ; transitions.len()
binary_search:
  jge lo, hi, use_default
  add mid, lo, hi
  shr mid, mid, 1         ; mid = (lo + hi) / 2
  index_2d tc, transitions_0, mid, 0  ; tc = transitions[mid][0] (char)
  jeq c, tc, found_transition
  jlt c, tc, search_left
  add lo, mid, 1
  jump binary_search
search_left:
  mov hi, mid
  jump binary_search
found_transition:
  index_2d state, transitions_0, mid, 1  ; state = transitions[mid][1]
  jump continue
use_default:
  load_const state, 0     ; failure link
continue:
  ...
```

### Optimization Cascade

The IR compiler applies optimizations in phases:

1. **Pattern recognition** - Identify state machine loops
2. **Constant propagation** - Fold automaton structure into constants
3. **Strategy selection** - Choose jump table vs switch vs binary search
4. **Instruction selection** - Emit specialized bytecode
5. **Register allocation** - Minimize loads/stores in hot loop

### Example: Full Compilation

FMPL source:
```fmpl
let detect = build_aho_corasick(["error", "warning", "fatal"])
run_aho_corasick(detect, log_text)
```

After optimization, emits:
```
; Precomputed at compile time
jump_tables: <256 * num_states entries>
outputs: [null, null, ..., "error", ..., "warning", ..., "fatal"]

; Hot loop: ~5 instructions per character
loop:
  index c, input, pos
  index_2d state, jump_tables, state, c
  index out, outputs, state
  jne out, null, found
  inc pos
  jlt pos, len, loop
```

This achieves performance comparable to the Rust `aho-corasick` crate while being fully expressed in FMPL.

---

## References

### VPRI / FONC / Maru

- [VPRI STEPS/FONC](http://www.vpri.org/pdf/tr2012001_steps.pdf) — "Steps Toward Expressive Programming Systems", grammars as first-class tool for the entire system
- [Open, Extensible Composition Models](https://tinlizzie.org/VPRIPapers/tr2011002_oecm.pdf) — Ian Piumarta, VPRI TR-2011-002. Metacircular compilation, level shifting, grammars all the way down
- [OMeta](http://www.vpri.org/pdf/tr2008003_experimenting.pdf) — Alessandro Warth's pattern matching language for parsing and tree transformation
- [Maru (original)](https://piumarta.com/software/maru/) — Ian Piumarta's metacircular Lisp with PEG grammars, ~1750 lines self-hosting
- [Maru (archive)](https://github.com/nnunley/maru) — Extended archive with documentation and multi-backend support

### Key Concepts from VPRI

**Grammars as Universal Tool**: In the STEPS vision, grammars are not just for parsing text. They are the universal mechanism for:
- Parsing (text → AST)
- Tree transformation (AST → AST)
- Code generation (AST → target code)
- Optimization (IR → optimized IR)
- Disassembly (machine code → IR → assembly)

**Level Shifting**: The compiler performs semantics-preserving transformations between abstraction levels. Each grammar rule is a level-shift that can go in either direction.

**IR as Universal Hub**: Rather than direct source-to-target compilation, use IR as the universal intermediate representation. All languages compile to IR, all targets generate from IR. This enables:
- Cross-compilation
- Optimization at IR level
- Bidirectional transformations (assembly ↔ IR ↔ high-level)

### Algorithms

- [Aho-Corasick Algorithm](https://en.wikipedia.org/wiki/Aho%E2%80%93Corasick_algorithm) — Multi-pattern string matching in O(n + m)
- [Packrat Parsing](https://bford.info/pub/lang/packrat-icfp02.pdf) — Memoization for PEG, linear time guarantee
- [ANTLR LL(*)](https://www.antlr.org/papers/LL-star-PLDI11.pdf) — Adaptive lookahead prediction with DFA
