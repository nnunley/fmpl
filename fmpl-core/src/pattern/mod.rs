//! Unified pattern type for FMPL
//!
//! This module provides a single Pattern enum that unifies:
//! - Let binding patterns (fast path: direct extraction)
//! - Grammar patterns (full path: PEG matching with backtracking)
//!
//! The same pattern can be compiled differently based on context.

use crate::ast::Expr;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// A character range for character classes (shared with grammar module).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharRange {
    /// Single character.
    Char(char),
    /// Range of characters (inclusive).
    Range(char, char),
}

impl CharRange {
    pub fn matches(&self, c: char) -> bool {
        match self {
            CharRange::Char(ch) => c == *ch,
            CharRange::Range(lo, hi) => c >= *lo && c <= *hi,
        }
    }
}

/// Unified pattern type for both let bindings and grammar rules.
/// Compilation behavior depends on context (fast vs full path).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Pattern {
    // === Core patterns ===
    /// Match nothing, always succeed (empty pattern)
    #[default]
    Empty,

    /// Wildcard - matches anything, binds nothing
    Any,

    /// Variable binding - matches anything, binds to name
    Var(SmolStr),

    /// Literal value - matches exact value
    Literal(LiteralValue),

    // === Structural patterns ===
    /// Map pattern - %{key1: pattern1, key2: pattern2}
    Map(Vec<(SmolStr, Pattern)>),

    /// List pattern - [p1, p2, p3] or [h | t] or [p*]
    List(ListPattern),

    // === Text/Character patterns ===
    /// Character pattern (for strings) - 'a' or [a-z]
    Char(CharPattern),

    /// Match a literal string
    StringLiteral(SmolStr),

    // === Combinators ===
    /// Sequence - p1 p2 p3 (ordered, all must match)
    Seq(Vec<Pattern>),

    /// Ordered choice - p1 | p2 | p3 (try first that matches)
    /// Each (pattern, bool) tuple indicates if that alternative uses backtracking.
    Choice(Vec<(Pattern, bool)>),

    /// Repetition - p* (zero or more) or p+ (one or more)
    Repeat {
        pattern: Box<Pattern>,
        kind: RepeatKind,
    },

    /// Optional - p? (zero or one)
    Optional(Box<Pattern>),

    /// Positive lookahead - &p (succeeds if pattern matches, doesn't consume input)
    Lookahead(Box<Pattern>),

    /// Negative lookahead - !p (succeeds if pattern doesn't match, doesn't consume input)
    Not(Box<Pattern>),

    // === Binding and guards ===
    /// Binding - name:pattern or name:?pattern (choice point)
    /// The is_choice flag indicates if this is a choice point (digit:?x syntax).
    Bind {
        name: SmolStr,
        pattern: Box<Pattern>,
        is_choice: bool,
    },

    /// Guard - pattern &{ predicate }
    /// The pattern is matched first, then the guard expression is evaluated.
    Guard {
        pattern: Box<Pattern>,
        predicate: Expr,
    },

    /// Semantic predicate (evaluate expression, succeed if truthy) - &{ expr }
    /// Unlike Guard, this has no pattern to match first.
    Predicate(Expr),

    /// Action - pattern => expr
    /// The action is evaluated when the pattern matches successfully.
    Action { pattern: Box<Pattern>, action: Expr },

    // === Rule application ===
    /// Rule application - applies named grammar rule
    ApplyRule(SmolStr),

    /// Apply a rule from parent grammar (super call)
    Super(SmolStr),

    // === Binary patterns ===
    /// Binary pattern operations (byte-level parsing)
    Binary(BinaryPattern),

    // === Value/Tree patterns (for AST transformation) ===
    /// Match a specific FMPL value exactly
    MatchValue(Value),

    /// Match any value of a specific type (null, bool, int, float, string, symbol, list, map, object)
    MatchType(SmolStr),

    /// Match a list with specific element patterns and optional rest pattern
    ListMatch(Vec<Pattern>, Option<Box<Pattern>>),

    /// Match a map with specific key patterns
    MapMatch(Vec<(SmolStr, Pattern)>),

    /// Match a symbol with a specific name
    SymbolMatch(SmolStr),

    /// Match a symbol literal (like :foo in patterns)
    SymbolLiteral(SmolStr),

    /// Descend into a value and apply a pattern (for tree walking)
    /// When parsing a list, this pops an element and matches against it.
    Apply(Box<Pattern>),

    /// Match the end of the current input stream/list
    End,
}

impl Pattern {
    /// If this pattern is a `Bind`, return the bind name.
    pub fn bind_name(&self) -> Option<&SmolStr> {
        match self {
            Pattern::Bind { name, .. } => Some(name),
            _ => None,
        }
    }
}

/// Binary pattern operations for byte-level parsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryPattern {
    /// Match a specific byte value
    Byte(u8),
    /// Match a byte in a range (inclusive)
    ByteRange(u8, u8),
    /// Consume exactly n bytes, return as list of ints
    Bytes(usize),
    /// Read unsigned 8-bit integer
    UInt8,
    /// Read unsigned 16-bit big-endian integer
    UInt16BE,
    /// Read unsigned 16-bit little-endian integer
    UInt16LE,
    /// Read unsigned 32-bit big-endian integer
    UInt32BE,
    /// Read unsigned 32-bit little-endian integer
    UInt32LE,
    /// Read signed 8-bit integer
    Int8,
    /// Read signed 16-bit big-endian integer
    Int16BE,
    /// Read signed 16-bit little-endian integer
    Int16LE,
    /// Read signed 32-bit big-endian integer
    Int32BE,
    /// Read signed 32-bit little-endian integer
    Int32LE,
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
    /// Exact list pattern - matches [p1, p2, p3]
    Exact(Vec<Pattern>),
    /// Head-tail pattern - matches [h | t]
    HeadTail {
        head: Box<Pattern>,
        tail: Option<SmolStr>,
    },
    /// Repeat pattern - matches [p*]
    Repeat { element: Box<Pattern> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CharPattern {
    /// Exact character match - 'a'
    Exact(char),
    /// Character class match - [a-z] using CharRange
    Class(Vec<CharRange>),
    /// Negated character class match - [^a-z] using CharRange
    NegatedClass(Vec<CharRange>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatKind {
    /// Zero or more repetitions - p*
    ZeroOrMore,
    /// One or more repetitions - p+
    OneOrMore,
}

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

impl Pattern {
    /// Determine if pattern requires full matching (backtracking/guards)
    pub fn requires_full_mode(&self) -> bool {
        // Special case: a `ListMatch` with leading `SymbolLiteral` is the
        // tagged-shape pattern `[:Tag, ...]` — fast-mode compatible.
        // Replaces the legacy `Pattern::Tagged` (ITER-0004d.1 T12).
        if let Pattern::ListMatch(elems, None) = self
            && matches!(elems.first(), Some(Pattern::SymbolLiteral(_)))
        {
            return false;
        }
        matches!(
            self,
            Pattern::Seq(_)
                | Pattern::Choice(_)
                | Pattern::Repeat { .. }
                | Pattern::Lookahead(_)
                | Pattern::Not(_)
                | Pattern::Guard { .. }
                | Pattern::Action { .. }
                | Pattern::Predicate(_)
                | Pattern::Char(_)
                | Pattern::StringLiteral(_)
                | Pattern::Binary(_)
                | Pattern::List(ListPattern::Repeat { .. })
                | Pattern::ListMatch(_, _)
                | Pattern::MapMatch(_)
                | Pattern::Apply(_)
                | Pattern::End
        )
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

// Note: grammar::Pattern is now a re-export of pattern::Pattern,
// so no From conversion is needed anymore. They are the same type.
