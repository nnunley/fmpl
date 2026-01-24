//! Tests for sleep/time builtin functionality
//!
//! Tests the time::sleep builtin which provides delay/sleep functionality
//! for implementing retry logic and rate limiting in FMPL programs.

use fmpl_core::{Value, Vm, eval};

fn run(src: &str) -> Result<Value, String> {
    let mut vm = Vm::new();
    eval(&mut vm, src).map_err(|e| e.to_string())
}

/// Helper to create an Int value
fn int(n: i64) -> Value {
    Value::Int(n)
}

/// T-1: Sleep builtin returns nil after delay
#[test]
fn test_sleep_returns_nil() {
    let code = r#"
        time::sleep(0)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Null),
        "Expected Null, got {:?}",
        value
    );
}

/// T-2: Sleep with negative duration is treated as 0
#[test]
fn test_sleep_negative_treated_as_zero() {
    let code = r#"
        time::sleep(-1)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Null),
        "Expected Null, got {:?}",
        value
    );
}

/// T-3: Sleep with non-integer argument returns error
#[test]
fn test_sleep_requires_integer() {
    let code = r#"
        time::sleep("not a number")
    "#;

    let result = run(code);

    // Should return an error value or runtime error
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

/// T-4: Sleep can be called via time symbol
#[test]
fn test_sleep_via_time_symbol() {
    let code = r#"
        let time_symbol = time
        time_symbol.sleep(0)
    "#;

    let result = run(code);
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result);

    let value = result.unwrap();
    assert!(
        matches!(value, Value::Null),
        "Expected Null, got {:?}",
        value
    );
}

/// T-5: Sleep builtin exists
#[test]
fn test_sleep_builtin_exists() {
    let code = r#"
        time
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
        assert_eq!(s, "__builtin_time");
    }
}
