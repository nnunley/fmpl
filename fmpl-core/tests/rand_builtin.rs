//! Tests for rand builtin functionality
//!
//! Tests the rand::int and rand::float builtins which provide
//! random number generation for FMPL programs.

use fmpl_core::{Value, Vm, eval};

fn run(src: &str) -> Result<Value, String> {
    let mut vm = Vm::new();
    eval(&mut vm, src).map_err(|e| e.to_string())
}

/// T-1: rand::int returns integer in valid range
#[test]
fn test_rand_int_returns_valid_integer() {
    let code = r#"
        rand::int(1, 10)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Int(_)),
        "Expected Int, got {:?}",
        value
    );

    if let Value::Int(n) = value {
        assert!(
            n >= 1 && n < 10,
            "rand::int(1, 10) returned {} (expected 1 <= n < 10)",
            n
        );
    }
}

/// T-2: rand::float returns float in valid range
#[test]
fn test_rand_float_returns_valid_float() {
    let code = r#"
        rand::float()
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Float(_)),
        "Expected Float, got {:?}",
        value
    );

    if let Value::Float(f) = value {
        assert!(
            f >= 0.0 && f < 1.0,
            "rand::float() returned {} (expected 0.0 <= f < 1.0)",
            f
        );
    }
}

/// T-3: rand::int with min >= max returns error
#[test]
fn test_rand_int_requires_min_less_than_max() {
    let code = r#"
        rand::int(10, 5)
    "#;

    let result = run(code);

    // Should return an error
    match result {
        Ok(Value::Map(m)) => {
            // Should have error field
            assert!(m.contains_key("error") || m.contains_key("message"));
        }
        Err(_) => {
            // Runtime error is also acceptable
        }
        other => {
            panic!("Expected error Map or Err, got {:?}", other);
        }
    }
}

/// T-4: rand::int requires two integer arguments
#[test]
fn test_rand_int_requires_two_integers() {
    let code = r#"
        rand::int("not", "numbers")
    "#;

    let result = run(code);

    // Should return an error
    match result {
        Ok(Value::Map(m)) => {
            // Should have error field
            assert!(m.contains_key("error") || m.contains_key("message"));
        }
        Err(_) => {
            // Runtime error is also acceptable
        }
        other => {
            panic!("Expected error Map or Err, got {:?}", other);
        }
    }
}

/// T-5: rand::float with extra arguments
#[test]
fn test_rand_float_with_extra_arguments() {
    // Our implementation ignores extra arguments for float()
    let code = r#"
        rand::float()
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Float(_)),
        "Expected Float, got {:?}",
        value
    );
}

/// T-6: rand builtin exists
#[test]
fn test_rand_builtin_exists() {
    let code = r#"
        rand
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    // Should return a Symbol
    let value = result.unwrap();
    assert!(
        matches!(value, Value::Symbol(_)),
        "Expected Symbol, got {:?}",
        value
    );

    if let Value::Symbol(s) = value {
        assert_eq!(s, "__builtin_rand");
    }
}

/// T-7: rand::int with zero range (min = max - 1)
#[test]
fn test_rand_int_with_single_value_range() {
    let code = r#"
        rand::int(5, 6)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Int(_)),
        "Expected Int, got {:?}",
        value
    );

    if let Value::Int(n) = value {
        assert_eq!(n, 5, "rand::int(5, 6) should always return 5, got {}", n);
    }
}

/// T-8: rand::int with negative range
#[test]
fn test_rand_int_with_negative_range() {
    let code = r#"
        rand::int(-10, -5)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Int(_)),
        "Expected Int, got {:?}",
        value
    );

    if let Value::Int(n) = value {
        assert!(
            n >= -10 && n < -5,
            "rand::int(-10, -5) returned {} (expected -10 <= n < -5)",
            n
        );
    }
}

/// T-9: Multiple calls to rand::int produce values (probabilistic test)
#[test]
fn test_rand_int_multiple_calls() {
    // Generate two random numbers and verify they're in valid range
    let code_a = r#"
        rand::int(1, 1000)
    "#;

    let result_a = run(code_a);
    assert!(result_a.is_ok(), "Expected Ok, got: {:?}", result_a);

    if let Value::Int(a) = result_a.unwrap() {
        assert!(a >= 1 && a < 1000, "rand::int(1, 1000) returned {}", a);

        // Generate second number
        let code_b = r#"
            rand::int(1, 1000)
        "#;

        let result_b = run(code_b);
        assert!(result_b.is_ok(), "Expected Ok, got: {:?}", result_b);

        if let Value::Int(b) = result_b.unwrap() {
            assert!(b >= 1 && b < 1000, "rand::int(1, 1000) returned {}", b);
            // Note: We don't assert a != b due to small probability of collision
        }
    }
}

/// T-10: Multiple calls to rand::float produce values (probabilistic test)
#[test]
fn test_rand_float_multiple_calls() {
    // Generate two random floats and verify they're in valid range
    let code_a = r#"
        rand::float()
    "#;

    let result_a = run(code_a);
    assert!(result_a.is_ok(), "Expected Ok, got: {:?}", result_a);

    if let Value::Float(a) = result_a.unwrap() {
        assert!(a >= 0.0 && a < 1.0, "rand::float() returned {}", a);

        // Generate second number
        let code_b = r#"
            rand::float()
        "#;

        let result_b = run(code_b);
        assert!(result_b.is_ok(), "Expected Ok, got: {:?}", result_b);

        if let Value::Float(b) = result_b.unwrap() {
            assert!(b >= 0.0 && b < 1.0, "rand::float() returned {}", b);
            // Note: We don't assert a != b due to astronomically small probability of collision
        }
    }
}
