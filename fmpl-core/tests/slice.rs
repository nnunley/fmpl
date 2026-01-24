//! Tests for the slice operation
//!
//! Slice syntax: `expr[start..end]`
//! - For lists: returns a new list with elements from start..end
//! - For strings: returns a new string with characters from start..end
//! - Negative indices count from the end
//! - Indices are clamped to valid range

use fmpl_core::{Value, Vm, eval};

// =============================================================================
// List slicing tests
// =============================================================================

mod list_slicing {
    use super::*;

    #[test]
    fn basic_list_slice() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][1..3]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(2));
            assert_eq!(list[1], Value::Int(3));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_from_beginning() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][0..2]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(1));
            assert_eq!(list[1], Value::Int(2));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_to_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][2..5]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 3);
            assert_eq!(list[0], Value::Int(3));
            assert_eq!(list[1], Value::Int(4));
            assert_eq!(list[2], Value::Int(5));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_beyond_end_clamped() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3][1..10]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(2));
            assert_eq!(list[1], Value::Int(3));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_negative_start() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][-3..4]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(3));
            assert_eq!(list[1], Value::Int(4));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_negative_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][1..-1]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 3);
            assert_eq!(list[0], Value::Int(2));
            assert_eq!(list[1], Value::Int(3));
            assert_eq!(list[2], Value::Int(4));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_both_negative() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][-3..-1]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(3));
            assert_eq!(list[1], Value::Int(4));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_start_greater_than_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][3..1]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 0);
        } else {
            panic!("expected empty list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_full_range() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][0..5]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 5);
            assert_eq!(list[0], Value::Int(1));
            assert_eq!(list[4], Value::Int(5));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_empty_list() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[][0..0]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 0);
        } else {
            panic!("expected empty list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_negative_beyond_start_clamped() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3][-10..2]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(1));
            assert_eq!(list[1], Value::Int(2));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_with_variables() {
        let mut vm = Vm::new();
        // Using separate let statements
        let result = eval(
            &mut vm,
            r#"let start = 1; let end = 3; [1, 2, 3, 4, 5][start..end]"#,
        )
        .unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(2));
            assert_eq!(list[1], Value::Int(3));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn list_slice_nested_lists() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[[1, 2], [3, 4], [5, 6]][0..2]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            if let Value::List(inner) = &list[0] {
                assert_eq!(inner.len(), 2);
            } else {
                panic!("expected inner list");
            }
        } else {
            panic!("expected list, got {:?}", result);
        }
    }
}

// =============================================================================
// String slicing tests
// =============================================================================

mod string_slicing {
    use super::*;

    #[test]
    fn basic_string_slice() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[1..4]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "ell"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_from_beginning() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[0..2]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "he"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_to_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[2..5]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "llo"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_beyond_end_clamped() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hi"[1..10]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "i"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_negative_start() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[-3..5]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "llo"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_negative_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[1..-1]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "ell"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_both_negative() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[-3..-1]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "ll"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_start_greater_than_end() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[3..1]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == ""),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_full_range() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[0..5]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_empty_string() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"""[0..0]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == ""),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_unicode() {
        let mut vm = Vm::new();
        // Test with emoji (multi-byte characters)
        // "hello🌍world" = h e l l o 🌍 w o r l d
        // positions:       0 1 2 3 4 5 6 7 8 9 10
        // To get just the emoji, slice from 5 to 6
        let result = eval(&mut vm, r#""hello🌍world"[5..6]"#).unwrap();
        // 🌍 is a single character (code point) in Rust strings
        assert!(
            matches!(result, Value::String(ref s) if s == "🌍"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_slice_with_variables() {
        let mut vm = Vm::new();
        // Using separate let statements
        let result = eval(
            &mut vm,
            r#"let start = 1; let end = 4; "hello"[start..end]"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "ell"),
            "got {:?}",
            result
        );
    }
}

// =============================================================================
// Error cases
// =============================================================================

mod errors {
    use super::*;

    #[test]
    fn slice_on_int_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"42[0..2]"#);
        assert!(result.is_err(), "expected type error, got {:?}", result);
    }

    #[test]
    fn slice_on_map_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"%{a: 1}[0..2]"#);
        assert!(result.is_err(), "expected type error, got {:?}", result);
    }

    #[test]
    fn slice_with_non_int_start_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3]["a"..2]"#);
        assert!(result.is_err(), "expected type error, got {:?}", result);
    }

    #[test]
    fn slice_with_non_int_end_fails() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3][0.."b"]"#);
        assert!(result.is_err(), "expected type error, got {:?}", result);
    }
}

// =============================================================================
// Edge cases and mixed types
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn slice_of_slice() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3, 4, 5][1..4][0..2]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::Int(2));
            assert_eq!(list[1], Value::Int(3));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn slice_single_element() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3][1..2]"#).unwrap();
        if let Value::List(list) = result {
            assert_eq!(list.len(), 1);
            assert_eq!(list[0], Value::Int(2));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn slice_string_single_char() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello"[2..3]"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "l"),
            "got {:?}",
            result
        );
    }
}
