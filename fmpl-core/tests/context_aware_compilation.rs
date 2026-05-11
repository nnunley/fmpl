//! Tests for context-aware pattern compilation with PatternMode
//!
//! This file tests Task 4.1: Mode parameter for pattern compilation
//! - Fast mode: Uses ExtractMapKey, ExtractListIndex, ExtractTaggedChild (no backtracking)
//! - Full mode: Uses grammar-style matching with backtracking/guards support

use fmpl_core::ast::Expr;
use fmpl_core::compiler::{Compiler, Instruction};
use fmpl_core::pattern::{
    CharPattern, CharRange, ListPattern, LiteralValue, Pattern, PatternMode, RepeatKind,
};
use smol_str::SmolStr;

/// Helper to compile a pattern and get the emitted instructions
fn compile_pattern_with_mode(pattern: &Pattern, mode: PatternMode) -> Vec<Instruction> {
    let mut compiler = Compiler::new();
    // Emit a dummy source value to extract from
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    // Compile the pattern with the specified mode
    let _ = compiler
        .compile_pattern_with_mode(pattern, source_idx, mode)
        .expect("pattern compilation failed");

    compiler.code().instructions.to_vec()
}

// ============================================================================
// Fast Mode Tests: Direct extraction without backtracking
// ============================================================================

#[test]
fn test_fast_mode_map_pattern_uses_extract_map_key() {
    // Pattern: %{name: n, age: a}
    let pattern = Pattern::Map(vec![
        (SmolStr::new("name"), Pattern::Var(SmolStr::new("n"))),
        (SmolStr::new("age"), Pattern::Var(SmolStr::new("a"))),
    ]);

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should contain ExtractMapKey instructions
    let has_extract_map_key = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ExtractMapKey { .. }));
    assert!(
        has_extract_map_key,
        "Fast mode map pattern should use ExtractMapKey instruction"
    );

    // Should NOT contain any backtracking instructions
    let has_checkpoint = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ParseCheckpoint));
    assert!(
        !has_checkpoint,
        "Fast mode should not generate ParseCheckpoint"
    );
}

#[test]
fn test_fast_mode_list_pattern_uses_extract_list_index() {
    // Pattern: [a, b, c]
    let pattern = Pattern::List(ListPattern::Exact(vec![
        Pattern::Var(SmolStr::new("a")),
        Pattern::Var(SmolStr::new("b")),
        Pattern::Var(SmolStr::new("c")),
    ]));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should contain ExtractListIndex instructions
    let extract_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::ExtractListIndex { .. }))
        .count();
    assert_eq!(
        extract_count, 3,
        "Fast mode list pattern should emit 3 ExtractListIndex instructions"
    );

    // Should NOT contain MatchList instruction (that's for full mode)
    let has_match_list = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchList { .. }));
    assert!(
        !has_match_list,
        "Fast mode should not use MatchList instruction"
    );
}

#[test]
fn test_fast_mode_tagged_pattern_uses_extract_tagged_child() {
    // List-pattern [:Point, x, y] (ITER-0004d.1 T12 — Pattern::Tagged deleted)
    let pattern = Pattern::ListMatch(
        vec![
            Pattern::SymbolLiteral(SmolStr::new("Point")),
            Pattern::Var(SmolStr::new("x")),
            Pattern::Var(SmolStr::new("y")),
        ],
        None,
    );

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should contain ExtractTaggedChild instructions
    let extract_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::ExtractTaggedChild { .. }))
        .count();
    assert_eq!(
        extract_count, 2,
        "Fast mode tagged pattern should emit 2 ExtractTaggedChild instructions"
    );

    // Should NOT contain MatchTagged (full mode matching)
    let has_match_tagged = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchTagged { .. }));
    assert!(
        !has_match_tagged,
        "Fast mode should not use MatchTagged instruction"
    );
}

#[test]
fn test_fast_mode_variable_pattern_uses_bind() {
    // Pattern: x
    let pattern = Pattern::Var(SmolStr::new("x"));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should contain Bind instruction
    let has_bind = instructions
        .iter()
        .any(|i| matches!(i, Instruction::Bind { name, .. } if name == "x"));
    assert!(
        has_bind,
        "Fast mode variable pattern should emit Bind instruction"
    );
}

#[test]
fn test_fast_mode_wildcard_pattern_no_bind() {
    // Pattern: _
    let pattern = Pattern::Any;

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should NOT contain Bind instruction (wildcard doesn't bind)
    let has_bind = instructions
        .iter()
        .any(|i| matches!(i, Instruction::Bind { .. }));
    assert!(
        !has_bind,
        "Fast mode wildcard pattern should not emit Bind instruction"
    );
}

#[test]
fn test_fast_mode_nested_map_pattern() {
    // Pattern: %{user: %{name: n}}
    let pattern = Pattern::Map(vec![(
        SmolStr::new("user"),
        Pattern::Map(vec![(
            SmolStr::new("name"),
            Pattern::Var(SmolStr::new("n")),
        )]),
    )]);

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should have nested ExtractMapKey calls
    let extract_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::ExtractMapKey { .. }))
        .count();
    assert!(
        extract_count >= 2,
        "Nested map pattern should emit multiple ExtractMapKey instructions"
    );
}

#[test]
fn test_fast_mode_head_tail_pattern() {
    // Pattern: [h | t]
    let pattern = Pattern::List(ListPattern::HeadTail {
        head: Box::new(Pattern::Var(SmolStr::new("h"))),
        tail: Some(SmolStr::new("t")),
    });

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Fast);

    // Should extract first element
    let has_extract_index = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ExtractListIndex { index: 0, .. }));
    assert!(
        has_extract_index,
        "Head-tail pattern should extract first element"
    );

    // Should use Slice for tail
    let has_slice = instructions
        .iter()
        .any(|i| matches!(i, Instruction::Slice { .. }));
    assert!(has_slice, "Head-tail pattern should use Slice for tail");
}

// ============================================================================
// Fast Mode Error Cases: Patterns that require full mode
// ============================================================================

#[test]
fn test_fast_mode_rejects_seq_pattern() {
    // Pattern: Seq requires backtracking
    let pattern = Pattern::Seq(vec![Pattern::Any, Pattern::Any]);

    let mut compiler = Compiler::new();
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    let result = compiler.compile_pattern_with_mode(&pattern, source_idx, PatternMode::Fast);

    assert!(
        result.is_err(),
        "Fast mode should reject Seq patterns (require backtracking)"
    );
}

#[test]
fn test_fast_mode_rejects_choice_pattern() {
    // Pattern: Choice requires backtracking
    let pattern = Pattern::Choice(vec![(Pattern::Any, false), (Pattern::Any, false)]);

    let mut compiler = Compiler::new();
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    let result = compiler.compile_pattern_with_mode(&pattern, source_idx, PatternMode::Fast);

    assert!(
        result.is_err(),
        "Fast mode should reject Choice patterns (require backtracking)"
    );
}

#[test]
fn test_fast_mode_rejects_guard_pattern() {
    // Pattern: Guard requires runtime check
    let pattern = Pattern::Guard {
        pattern: Box::new(Pattern::Any),
        predicate: Expr::Bool(true),
    };

    let mut compiler = Compiler::new();
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    let result = compiler.compile_pattern_with_mode(&pattern, source_idx, PatternMode::Fast);

    assert!(result.is_err(), "Fast mode should reject Guard patterns");
}

#[test]
fn test_fast_mode_rejects_repeat_pattern() {
    // Pattern: Repeat requires backtracking
    let pattern = Pattern::Repeat {
        pattern: Box::new(Pattern::Any),
        kind: RepeatKind::ZeroOrMore,
    };

    let mut compiler = Compiler::new();
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    let result = compiler.compile_pattern_with_mode(&pattern, source_idx, PatternMode::Fast);

    assert!(result.is_err(), "Fast mode should reject Repeat patterns");
}

#[test]
fn test_fast_mode_rejects_char_pattern() {
    // Pattern: Char is for string parsing
    let pattern = Pattern::Char(CharPattern::Exact('a'));

    let mut compiler = Compiler::new();
    let source_idx = compiler.code_mut().emit(Instruction::LoadNull);

    let result = compiler.compile_pattern_with_mode(&pattern, source_idx, PatternMode::Fast);

    assert!(
        result.is_err(),
        "Fast mode should reject Char patterns (for string parsing)"
    );
}

// ============================================================================
// Full Mode Tests: Grammar-style matching with backtracking
// ============================================================================

#[test]
fn test_full_mode_map_pattern_uses_match_map() {
    // Pattern: %{type: t}
    let pattern = Pattern::Map(vec![(
        SmolStr::new("type"),
        Pattern::Var(SmolStr::new("t")),
    )]);

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchMap instruction
    let has_match_map = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchMap { .. }));
    assert!(
        has_match_map,
        "Full mode map pattern should use MatchMap instruction"
    );
}

#[test]
fn test_full_mode_list_pattern_uses_match_list() {
    // Pattern: [a, b]
    let pattern = Pattern::List(ListPattern::Exact(vec![
        Pattern::Var(SmolStr::new("a")),
        Pattern::Var(SmolStr::new("b")),
    ]));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchList instruction
    let has_match_list = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchList { .. }));
    assert!(
        has_match_list,
        "Full mode list pattern should use MatchList instruction"
    );
}

// ITER-0004d.1 T12: deleted test_full_mode_tagged_pattern_uses_match_tagged.
// It asserted that Full-mode tagged-pattern compilation emits the
// Instruction::MatchTagged opcode. That assertion was an implementation-
// detail check (which opcode is chosen), not a behavioral check. The opcode
// is also scheduled for rename in ITER-0004d.2 (MatchTagged -> MatchListNode).
// End-to-end behavior is covered by scenario_0103 and ast_to_ir_parity.

#[test]
fn test_full_mode_seq_pattern_uses_match_seq() {
    // Pattern: a b c (sequence)
    let pattern = Pattern::Seq(vec![Pattern::Any, Pattern::Any, Pattern::Any]);

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchSeq instruction
    let has_match_seq = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchSeq { .. }));
    assert!(
        has_match_seq,
        "Full mode seq pattern should use MatchSeq instruction"
    );
}

#[test]
fn test_full_mode_choice_pattern_with_backtracking() {
    // Pattern: a | b (choice)
    let pattern = Pattern::Choice(vec![
        (
            Pattern::Literal(LiteralValue::String(SmolStr::new("a"))),
            false,
        ),
        (
            Pattern::Literal(LiteralValue::String(SmolStr::new("b"))),
            false,
        ),
    ]);

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain ParseCheckpoint for backtracking
    let has_checkpoint = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ParseCheckpoint));
    assert!(
        has_checkpoint,
        "Full mode choice pattern should use ParseCheckpoint for backtracking"
    );

    // Should contain ParseRestore for backtracking
    let has_restore = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ParseRestore { .. }));
    assert!(
        has_restore,
        "Full mode choice pattern should use ParseRestore for backtracking"
    );
}

#[test]
fn test_full_mode_guard_pattern_uses_match_guard() {
    // Pattern: _ when x > 0
    let pattern = Pattern::Guard {
        pattern: Box::new(Pattern::Var(SmolStr::new("x"))),
        predicate: Expr::Bool(true),
    };

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchGuard instruction
    let has_match_guard = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchGuard { .. }));
    assert!(
        has_match_guard,
        "Full mode guard pattern should use MatchGuard instruction"
    );
}

#[test]
fn test_full_mode_optional_pattern_uses_match_optional() {
    // Pattern: x?
    let pattern = Pattern::Optional(Box::new(Pattern::Var(SmolStr::new("x"))));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchOptional instruction
    let has_match_optional = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchOptional { .. }));
    assert!(
        has_match_optional,
        "Full mode optional pattern should use MatchOptional instruction"
    );
}

#[test]
fn test_full_mode_lookahead_positive() {
    // Pattern: &p (positive lookahead)
    let pattern = Pattern::Lookahead(Box::new(Pattern::Any));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchLookahead instruction
    let has_lookahead = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchLookahead { .. }));
    assert!(
        has_lookahead,
        "Full mode positive lookahead should use MatchLookahead instruction"
    );
}

#[test]
fn test_full_mode_lookahead_negative() {
    // Pattern: !p (negative lookahead)
    let pattern = Pattern::Not(Box::new(Pattern::Any));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchNot instruction
    let has_not = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchNot { .. }));
    assert!(
        has_not,
        "Full mode negative lookahead should use MatchNot instruction"
    );
}

#[test]
fn test_full_mode_char_pattern() {
    // Pattern: 'a' (character match)
    let pattern = Pattern::Char(CharPattern::Exact('a'));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchChar instruction
    let has_match_char = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchChar { .. }));
    assert!(
        has_match_char,
        "Full mode char pattern should use MatchChar instruction"
    );
}

#[test]
fn test_full_mode_char_class_pattern() {
    // Pattern: [a-z]
    let pattern = Pattern::Char(CharPattern::Class(vec![CharRange::Range('a', 'z')]));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain MatchCharClass instruction
    let has_match_char_class = instructions
        .iter()
        .any(|i| matches!(i, Instruction::MatchCharClass { .. }));
    assert!(
        has_match_char_class,
        "Full mode char class pattern should use MatchCharClass instruction"
    );
}

#[test]
fn test_full_mode_apply_rule_pattern() {
    // Pattern: rule
    let pattern = Pattern::ApplyRule(SmolStr::new("digit"));

    let instructions = compile_pattern_with_mode(&pattern, PatternMode::Full);

    // Should contain ApplyRule instruction
    let has_apply_rule = instructions
        .iter()
        .any(|i| matches!(i, Instruction::ApplyRule { .. }));
    assert!(
        has_apply_rule,
        "Full mode ApplyRule pattern should use ApplyRule instruction"
    );
}

// ============================================================================
// Mode Selection Tests: Verify patterns use correct mode
// ============================================================================

#[test]
fn test_mode_auto_selection_fast_patterns() {
    // These patterns should recommend Fast mode
    assert_eq!(Pattern::Any.recommended_mode(), PatternMode::Fast);
    assert_eq!(
        Pattern::Var(SmolStr::new("x")).recommended_mode(),
        PatternMode::Fast
    );
    assert_eq!(Pattern::Map(vec![]).recommended_mode(), PatternMode::Fast);
    assert_eq!(
        Pattern::List(ListPattern::Exact(vec![])).recommended_mode(),
        PatternMode::Fast
    );
    assert_eq!(
        Pattern::ListMatch(vec![Pattern::SymbolLiteral(SmolStr::new("X"))], None,)
            .recommended_mode(),
        PatternMode::Fast
    );
}

#[test]
fn test_mode_auto_selection_full_patterns() {
    // These patterns should recommend Full mode
    assert_eq!(Pattern::Seq(vec![]).recommended_mode(), PatternMode::Full);
    assert_eq!(
        Pattern::Choice(vec![(Pattern::Any, false)]).recommended_mode(),
        PatternMode::Full
    );
    assert_eq!(
        Pattern::Repeat {
            pattern: Box::new(Pattern::Any),
            kind: RepeatKind::ZeroOrMore
        }
        .recommended_mode(),
        PatternMode::Full
    );
    assert_eq!(
        Pattern::Guard {
            pattern: Box::new(Pattern::Any),
            predicate: Expr::Bool(true)
        }
        .recommended_mode(),
        PatternMode::Full
    );
    assert_eq!(
        Pattern::Char(CharPattern::Exact('a')).recommended_mode(),
        PatternMode::Full
    );
}
