//! Tests for unified Pattern type

use fmpl_core::ast::Expr;
use fmpl_core::pattern::*;
use smol_str::SmolStr;

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
    let p = Pattern::Map(vec![(
        SmolStr::new("type"),
        Pattern::Var(SmolStr::new("t")),
    )]);
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
fn test_pattern_tagged_via_list() {
    // ITER-0004d.1 T12: Pattern::Tagged was deleted; tagged values are now
    // matched via list-pattern with a leading symbol: [:Tag, ...children].
    let p = Pattern::ListMatch(
        vec![
            Pattern::SymbolLiteral(SmolStr::new("Some")),
            Pattern::Var(SmolStr::new("x")),
        ],
        None,
    );
    assert!(matches!(p, Pattern::ListMatch(_, None)));
}

#[test]
fn test_pattern_literal() {
    let p = Pattern::Literal(LiteralValue::Int(42));
    assert!(matches!(p, Pattern::Literal(LiteralValue::Int(42))));
}

#[test]
fn test_pattern_char_class() {
    let p = Pattern::Char(CharPattern::Class(vec![
        CharRange::Range('a', 'z'),
        CharRange::Range('A', 'Z'),
    ]));
    assert!(matches!(p, Pattern::Char(CharPattern::Class(_))));
}

#[test]
fn test_pattern_seq() {
    let p = Pattern::Seq(vec![
        Pattern::Any,
        Pattern::Var(SmolStr::new("x")),
        Pattern::Any,
    ]);
    assert!(matches!(p, Pattern::Seq(_)));
}

#[test]
fn test_pattern_choice() {
    // Choice now takes (Pattern, bool) tuples
    let p = Pattern::Choice(vec![
        (
            Pattern::Literal(LiteralValue::String(SmolStr::new("hello"))),
            false,
        ),
        (
            Pattern::Literal(LiteralValue::String(SmolStr::new("world"))),
            false,
        ),
    ]);
    assert!(matches!(p, Pattern::Choice(_)));
}

#[test]
fn test_pattern_repeat() {
    let p = Pattern::Repeat {
        pattern: Box::new(Pattern::Any),
        kind: RepeatKind::ZeroOrMore,
    };
    assert!(matches!(
        p,
        Pattern::Repeat {
            kind: RepeatKind::ZeroOrMore,
            ..
        }
    ));
}

#[test]
fn test_pattern_optional() {
    let p = Pattern::Optional(Box::new(Pattern::Var(SmolStr::new("x"))));
    assert!(matches!(p, Pattern::Optional(_)));
}

#[test]
fn test_pattern_lookahead() {
    let p = Pattern::Lookahead(Box::new(Pattern::Any));
    assert!(matches!(p, Pattern::Lookahead(_)));
}

#[test]
fn test_pattern_not() {
    let p = Pattern::Not(Box::new(Pattern::Any));
    assert!(matches!(p, Pattern::Not(_)));
}

#[test]
fn test_pattern_bind() {
    let p = Pattern::Bind {
        name: SmolStr::new("x"),
        pattern: Box::new(Pattern::Any),
        is_choice: false,
    };
    assert!(matches!(p, Pattern::Bind { .. }));
}

#[test]
fn test_pattern_guard() {
    let guard_expr = Expr::Ident(SmolStr::new("x"));
    let p = Pattern::Guard {
        pattern: Box::new(Pattern::Any),
        predicate: guard_expr,
    };
    assert!(matches!(p, Pattern::Guard { .. }));
}

#[test]
fn test_pattern_action() {
    let action_expr = Expr::Ident(SmolStr::new("x"));
    let p = Pattern::Action {
        pattern: Box::new(Pattern::Any),
        action: action_expr,
    };
    assert!(matches!(p, Pattern::Action { .. }));
}

#[test]
fn test_pattern_apply_rule() {
    let p = Pattern::ApplyRule(SmolStr::new("digit"));
    assert!(matches!(p, Pattern::ApplyRule(name) if name == "digit"));
}

#[test]
fn test_fast_mode_patterns() {
    assert_eq!(Pattern::Any.recommended_mode(), PatternMode::Fast);
    assert_eq!(
        Pattern::Var(SmolStr::new("x")).recommended_mode(),
        PatternMode::Fast
    );

    let map_p = Pattern::Map(vec![(SmolStr::new("x"), Pattern::Var(SmolStr::new("y")))]);
    assert_eq!(map_p.recommended_mode(), PatternMode::Fast);

    // Literal patterns use fast mode
    let lit_p = Pattern::Literal(LiteralValue::Int(42));
    assert_eq!(lit_p.recommended_mode(), PatternMode::Fast);

    // Exact list patterns use fast mode
    let list_p = Pattern::List(ListPattern::Exact(vec![Pattern::Any]));
    assert_eq!(list_p.recommended_mode(), PatternMode::Fast);

    // List-pattern with leading symbol (tagged) uses fast mode
    let tagged_p = Pattern::ListMatch(
        vec![
            Pattern::SymbolLiteral(SmolStr::new("Some")),
            Pattern::Var(SmolStr::new("x")),
        ],
        None,
    );
    assert_eq!(tagged_p.recommended_mode(), PatternMode::Fast);

    // Optional uses fast mode
    let opt_p = Pattern::Optional(Box::new(Pattern::Any));
    assert_eq!(opt_p.recommended_mode(), PatternMode::Fast);

    // Bind uses fast mode
    let bind_p = Pattern::Bind {
        name: SmolStr::new("x"),
        pattern: Box::new(Pattern::Any),
        is_choice: false,
    };
    assert_eq!(bind_p.recommended_mode(), PatternMode::Fast);

    // ApplyRule uses fast mode
    let apply_p = Pattern::ApplyRule(SmolStr::new("digit"));
    assert_eq!(apply_p.recommended_mode(), PatternMode::Fast);
}

#[test]
fn test_full_mode_patterns() {
    // Seq requires full mode
    let seq_p = Pattern::Seq(vec![Pattern::Any, Pattern::Any]);
    assert_eq!(seq_p.recommended_mode(), PatternMode::Full);

    // Choice requires full mode (now with backtracking flags)
    let choice_p = Pattern::Choice(vec![(Pattern::Any, false), (Pattern::Any, false)]);
    assert_eq!(choice_p.recommended_mode(), PatternMode::Full);

    // Guard requires full mode (now with Box<Expr>)
    let guard_expr = Expr::Ident(SmolStr::new("true"));
    let guard_p = Pattern::Guard {
        pattern: Box::new(Pattern::Any),
        predicate: guard_expr,
    };
    assert_eq!(guard_p.recommended_mode(), PatternMode::Full);

    // Repeat requires full mode
    let repeat_p = Pattern::Repeat {
        pattern: Box::new(Pattern::Any),
        kind: RepeatKind::ZeroOrMore,
    };
    assert_eq!(repeat_p.recommended_mode(), PatternMode::Full);

    // Lookahead requires full mode
    let lookahead_p = Pattern::Lookahead(Box::new(Pattern::Any));
    assert_eq!(lookahead_p.recommended_mode(), PatternMode::Full);

    // Not requires full mode
    let not_p = Pattern::Not(Box::new(Pattern::Any));
    assert_eq!(not_p.recommended_mode(), PatternMode::Full);

    // Action requires full mode
    let action_expr = Expr::Ident(SmolStr::new("x"));
    let action_p = Pattern::Action {
        pattern: Box::new(Pattern::Any),
        action: action_expr,
    };
    assert_eq!(action_p.recommended_mode(), PatternMode::Full);

    // Char patterns require full mode (string parsing)
    let char_p = Pattern::Char(CharPattern::Exact('a'));
    assert_eq!(char_p.recommended_mode(), PatternMode::Full);

    // List repeat patterns require full mode
    let list_repeat_p = Pattern::List(ListPattern::Repeat {
        element: Box::new(Pattern::Any),
    });
    assert_eq!(list_repeat_p.recommended_mode(), PatternMode::Full);
}

#[test]
fn test_requires_full_mode() {
    // Fast patterns should not require full mode
    assert!(!Pattern::Any.requires_full_mode());
    assert!(!Pattern::Var(SmolStr::new("x")).requires_full_mode());
    assert!(!Pattern::Literal(LiteralValue::Bool(true)).requires_full_mode());

    // Full patterns should require full mode
    assert!(Pattern::Seq(vec![]).requires_full_mode());
    assert!(Pattern::Choice(vec![]).requires_full_mode());
    assert!(
        Pattern::Repeat {
            pattern: Box::new(Pattern::Any),
            kind: RepeatKind::OneOrMore
        }
        .requires_full_mode()
    );
}

// ============================================================================
// Crate root access tests
// ============================================================================

#[test]
fn test_unified_pattern_from_crate_root() {
    // Verify that Pattern is exported at crate root level
    use fmpl_core::Pattern;

    let p = Pattern::ListMatch(
        vec![
            Pattern::SymbolLiteral(SmolStr::new("Some")),
            Pattern::Var(SmolStr::new("x")),
        ],
        None,
    );

    assert!(
        matches!(&p, Pattern::ListMatch(elems, None) if matches!(elems.first(), Some(Pattern::SymbolLiteral(s)) if s == "Some"))
    );
}

#[test]
fn test_map_pattern_mode() {
    // Pattern that uses unified pattern type
    use fmpl_core::Pattern;

    let p = Pattern::Map(vec![
        (SmolStr::new("type"), Pattern::Var(SmolStr::new("t"))),
        (SmolStr::new("value"), Pattern::Var(SmolStr::new("v"))),
    ]);

    // Should use fast mode (no guards/backtracking)
    assert_eq!(p.recommended_mode(), PatternMode::Fast);
}

#[test]
fn test_guarded_pattern_requires_full() {
    // Pattern with guard requires full mode
    let guard_expr = Expr::Ident(SmolStr::new("v"));
    let p = Pattern::Guard {
        pattern: Box::new(Pattern::Map(vec![(
            SmolStr::new("x"),
            Pattern::Var(SmolStr::new("v")),
        )])),
        predicate: guard_expr,
    };

    // Should require full mode
    assert_eq!(p.recommended_mode(), PatternMode::Full);
}
