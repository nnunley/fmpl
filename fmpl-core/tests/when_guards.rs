//! Tests for `when` guards in grammar rule definitions
use fmpl_core::{Value, Vm, eval};
use std::sync::Arc;

#[test]
fn test_when_guard_basic() {
    // Test that when guard filters pattern matches
    let source = r#"
        let Test = grammar Test {
            digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9";
            non_zero = digit:d when !(d in ["0"])
        }

        "5" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with when guard: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::String("5".into()));
}

#[test]
fn test_when_guard_filters_out_match() {
    // Test that when guard correctly filters out 0
    let source = r#"
        let Test = grammar Test {
            digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9";
            non_zero = digit:d when !(d in ["0"])
        }

        "0" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_err(),
        "Should have failed to match 0 with !(d in [0]) guard"
    );
}

#[test]
fn test_when_guard_with_comparison() {
    // Test when guard with comparison operator (needs integer comparison)
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4;
            small = digit:d when d < 5
        }

        "3" @ Test.small
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with comparison guard: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn test_when_guard_filters_large_number() {
    // Test that comparison guard filters out larger numbers
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4;
            small = digit:d when d < 5
        }

        "4" @ Test.small
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with d < 5 guard: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::Int(4));
}

#[test]
fn test_when_guard_with_action() {
    // Test when guard combined with action
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5;
            non_zero = digit:d when !(d in [0]) => d * 10
        }

        "5" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with when guard and action: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::Int(50));
}

#[test]
fn test_when_guard_multiple_conditions() {
    // Test when guard with multiple conditions
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5 | "6" => 6;
            medium = digit:d when d > 2 && d < 7
        }

        "4" @ Test.medium
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with complex guard: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::Int(4));
}

#[test]
fn test_when_guard_in_sequence() {
    // Test when guard used in a sequence rule
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9;
            two_digits = digit:a when !(a in [0]) digit:b when !(b in [0]) => [a, b]
        }

        "34" @ Test.two_digits
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse sequence with when guards: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(
        result,
        Value::List(Arc::new(vec![Value::Int(3), Value::Int(4)]))
    );
}

#[test]
fn test_when_guard_fails_first_in_sequence() {
    // Test that first when guard failing causes whole sequence to fail
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4;
            two_digits = digit:a when !(a in [0]) digit:b when !(b in [0])
        }

        "04" @ Test.two_digits
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_err(),
        "Should have failed to match 04 with when guards"
    );
}

#[test]
fn test_when_guard_in_choice() {
    // Test when guard in a choice rule
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9;
            classified = digit:d when d < 5 => "small"
                       | digit:d when d > 5 => "large"
                       | digit:d => "medium"
        }

        "3" @ Test.classified
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse choice with when guards: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::String("small".into()));
}

#[test]
fn test_when_guard_with_not_in_syntax() {
    // Test the '!(x in y)' syntax specifically mentioned
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5 | "6" => 6 | "7" => 7;
            not_zero_or_five = digit:d when !(d in [0, 5])
        }

        "7" @ Test.not_zero_or_five
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Failed to parse with !(d in [...]) guard: {:?}",
        result.err()
    );
    let result = result.unwrap();
    assert_eq!(result, Value::Int(7));
}

#[test]
fn test_when_guard_not_in_fails() {
    // Test that !(d in [...]) guard correctly filters
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4 | "5" => 5;
            not_zero_or_five = digit:d when !(d in [0, 5])
        }

        "5" @ Test.not_zero_or_five
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_err(),
        "Should have failed to match 5 with !(d in [0, 5]) guard"
    );
}
