//! Tests for pattern matching in @ (match) expressions
//!
//! Tests map and list pattern matching in @ expressions (value pattern matching),
//! as opposed to grammar application.
//!
//! The @ operator can be used in two ways:
//! 1. Grammar application: `"hello" @ grammar.rule` (already works)
//! 2. Value pattern matching: `value @ { %{key: val} => ... }` (✅ IMPLEMENTED)
//!
//! This test file focuses on case #2 - map and list destructuring in match expressions.

use fmpl_core::{Value, Vm, eval};

// =============================================================================
// Map pattern matching tests
// =============================================================================

mod map_patterns {
    use super::*;

    #[test]
    fn map_pattern_single_key() {
        let mut vm = Vm::new();
        // Map pattern should extract key and bind it
        let result = eval(&mut vm, r#"%{tool: "curl"} @ { %{tool: t} => t }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "curl"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn map_pattern_multiple_keys() {
        let mut vm = Vm::new();
        // Extract multiple keys from map
        let result = eval(&mut vm, r#"%{tool: "curl", args: %{url: "https://api.example.com"}} @ { %{tool: t, args: a} => [t, a] }"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert!(matches!(&list[0], Value::String(s) if s == "curl"));
            assert!(matches!(&list[1], Value::Map(_)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn map_pattern_with_wildcard_fallback() {
        let mut vm = Vm::new();
        // First pattern matches, returns tool name
        let result = eval(
            &mut vm,
            r#"%{tool: "get"} @ { %{tool: t} => t; _ => "other" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "get"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn map_pattern_wildcard_catch_non_matching() {
        let mut vm = Vm::new();
        // First pattern doesn't match (no "tool" key), falls through to wildcard
        let result = eval(
            &mut vm,
            r#"%{data: "value"} @ { %{tool: t} => t; _ => "other" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "other"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn map_pattern_nested() {
        let mut vm = Vm::new();
        // Nested map pattern - extract inner value
        let result = eval(
            &mut vm,
            r#"%{outer: %{inner: "value"}} @ { %{outer: %{inner: i}} => i }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "value"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn map_pattern_with_guard() {
        let mut vm = Vm::new();
        // Map pattern with guard - only match if status is 200
        let result = eval(&mut vm, r#"%{status: 200, body: "ok"} @ { %{status: s} when s == 200 => "success"; %{status: s} => "failed" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "success"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn map_pattern_guard_fails_to_next_case() {
        let mut vm = Vm::new();
        // Guard fails, should fall through to next pattern
        let result = eval(&mut vm, r#"%{status: 404} @ { %{status: s} when s == 200 => "success"; %{status: s} => "failed" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "failed"),
            "got {:?}",
            result
        );
    }
}

// =============================================================================
// List pattern matching tests
// =============================================================================

mod list_patterns {
    use super::*;

    #[test]
    fn list_pattern_single_element() {
        let mut vm = Vm::new();
        // List pattern should match single element
        let result = eval(&mut vm, r#"["hello"] @ { [x] => x }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn list_pattern_multiple_elements() {
        let mut vm = Vm::new();
        // List pattern with multiple elements
        let result = eval(&mut vm, r#"["a", "b", "c"] @ { [x, y, z] => [x, y, z] }"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 3);
            assert!(matches!(&list[0], Value::String(s) if s == "a"));
            assert!(matches!(&list[1], Value::String(s) if s == "b"));
            assert!(matches!(&list[2], Value::String(s) if s == "c"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_pattern_empty() {
        let mut vm = Vm::new();
        // Empty list pattern
        let result = eval(&mut vm, r#"[] @ { [] => "empty" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "empty"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn list_pattern_wrong_length_fails() {
        let mut vm = Vm::new();
        // Pattern expects 2 elements but list has 3
        let result = eval(&mut vm, r#"[1, 2, 3] @ { [x, y] => "two" }"#);
        assert!(
            result.is_err(),
            "expected failure for wrong length, got {:?}",
            result
        );
    }

    #[test]
    fn list_pattern_nested() {
        let mut vm = Vm::new();
        // Nested list pattern
        let result = eval(
            &mut vm,
            r#"[[1, 2], [3, 4]] @ { [[a, b], [c, d]] => a + b + c + d }"#,
        )
        .unwrap();
        assert!(matches!(result, Value::Int(10)), "got {:?}", result);
    }

    #[test]
    fn list_pattern_with_guard() {
        let mut vm = Vm::new();
        // List pattern with guard
        let result = eval(
            &mut vm,
            r#"[5] @ { [x] when x > 0 => "positive"; [x] => "not positive" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "positive"),
            "got {:?}",
            result
        );
    }
}

// =============================================================================
// Mixed pattern tests (map and list in same match)
// =============================================================================

mod mixed_patterns {
    use super::*;

    #[test]
    fn map_and_list_patterns_same_match() {
        let mut vm = Vm::new();
        // Match on map or list
        let result = eval(
            &mut vm,
            r#"%{type: "map"} @ { %{type: t} => "map: " + t; [x] => "list" }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "map: map"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn list_pattern_matches_in_mixed_match() {
        let mut vm = Vm::new();
        // List pattern should match
        let result = eval(
            &mut vm,
            r#"["item"] @ { %{type: t} => "map: " + t; [x] => "list: " + x }"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "list: item"),
            "got {:?}",
            result
        );
    }
}
