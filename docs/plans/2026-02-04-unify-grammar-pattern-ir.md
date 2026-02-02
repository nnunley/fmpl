# Unify Grammar and Pattern Matching at IR Layer

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Unify FMPL's pattern matching and grammar systems at the IR layer, enabling polymorphic stream coercion, anonymous inline pattern blocks, and a single pattern syntax that works everywhere with context-aware compilation.

**Architecture:** Hybrid unification - single `Pattern` enum with two compilation modes: fast path (direct extraction) for simple let bindings, and full path (grammar matching with backtracking/guards) for `@` operator and complex patterns. Stream coercion enables `@` to work on strings, lists, maps, and tagged values uniformly.

**Tech Stack:** Rust, FMPL compiler (Indexed RPN), OMeta-style PEG grammar runtime, Arc/SmolStr for efficient string handling, rkyv for serialization.

---

## Overview

This plan unifies two currently separate pattern systems:

1. **`ast::Pattern`** - Used in `let` bindings, supports destructuring with direct extraction (fast, no guards)
2. **`grammar::Pattern`** - Used in grammar rules, supports full PEG matching with backtracking, guards, actions

The unified approach:
- Single `Pattern` type in `grammar::Pattern` (rename/refactor)
- Context-aware compilation: `let` uses direct extraction, `@` uses grammar matching
- Polymorphic stream coercion: String→chars, List→items, Map/Tagged→single-element
- Anonymous inline pattern blocks: `x @ { %{a: b} => b }`

---

## Phase 1: Unified Pattern Type

**Goal:** Consolidate `ast::Pattern` and `grammar::Pattern` into a single type with mode parameter.

### Task 1.1: Create unified `Pattern` type

**Files:**
- Create: `fmpl-core/src/pattern/mod.rs`
- Modify: `fmpl-core/src/ast.rs` (keep for backward compatibility during transition)
- Modify: `fmpl-core/src/grammar/mod.rs`
- Test: `fmpl-core/tests/pattern_unification.rs`

**Step 1: Create new pattern module**

```rust
// fmpl-core/src/pattern/mod.rs

use smol_str::SmolStr;
use serde::{Serialize, Deserialize};

/// Unified pattern type for both let bindings and grammar rules.
/// Compilation behavior depends on context (fast vs full path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// Wildcard - matches anything, binds nothing
    Any,

    /// Variable binding - matches anything, binds to name
    Var(SmolStr),

    /// Literal value - matches exact value
    Literal(LiteralValue),

    /// Map pattern - %{key1: pattern1, key2: pattern2}
    Map(Vec<(SmolStr, Pattern)>),

    /// List pattern - [p1, p2, p3] or [head | tail] or [p*]
    List(ListPattern),

    /// Tagged/constructor pattern - :Tag(p1, p2, ...)
    Tagged { tag: SmolStr, patterns: Vec<Pattern> },

    /// Character pattern (for strings) - 'a' or [a-z]
    Char(CharPattern),

    /// Sequence - p1 p2 p3 (ordered, all must match)
    Seq(Vec<Pattern>),

    /// Ordered choice - p1 | p2 | p3 (try first that matches)
    Choice(Vec<Pattern>),

    /// Repetition - p* (zero or more) or p+ (one or more)
    Repeat { pattern: Box<Pattern>, kind: RepeatKind },

    /// Optional - p? (zero or one)
    Optional(Box<Pattern>),

    /// Lookahead - &p (positive) or !p (negative)
    Lookahead { pattern: Box<Pattern>, positive: bool },

    /// Binding - name: pattern or pattern when guard
    Bind { name: SmolStr, pattern: Box<Pattern> },

    /// Guard - pattern when predicate
    Guard { pattern: Box<Pattern>, predicate: GuardPredicate },

    /// Action - pattern => expr
    Action { pattern: Box<Pattern>, action: SmolStr }, // action is expr string

    /// Rule application - applies named grammar rule
    ApplyRule(SmolStr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralValue {
    String(SmolStr),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ListPattern {
    Exact(Vec<Pattern>),           // [p1, p2, p3]
    HeadTail { head: Box<Pattern>, tail: Option<SmolStr> },  // [h | t]
    Repeat { element: Box<Pattern> },  // [p*]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharPattern {
    Exact(char),
    Class(Vec<(char, char)>),      // [a-z]
    NegatedClass(Vec<(char, char)>), // [^a-z]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RepeatKind {
    ZeroOrMore,   // p*
    OneOrMore,    // p+
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardPredicate {
    Expr(SmolStr),  // Expression to evaluate
    TypeCheck(SmolStr),  // Check type: is_list, is_map, etc.
}
```

**Step 2: Add tests**

```rust
// fmpl-core/tests/pattern_unification.rs

use fmpl_core::pattern::*;

#[test]
fn test_pattern_wildcard() {
    let p = Pattern::Any;
    assert_eq!(p, Pattern::Any);
}

#[test]
fn test_pattern_variable() {
    let p = Pattern::Var(SmolStr::new("x"));
    assert!(matches!(p, Pattern::Var(name) if name == "x"));
}

#[test]
fn test_pattern_map() {
    let p = Pattern::Map(vec![
        (SmolStr::new("type"), Pattern::Var(SmolStr::new("t"))),
    ]);
    assert!(matches!(p, Pattern::Map(_)));
}

#[test]
fn test_pattern_list_exact() {
    let p = Pattern::List(ListPattern::Exact(vec![
        Pattern::Any,
        Pattern::Var(SmolStr::new("x")),
    ]));
    assert!(matches!(p, Pattern::List(ListPattern::Exact(_))));
}

#[test]
fn test_pattern_tagged() {
    let p = Pattern::Tagged {
        tag: SmolStr::new("Some"),
        patterns: vec![Pattern::Var(SmolStr::new("x"))],
    };
    assert!(matches!(p, Pattern::Tagged { .. }));
}
```

**Step 3: Run tests**

Run: `cargo test -p fmpl-core pattern_unification`
Expected: COMPILE FAIL (module doesn't exist yet)

**Step 4: Create module directory and add to lib.rs**

```bash
mkdir -p fmpl-core/src/pattern
touch fmpl-core/src/pattern/mod.rs
```

Add to `fmpl-core/src/lib.rs`:
```rust
pub mod pattern;
```

**Step 5: Run tests again**

Run: `cargo test -p fmpl-core pattern_unification`
Expected: PASS

**Step 6: Commit**

```bash
jj add fmpl-core/src/pattern/mod.rs fmpl-core/src/lib.rs fmpl-core/tests/pattern_unification.rs
jj commit -m "feat(pattern): add unified Pattern type

- Consolidate ast::Pattern and grammar::Pattern into single type
- Support all pattern forms: Any, Var, Literal, Map, List, Tagged
- Add grammar-specific forms: Seq, Choice, Repeat, Lookahead, Guard, Action
- Add character patterns for string matching
- Type parameterized for context-aware compilation
"
```

---

### Task 1.2: Add `PatternMode` for context-aware compilation

**Files:**
- Modify: `fmpl-core/src/pattern/mod.rs`
- Test: `fmpl-core/tests/pattern_unification.rs`

**Step 1: Add PatternMode enum**

```rust
// fmpl-core/src/pattern/mod.rs

/// Compilation mode for patterns - determines strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternMode {
    /// Fast path: direct extraction, no backtracking (for let bindings)
    /// Uses ExtractMapKey, ExtractListIndex, ExtractTaggedChild
    Fast,

    /// Full path: grammar matching with backtracking (for @ operator)
    /// Uses MatchSeq, MatchChoice, MatchGuard, etc.
    Full,
}
```

**Step 2: Add mode helper method**

```rust
impl Pattern {
    /// Determine if pattern requires full matching (backtracking/guards)
    pub fn requires_full_mode(&self) -> bool {
        match self {
            Pattern::Seq(_) | Pattern::Choice(_) | Pattern::Repeat(_) => true,
            Pattern::Lookahead { .. } | Pattern::Guard { .. } => true,
            Pattern::Action { .. } => true,
            Pattern::Char(_) => true,  // Only for string parsing
            Pattern::List(ListPattern::Repeat { .. }) => true,
            _ => false,
        }
    }

    /// Get recommended compilation mode for this pattern
    pub fn recommended_mode(&self) -> PatternMode {
        if self.requires_full_mode() {
            PatternMode::Full
        } else {
            PatternMode::Fast
        }
    }
}
```

**Step 3: Add tests**

```rust
// fmpl-core/tests/pattern_unification.rs

#[test]
fn test_fast_mode_patterns() {
    assert_eq!(Pattern::Any.recommended_mode(), PatternMode::Fast);
    assert_eq!(Pattern::Var(SmolStr::new("x")).recommended_mode(), PatternMode::Fast);

    let map_p = Pattern::Map(vec![
        (SmolStr::new("x"), Pattern::Var(SmolStr::new("y"))),
    ]);
    assert_eq!(map_p.recommended_mode(), PatternMode::Fast);
}

#[test]
fn test_full_mode_patterns() {
    // Seq requires full mode
    let seq_p = Pattern::Seq(vec![Pattern::Any, Pattern::Any]);
    assert_eq!(seq_p.recommended_mode(), PatternMode::Full);

    // Choice requires full mode
    let choice_p = Pattern::Choice(vec![Pattern::Any, Pattern::Any]);
    assert_eq!(choice_p.recommended_mode(), PatternMode::Full);

    // Guard requires full mode
    let guard_p = Pattern::Guard {
        pattern: Box::new(Pattern::Any),
        predicate: GuardPredicate::Expr(SmolStr::new("true")),
    };
    assert_eq!(guard_p.recommended_mode(), PatternMode::Full);
}
```

**Step 4: Run tests**

Run: `cargo test -p fmpl-core pattern_unification`
Expected: PASS

**Step 5: Commit**

```bash
jj add fmpl-core/src/pattern/mod.rs fmpl-core/tests/pattern_unification.rs
jj commit -m "feat(pattern): add PatternMode for context-aware compilation

- Add PatternMode enum: Fast (direct extraction) vs Full (grammar matching)
- Add requires_full_mode() to detect patterns needing backtracking/guards
- Fast mode uses Extract* instructions (no backtracking)
- Full mode uses Match* instructions (full PEG semantics)
"
```

---

### Task 1.3: Migrate `ast::Pattern` to use unified type

**Files:**
- Modify: `fmpl-core/src/ast.rs`
- Modify: `fmpl-core/src/compiler.rs`
- Test: `fmpl-core/tests/pattern_unification.rs`

**Step 1: Update ast.rs to re-export unified pattern**

```rust
// fmpl-core/src/ast.rs

// Re-export unified pattern type
pub use crate::pattern::Pattern as AstPattern;

// Keep old type alias for backward compatibility during transition
pub type Pattern = AstPattern;

// The rest of ast.rs remains unchanged...
```

**Step 2: Update grammar/mod.rs to re-export pattern**

```rust
// fmpl-core/src/grammar/mod.rs

// Re-export unified pattern type
pub use crate::pattern::Pattern as GrammarPattern;

// Keep old type alias
pub type Pattern = GrammarPattern;

// The rest of grammar/mod.rs remains unchanged...
```

**Step 3: Add migration test**

```rust
// fmpl-core/tests/pattern_unification.rs

use fmpl_core::ast::Pattern as AstPattern;
use fmpl_core::grammar::Pattern as GrammarPattern;

#[test]
fn test_pattern_unification_ast_grammar_same() {
    let ast_p = AstPattern::Var(SmolStr::new("x"));
    let grammar_p = GrammarPattern::Var(SmolStr::new("x"));

    // They should be the same type
    assert_eq!(ast_p, grammar_p);
}

#[test]
fn test_map_pattern_both_contexts() {
    // Pattern that works in both ast and grammar context
    let p = AstPattern::Map(vec![
        (SmolStr::new("type"), AstPattern::Var(SmolStr::new("t"))),
        (SmolStr::new("value"), AstPattern::Var(SmolStr::new("v"))),
    ]);

    // Should use fast mode (no guards/backtracking)
    assert_eq!(p.recommended_mode(), crate::pattern::PatternMode::Fast);
}

#[test]
fn test_guarded_pattern_requires_full() {
    use fmpl_core::pattern::{GuardPredicate, PatternMode};

    // Pattern with guard only works in grammar context
    let p = GrammarPattern::Guard {
        pattern: Box::new(GrammarPattern::Map(vec![
            (SmolStr::new("x"), GrammarPattern::Var(SmolStr::new("v"))),
        ])),
        predicate: GuardPredicate::Expr(SmolStr::new("v > 0")),
    };

    // Should require full mode
    assert_eq!(p.recommended_mode(), PatternMode::Full);
}
```

**Step 4: Run tests**

Run: `cargo test -p fmpl-core pattern_unification`
Expected: PASS

**Step 5: Run existing compiler tests**

Run: `cargo test -p fmpl-core compiler`
Expected: PASS (verify backward compatibility)

**Step 6: Commit**

```bash
jj add fmpl-core/src/ast.rs fmpl-core/src/grammar/mod.rs fmpl-core/tests/pattern_unification.rs
jj commit -m "refactor(pattern): migrate ast and grammar to use unified Pattern

- Re-export unified Pattern as AstPattern in ast.rs
- Re-export unified Pattern as GrammarPattern in grammar/mod.rs
- Add type aliases for backward compatibility
- Verify ast::Pattern and grammar::Pattern are now same type
"
```

---

## Phase 2: Stream Coercion

**Goal:** Enable `@` operator to coerce input to appropriate stream type (chars, items, once).

### Task 2.1: Add `CoerceStream` instruction

**Files:**
- Modify: `fmpl-core/src/compiler.rs` (Instruction enum)
- Modify: `fmpl-core/src/vm.rs` (execution)
- Test: `fmpl-core/tests/stream_coercion.rs`

**Step 1: Add instruction to compiler.rs**

```rust
// fmpl-core/src/compiler.rs

pub enum Instruction {
    // ... existing instructions ...

    // Stream coercion for @ operator
    /// Coerce value to stream based on type:
    /// - String -> character stream
    /// - List -> element stream
    /// - Map/Tagged/other -> single-element stream
    CoerceStream { value: InstrIndex, mode: StreamMode },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StreamMode {
    Chars,   // String -> character stream
    Items,   // List -> element stream
    Once,    // Single value -> single-element stream
    Auto,    // Detect from input type at runtime
}
```

**Step 2: Implement VM execution**

```rust
// fmpl-core/src/vm.rs

impl Frame {
    pub fn exec_instruction(
        &mut self,
        ip: InstrIndex,
        code: &CompiledCode,
        vm: &mut VM,
    ) -> Result<ExecResult, Error> {
        // ... existing match arms ...

        Instruction::CoerceStream { value, mode } => {
            let value = self.get_value(value, code)?;
            let stream = match mode {
                StreamMode::Auto => {
                    // Auto-detect based on value type
                    match &*value {
                        Value::String(s) => {
                            // Convert to character stream
                            let chars: Vec<Value> = s.chars()
                                .map(|c| Value::String(SmolStr::new(c)))
                                .collect();
                            Value::List(Arc::new(chars))
                        }
                        Value::List(items) => {
                            // Already a list, treat as element stream
                            value.clone()
                        }
                        _ => {
                            // Single-element stream
                            Value::List(Arc::new(vec![value.clone()]))
                        }
                    }
                }
                StreamMode::Chars => {
                    // Force character stream (string input)
                    let s = value.as_string()?;
                    let chars: Vec<Value> = s.chars()
                        .map(|c| Value::String(SmolStr::new(c)))
                        .collect();
                    Value::List(Arc::new(chars))
                }
                StreamMode::Items => {
                    // Force element stream (list input)
                    value.as_list()?.clone()
                }
                StreamMode::Once => {
                    // Single-element stream
                    Value::List(Arc::new(vec![value.clone()]))
                }
            };
            Ok(ExecResult::Advance(stream))
        }

        // ... rest of instructions ...
    }
}
```

**Step 3: Add tests**

```rust
// fmpl-core/tests/stream_coercion.rs

use fmpl_core::compiler::{Compiler, Instruction, StreamMode};
use fmpl_core::vm::VM;

fn eval(source: &str) -> Result<fmpl_core::value::Value, String> {
    let mut vm = VM::new();
    let code = fmpl_core::compile(source).map_err(|e| e.to_string())?;
    vm.load_code("test", code);
    vm.run_function("test", &[]).map_err(|e| e.to_string())
}

#[test]
fn test_coerce_string_to_chars() {
    let source = r#"
        fn test() {
          let s = "hello"
          s @ string_chars  -- This will use CoerceStream
        }
    "#;
    // Result should be character list
    let result = eval(source);
    // TODO: verify once @ operator uses CoerceStream
}

#[test]
fn test_coerce_list_to_items() {
    let source = r#"
        fn test() {
          let lst = [1, 2, 3]
          lst @ list_parser  -- This will use CoerceStream
        }
    "#;
    // Result should pass list as-is
}
```

**Step 4: Run tests**

Run: `cargo test -p fmpl-core stream_coercion`
Expected: COMPILE FAIL (Instruction doesn't exist yet)

**Step 5: Add instruction**

Add the instruction enum and VM execution code from Step 1 and Step 2.

**Step 6: Run tests again**

Run: `cargo test -p fmpl-core stream_coercion`
Expected: PASS

**Step 7: Commit**

```bash
jj add fmpl-core/src/compiler.rs fmpl-core/src/vm.rs fmpl-core/tests/stream_coercion.rs
jj commit -m "feat(vm): add CoerceStream instruction for polymorphic stream coercion

- Add CoerceStream instruction with Auto/Chars/Items/Once modes
- Auto mode detects input type: String->chars, List->items, other->once
- Enables @ operator to work uniformly on different input types
"
```

---

### Task 2.2: Integrate CoerceStream into @ operator compilation

**Files:**
- Modify: `fmpl-core/src/compiler.rs`
- Test: `fmpl-core/tests/stream_coercion.rs`

**Step 1: Update GrammarApply compilation**

```rust
// fmpl-core/src/compiler.rs

Expr::GrammarApply { input, grammar, rule } => {
    // First, coerce input to appropriate stream
    let input_idx = self.compile_expr(input)?;

    // Add coercion instruction (Auto mode detects type at runtime)
    let coerced = self.code.emit(Instruction::CoerceStream {
        value: input_idx,
        mode: StreamMode::Auto,
    });

    let grammar_idx = match grammar.as_ref() {
        Expr::Qualified(qn) => {
            self.code.emit(Instruction::LoadString(SmolStr::new(qn.to_string())))
        }
        _ => self.compile_expr(grammar)?,
    };

    Ok(self.code.emit(Instruction::GrammarApply {
        input: coerced,  // Use coerced stream
        grammar: grammar_idx,
        rule: rule.clone(),
    }))
}
```

**Step 2: Add integration test**

```rust
// fmpl-core/tests/stream_coercion.rs

#[test]
fn test_at_operator_string_input() {
    let source = r#"
        grammar Test {
          main = "hello"
        }

        fn test() {
          "hello" @ Test.main
        }
    "#;
    let result = eval(source);
    // Should successfully parse string
    assert!(result.is_ok());
}

#[test]
fn test_at_operator_list_input() {
    let source = r#"
        grammar Test {
          main = [1, 2, 3]
        }

        fn test() {
          [1, 2, 3] @ Test.main
        }
    "#;
    let result = eval(source);
    // Should successfully parse list
    assert!(result.is_ok());
}

#[test]
fn test_at_operator_map_input() {
    let source = r#"
        grammar Test {
          main = %{x: 1, y: 2}
        }

        fn test() {
          %{x: 1, y: 2} @ Test.main
        }
    "#;
    let result = eval(source);
    // Should treat map as single-element stream and match
    assert!(result.is_ok());
}
```

**Step 3: Run tests**

Run: `cargo test -p fmpl-core stream_coercion`
Expected: PASS

**Step 4: Commit**

```bash
jj add fmpl-core/src/compiler.rs fmpl-core/tests/stream_coercion.rs
jj commit -m "feat(compiler): integrate CoerceStream into @ operator

- @ operator now coerces input before grammar application
- Auto mode detects type at runtime for polymorphic behavior
- String -> char stream, List -> element stream, other -> single-element
"
```

---

## Phase 3: Anonymous Inline Pattern Blocks

**Goal:** Support inline pattern blocks like `x @ { %{a: b} => b }`.

### Task 3.1: Extend AST for inline pattern blocks

**Files:**
- Modify: `fmpl-core/src/ast.rs`
- Modify: `fmpl-core/src/parser.rs`
- Test: `fmpl-core/tests/anonymous_patterns.rs`

**Step 1: Add InlineBlock variant to Expr**

```rust
// fmpl-core/src/ast.rs

pub enum Expr {
    // ... existing variants ...

    /// Inline pattern block for @ operator
    /// Example: x @ { %{a: b} => b, _ => default }
    InlineBlock {
        cases: Vec<PatternCase>,
    },
}

pub struct PatternCase {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}
```

**Step 2: Add parser support**

```rust
// fmpl-core/src/parser.rs

// In the expression parser, after handling @ grammar.rule:
fn parse_expr_at(&mut self) -> Result<Expr, ParseError> {
    self.parse_expr_pipe()?;

    if self.match_token(Token::At)? {
        // Check for inline block { pattern => body, ... }
        if self.peek_token()? == Token::LBrace {
            let cases = self.parse_inline_block()?;
            Ok(Expr::GrammarApply {
                input: Box::new(/* previous expr */),
                grammar: Box::new(Expr::InlineBlock { cases }),
                rule: SmolStr::new("main"),
            })
        } else {
            // Named grammar.rule
            // ... existing code ...
        }
    } else {
        // ... existing code ...
    }
}

fn parse_inline_block(&mut self) -> Result<Vec<PatternCase>, ParseError> {
    let mut cases = Vec::new();

    self.expect_token(Token::LBrace)?;

    loop {
        if self.match_token(Token::RBrace)? {
            break;
        }

        let pattern = self.parse_pattern()?;
        let mut guard = None;

        // Optional 'when' guard
        if self.match_keyword("when")? {
            guard = Some(self.parse_expr()?);
        }

        self.expect_token(Token::Arrow)?;

        let body = self.parse_expr()?;

        cases.push(PatternCase {
            pattern,
            guard,
            body,
        });

        // Optional comma
        self.match_token(Token::Comma)?;
    }

    Ok(cases)
}
```

**Step 3: Add parser tests**

```rust
// fmpl-core/tests/anonymous_patterns.rs

#[test]
fn test_parse_inline_pattern_block() {
    let source = r#"
        fn test() {
          let x = %{foo: 1, bar: 2}
          x @ {
            %{foo: f} => f
            %{bar: b} => b
            _ => 0
          }
        }
    "#;

    let result = fmpl_core::parse(source);
    assert!(result.is_ok());

    let expr = result.unwrap();
    // Verify structure contains InlineBlock
}
```

**Step 4: Run tests**

Run: `cargo test -p fmpl-core anonymous_patterns`
Expected: COMPILE FAIL (parser changes needed)

**Step 5: Implement parser**

Add the parsing code from Step 2.

**Step 6: Run tests again**

Run: `cargo test -p fmpl-core anonymous_patterns`
Expected: PASS

**Step 7: Commit**

```bash
jj add fmpl-core/src/ast.rs fmpl-core/src/parser.rs fmpl-core/tests/anonymous_patterns.rs
jj commit -m "feat(parser): add inline pattern block syntax

- Add Expr::InlineBlock for anonymous pattern blocks
- Support syntax: x @ { %{a: b} => b, _ => default }
- Add optional 'when' guards: x @ { p when guard => body }
- Parse pattern cases with pattern, optional guard, and body
"
```

---

### Task 3.2: Compile inline blocks to anonymous grammars

**Files:**
- Modify: `fmpl-core/src/compiler.rs`
- Test: `fmpl-core/tests/anonymous_patterns.rs`

**Step 1: Add inline block compilation**

```rust
// fmpl-core/src/compiler.rs

Expr::GrammarApply { input, grammar, rule } => {
    let input_idx = self.compile_expr(input)?;

    // Handle inline blocks by creating anonymous grammar
    let grammar_value = match grammar.as_ref() {
        Expr::InlineBlock { cases } => {
            // Create anonymous grammar from inline block
            let mut compiler = Self::new();

            // Create a grammar with one rule per case
            let mut rules = Vec::new();
            for (i, case) in cases.iter().enumerate() {
                let rule_name = format!("_case_{}", i);
                let pattern = compiler.compile_grammar_pattern(&case.pattern)?;

                // Compile guard if present
                let guard_idx = case.guard.as_ref()
                    .map(|g| compiler.compile_expr(g))
                    .transpose()?;

                // Compile body
                let body_idx = compiler.compile_expr(&case.body)?;

                // Create rule: pattern [when guard] => body
                // This becomes a sequence with optional guard and action
                let rule_instr = if let Some(guard) = guard_idx {
                    compiler.code.emit(Instruction::MatchGuard {
                        pattern,
                        predicate: guard,
                    });
                    compiler.code.emit(Instruction::MatchAction {
                        pattern: compiler.code.last_ip(),
                        action: body_idx,
                    })
                } else {
                    compiler.code.emit(Instruction::MatchAction {
                        pattern,
                        action: body_idx,
                    })
                };

                rules.push((SmolStr::new(&rule_name), rule_instr));
            }

            // Combine all cases with Choice (ordered choice)
            let main_rule = if rules.len() == 1 {
                rules[0].1
            } else {
                let choice_patterns: Vec<InstrIndex> = rules.iter()
                    .map(|(_, p)| *p)
                    .collect();
                compiler.code.emit(Instruction::MatchChoice {
                    patterns: choice_patterns,
                })
            };

            // Create anonymous grammar
            let grammar = Grammar {
                name: SmolStr::new("_anonymous"),
                rules: vec![(SmolStr::new("main"), main_rule)],
                parent: None,
            };

            Value::Grammar(Arc::new(grammar))
        }
        Expr::Qualified(qn) => {
            // Named grammar - look up in environment
            // ... existing lookup code ...
            Value::Null  // placeholder
        }
        _ => {
            // Expression that evaluates to grammar
            let idx = self.compile_expr(grammar)?;
            self.get_temp_value(idx)
        }
    };

    let grammar_idx = self.code.emit(Instruction::LoadGrammar(
        grammar_value.as_grammar()?.clone()
    ));

    // Coerce input to stream
    let coerced = self.code.emit(Instruction::CoerceStream {
        value: input_idx,
        mode: StreamMode::Auto,
    });

    Ok(self.code.emit(Instruction::GrammarApply {
        input: coerced,
        grammar: grammar_idx,
        rule: rule.clone(),
    }))
}
```

**Step 2: Add execution tests**

```rust
// fmpl-core/tests/anonymous_patterns.rs

use fmpl_core::compiler::Compiler;

fn eval(source: &str) -> Result<fmpl_core::value::Value, String> {
    let mut vm = VM::new();
    let code = fmpl_core::compile(source).map_err(|e| e.to_string())?;
    vm.load_code("test", code);
    vm.run_function("test", &[]).map_err(|e| e.to_string())
}

#[test]
fn test_inline_pattern_block_map() {
    let source = r#"
        fn test() {
          let x = %{foo: 42, bar: 99}
          x @ {
            %{foo: f} => f
            _ => 0
          }
        }
    "#;

    let result = eval(source).unwrap();
    // Should extract foo value 42
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}

#[test]
fn test_inline_pattern_block_tagged() {
    let source = r#"
        fn test() {
          let x = :Some(42)
          x @ {
            :Some(v) => v
            :None => 0
          }
        }
    "#;

    let result = eval(source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}

#[test]
fn test_inline_pattern_block_with_guard() {
    let source = r#"
        fn test() {
          let x = 42
          x @ {
            n when n > 50 => 100
            n => n
          }
        }
    "#;

    let result = eval(source).unwrap();
    // 42 is not > 50, so return n
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}
```

**Step 3: Run tests**

Run: `cargo test -p fmpl-core anonymous_patterns`
Expected: FAIL (compilation needs work)

**Step 4: Fix compilation issues**

The inline block compilation needs to create a proper anonymous grammar. This requires integrating with the existing grammar system.

**Step 5: Run tests again**

Run: `cargo test -p fmpl-core anonymous_patterns`
Expected: PASS

**Step 6: Commit**

```bash
jj add fmpl-core/src/compiler.rs fmpl-core/tests/anonymous_patterns.rs
jj commit -m "feat(compiler): compile inline pattern blocks to anonymous grammars

- Inline blocks { p => body, _ => default } compile to anonymous grammars
- Each case becomes a rule with pattern and action
- Guards compile to MatchGuard instructions
- All cases combined with ordered choice (MatchChoice)
"
```

---

## Phase 4: Context-Aware Pattern Compilation

**Goal:** Compile patterns differently based on context (fast for let, full for @).

### Task 4.1: Add mode parameter to pattern compilation

**Files:**
- Modify: `fmpl-core/src/compiler.rs`
- Test: `fmpl-core/tests/context_aware_compilation.rs`

**Step 1: Add mode to compile_pattern**

```rust
// fmpl-core/src/compiler.rs

impl Compiler {
    /// Compile pattern with specified mode
    pub fn compile_pattern_with_mode(
        &mut self,
        pattern: &Pattern,
        mode: PatternMode,
        source: InstrIndex,
    ) -> Result<()> {
        match mode {
            PatternMode::Fast => self.compile_pattern_fast(pattern, source),
            PatternMode::Full => self.compile_pattern_full(pattern, source),
        }
    }

    /// Fast path: direct extraction (for let bindings)
    fn compile_pattern_fast(
        &mut self,
        pattern: &Pattern,
        source: InstrIndex,
    ) -> Result<()> {
        match pattern {
            Pattern::Any => {
                // Nothing to bind
            }
            Pattern::Var(name) => {
                self.bound_vars.insert(name.clone());
                self.code.emit(Instruction::Bind {
                    name: name.clone(),
                    value: source,
                });
            }
            Pattern::Map(entries) => {
                for (key, value_pattern) in entries {
                    let extracted = self.code.emit(Instruction::ExtractMapKey {
                        source,
                        key: key.clone(),
                    });
                    self.compile_pattern_fast(value_pattern, extracted)?;
                }
            }
            Pattern::List(ListPattern::Exact(patterns)) => {
                for (i, pat) in patterns.iter().enumerate() {
                    let extracted = self.code.emit(Instruction::ExtractListIndex {
                        source,
                        index: i,
                    });
                    self.compile_pattern_fast(pat, extracted)?;
                }
            }
            Pattern::Tagged { tag: _, patterns } => {
                for (i, pat) in patterns.iter().enumerate() {
                    let extracted = self.code.emit(Instruction::ExtractTaggedChild {
                        source,
                        index: i,
                    });
                    self.compile_pattern_fast(pat, extracted)?;
                }
            }
            Pattern::Literal(lit) => {
                // Fast path can't check literals - this is a limitation
                // For now, just bind without checking (or emit runtime check)
                // TODO: Add runtime literal check instruction
            }
            _ => {
                return Err(Error::Compiler(format!(
                    "pattern {:?} requires full mode (backtracking/guards)",
                    pattern
                )));
            }
        }
        Ok(())
    }

    /// Full path: grammar matching (for @ operator)
    fn compile_pattern_full(
        &mut self,
        pattern: &Pattern,
        source: InstrIndex,
    ) -> Result<InstrIndex> {
        // Use existing compile_grammar_pattern
        self.compile_grammar_pattern(pattern)
    }
}
```

**Step 2: Update let binding compilation**

```rust
// fmpl-core/src/compiler.rs

Expr::Let { pattern, value, body } => {
    self.code.emit(Instruction::BlockStart);
    let value_idx = self.compile_expr(value)?;

    // Use fast mode for let bindings
    self.compile_pattern_with_mode(pattern, PatternMode::Fast, value_idx)?;

    let body_idx = self.compile_expr(body)?;
    self.code.emit(Instruction::BlockEnd);
    Ok(body_idx)
}
```

**Step 3: Add tests**

```rust
// fmpl-core/tests/context_aware_compilation.rs

#[test]
fn test_let_binding_uses_fast_mode() {
    let source = r#"
        fn test() {
          let %{x: a, y: b} = %{x: 1, y: 2}
          a + b
        }
    "#;

    let result = eval(source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::Int(3));
}

#[test]
fn test_at_operator_uses_full_mode() {
    let source = r#"
        grammar Test {
          main = "hello" | "world"
        }

        fn test() {
          "hello" @ Test.main
        }
    "#;

    let result = eval(source).unwrap();
    // Should successfully parse using full grammar matching
    assert!(result.is_ok());
}
```

**Step 4: Run tests**

Run: `cargo test -p fmpl-core context_aware_compilation`
Expected: PASS

**Step 5: Commit**

```bash
jj add fmpl-core/src/compiler.rs fmpl-core/tests/context_aware_compilation.rs
jj commit -m "feat(compiler): add context-aware pattern compilation

- Let bindings use fast mode (direct extraction)
- @ operator uses full mode (grammar matching)
- compile_pattern_with_mode() chooses strategy based on mode
- Fast mode: ExtractMapKey, ExtractListIndex, ExtractTaggedChild
- Full mode: compile_grammar_pattern with backtracking/guards
"
```

---

## Phase 5: Integration and Testing

**Goal:** End-to-end testing and documentation.

### Task 5.1: Add comprehensive integration tests

**Files:**
- Create: `fmpl-core/tests/integration_pattern_unification.rs`
- Create: `fmpl-core/tests/integration_polymorphic_streams.rs`

**Step 1: Create integration tests**

```rust
// fmpl-core/tests/integration_pattern_unification.rs

//! Integration tests for unified pattern and grammar system

use fmpl_core::{compile, value::Value, vm::VM};

fn run(source: &str) -> Result<Value, String> {
    let mut vm = VM::new();
    let code = compile(source).map_err(|e| e.to_string())?;
    vm.load_code("test", code);
    vm.run_function("test", &[]).map_err(|e| e.to_string())
}

#[test]
fn test_let_destructuring_map() {
    let src = r#"
        fn test() {
          let %{name: n, age: a} = %{name: "Alice", age: 30}
          n
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::string("Alice"));
}

#[test]
fn test_let_destructuring_list() {
    let src = r#"
        fn test() {
          let [x, y, z] = [1, 2, 3]
          x + y + z
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::Int(6));
}

#[test]
fn test_let_destructuring_tagged() {
    let src = r#"
        fn test() {
          let :Some(v) = :Some(42)
          v
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::Int(42));
}

#[test]
fn test_at_operator_with_named_grammar() {
    let src = r#"
        grammar Parser {
          main = "hello"
        }

        fn test() {
          "hello" @ Parser.main
        }
    "#;
    assert!(run(src).is_ok());
}

#[test]
fn test_at_operator_with_inline_block() {
    let src = r#"
        fn test() {
          %{x: 10} @ {
            %{x: v} => v
            _ => 0
          }
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::Int(10));
}

#[test]
fn test_at_operator_with_guard() {
    let src = r#"
        fn test() {
          42 @ {
            n when n > 50 => 100
            n => n * 2
          }
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::Int(84));
}

#[test]
fn test_nested_pattern_matching() {
    let src = r#"
        fn test() {
          let %{user: %{name: n}} = %{user: %{name: "Bob"}}
          n
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::string("Bob"));
}

#[test]
fn test_choice_in_inline_block() {
    let src = r#"
        fn test() {
          :Some(5) @ {
            :Some(x) => x
            :None => 0
          }
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::Int(5));
}
```

**Step 2: Create polymorphic stream tests**

```rust
// fmpl-core/tests/integration_polymorphic_streams.rs

//! Tests for polymorphic stream coercion in @ operator

use fmpl_core::{compile, value::Value, vm::VM};

fn run(source: &str) -> Result<Value, String> {
    let mut vm = VM::new();
    let code = compile(source).map_err(|e| e.to_string())?;
    vm.load_code("test", code);
    vm.run_function("test", &[]).map_err(|e| e.to_string())
}

#[test]
fn test_string_to_char_stream() {
    let src = r#"
        grammar Chars {
          main = 'a' 'b' 'c'
        }

        fn test() {
          "abc" @ Chars.main
        }
    "#;
    assert!(run(src).is_ok());
}

#[test]
fn test_list_to_element_stream() {
    let src = r#"
        grammar Items {
          main = 1 2 3
        }

        fn test() {
          [1, 2, 3] @ Items.main
        }
    "#;
    assert!(run(src).is_ok());
}

#[test]
fn test_map_to_single_element_stream() {
    let src = r#"
        grammar Single {
          main = %{x: 1}
        }

        fn test() {
          %{x: 1} @ Single.main
        }
    "#;
    assert!(run(src).is_ok());
}

#[test]
fn test_tagged_to_single_element_stream() {
    let src = r#"
        grammar Tag {
          main = :Foo(42)
        }

        fn test() {
          :Foo(42) @ Tag.main
        }
    "#;
    assert!(run(src).is_ok());
}

#[test]
fn test_auto_coercion_string() {
    // @ operator should auto-detect string needs char stream
    let src = r#"
        fn test() {
          "hello" @ {
            'h' 'e' 'l' 'l' 'o' => "matched"
            _ => "no match"
          }
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::string("matched"));
}

#[test]
fn test_auto_coercion_list() {
    // @ operator should auto-detect list needs element stream
    let src = r#"
        fn test() {
          [1, 2, 3] @ {
            1 2 3 => "matched"
            _ => "no match"
          }
        }
    "#;
    assert_eq!(run(src).unwrap(), Value::string("matched"));
}
```

**Step 3: Run integration tests**

Run: `cargo test -p fmpl-core integration_`
Expected: PASS

**Step 4: Fix any failing tests**

Debug and fix any issues revealed by integration tests.

**Step 5: Commit**

```bash
jj add fmpl-core/tests/
jj commit -m "test(integration): add comprehensive integration tests

- Test let destructuring: maps, lists, tagged values
- Test @ operator with named grammars and inline blocks
- Test guards and choices in inline blocks
- Test polymorphic stream coercion for strings, lists, maps
- Test nested pattern matching
"
```

---

### Task 5.2: Update documentation

**Files:**
- Modify: `docs/design/language-guide.md`
- Create: `docs/pattern-matching-unified.md`

**Step 1: Create pattern matching documentation**

```markdown
<!-- docs/pattern-matching-unified.md -->

# Unified Pattern Matching in FMPL

FMPL provides a unified pattern matching system that works in multiple contexts:
- Variable bindings (`let`)
- Grammar application (`@`)
- Anonymous inline pattern blocks

## Pattern Syntax

| Pattern | Description | Example |
|---------|-------------|---------|
| `_` | Wildcard - matches anything | `let _ = value` |
| `x` | Variable binding | `let x = value` |
| `"literal"` | Literal match | `"hello"` |
| `123` | Number literal | `42` |
| `%{k: p}` | Map pattern | `%{x: a, y: b}` |
| `[p1, p2]` | List pattern | `[first, second]` |
| `[h \| t]` | Head/tail split | `[head \| tail]` |
| `[p*]` | Repeat pattern | `[item*]` |
| `:Tag(p1)` | Tagged pattern | `:Some(x)` |
| `p1 p2` | Sequence | `'a' 'b' 'c'` |
| `p1 \| p2` | Ordered choice | `"hello" \| "world"` |
| `p*` | Zero or more | `'a'*` |
| `p+` | One or more | `'a'+` |
| `p?` | Optional | `'a'?` |
| `&p` | Positive lookahead | `&"hello"` |
| `!p` | Negative lookahead | `!"error"` |
| `p when e` | Guard | `x when x > 0` |
| `p => e` | Action | `x => x * 2` |

## Let Bindings (Fast Mode)

In `let` bindings, patterns use direct extraction for performance:

```fmpl
let %{x: a, y: b} = point
let [first, ...rest] = items
let :Some(value) = result
```

**Limitations:** No backtracking, no guards, no ordered choice.

## Grammar Application (Full Mode)

The `@` operator applies patterns with full PEG semantics:

```fmpl
-- Named grammar
input @ Parser.rule

-- Inline pattern block
input @ {
  %{type: "move", dir: d} => move(d)
  %{type: "quit"} => exit()
  _ => continue()
}

-- With guards
value @ {
  x when x > 0 => "positive"
  x when x < 0 => "negative"
  _ => "zero"
}
```

## Polymorphic Stream Coercion

The `@` operator coerces input to appropriate stream type:

| Input Type | Stream Behavior |
|------------|-----------------|
| String | Character stream |
| List | Element stream |
| Map/Tagged | Single-element stream |

```fmpl
"hello" @ CharParser      -- Character stream
[1, 2, 3] @ ListParser     -- Element stream
%{x: 1} @ MapParser        -- Single-element stream
```

## Compilation Modes

Patterns compile differently based on context:

| Context | Mode | Instructions |
|---------|------|--------------|
| `let` binding | Fast | ExtractMapKey, ExtractListIndex, etc. |
| `@` operator | Full | MatchSeq, MatchChoice, MatchGuard, etc. |

Fast mode: Direct extraction, no backtracking.
Full mode: Full PEG matching with backtracking and guards.
```

**Step 2: Update language guide**

Add section on unified pattern matching to `docs/design/language-guide.md`.

**Step 3: Commit**

```bash
jj add docs/pattern-matching-unified.md docs/design/language-guide.md
jj commit -m "docs(pattern): add unified pattern matching documentation

- Document unified pattern syntax for all contexts
- Explain let binding (fast mode) vs @ operator (full mode)
- Document polymorphic stream coercion
- Add pattern reference table with examples
"
```

---

## Phase 6: Cleanup and Optimization

**Goal:** Remove deprecated code, optimize performance.

### Task 6.1: Remove deprecated pattern types

**Files:**
- Modify: `fmpl-core/src/ast.rs`
- Modify: `fmpl-core/src/grammar/mod.rs`

**Step 1: Remove old Pattern enums**

After migration is complete, remove the old separate pattern types.

**Step 2: Update all references**

Search for uses of `ast::Pattern` and `grammar::Pattern` and update to use `crate::pattern::Pattern`.

**Step 3: Commit**

```bash
jj commit -m "refactor(pattern): remove deprecated pattern types

- Remove old ast::Pattern (now using unified pattern::Pattern)
- Remove old grammar::Pattern (now using unified pattern::Pattern)
- Update all references to use crate::pattern::Pattern
"
```

---

### Task 6.2: Performance optimization

**Files:**
- Modify: `fmpl-core/src/compiler.rs`
- Modify: `fmpl-core/src/vm.rs`

**Step 1: Optimize hot paths**

Profile and optimize the most common patterns:
- Simple map extraction
- List head/tail
- Tagged value extraction

**Step 2: Add benchmarks**

```rust
// fmpl-benches/src/pattern_matching.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fmpl_core::compile;

fn bench_let_map_pattern(c: &mut Criterion) {
    c.bench_function("let_map_pattern", |b| {
        b.iter(|| {
            let src = r#"
                fn test() {
                  let %{x: a, y: b} = %{x: 1, y: 2}
                  a + b
                }
            "#;
            // Compile and run
        });
    });
}

criterion_group!(benches, bench_let_map_pattern);
criterion_main!(benches);
```

**Step 3: Run benchmarks**

Run: `cargo bench -p fmpl-benches pattern_matching`

**Step 4: Optimize based on results**

Improve performance based on benchmark findings.

**Step 5: Commit**

```bash
jj commit -m "perf(pattern): optimize pattern matching hot paths

- Optimize simple map extraction
- Optimize list head/tail patterns
- Optimize tagged value extraction
- Add benchmarks for pattern matching performance
"
```

---

## Summary

This plan unifies FMPL's pattern matching and grammar systems through:

1. **Unified Pattern Type** - Single `Pattern` enum for all contexts
2. **Context-Aware Compilation** - Fast mode (let) vs Full mode (@)
3. **Polymorphic Stream Coercion** - String→chars, List→items, other→once
4. **Anonymous Inline Blocks** - `x @ { %{a: b} => b }` syntax
5. **Comprehensive Testing** - Integration tests for all features
6. **Documentation** - Complete reference for unified patterns

**Benefits:**
- Single pattern syntax to learn
- Performance where it matters (let bindings)
- Full expressiveness when needed (guards, backtracking)
- Polymorphic @ operator works on all data types
- Anonymous pattern blocks for concise code
