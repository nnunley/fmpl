//! Unified pattern type for FMPL

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

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
    Tagged {
        tag: SmolStr,
        patterns: Vec<Pattern>,
    },

    /// Character pattern (for strings) - 'a' or [a-z]
    Char(CharPattern),

    /// Sequence - p1 p2 p3 (ordered, all must match)
    Seq(Vec<Pattern>),

    /// Ordered choice - p1 | p2 | p3 (try first that matches)
    Choice(Vec<Pattern>),

    /// Repetition - p* (zero or more) or p+ (one or more)
    Repeat {
        pattern: Box<Pattern>,
        kind: RepeatKind,
    },

    /// Optional - p? (zero or one)
    Optional(Box<Pattern>),

    /// Lookahead - &p (positive) or !p (negative)
    Lookahead {
        pattern: Box<Pattern>,
        positive: bool,
    },

    /// Binding - name: pattern or pattern when guard
    Bind {
        name: SmolStr,
        pattern: Box<Pattern>,
    },

    /// Guard - pattern when predicate
    Guard {
        pattern: Box<Pattern>,
        predicate: GuardPredicate,
    },

    /// Action - pattern => expr
    Action {
        pattern: Box<Pattern>,
        action: SmolStr,
    }, // action is expr string

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
    Exact(Vec<Pattern>), // [p1, p2, p3]
    HeadTail {
        head: Box<Pattern>,
        tail: Option<SmolStr>,
    }, // [h | t]
    Repeat {
        element: Box<Pattern>,
    }, // [p*]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharPattern {
    Exact(char),
    Class(Vec<(char, char)>),        // [a-z]
    NegatedClass(Vec<(char, char)>), // [^a-z]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RepeatKind {
    ZeroOrMore, // p*
    OneOrMore,  // p+
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardPredicate {
    Expr(SmolStr),      // Expression to evaluate
    TypeCheck(SmolStr), // Check type: is_list, is_map, etc.
}
