//! Tests for Prolog-style guard-triggered backtracking
//!
//! When a `when` guard fails, the runtime should automatically backtrack
//! to the most recent choice point (marked with `?`) and try the next alternative.
//!
//! This enables CSP-style constraint solving with early pruning.

use fmpl_core::{Value, Vm, eval};
use std::sync::Arc;
use std::time::Instant;

/// Basic test: guard failure should trigger backtracking to try next alternative
#[test]
fn test_guard_failure_triggers_backtrack() {
    // Grammar: ?digit:d when d != 0
    // Input doesn't matter for generation - we're testing that when d=0 fails the guard,
    // it backtracks to try d=1, d=2, etc.
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2 | ?"3" => 3
            non_zero = digit:d when d != 0
        }

        "1" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Should find non-zero digit via backtracking: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Int(1));
}

/// Guard failure with first alternative should backtrack to second
#[test]
fn test_guard_backtrack_skips_failing_alternative() {
    // The first alternative "0"=>0 will match, but guard d!=0 fails
    // Should backtrack and try "1"=>1, which passes
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2
            non_zero = digit:d when d != 0
        }

        "0" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    // This should FAIL because input "0" only matches the first alternative
    // and with Prolog-style backtracking on input mismatch, there's no valid parse
    assert!(result.is_err(), "Input '0' cannot match non_zero rule");
}

/// Sequence with multiple guards - each guard failure should backtrack
#[test]
fn test_sequence_guard_backtracking() {
    // Two digits where second must differ from first
    // digit:a digit:b when !(b in [a])
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2
            two_different = digit:a digit:b when !(b in [a]) => [a, b]
        }

        "01" @ Test.two_different
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Should parse two different digits: {:?}",
        result.err()
    );
    assert_eq!(
        result.unwrap(),
        Value::List(Arc::new(vec![Value::Int(0), Value::Int(1)]))
    );
}

/// Same digits should fail even with backtracking (no valid combination)
#[test]
fn test_sequence_guard_no_valid_combination() {
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2
            two_different = digit:a digit:b when !(b in [a]) => [a, b]
        }

        "00" @ Test.two_different
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    // Input "00" means both digits must be 0, but constraint requires them different
    assert!(
        result.is_err(),
        "Should fail: both digits are 0, constraint requires different"
    );
}

/// CSP-style: generate and constrain
/// This tests the core CSP pattern: generate values with ?, constrain with when
#[test]
fn test_csp_generate_and_constrain() {
    // Find a digit > 5
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2 | ?"3" => 3 | ?"4" => 4
                  | ?"5" => 5 | ?"6" => 6 | ?"7" => 7 | ?"8" => 8 | ?"9" => 9
            large = digit:d when d > 5 => d
        }

        "7" @ Test.large
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(result.is_ok(), "Should find digit > 5: {:?}", result.err());
    assert_eq!(result.unwrap(), Value::Int(7));
}

/// Multiple constraints in sequence - early pruning
#[test]
fn test_multiple_constraints_early_pruning() {
    // Three digits: a != 0, b != a, c != a && c != b
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2 | ?"3" => 3
            triple = digit:a when a != 0
                     digit:b when !(b in [a])
                     digit:c when !(c in [a, b])
                     => [a, b, c]
        }

        "123" @ Test.triple
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Should find three distinct non-zero digits: {:?}",
        result.err()
    );
    assert_eq!(
        result.unwrap(),
        Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]))
    );
}

/// Test that backtracking explores alternatives in order
#[test]
fn test_backtrack_explores_in_order() {
    // First valid match should be returned
    let source = r#"
        let Test = grammar Test {
            digit = ?"1" => 1 | ?"2" => 2 | ?"3" => 3
            even = digit:d when d % 2 == 0 => d
        }

        "2" @ Test.even
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(result.is_ok(), "Should find even digit: {:?}", result.err());
    // 2 is the first even digit that matches input
    assert_eq!(result.unwrap(), Value::Int(2));
}

/// Complex constraint: sum constraint
#[test]
fn test_arithmetic_constraint() {
    // Two digits that sum to 5
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2 | ?"3" => 3 | ?"4" => 4 | ?"5" => 5
            sum_five = digit:a digit:b when a + b == 5 => [a, b]
        }

        "23" @ Test.sum_five
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Should find digits summing to 5: {:?}",
        result.err()
    );
    assert_eq!(
        result.unwrap(),
        Value::List(Arc::new(vec![Value::Int(2), Value::Int(3)]))
    );
}

/// Nested choice with backtracking
#[test]
fn test_nested_choice_backtracking() {
    // Rule references another rule with choices
    let source = r#"
        let Test = grammar Test {
            low = ?"0" => 0 | ?"1" => 1 | ?"2" => 2
            high = ?"7" => 7 | ?"8" => 8 | ?"9" => 9
            digit = low | high
            non_zero = digit:d when d != 0 => d
        }

        "1" @ Test.non_zero
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    assert!(
        result.is_ok(),
        "Should handle nested choice backtracking: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Int(1));
}

/// Performance test: verify early pruning via constraint propagation
/// Without early pruning, this would explore 10^4 = 10000 combinations
/// With early pruning, it should explore far fewer
#[test]
fn test_early_pruning_performance() {
    // Four distinct digits - with early pruning, constraints eliminate branches early
    let source = r#"
        let Test = grammar Test {
            digit = ?"0" => 0 | ?"1" => 1 | ?"2" => 2 | ?"3" => 3 | ?"4" => 4
                  | ?"5" => 5 | ?"6" => 6 | ?"7" => 7 | ?"8" => 8 | ?"9" => 9
            four_distinct = digit:a when a != 0
                           digit:b when !(b in [a])
                           digit:c when !(c in [a, b])
                           digit:d when !(d in [a, b, c])
                           => [a, b, c, d]
        }

        "1234" @ Test.four_distinct
    "#;

    let mut vm = Vm::new();
    let start = Instant::now();
    let result = eval(&mut vm, source);
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "Should find four distinct digits: {:?}",
        result.err()
    );
    assert_eq!(
        result.unwrap(),
        Value::List(Arc::new(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4)
        ]))
    );

    // With early pruning, this should complete very quickly (< 100ms)
    // Without pruning (brute force 10^4), it would still be fast but this
    // establishes a baseline for CSP performance
    println!("Four distinct digits found in {:?}", elapsed);
    assert!(
        elapsed.as_millis() < 1000,
        "Should complete quickly with early pruning"
    );
}

/// Test that proves guard-triggered backtracking is working:
/// When input matches but guard fails, try next alternative
#[test]
fn test_guard_triggers_alternative_exploration() {
    // This grammar has overlapping patterns - "a" can match both alternatives
    // The first alternative returns 1, the second returns 2
    // The guard requires the value to be 2, so first must fail and backtrack
    let source = r#"
        let Test = grammar Test {
            item = ?"a" => 1 | ?"a" => 2
            guarded = item:x when x == 2 => x
        }

        "a" @ Test.guarded
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    // This is the KEY test: if backtracking works, x=1 fails guard,
    // backtrack tries x=2 which passes guard
    assert!(
        result.is_ok(),
        "Should backtrack to second alternative: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Int(2));
}
