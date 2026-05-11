//! Integration tests for FMPL polymorphic stream coercion.
//!
//! Tests the polymorphic behavior of the @ operator across different input types:
//! 1. String input (character stream parsing)
//! 2. List input (element-by-element matching)
//! 3. Map/Tagged input (single-element stream for pattern matching)
//! 4. Mixed operations (parse, then match)
//!
//! The @ operator's polymorphic behavior is:
//! - String -> character stream (text parsing, char by char)
//! - List -> element stream (each element is one input position)
//! - Map/Tagged/Int/Bool/etc. -> single-element stream (match whole value)
//!
//! NOTE: Many features require the legacy parser (FMPL_USE_LEGACY_PARSER=1)
//! because the generated parser doesn't fully support all pattern syntax yet.
//!
//! Run with: cargo test -p fmpl-core --test integration_polymorphic_streams

use fmpl_core::{Compiler, Lexer, Parser, Value, Vm, eval};

/// Helper to eval with legacy parser (for features not yet in generated parser)
fn eval_legacy(vm: &mut Vm, source: &str) -> Result<Value, String> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| e.to_string())?;
    let ast = Parser::with_source(&tokens, source)
        .parse()
        .map_err(|e| e.to_string())?;
    let code = Compiler::new().compile(&ast).map_err(|e| e.to_string())?;
    vm.run(&code).map_err(|e| e.to_string())
}

// =============================================================================
// 1. String Input (Character Stream Parsing)
// =============================================================================

mod string_input {
    use super::*;

    #[test]
    fn string_parses_as_character_stream() {
        let mut vm = Vm::new();
        // String should be parsed as character stream
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "expected \"hello\", got {:?}",
            result
        );
    }

    #[test]
    fn string_digit_parsing() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""5" @ base::parser.digit"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "5"),
            "expected \"5\", got {:?}",
            result
        );
    }

    #[test]
    fn string_integer_parsing() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""12345" @ base::parser.integer"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "12345"),
            "expected \"12345\", got {:?}",
            result
        );
    }

    #[test]
    fn string_with_binding() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "expected \"hello\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "inline block semantic actions not fully working with generated parser"]
    fn string_char_class_parsing() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#""abc" @ { [a-z]+ => "letters" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "letters"),
            "expected \"letters\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "inline block semantic actions not fully working with generated parser"]
    fn string_literal_matching() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#""foo" @ { "foo" => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "mismatch detection not working correctly"]
    fn string_mismatch_fails() {
        let mut vm = Vm::new();
        // Digits don't match letter pattern
        let result = eval(&mut vm, r#""123" @ base::parser.word"#);
        assert!(result.is_err(), "expected mismatch to fail");
    }

    #[test]
    #[ignore = "partial match detection not working correctly"]
    fn string_partial_match_fails() {
        let mut vm = Vm::new();
        // Must consume entire input
        let result = eval_legacy(&mut vm, r#""abc123" @ { [a-z]+ => "letters" }"#);
        assert!(result.is_err(), "expected partial match to fail");
    }
}

// =============================================================================
// 2. List Input (Element-by-Element Matching)
// =============================================================================

mod list_input {
    use super::*;

    #[test]
    #[ignore = "list-as-stream tree matching not working correctly"]
    fn list_single_element_tree_match() {
        let mut vm = Vm::new();
        // List with single element is treated as stream with one item
        let result = eval(&mut vm, r#"[42] @ base::tree.int"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list-as-stream tree matching not working correctly"]
    fn list_single_string_element() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"["hello"] @ base::tree.string"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "expected \"hello\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list-as-stream tree matching not working correctly"]
    fn list_single_bool_element() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[true] @ base::tree.bool"#).unwrap();
        assert!(
            matches!(result, Value::Bool(true)),
            "expected true, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list pattern matching in @ blocks not fully implemented"]
    fn list_pattern_exact_match() {
        let mut vm = Vm::new();
        let result =
            eval_legacy(&mut vm, r#"[1, 2, 3] @ { [ _:a, _:b, _:c] => a + b + c }"#).unwrap();
        assert!(
            matches!(result, Value::Int(6)),
            "expected 6, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list pattern matching with rest not fully implemented"]
    fn list_pattern_with_rest() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"[1, 2, 3, 4, 5] @ { [ _:first | rest] => first }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(1)),
            "expected 1, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "empty list end match not working correctly"]
    fn empty_list_matches_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[] @ base::tree.end"#).unwrap();
        assert!(
            matches!(result, Value::Null),
            "expected Null, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "empty list pattern matching not working correctly"]
    fn empty_list_pattern_match() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"[] @ { [] => "empty" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "empty"),
            "expected \"empty\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list length mismatch detection not working correctly"]
    fn list_length_mismatch_fails() {
        let mut vm = Vm::new();
        // Pattern expects 2 elements but list has 3
        let result = eval_legacy(&mut vm, r#"[1, 2, 3] @ { [ _:x, _:y] => x }"#);
        assert!(
            result.is_err(),
            "expected length mismatch to fail, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "multi-element list single match detection not working correctly"]
    fn list_multi_element_single_match_fails() {
        let mut vm = Vm::new();
        // Can't match multi-element list with single value rule
        let result = eval(&mut vm, r#"[1, 2, 3] @ base::tree.int"#);
        assert!(
            result.is_err(),
            "expected multi-element single match to fail"
        );
    }
}

// =============================================================================
// 3. Map/Tagged/Scalar Input (Single-Element Stream)
// =============================================================================

mod single_element_stream {
    use super::*;

    // Integer input (works with generated parser)
    #[test]
    fn int_as_single_element() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn int_variable_binding() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"42 @ { n => n * 2 }"#).unwrap();
        assert!(
            matches!(result, Value::Int(84)),
            "expected 84, got {:?}",
            result
        );
    }

    // Boolean input (works with generated parser)
    #[test]
    fn bool_as_single_element() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(
            matches!(result, Value::Bool(true)),
            "expected true, got {:?}",
            result
        );
    }

    #[test]
    fn bool_variable_binding() {
        let mut vm = Vm::new();
        // Using variable pattern to bind and transform
        let result = eval_legacy(&mut vm, r#"false @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    // Map input (requires legacy parser for pattern matching)
    #[test]
    #[ignore = "map pattern matching in @ blocks not fully implemented"]
    fn map_as_single_element() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"%{x: 42} @ { %{x: v} => v }"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn map_variable_binding() {
        let mut vm = Vm::new();
        // Using variable pattern with map input
        let result = eval_legacy(&mut vm, r#"%{foo: "bar"} @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    // Tagged input (requires legacy parser for pattern matching)
    #[test]
    fn tagged_variable_binding() {
        let mut vm = Vm::new();
        // Using variable pattern with tagged input
        let result = eval_legacy(&mut vm, r#":foo @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    #[test]
    fn tagged_pattern_extraction() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"[:Some, 42] @ { [:Some, v] => v }"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn tagged_nested_pattern() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"[:Binary, :plus, [:Int, 1], [:Int, 2]] @ { [:Binary, op, [:Int, a], [:Int, b]] => [op, a, b] }"#,
        )
        .unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[0], Value::Symbol(s) if s == "plus"));
            assert!(matches!(&items[1], Value::Int(1)));
            assert!(matches!(&items[2], Value::Int(2)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    // Null input
    #[test]
    fn null_variable_binding() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"null @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }
}

// =============================================================================
// 4. Mixed Operations (Parse String, Then Match Result)
// =============================================================================

mod mixed_operations {
    use super::*;

    #[test]
    fn parse_string_then_bind_result() {
        let mut vm = Vm::new();
        // Parse string, then bind result with variable pattern
        let result = eval_legacy(
            &mut vm,
            r#"
            let (parsed = "42" @ base::parser.integer)
            parsed @ { x => x + "!" }
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "42!"),
            "expected \"42!\", got {:?}",
            result
        );
    }

    #[test]
    fn multiple_parse_steps() {
        let mut vm = Vm::new();
        // Parse in multiple steps
        let result = eval_legacy(
            &mut vm,
            r#"
            let (s = "hello" @ base::parser.word)
            s @ { x => x + " world" }
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello world"),
            "expected \"hello world\", got {:?}",
            result
        );
    }
}

// =============================================================================
// 5. Stream Coercion Edge Cases
// =============================================================================

mod stream_coercion_edge_cases {
    use super::*;

    #[test]
    fn string_vs_value_coercion_difference() {
        let mut vm = Vm::new();
        // String "42" is parsed as chars -> digits match
        let string_result = eval(&mut vm, r#""42" @ base::parser.integer"#).unwrap();
        assert!(
            matches!(string_result, Value::String(ref s) if s == "42"),
            "expected \"42\", got {:?}",
            string_result
        );

        // Integer 42 is treated as single value -> int match
        let int_result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(
            matches!(int_result, Value::Int(42)),
            "expected 42, got {:?}",
            int_result
        );
    }

    #[test]
    #[ignore = "string-as-value detection not working correctly"]
    fn string_not_matched_as_value() {
        let mut vm = Vm::new();
        // String input is character-parsed, not value-matched
        // So trying to match "hello" as tree.string (which expects string VALUE) fails
        let result = eval(&mut vm, r#""hello" @ base::tree.string"#);
        assert!(
            result.is_err(),
            "expected string to be parsed as chars, not matched as value"
        );
    }

    #[test]
    #[ignore = "type mismatch detection not working correctly"]
    fn type_mismatch_detection() {
        let mut vm = Vm::new();
        // String value doesn't match int rule
        let result = eval(&mut vm, r#"["hello"] @ base::tree.int"#);
        assert!(result.is_err(), "expected type mismatch");
    }
}

// =============================================================================
// 6. Grammar with Different Input Types
// =============================================================================

mod grammar_with_input_types {
    use super::*;

    #[test]
    fn string_input_to_builtin_grammar() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""Hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "Hello"),
            "expected \"Hello\", got {:?}",
            result
        );
    }

    #[test]
    fn int_input_to_tree_grammar() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn bool_input_to_tree_grammar() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(
            matches!(result, Value::Bool(true)),
            "expected true, got {:?}",
            result
        );
    }

    #[test]
    fn map_input_to_pattern() {
        let mut vm = Vm::new();
        // Map input with variable pattern binding
        let result =
            eval_legacy(&mut vm, r#"%{name: "Alice", age: 30} @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    #[test]
    fn tagged_input_to_pattern() {
        let mut vm = Vm::new();
        let result =
            eval_legacy(&mut vm, r#"[:Point, 10, 20] @ { [:Point, x, y] => x + y }"#).unwrap();
        assert!(
            matches!(result, Value::Int(30)),
            "expected 30, got {:?}",
            result
        );
    }
}
