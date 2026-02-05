//! Integration tests for FMPL pattern unification.
//!
//! Tests the unified pattern system across:
//! 1. Let destructuring (maps, lists, tagged values)
//! 2. @ operator with named grammars
//! 3. @ operator with inline pattern blocks
//! 4. Guards and choices in patterns
//! 5. Nested patterns (maps in lists, lists in maps, etc.)
//!
//! NOTE: Many pattern features require the legacy parser (FMPL_USE_LEGACY_PARSER=1)
//! because the generated parser doesn't fully support all pattern syntax yet.
//!
//! Run with: cargo test -p fmpl-core --test integration_pattern_unification
//! For full feature testing: FMPL_USE_LEGACY_PARSER=1 cargo test -p fmpl-core --test integration_pattern_unification

use fmpl_core::{Compiler, Lexer, Parser, Value, Vm};

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
// 1. Let Destructuring Tests (requires legacy parser for full functionality)
// =============================================================================

mod let_destructuring {
    use super::*;

    mod map_destructuring {
        use super::*;

        #[test]
        fn basic_map_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{name: n} = %{name: "Alice", age: 30})
                n
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::String(ref s) if s == "Alice"),
                "expected \"Alice\", got {:?}",
                result
            );
        }

        #[test]
        fn map_destructuring_multiple_keys() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{a: x, b: y} = %{a: 1, b: 2, c: 3})
                x + y
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(3)),
                "expected 3, got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "symbol keys in map destructuring patterns not yet supported"]
        fn map_destructuring_with_symbol_keys() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{:type: t, :value: v} = %{:type: "number", :value: 42})
                [t, v]
            "#,
            )
            .unwrap();
            if let Value::List(items) = result {
                assert_eq!(items.len(), 2);
                assert!(matches!(&items[0], Value::String(s) if s == "number"));
                assert!(matches!(&items[1], Value::Int(42)));
            } else {
                panic!("expected list, got {:?}", result);
            }
        }

        #[test]
        fn nested_map_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{outer: %{inner: val}} = %{outer: %{inner: 42}})
                val
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(42)),
                "expected 42, got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "map key missing error handling not fully implemented"]
        fn map_destructuring_missing_key_fails() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{missing: x} = %{name: "Alice"})
                x
            "#,
            );
            assert!(result.is_err(), "expected error for missing key");
        }
    }

    mod list_destructuring {
        use super::*;

        #[test]
        fn basic_list_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([a, b] = [1, 2])
                a + b
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(3)),
                "expected 3, got {:?}",
                result
            );
        }

        #[test]
        fn list_destructuring_single_element() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([x] = ["hello"])
                x
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::String(ref s) if s == "hello"),
                "expected \"hello\", got {:?}",
                result
            );
        }

        #[test]
        fn list_destructuring_multiple_elements() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([a, b, c, d] = [1, 2, 3, 4])
                a + b + c + d
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(10)),
                "expected 10, got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "list rest pattern (head | tail) not yet supported in let binding"]
        fn list_destructuring_with_rest() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([head | tail] = [1, 2, 3, 4])
                head
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(1)),
                "expected 1, got {:?}",
                result
            );
        }

        #[test]
        fn nested_list_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([[a, b], [c, d]] = [[1, 2], [3, 4]])
                a + b + c + d
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(10)),
                "expected 10, got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "list length mismatch error handling not fully implemented in let"]
        fn list_destructuring_length_mismatch_fails() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([a, b, c] = [1, 2])
                a
            "#,
            );
            assert!(result.is_err(), "expected error for length mismatch");
        }
    }

    mod tagged_destructuring {
        use super::*;

        #[test]
        fn basic_tagged_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Some(x) = :Some(42))
                x
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(42)),
                "expected 42, got {:?}",
                result
            );
        }

        #[test]
        fn tagged_destructuring_multiple_args() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Pair(a, b) = :Pair(1, 2))
                a + b
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(3)),
                "expected 3, got {:?}",
                result
            );
        }

        #[test]
        fn nested_tagged_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Binary(op, :Int(lhs), :Int(rhs)) = :Binary(:plus, :Int(10), :Int(20)))
                lhs + rhs
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(30)),
                "expected 30, got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "symbol pattern in tagged destructuring not yet supported in let binding"]
        fn tagged_destructuring_with_symbol_arg() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Node(name, :leaf) = :Node("root", :leaf))
                name
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::String(ref s) if s == "root"),
                "expected \"root\", got {:?}",
                result
            );
        }

        #[test]
        #[ignore = "tag mismatch error handling not fully implemented in let"]
        fn tagged_destructuring_tag_mismatch_fails() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Some(x) = :None())
                x
            "#,
            );
            assert!(result.is_err(), "expected error for tag mismatch");
        }
    }

    mod mixed_destructuring {
        use super::*;

        #[test]
        fn map_in_list_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([%{x: a}, %{x: b}] = [%{x: 1}, %{x: 2}])
                a + b
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(3)),
                "expected 3, got {:?}",
                result
            );
        }

        #[test]
        fn list_in_map_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (%{items: [a, b, c]} = %{items: [10, 20, 30]})
                a + b + c
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(60)),
                "expected 60, got {:?}",
                result
            );
        }

        #[test]
        fn tagged_in_list_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let ([:Some(a), :Some(b)] = [:Some(5), :Some(7)])
                a + b
            "#,
            )
            .unwrap();
            assert!(
                matches!(result, Value::Int(12)),
                "expected 12, got {:?}",
                result
            );
        }

        #[test]
        fn map_in_tagged_destructuring() {
            let mut vm = Vm::new();
            let result = eval_legacy(
                &mut vm,
                r#"
                let (:Config(%{name: n, port: p}) = :Config(%{name: "server", port: 8080}))
                [n, p]
            "#,
            )
            .unwrap();
            if let Value::List(items) = result {
                assert_eq!(items.len(), 2);
                assert!(matches!(&items[0], Value::String(s) if s == "server"));
                assert!(matches!(&items[1], Value::Int(8080)));
            } else {
                panic!("expected list, got {:?}", result);
            }
        }
    }
}

// =============================================================================
// 2. @ Operator with Named Grammars (these work with generated parser)
// =============================================================================

mod at_operator_named_grammars {
    use super::*;
    use fmpl_core::eval;

    #[test]
    fn base_parser_integer() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""12345" @ base::parser.integer"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "12345"),
            "expected \"12345\", got {:?}",
            result
        );
    }

    #[test]
    fn base_parser_word() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "expected \"hello\", got {:?}",
            result
        );
    }

    #[test]
    fn base_parser_digit() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""5" @ base::parser.digit"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "5"),
            "expected \"5\", got {:?}",
            result
        );
    }

    #[test]
    fn base_tree_int() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn base_tree_bool() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(
            matches!(result, Value::Bool(true)),
            "expected true, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list-wrapped tree matching not working with generated parser"]
    fn base_tree_string() {
        let mut vm = Vm::new();
        // String values need to be wrapped in list for tree matching
        let result = eval(&mut vm, r#"["hello"] @ base::tree.string"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "expected \"hello\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "custom grammar definition not fully working in generated parser"]
    fn custom_grammar_with_rule() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"
            let Test = grammar Test {
                digit = "0" | "1" | "2" | "3" | "4" | "5"
            }
            "3" @ Test.digit
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "3"),
            "expected \"3\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "custom grammar with semantic action not working"]
    fn custom_grammar_with_semantic_action() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"
            let Test = grammar Test {
                digit = "0" => 0 | "1" => 1 | "2" => 2
            }
            "2" @ Test.digit
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(2)),
            "expected 2, got {:?}",
            result
        );
    }
}

// =============================================================================
// 3. @ Operator with Inline Pattern Blocks (requires legacy parser)
// =============================================================================

mod at_operator_inline_blocks {
    use super::*;

    #[test]
    fn variable_pattern_binds_value() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"42 @ { n => n + 1 }"#).unwrap();
        assert!(
            matches!(result, Value::Int(43)),
            "expected 43, got {:?}",
            result
        );
    }

    #[test]
    fn variable_pattern_with_complex_body() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"10 @ { n => n * n + n }"#).unwrap();
        assert!(
            matches!(result, Value::Int(110)),
            "expected 110, got {:?}",
            result
        );
    }

    #[test]
    fn string_concat_in_body() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#""world" @ { s => "hello " + s }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello world"),
            "expected \"hello world\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "map pattern compilation not yet implemented"]
    fn map_pattern_in_inline_block() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"%{name: "Alice"} @ { %{name: n} => n }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "Alice"),
            "expected \"Alice\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "list pattern compilation not yet implemented"]
    fn list_pattern_in_inline_block() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"[1, 2, 3] @ { [a, b, c] => a + b + c }"#).unwrap();
        assert!(
            matches!(result, Value::Int(6)),
            "expected 6, got {:?}",
            result
        );
    }

    #[test]
    fn wildcard_pattern_returns_body() {
        let mut vm = Vm::new();
        // Using named variable as wildcard
        let result = eval_legacy(&mut vm, r#"42 @ { x => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "literal integer pattern compilation not yet implemented"]
    fn literal_int_pattern_in_inline_block() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"1 @ { 1 => "one", n => "other" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "one"),
            "expected \"one\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "symbol pattern compilation not yet implemented"]
    fn symbol_pattern_in_inline_block() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#":foo @ { :foo => "matched", _ => "other" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "expected \"matched\", got {:?}",
            result
        );
    }
}

// =============================================================================
// 4. Guards and Choices (requires legacy parser for inline blocks)
// =============================================================================

mod guards_and_choices {
    use super::*;

    #[test]
    fn guard_clause_filters_matches() {
        let mut vm = Vm::new();
        let result =
            eval_legacy(&mut vm, r#"5 @ { n when n > 10 => "big", n => "small" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "small"),
            "expected \"small\", got {:?}",
            result
        );
    }

    #[test]
    fn guard_clause_passes() {
        let mut vm = Vm::new();
        let result =
            eval_legacy(&mut vm, r#"15 @ { n when n > 10 => "big", n => "small" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "big"),
            "expected \"big\", got {:?}",
            result
        );
    }

    #[test]
    fn multiple_guards_classify_value() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"7 @ { n when n < 5 => "small", n when n > 10 => "large", n => "medium" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "medium"),
            "expected \"medium\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "grammar when-guards in grammars require legacy parser"]
    fn when_guard_in_grammar_definition() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"
            let Test = grammar Test {
                digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5;
                non_zero = digit:d when !(d in [0])
            }
            "3" @ Test.non_zero
        "#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(3)),
            "expected 3, got {:?}",
            result
        );
    }
}

// =============================================================================
// 5. Nested Patterns (requires legacy parser for full functionality)
// =============================================================================

mod nested_patterns {
    use super::*;

    #[test]
    #[ignore = "nested map/list patterns in @ blocks not fully implemented"]
    fn map_containing_list() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"%{items: [1, 2, 3]} @ { %{items: [a, b, c]} => a + b + c }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(6)),
            "expected 6, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "nested map/list patterns in @ blocks not fully implemented"]
    fn list_containing_maps() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"[%{x: 1}, %{x: 2}] @ { [%{x: a}, %{x: b}] => a + b }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(3)),
            "expected 3, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "nested map patterns in @ blocks not fully implemented"]
    fn nested_maps_deep() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"%{level1: %{level2: %{level3: 42}}} @ { %{level1: %{level2: %{level3: v}}} => v }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn tagged_pattern_match() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#":Int(42) @ { :Int(n) => n }"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn nested_tagged_patterns() {
        let mut vm = Vm::new();
        let result = eval_legacy(
            &mut vm,
            r#"
            :Binary(:plus, :Int(1), :Int(2)) @ {
                :Binary(op, :Int(a), :Int(b)) => [op, a, b]
            }
        "#,
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
}

// =============================================================================
// 6. Edge Cases and Error Handling
// =============================================================================

mod edge_cases {
    use super::*;
    use fmpl_core::eval;

    #[test]
    #[ignore = "empty list pattern matching not working with generated parser"]
    fn empty_list_matches_empty_pattern() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#"[] @ { [] => "empty" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "empty"),
            "expected \"empty\", got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "partial match detection not working with generated parser"]
    fn partial_match_fails() {
        let mut vm = Vm::new();
        let result = eval_legacy(&mut vm, r#""abc123" @ { [a-z]+ => "letters" }"#);
        assert!(result.is_err(), "expected partial match to fail");
    }

    #[test]
    #[ignore = "empty list end match not working with generated parser"]
    fn empty_list_end_match() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[] @ base::tree.end"#).unwrap();
        assert!(
            matches!(result, Value::Null),
            "expected Null, got {:?}",
            result
        );
    }

    #[test]
    #[ignore = "multi-element list single match detection not working"]
    fn list_multi_element_single_match_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3] @ base::tree.int"#);
        assert!(
            result.is_err(),
            "expected multi-element list to fail single match"
        );
    }

    #[test]
    #[ignore = "type mismatch detection not working with generated parser"]
    fn type_mismatch_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello" @ base::tree.int"#);
        assert!(result.is_err(), "expected type mismatch to fail");
    }
}
