//! Tests for assignment syntax (=)
//!
//! Tests variable mutation using the assignment operator.

use fmpl_core::{Value, Vm, eval};

fn run(src: &str) -> Result<Value, String> {
    let mut vm = Vm::new();
    eval(&mut vm, src).map_err(|e| e.to_string())
}

/// Test basic variable assignment
#[test]
fn test_basic_assignment() {
    let code = r#"
        let x = 10
        x = 20
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(20));
}

/// Test assignment in sequence
#[test]
fn test_assignment_in_sequence() {
    let code = r#"
        let x = 5
        {
            x = 10;
            x = 15
        }
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(15));
}

/// Test assignment with expressions
#[test]
fn test_assignment_with_expressions() {
    let code = r#"
        let x = 10
        x = x + 5
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(15));
}

/// Test assignment returns assigned value
#[test]
fn test_assignment_returns_value() {
    let code = r#"
        let x = 10
        let y = x = 20
        y
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(20));
}

/// Test right-associative assignment
#[test]
fn test_right_associative_assignment() {
    // Test that assignment returns the assigned value
    let code = r#"
        let a = 1
        let b = 2
        let c = 3
        let result = b = c
        result
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(3));

    // Test that chaining works
    let code2 = r#"
        let a = 1
        let b = 2
        let c = 3
        b = c;
        a = b;
        a
    "#;

    let result2 = run(code2).expect("runtime error");
    assert_eq!(result2, Value::Int(3));
}

/// Test assignment with different types
#[test]
fn test_assignment_string() {
    let code = r#"
        let s = "hello"
        s = "world"
        s
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::String("world".into()));
}

/// Test assignment with boolean
#[test]
fn test_assignment_bool() {
    let code = r#"
        let b = true
        b = false
        b
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Bool(false));
}

/// Test assignment with null
#[test]
fn test_assignment_null() {
    let code = r#"
        let x = 10
        x = null
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Null);
}

/// Test assignment in conditional
#[test]
fn test_assignment_in_conditional() {
    let code = r#"
        let x = 10
        if true then x = 20 else x = 30
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(20));
}

/// Test assignment with map operations
#[test]
fn test_assignment_with_map() {
    let code = r#"
        let m = %{a: 1, b: 2}
        let x = m.a
        x = 10
        x
    "#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(10));
}

/// Test property assignment on maps
#[test]
fn test_property_assignment_on_map() {
    // Test property assignment on map
    let code = r#"
        let m = %{a: 1, b: 2}
        m.a = 10
        m.a
    "#;

    let result = run(code);
    // This should fail because map property assignment is not yet implemented
    // (maps are Arc<HashMap> and are immutable)
    assert!(result.is_err());
}

/// Test property assignment on objects
#[test]
fn test_property_assignment_on_object() {
    // Test property assignment on object instances
    // Note: accessing properties from methods requires explicit self.count
    // This test focuses on external property mutation which we're implementing
    let code = r#"
object counter {
  init(start):
    self.count = start

  get_count(): self.count

  count: 0
}

let c = spawn counter(10)
let original = c.get_count()
c.count = 42
let updated = c.get_count()
updated
"#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(42));
}

/// Test property assignment returns assigned value for objects
#[test]
fn test_property_assignment_returns_value() {
    let code = r#"
object container {
  value: 0

  get_value(): value
}

let c = spawn container()
let x = c.value = 100
x
"#;

    let result = run(code).expect("runtime error");
    assert_eq!(result, Value::Int(100));
}
