//! Grammar optimizer infrastructure.
//!
//! This module provides the `FirstSet` type for representing what characters
//! a grammar pattern can start with, enabling disjointness checks for
//! optimized dispatch.

use crate::pattern::{CharPattern, CharRange, LiteralValue, Pattern, RepeatKind};
use smol_str::SmolStr;
use std::collections::HashSet;

/// What a grammar pattern can start with.
#[derive(Debug, Clone, PartialEq)]
pub enum FirstSet {
    /// Specific characters.
    Chars(HashSet<char>),
    /// Character ranges (e.g., `[a-z]`).
    CharClass(Vec<CharRange>),
    /// Any character (`.` wildcard).
    AnyChar,
    /// A named rule reference (opaque until resolved).
    Rule(SmolStr),
    /// Can match the empty string.
    Epsilon,
}

impl FirstSet {
    /// Check whether a character is in this first set.
    pub fn contains(&self, c: char) -> bool {
        match self {
            FirstSet::Chars(set) => set.contains(&c),
            FirstSet::CharClass(ranges) => ranges.iter().any(|r| char_range_contains(r, c)),
            FirstSet::AnyChar => true,
            FirstSet::Rule(_) => false,
            FirstSet::Epsilon => false,
        }
    }

    /// Whether this first set includes epsilon (empty match).
    pub fn contains_epsilon(&self) -> bool {
        matches!(self, FirstSet::Epsilon)
    }

    /// Return a new `FirstSet` that also includes epsilon.
    ///
    /// If `self` is already `Epsilon`, returns `Epsilon`.
    /// Otherwise wraps the existing set so both the original chars
    /// and epsilon are representable. For simplicity, `AnyChar` with
    /// epsilon is still `AnyChar` (it already matches everything).
    pub fn with_epsilon(&self) -> FirstSet {
        match self {
            FirstSet::Epsilon => FirstSet::Epsilon,
            FirstSet::AnyChar => FirstSet::AnyChar,
            other => other.clone(),
        }
    }

    /// Compute the union of two first sets.
    pub fn union(&self, other: &FirstSet) -> FirstSet {
        match (self, other) {
            // Either is AnyChar → AnyChar dominates
            (FirstSet::AnyChar, _) | (_, FirstSet::AnyChar) => FirstSet::AnyChar,

            // Both epsilon
            (FirstSet::Epsilon, FirstSet::Epsilon) => FirstSet::Epsilon,

            // Epsilon with something else → the something else (epsilon absorbed)
            (FirstSet::Epsilon, other) | (other, FirstSet::Epsilon) => other.clone(),

            // Both Chars → merge
            (FirstSet::Chars(a), FirstSet::Chars(b)) => {
                FirstSet::Chars(a.union(b).copied().collect())
            }

            // Both CharClass → concatenate ranges
            (FirstSet::CharClass(a), FirstSet::CharClass(b)) => {
                let mut combined = a.clone();
                combined.extend(b.iter().cloned());
                FirstSet::CharClass(combined)
            }

            // Chars + CharClass → expand chars into the class
            (FirstSet::Chars(chars), FirstSet::CharClass(ranges))
            | (FirstSet::CharClass(ranges), FirstSet::Chars(chars)) => {
                let mut combined = ranges.clone();
                for &c in chars {
                    combined.push(CharRange::Char(c));
                }
                FirstSet::CharClass(combined)
            }

            // Rule + anything → conservative AnyChar
            (FirstSet::Rule(_), _) | (_, FirstSet::Rule(_)) => FirstSet::AnyChar,
        }
    }
}

/// Check whether a `CharRange` contains a given character.
fn char_range_contains(range: &CharRange, c: char) -> bool {
    match range {
        CharRange::Char(rc) => *rc == c,
        CharRange::Range(start, end) => c >= *start && c <= *end,
    }
}

/// Check whether a pattern can match the empty string.
fn is_nullable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Empty | Pattern::End | Pattern::Predicate(_) => true,
        Pattern::Optional(_) => true,
        Pattern::Repeat { kind, .. } => matches!(kind, RepeatKind::ZeroOrMore),
        Pattern::Seq(parts) => parts.iter().all(is_nullable),
        Pattern::Choice(alts) => alts.iter().any(|(alt, _)| is_nullable(alt)),
        Pattern::Bind { pattern, .. }
        | Pattern::Guard { pattern, .. }
        | Pattern::Action { pattern, .. } => is_nullable(pattern),
        _ => false,
    }
}

/// Compute the first set for a grammar pattern.
///
/// Returns the set of characters that the pattern can start matching with.
/// Used by the optimizer to determine if choice alternatives are disjoint.
pub fn compute_first_set(pattern: &Pattern) -> FirstSet {
    match pattern {
        Pattern::Empty => FirstSet::Epsilon,
        Pattern::Any => FirstSet::AnyChar,
        Pattern::Var(_) => FirstSet::AnyChar,

        Pattern::Literal(lit) => match lit {
            LiteralValue::String(s) => {
                if let Some(c) = s.chars().next() {
                    FirstSet::Chars([c].into_iter().collect())
                } else {
                    FirstSet::Epsilon
                }
            }
            // Non-string literals don't consume character input
            _ => FirstSet::AnyChar,
        },

        Pattern::StringLiteral(s) => {
            if let Some(c) = s.chars().next() {
                FirstSet::Chars([c].into_iter().collect())
            } else {
                FirstSet::Epsilon
            }
        }

        Pattern::Char(cp) => match cp {
            CharPattern::Exact(c) => FirstSet::Chars([*c].into_iter().collect()),
            CharPattern::Class(ranges) => FirstSet::CharClass(ranges.clone()),
            CharPattern::NegatedClass(_) => FirstSet::AnyChar,
        },

        Pattern::Seq(parts) => {
            let mut result = FirstSet::Epsilon;
            for part in parts {
                let fs = compute_first_set(part);
                result = if matches!(result, FirstSet::Epsilon) {
                    fs.clone()
                } else {
                    result.union(&fs)
                };
                if !is_nullable(part) {
                    return result;
                }
            }
            result
        }

        Pattern::Choice(alts) => {
            let mut combined = FirstSet::Epsilon;
            for (alt, _backtrack) in alts {
                combined = combined.union(&compute_first_set(alt));
            }
            combined
        }

        Pattern::Repeat {
            pattern: inner,
            kind,
        } => {
            let inner_fs = compute_first_set(inner);
            match kind {
                RepeatKind::ZeroOrMore => inner_fs.with_epsilon(),
                RepeatKind::OneOrMore => inner_fs,
            }
        }

        Pattern::Optional(inner) => compute_first_set(inner).with_epsilon(),

        Pattern::Lookahead(_) | Pattern::Not(_) => {
            // Lookaheads don't consume input; conservative
            FirstSet::AnyChar
        }

        Pattern::Bind { pattern: inner, .. } => compute_first_set(inner),

        Pattern::Guard { pattern: inner, .. } => compute_first_set(inner),

        Pattern::Action { pattern: inner, .. } => compute_first_set(inner),

        Pattern::ApplyRule(name) => FirstSet::Rule(name.clone()),

        Pattern::Super(name) => FirstSet::Rule(name.clone()),

        Pattern::End => FirstSet::Epsilon,

        Pattern::Predicate(_) => FirstSet::Epsilon,

        // Structural/value patterns — conservative
        Pattern::Map(_)
        | Pattern::List(_)
        | Pattern::Binary(_)
        | Pattern::MatchValue(_)
        | Pattern::MatchType(_)
        | Pattern::ListMatch(_, _)
        | Pattern::MapMatch(_)
        | Pattern::SymbolMatch(_)
        | Pattern::SymbolLiteral(_)
        | Pattern::Apply(_) => FirstSet::AnyChar,
    }
}

/// Check whether two first sets are disjoint (no character in common).
pub fn are_disjoint(a: &FirstSet, b: &FirstSet) -> bool {
    match (a, b) {
        (FirstSet::Chars(ca), FirstSet::Chars(cb)) => ca.is_disjoint(cb),
        (FirstSet::Chars(chars), FirstSet::CharClass(ranges))
        | (FirstSet::CharClass(ranges), FirstSet::Chars(chars)) => !chars
            .iter()
            .any(|c| ranges.iter().any(|r| char_range_contains(r, *c))),
        (FirstSet::CharClass(ra), FirstSet::CharClass(rb)) => !ranges_overlap(ra, rb),
        (FirstSet::AnyChar, _) | (_, FirstSet::AnyChar) => false,
        (FirstSet::Epsilon, FirstSet::Epsilon) => false,
        (FirstSet::Epsilon, _) | (_, FirstSet::Epsilon) => true,
        (FirstSet::Rule(a), FirstSet::Rule(b)) => a != b,
        (FirstSet::Rule(_), _) | (_, FirstSet::Rule(_)) => false,
    }
}

/// Check whether any character range in `ra` overlaps with any in `rb`.
fn ranges_overlap(ra: &[CharRange], rb: &[CharRange]) -> bool {
    for a in ra {
        for b in rb {
            if char_ranges_intersect(a, b) {
                return true;
            }
        }
    }
    false
}

/// Check whether two individual `CharRange` values intersect.
fn char_ranges_intersect(a: &CharRange, b: &CharRange) -> bool {
    let (a_lo, a_hi) = char_range_bounds(a);
    let (b_lo, b_hi) = char_range_bounds(b);
    a_lo <= b_hi && b_lo <= a_hi
}

fn char_range_bounds(r: &CharRange) -> (char, char) {
    match r {
        CharRange::Char(c) => (*c, *c),
        CharRange::Range(lo, hi) => (*lo, *hi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chars_contains() {
        let fs = FirstSet::Chars(['a', 'b'].into_iter().collect());
        assert!(fs.contains('a'));
        assert!(fs.contains('b'));
        assert!(!fs.contains('c'));
    }

    #[test]
    fn test_char_class_contains() {
        let fs = FirstSet::CharClass(vec![CharRange::Range('a', 'z')]);
        assert!(fs.contains('a'));
        assert!(fs.contains('m'));
        assert!(fs.contains('z'));
        assert!(!fs.contains('A'));
        assert!(!fs.contains('0'));
    }

    #[test]
    fn test_any_char_contains() {
        let fs = FirstSet::AnyChar;
        assert!(fs.contains('a'));
        assert!(fs.contains('\0'));
    }

    #[test]
    fn test_epsilon_contains_nothing() {
        let fs = FirstSet::Epsilon;
        assert!(!fs.contains('a'));
    }

    #[test]
    fn test_rule_contains_nothing() {
        let fs = FirstSet::Rule("foo".into());
        assert!(!fs.contains('a'));
    }

    #[test]
    fn test_union_chars() {
        let a = FirstSet::Chars(['a', 'b'].into_iter().collect());
        let b = FirstSet::Chars(['b', 'c'].into_iter().collect());
        let u = a.union(&b);
        if let FirstSet::Chars(set) = u {
            assert_eq!(set.len(), 3);
            assert!(set.contains(&'a'));
            assert!(set.contains(&'b'));
            assert!(set.contains(&'c'));
        } else {
            panic!("Expected Chars");
        }
    }

    #[test]
    fn test_union_with_any_char() {
        let a = FirstSet::Chars(['a'].into_iter().collect());
        let b = FirstSet::AnyChar;
        assert_eq!(a.union(&b), FirstSet::AnyChar);
    }

    #[test]
    fn test_union_with_epsilon() {
        let a = FirstSet::Chars(['a'].into_iter().collect());
        let b = FirstSet::Epsilon;
        let u = a.union(&b);
        assert!(u.contains('a'));
    }

    #[test]
    fn test_with_epsilon() {
        let fs = FirstSet::Chars(['a'].into_iter().collect());
        let we = fs.with_epsilon();
        assert!(we.contains('a'));
        assert_eq!(FirstSet::Epsilon.with_epsilon(), FirstSet::Epsilon);
        assert_eq!(FirstSet::AnyChar.with_epsilon(), FirstSet::AnyChar);
    }

    #[test]
    fn test_disjoint_chars() {
        let a = FirstSet::Chars(['a'].into_iter().collect());
        let b = FirstSet::Chars(['b'].into_iter().collect());
        assert!(are_disjoint(&a, &b));
    }

    #[test]
    fn test_not_disjoint_chars() {
        let a = FirstSet::Chars(['a', 'b'].into_iter().collect());
        let b = FirstSet::Chars(['b', 'c'].into_iter().collect());
        assert!(!are_disjoint(&a, &b));
    }

    #[test]
    fn test_disjoint_char_class() {
        let a = FirstSet::CharClass(vec![CharRange::Range('a', 'f')]);
        let b = FirstSet::CharClass(vec![CharRange::Range('g', 'z')]);
        assert!(are_disjoint(&a, &b));
    }

    #[test]
    fn test_not_disjoint_char_class() {
        let a = FirstSet::CharClass(vec![CharRange::Range('a', 'm')]);
        let b = FirstSet::CharClass(vec![CharRange::Range('k', 'z')]);
        assert!(!are_disjoint(&a, &b));
    }

    #[test]
    fn test_disjoint_chars_vs_class() {
        let a = FirstSet::Chars(['x'].into_iter().collect());
        let b = FirstSet::CharClass(vec![CharRange::Range('a', 'f')]);
        assert!(are_disjoint(&a, &b));
    }

    #[test]
    fn test_not_disjoint_chars_vs_class() {
        let a = FirstSet::Chars(['c'].into_iter().collect());
        let b = FirstSet::CharClass(vec![CharRange::Range('a', 'f')]);
        assert!(!are_disjoint(&a, &b));
    }

    #[test]
    fn test_any_char_not_disjoint() {
        let a = FirstSet::AnyChar;
        let b = FirstSet::Chars(['a'].into_iter().collect());
        assert!(!are_disjoint(&a, &b));
    }

    #[test]
    fn test_epsilon_disjoint_with_chars() {
        let a = FirstSet::Epsilon;
        let b = FirstSet::Chars(['a'].into_iter().collect());
        assert!(are_disjoint(&a, &b));
    }

    #[test]
    fn test_epsilon_not_disjoint_with_epsilon() {
        assert!(!are_disjoint(&FirstSet::Epsilon, &FirstSet::Epsilon));
    }

    // --- compute_first_set tests ---

    #[test]
    fn test_first_set_literal_string() {
        let pat = Pattern::Literal(LiteralValue::String("hello".into()));
        let fs = compute_first_set(&pat);
        assert!(fs.contains('h'));
        assert!(!fs.contains('e'));
    }

    #[test]
    fn test_first_set_empty_string() {
        let pat = Pattern::Literal(LiteralValue::String("".into()));
        assert_eq!(compute_first_set(&pat), FirstSet::Epsilon);
    }

    #[test]
    fn test_first_set_string_literal() {
        let pat = Pattern::StringLiteral("world".into());
        let fs = compute_first_set(&pat);
        assert!(fs.contains('w'));
        assert!(!fs.contains('o'));
    }

    #[test]
    fn test_first_set_choice() {
        let pat = Pattern::Choice(vec![
            (Pattern::Literal(LiteralValue::String("a".into())), false),
            (Pattern::Literal(LiteralValue::String("b".into())), false),
        ]);
        let fs = compute_first_set(&pat);
        assert!(fs.contains('a'));
        assert!(fs.contains('b'));
        assert!(!fs.contains('c'));
    }

    #[test]
    fn test_first_set_star_any() {
        let pat = Pattern::Repeat {
            pattern: Box::new(Pattern::Any),
            kind: RepeatKind::ZeroOrMore,
        };
        let fs = compute_first_set(&pat);
        // Star(Any) matches anything (including epsilon)
        assert!(fs.contains('x'));
        assert!(fs.contains('0'));
    }

    #[test]
    fn test_first_set_plus() {
        let pat = Pattern::Repeat {
            pattern: Box::new(Pattern::Char(CharPattern::Exact('x'))),
            kind: RepeatKind::OneOrMore,
        };
        let fs = compute_first_set(&pat);
        assert!(fs.contains('x'));
        assert!(!fs.contains('y'));
    }

    #[test]
    fn test_first_set_seq_simple() {
        let pat = Pattern::Seq(vec![
            Pattern::Char(CharPattern::Exact('a')),
            Pattern::Char(CharPattern::Exact('b')),
        ]);
        let fs = compute_first_set(&pat);
        assert!(fs.contains('a'));
        assert!(!fs.contains('b'));
    }

    #[test]
    fn test_first_set_seq_nullable_prefix() {
        let pat = Pattern::Seq(vec![
            Pattern::Optional(Box::new(Pattern::Char(CharPattern::Exact('a')))),
            Pattern::Char(CharPattern::Exact('b')),
        ]);
        let fs = compute_first_set(&pat);
        // Optional('a') can be epsilon, so first-set includes both 'a' and 'b'
        assert!(fs.contains('a'));
        assert!(fs.contains('b'));
        assert!(!fs.contains('c'));
    }

    #[test]
    fn test_first_set_empty_pattern() {
        assert_eq!(compute_first_set(&Pattern::Empty), FirstSet::Epsilon);
    }

    #[test]
    fn test_first_set_any() {
        assert_eq!(compute_first_set(&Pattern::Any), FirstSet::AnyChar);
    }

    #[test]
    fn test_first_set_apply_rule() {
        let pat = Pattern::ApplyRule("digit".into());
        assert_eq!(compute_first_set(&pat), FirstSet::Rule("digit".into()));
    }

    #[test]
    fn test_first_set_bind_delegates() {
        let pat = Pattern::Bind {
            name: "x".into(),
            pattern: Box::new(Pattern::Char(CharPattern::Exact('z'))),
            is_choice: false,
        };
        let fs = compute_first_set(&pat);
        assert!(fs.contains('z'));
        assert!(!fs.contains('a'));
    }

    #[test]
    fn test_first_set_char_class() {
        let pat = Pattern::Char(CharPattern::Class(vec![CharRange::Range('a', 'z')]));
        let fs = compute_first_set(&pat);
        assert!(fs.contains('m'));
        assert!(!fs.contains('A'));
    }

    #[test]
    fn test_first_set_end() {
        assert_eq!(compute_first_set(&Pattern::End), FirstSet::Epsilon);
    }
}
