//! Tests for the @ apply operator
//!
//! The @ operator applies a grammar to a value:
//! - Named grammar: `value @ grammar.rule`
//! - Anonymous block: `value @ { pattern => action; ... }`
//!
//! Input coercion:
//! - String -> character stream (text parsing)
//! - List -> element stream (each element is one input position)
//! - Other -> single-element stream (pattern matching)

use fmpl_core::{Value, Vm, eval};

// =============================================================================
// Named grammar tests: value @ grammar.rule
// =============================================================================

mod named_grammar {
    use super::*;

    #[test]
    fn string_to_text_parser() {
        let mut vm = Vm::new();
        // Parse digits from a string
        let result = eval(&mut vm, r#""12345" @ base::parser.integer"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "12345"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_to_text_parser_word() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_parse_failure() {
        let mut vm = Vm::new();
        // Trying to parse letters with digit rule should fail
        let result = eval(&mut vm, r#""abc" @ base::parser.digit"#);
        assert!(result.is_err(), "expected parse failure, got {:?}", result);
    }

    #[test]
    fn int_to_tree_parser() {
        let mut vm = Vm::new();
        // Match an integer using tree grammar
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(matches!(result, Value::Int(42)), "got {:?}", result);
    }

    #[test]
    fn bool_to_tree_parser() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(matches!(result, Value::Bool(true)), "got {:?}", result);
    }

    // Note: `null @ base::tree.null` doesn't work because `null` is a keyword
    // and can't be used as a rule name. This is a parser limitation.

    #[test]
    fn string_value_in_list_to_tree_parser() {
        let mut vm = Vm::new();
        // Strings as input get text-parsed, not value-matched
        // To match a string VALUE, wrap it in a list
        let result = eval(&mut vm, r#"["hello"] @ base::tree.string"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn list_single_element_to_tree_parser() {
        let mut vm = Vm::new();
        // List with single int - consume the whole list
        let result = eval(&mut vm, r#"[42] @ base::tree.int"#).unwrap();
        assert!(matches!(result, Value::Int(42)), "got {:?}", result);
    }

    #[test]
    fn list_multi_element_fails_single_match() {
        let mut vm = Vm::new();
        // List with multiple elements won't match a single int rule
        // because it doesn't consume the whole list
        let result = eval(&mut vm, r#"[1, 2, 3] @ base::tree.int"#);
        assert!(
            result.is_err(),
            "expected failure for multi-element list, got {:?}",
            result
        );
    }

    #[test]
    fn type_mismatch_fails() {
        let mut vm = Vm::new();
        // String value doesn't match int rule
        let result = eval(&mut vm, r#""hello" @ base::tree.int"#);
        assert!(
            result.is_err(),
            "expected type mismatch failure, got {:?}",
            result
        );
    }
}

// =============================================================================
// Anonymous grammar block tests: value @ { pattern => action }
// =============================================================================

mod anonymous_block {
    use super::*;

    #[test]
    fn simple_char_class() {
        let mut vm = Vm::new();
        // Parse lowercase letters - semantic action is evaluated
        let result = eval(&mut vm, r#""hello" @ { [a-z]+ => "word" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "word"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn wildcard_pattern() {
        let mut vm = Vm::new();
        // _ matches any single value - action returns "any"
        let result = eval(&mut vm, r#"42 @ { _ => "any" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "any"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn dot_any_pattern() {
        let mut vm = Vm::new();
        // . also matches any single value - action returns "any"
        let result = eval(&mut vm, r#"true @ { . => "any" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "any"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn literal_string_pattern() {
        let mut vm = Vm::new();
        // Match literal string - action returns "matched"
        let result = eval(&mut vm, r#""foo" @ { "foo" => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn literal_string_pattern_fails() {
        let mut vm = Vm::new();
        // Literal doesn't match
        let result = eval(&mut vm, r#""bar" @ { "foo" => "matched" }"#);
        assert!(
            result.is_err(),
            "expected literal mismatch failure, got {:?}",
            result
        );
    }

    #[test]
    fn multiple_cases_first_match() {
        let mut vm = Vm::new();
        // Multiple cases - matches second case, returns "letters"
        let result = eval(
            &mut vm,
            r#""abc" @ { [0-9]+ => "digits"; [a-z]+ => "letters" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "letters"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn multiple_cases_second_match() {
        let mut vm = Vm::new();
        // First case fails, second matches - returns "digits"
        let result = eval(
            &mut vm,
            r#""123" @ { [a-z]+ => "letters"; [0-9]+ => "digits" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "digits"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn multiple_cases_all_fail() {
        let mut vm = Vm::new();
        // No case matches
        let result = eval(
            &mut vm,
            r#""!!!" @ { [a-z]+ => "letters"; [0-9]+ => "digits" }"#,
        );
        assert!(
            result.is_err(),
            "expected no match failure, got {:?}",
            result
        );
    }
}

// =============================================================================
// Dynamic grammar tests
// =============================================================================

mod dynamic_grammar {
    use super::*;

    #[test]
    fn grammar_literal() {
        let mut vm = Vm::new();
        // Create grammar, then apply it
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { digit = [0-9] })
            "5" @ g.digit
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "5"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn grammar_extension() {
        let mut vm = Vm::new();
        // Extend a grammar
        let result = eval(
            &mut vm,
            r#"
            let (base = grammar { letter = [a-z] })
            let (ext = base <: { digit = [0-9] })
            "5" @ ext.digit
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "5"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn grammar_extension_inherits_parent_rule() {
        let mut vm = Vm::new();
        // Extended grammar should be able to use parent rules
        let result = eval(
            &mut vm,
            r#"
            let (base = grammar { letter = [a-z] })
            let (ext = base <: { digit = [0-9] })
            "a" @ ext.letter
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "a"),
            "got {:?}",
            result
        );
    }
}

// =============================================================================
// Semantic actions with bindings
// =============================================================================

mod semantic_actions {
    use super::*;

    #[test]
    fn single_binding_returns_bound_value() {
        let mut vm = Vm::new();
        // Binding captures matched text, action returns it
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { num = [0-9]+:n => n })
            "42" @ g.num
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "42"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn multiple_bindings_action_uses_first() {
        let mut vm = Vm::new();
        // Multiple bindings, action selects first
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { pair = [0-9]:a "+" [0-9]:b => a })
            "1+2" @ g.pair
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "1"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn multiple_bindings_action_uses_second() {
        let mut vm = Vm::new();
        // Multiple bindings, action selects second
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { pair = [0-9]:a "+" [0-9]:b => b })
            "1+2" @ g.pair
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "2"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn binding_in_anonymous_block() {
        let mut vm = Vm::new();
        // Anonymous grammar with binding
        let result = eval(&mut vm, r#""hello" @ { [a-z]+:word => word }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn action_returns_literal_ignoring_binding() {
        let mut vm = Vm::new();
        // Action ignores binding, returns literal
        let result = eval(&mut vm, r#""42" @ { [0-9]+:n => "number" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "number"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn action_returns_integer_literal() {
        let mut vm = Vm::new();
        // Action returns an integer
        let result = eval(&mut vm, r#""abc" @ { [a-z]+ => 999 }"#).unwrap();
        assert!(matches!(result, Value::Int(999)), "got {:?}", result);
    }

    #[test]
    fn action_returns_boolean() {
        let mut vm = Vm::new();
        // Action returns a boolean
        let result = eval(&mut vm, r#""yes" @ { "yes" => true; "no" => false }"#).unwrap();
        assert!(matches!(result, Value::Bool(true)), "got {:?}", result);
    }

    #[test]
    fn action_returns_list() {
        let mut vm = Vm::new();
        // Action returns a list containing the binding
        let result = eval(&mut vm, r#""x" @ { [a-z]:c => [c] }"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 1);
            assert!(matches!(&list[0], Value::String(s) if s == "x"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn nested_rule_with_binding() {
        let mut vm = Vm::new();
        // Rule calls another rule and binds result
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar {
                digit = [0-9];
                num = digit+:d => d
            })
            "123" @ g.num
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "123"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn action_with_conditional() {
        let mut vm = Vm::new();
        // Action uses if/then/else with binding
        let result = eval(
            &mut vm,
            r#""1" @ { [0-9]:d => if (d == "0") then "zero" else "nonzero" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "nonzero"),
            "got {:?}",
            result
        );
    }
}

// =============================================================================
// Edge cases and error handling
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn empty_string_with_star() {
        let mut vm = Vm::new();
        // Zero-or-more should match empty string - action returns "maybe"
        let result = eval(&mut vm, r#""" @ { [a-z]* => "maybe" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "maybe"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn empty_string_with_plus_fails() {
        let mut vm = Vm::new();
        // One-or-more should fail on empty string
        let result = eval(&mut vm, r#""" @ { [a-z]+ => "some" }"#);
        assert!(
            result.is_err(),
            "expected + to fail on empty, got {:?}",
            result
        );
    }

    #[test]
    fn partial_match_fails() {
        let mut vm = Vm::new();
        // Must consume entire input
        let result = eval(&mut vm, r#""abc123" @ { [a-z]+ => "letters" }"#);
        assert!(
            result.is_err(),
            "expected partial match to fail, got {:?}",
            result
        );
    }

    #[test]
    fn empty_list_with_end() {
        let mut vm = Vm::new();
        // Empty list - end should match
        let result = eval(&mut vm, r#"[] @ base::tree.end"#).unwrap();
        assert!(matches!(result, Value::Null), "got {:?}", result);
    }
}
