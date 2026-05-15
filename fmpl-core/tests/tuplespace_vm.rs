//! VM-level integration tests for tuplespace operations.
//!
//! `space.out` takes a single tagged-map argument
//! `%{type: T, data: D, durable?: Bool, namespace?: NS}`.

use fmpl_core::eval;

#[test]
fn test_tuplespace_out_and_in() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: "event", data: 42})
        let result = space.in("event")
        result.data
    "#;
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}

#[test]
fn test_tuplespace_out_and_rd() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: "event", data: 42})
        let result = space.rd("event")
        result.data
    "#;
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}

#[test]
fn test_tuplespace_rd_is_non_destructive() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: "event", data: 42})
        let r1 = space.rd("event")
        let r2 = space.rd("event")
        r2.data
    "#;
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::Int(42));
}

#[test]
fn test_tuplespace_out_with_keyword_type() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: :log, data: "error message"})
        let result = space.in(:log)
        result.data
    "#;
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(
        result,
        fmpl_core::value::Value::String("error message".into())
    );
}

#[test]
fn test_tuplespace_with_map_data() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: "event", data: %{level: :error, message: "failed"}})
        let result = space.in("event")
        result.data.message
    "#;
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::String("failed".into()));
}

#[test]
fn test_tuplespace_out_missing_type_errors() {
    // Missing required `type` key surfaces a clear runtime error,
    // not a panic.
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{data: 42})
    "#;
    let err = eval(&mut vm, source).unwrap_err().to_string();
    assert!(
        err.contains("missing required key `type`"),
        "expected `type` missing-key error, got: {err}"
    );
}

#[test]
fn test_tuplespace_out_missing_data_errors() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: :event})
    "#;
    let err = eval(&mut vm, source).unwrap_err().to_string();
    assert!(
        err.contains("missing required key `data`"),
        "expected `data` missing-key error, got: {err}"
    );
}

#[test]
fn test_tuplespace_out_durable_without_backing_errors() {
    // durable=true on an in-memory space is a hard error.
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let space = tuplespace.new()
        space.out(%{type: :event, data: 1, durable: true})
    "#;
    let err = eval(&mut vm, source).unwrap_err().to_string();
    assert!(
        err.contains("durable") && err.contains("no backing store"),
        "expected durable+no-backing error, got: {err}"
    );
}
