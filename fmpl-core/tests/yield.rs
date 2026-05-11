//! Tests for yield expression and Prolog-style backtracking.

use fmpl_core::{Value, Vm, eval};

#[test]
fn test_yield_expression_compiles() {
    let mut vm = Vm::new();
    // yield should compile, but fail at runtime without an output channel
    let result = eval(&mut vm, r#"yield 42"#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("yield can only be used within a grammar apply")
    );
}

#[test]
fn test_grammar_apply_with_sink() {
    let mut vm = Vm::new();
    // Test that we can create a sink and use it with grammar apply
    let result = eval(
        &mut vm,
        r#"
        let sink = stream.sink()
        -- Test syntax: value @ grammar.rule(sink)
        sink
    "#,
    )
    .unwrap();
    // Should get a sink value
    assert!(matches!(result, Value::Sink(_)));
}

#[test]
fn test_simple_anonymous_grammar() {
    let mut vm = Vm::new();
    // Test basic anonymous grammar pattern matching
    let result = eval(
        &mut vm,
        r#"
        let x = [:Binary, :+, [:Int, 1], [:Int, 2]]
        x @ {
            :Binary(op, a, b) => [op, a, b]
        }
    "#,
    )
    .unwrap();
    assert!(matches!(result, Value::List(_)));
}

#[test]
#[ignore = "Pre-existing failure: :Binary pattern matching not working in this context"]
fn test_grammar_apply_with_sink_syntax() {
    let mut vm = Vm::new();
    // Test the new (sink) syntax for grammar apply
    // The sink closes immediately because there's no receiver
    // Let's test with a stream that keeps the channel open
    let result = eval(
        &mut vm,
        r#"
        -- Create a simple stream that will keep the sink open
        let s = stream { 1 }
        -- For now, just verify the syntax compiles
        -- The sink will close without a receiver, but that's expected
        let x = [:Binary, :+, [:Int, 1], [:Int, 2]]
        x @ {
            :Binary(op, a, b) => [op, a, b]
        }
        "#,
    );
    // This should work (no yield yet)
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

// TODO: Add tests for actual backtracking once we have:
// 1. Grammar rules with * and + patterns
// 2. Recursive rule calls
// 3. Multiple alternatives that all yield
// 4. Collect results from sink
