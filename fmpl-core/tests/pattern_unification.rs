//! Tests for unified Pattern type

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
fn test_pattern_tagged() {
    let p = Pattern::Tagged {
        tag: SmolStr::new("Some"),
        patterns: vec![Pattern::Var(SmolStr::new("x"))],
    };
    assert!(matches!(p, Pattern::Tagged { .. }));
}

#[test]
fn test_pattern_literal() {
    let p = Pattern::Literal(LiteralValue::Int(42));
    assert!(matches!(p, Pattern::Literal(LiteralValue::Int(42))));
}

#[test]
fn test_pattern_char_class() {
    let p = Pattern::Char(CharPattern::Class(vec![('a', 'z'), ('A', 'Z')]));
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
    let p = Pattern::Choice(vec![
        Pattern::Literal(LiteralValue::String(SmolStr::new("hello"))),
        Pattern::Literal(LiteralValue::String(SmolStr::new("world"))),
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
    let p = Pattern::Lookahead {
        pattern: Box::new(Pattern::Any),
        positive: true,
    };
    assert!(matches!(p, Pattern::Lookahead { positive: true, .. }));
}

#[test]
fn test_pattern_bind() {
    let p = Pattern::Bind {
        name: SmolStr::new("x"),
        pattern: Box::new(Pattern::Any),
    };
    assert!(matches!(p, Pattern::Bind { .. }));
}

#[test]
fn test_pattern_guard() {
    let p = Pattern::Guard {
        pattern: Box::new(Pattern::Any),
        predicate: GuardPredicate::Expr(SmolStr::new("x > 0")),
    };
    assert!(matches!(p, Pattern::Guard { .. }));
}

#[test]
fn test_pattern_action() {
    let p = Pattern::Action {
        pattern: Box::new(Pattern::Any),
        action: SmolStr::new("x * 2"),
    };
    assert!(matches!(p, Pattern::Action { .. }));
}

#[test]
fn test_pattern_apply_rule() {
    let p = Pattern::ApplyRule(SmolStr::new("digit"));
    assert!(matches!(p, Pattern::ApplyRule(name) if name == "digit"));
}
