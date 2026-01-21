# Implementation Plan: The `@` Apply Operator

**Status: Complete**
This implementation plan has been marked as complete. The tasks outlined below have been implemented.

## Goal

Implement the `@` operator for grammar application, enabling:

```fmpl
-- Named grammar, named rule
"hello" @ parser.word

-- Anonymous grammar block
42 @ { n:int => n * 2 }

-- Single value pattern match (replaces match keyword)
obj @ {
  %{type: "move", dir: d} => move(d)
  _ => default()
}
```

This is the foundation for unified grammars and agentic control flow.

---

## Tasks

### 1. Lexer: Add `@` token

**File:** `fmpl-core/src/lexer.rs`

Add `At` token to the lexer. It's already likely used for curry (`@`), so verify current usage and ensure it can be used as a binary operator.

**Acceptance:** `@` lexes as its own token.

---

### 2. AST: Add Apply expression

**File:** `fmpl-core/src/ast.rs`

Add a new expression variant:

```rust
pub enum Expr {
    // ... existing variants ...

    /// Grammar application: `expr @ grammar_ref`
    Apply {
        /// The input value/expression
        input: Box<Expr>,
        /// The grammar to apply
        grammar: GrammarRef,
    },
}

/// Reference to a grammar and optional rule
pub enum GrammarRef {
    /// Named grammar with rule: `parser.word` or `parser` (default rule)
    Named {
        grammar: QualifiedName,
        rule: Option<SmolStr>,
    },
    /// Anonymous grammar block: `{ patterns }`
    Anonymous(Vec<RuleCase>),
}

/// A single case in an anonymous grammar block
pub struct RuleCase {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,  // `when` clause
    pub action: Box<Expr>,
}
```

**Acceptance:** AST can represent `x @ foo.bar` and `x @ { p => e }`.

---

### 3. Parser: Parse `@` as binary operator

**File:** `fmpl-core/src/parser.rs`

Add `@` as a binary operator with appropriate precedence (lower than arithmetic, higher than comparison seems reasonable - test what feels natural).

Parse the right-hand side as either:
- `qualified_name` (grammar reference, optionally `.rule`)
- `{ rule_cases }` (anonymous block)

For anonymous blocks, reuse or adapt the existing grammar rule parsing for the case syntax:
```
pattern => expr
pattern when expr => expr
```

**Acceptance:**
- `x @ foo.bar` parses
- `x @ foo` parses (default rule)
- `x @ { p => e }` parses
- `x @ { p when g => e }` parses
- Multiple cases: `x @ { p1 => e1; p2 => e2 }` parses

---

### 4. Compiler: Emit Apply bytecode

**File:** `fmpl-core/src/compiler.rs`

Options:

**Option A: Single opcode**
- Add `OP_APPLY` that takes grammar ref from constant pool
- For anonymous blocks, compile to a grammar object in constant pool

**Option B: Runtime call**
- Compile to a call to a built-in `$apply(input, grammar, rule)` function
- Simpler initially, can optimize later

Recommend **Option B** for initial implementation - less bytecode churn.

**Acceptance:** `x @ { n:int => n * 2 }` compiles without error.

---

### 5. Grammar Runtime: Execute patterns against values

**File:** `fmpl-core/src/grammar/runtime.rs`

This is the core work. The runtime needs to:

1. **Coerce input to stream** based on type:
   - `String` → character stream (existing)
   - `List` → element stream (existing `Input::Values`)
   - Other values → single-element stream

2. **Execute pattern matching** against the stream:
   - For anonymous blocks: try each case in order (ordered choice)
   - For named grammars: look up rule and execute

3. **Bind variables** during pattern match and make available to action

4. **Evaluate semantic action** with bindings in scope

5. **Return result** or signal failure

Key patterns to support initially:
- `MatchType` - match by type (int, string, etc.)
- `MatchValue` - match exact value
- `MapMatch` - match map with key patterns
- `ListMatch` - match list structure
- `Bind` - capture value to variable
- `Choice` - ordered alternatives

**Acceptance:**
```fmpl
42 @ { n:int => n * 2 }  -- returns 84
"hello" @ { s:string => s }  -- returns "hello"
%{x: 1, y: 2} @ { %{x: a, y: b} => a + b }  -- returns 3
[1, 2, 3] @ { [a, b, c] => a + b + c }  -- returns 6
```

---

### 6. VM: Wire up Apply execution

**File:** `fmpl-core/src/vm.rs`

If using Option B (runtime call), add a built-in function `$apply` that:
1. Takes input value and grammar reference
2. Calls into grammar runtime
3. Returns result or raises error on match failure

**Acceptance:** Full round-trip works - parse, compile, execute `@` expressions.

---

### 7. Tests

**File:** `fmpl-core/tests/apply_operator.rs` (new)

Test cases:

```rust
#[test]
fn test_apply_int_pattern() {
    // 42 @ { n:int => n * 2 } == 84
}

#[test]
fn test_apply_string_pattern() {
    // "hello" @ { s:string => s } == "hello"
}

#[test]
fn test_apply_map_pattern() {
    // %{x: 1, y: 2} @ { %{x: a, y: b} => a + b } == 3
}

#[test]
fn test_apply_list_pattern() {
    // [1, 2, 3] @ { [a, b, c] => a + b + c } == 6
}

#[test]
fn test_apply_multiple_cases() {
    // 42 @ { s:string => 0; n:int => n } == 42
}

#[test]
fn test_apply_with_guard() {
    // 5 @ { n:int when n > 3 => "big"; _ => "small" } == "big"
}

#[test]
fn test_apply_nested_pattern() {
    // %{user: %{name: "alice"}} @ { %{user: %{name: n}} => n } == "alice"
}

#[test]
fn test_apply_failure() {
    // "hello" @ { n:int => n } should error
}
```

---

## Order of Implementation

1. **Lexer** - trivial, verify `@` token exists
2. **AST** - add types, straightforward
3. **Parser** - moderate complexity, need to handle both forms
4. **Grammar Runtime** - bulk of the work, pattern execution
5. **Compiler** - simple if using runtime call approach
6. **VM** - wire up the built-in
7. **Tests** - validate everything works

---

## Out of Scope (for this plan)

- Streaming grammars (push model, cut operators)
- Named grammar definitions in FMPL source
- Grammar inheritance (`<:`)
- Semantic predicates with arbitrary expressions
- Character-level parsing (focus on value/tree patterns first)

These build on the foundation but aren't needed for the core `@` operator to work.

---

## Success Criteria

The following REPL session works:

```
fmpl> 42 @ { n:int => n * 2 }
84

fmpl> %{name: "alice", age: 30} @ { %{name: n, age: a} => n }
"alice"

fmpl> [1, 2, 3] @ { [h | t] => h }
1

fmpl> "test" @ { n:int => "int"; s:string => "string" }
"string"
```
