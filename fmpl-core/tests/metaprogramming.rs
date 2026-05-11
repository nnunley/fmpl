//! Tests for metaprogramming builtins: ast::parse, ir::compile, code::eval.

use fmpl_core::{Value, Vm, eval};

// Full pipeline integration tests

#[test]
fn test_full_pipeline_simple() {
    let mut vm = Vm::new();
    // Parse source, manually convert to IR, compile, eval
    let result = eval(
        &mut vm,
        r#"
        let (ast = ast::parse("42"))
        let (ir = ast @ { [:Int, n] => [:LoadInt, n] })
        let (code = ir::compile(ir))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(42)),
        "expected Int(42), got {:?}",
        result
    );
}

#[test]
fn test_full_pipeline_addition() {
    let mut vm = Vm::new();
    // Demonstrate full pipeline with manually constructed IR
    // Note: ast::parse("1 + 2") returns :Binary(:+, :Int(1), :Int(2))
    // but FMPL doesn't support operator symbol literals like :+ in expressions
    // So we test the pipeline with direct IR construction
    let result = eval(
        &mut vm,
        r#"
        let (ir = [:Add, [:LoadInt, 1], [:LoadInt, 2]])
        let (code = ir::compile(ir))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(3)),
        "expected Int(3), got {:?}",
        result
    );
}

#[test]
fn test_full_pipeline_with_ast_pattern_match() {
    let mut vm = Vm::new();
    // Parse "1 + 2", match on :+ operator symbol, construct IR
    let result = eval(
        &mut vm,
        r#"
        let (ast = ast::parse("1 + 2"))
        let (ir = ast @ {
            [:Binary, :+, [:Int, a], [:Int, b]] => [:Add, [:LoadInt, a], [:LoadInt, b]]
            [:Binary, :-, [:Int, a], [:Int, b]] => [:Sub, [:LoadInt, a], [:LoadInt, b]]
            [:Binary, :*, [:Int, a], [:Int, b]] => [:Mul, [:LoadInt, a], [:LoadInt, b]]
            [:Binary, :/, [:Int, a], [:Int, b]] => [:Div, [:LoadInt, a], [:LoadInt, b]]
        })
        let (code = ir::compile(ir))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(3)),
        "expected Int(3), got {:?}",
        result
    );
}

#[test]
fn test_full_pipeline_bool() {
    let mut vm = Vm::new();
    // Parse "true", transform to IR, compile, eval
    let result = eval(
        &mut vm,
        r#"
        let (ast = ast::parse("true"))
        let (ir = ast @ { [:Bool, b] => [:LoadBool, b] })
        let (code = ir::compile(ir))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Bool(true)),
        "expected Bool(true), got {:?}",
        result
    );
}

// code::eval tests

#[test]
fn test_code_eval_simple() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let (code = ir::compile([:LoadInt, 42]))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(42)),
        "expected Int(42), got {:?}",
        result
    );
}

#[test]
fn test_code_eval_arithmetic() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let (code = ir::compile([:Add, [:LoadInt, 1], [:LoadInt, 2]]))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(3)),
        "expected Int(3), got {:?}",
        result
    );
}

#[test]
fn test_code_eval_if() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let (code = ir::compile([:If, [:LoadBool, true], [:LoadInt, 1], [:LoadInt, 2]]))
        code::eval(code)
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Int(1)),
        "expected Int(1), got {:?}",
        result
    );
}

// ir::compile tests

#[test]
fn test_ir_compile_load_int() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ir::compile([:LoadInt, 42])"#).unwrap();
    assert!(
        matches!(result, Value::Code(_)),
        "expected Value::Code, got {:?}",
        result
    );
}

#[test]
fn test_ir_compile_add() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"ir::compile([:Add, [:LoadInt, 1], [:LoadInt, 2]])"#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Code(_)),
        "expected Value::Code, got {:?}",
        result
    );
}

#[test]
fn test_ir_compile_let() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"ir::compile([:Let, :x, [:LoadInt, 42], [:Var, :x]])"#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Code(_)),
        "expected Value::Code, got {:?}",
        result
    );
}

#[test]
fn test_ir_compile_if() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"ir::compile([:If, [:LoadBool, true], [:LoadInt, 1], [:LoadInt, 2]])"#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Code(_)),
        "expected Value::Code, got {:?}",
        result
    );
}

#[test]
fn test_ir_compile_seq() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"ir::compile([:Seq, [[:LoadInt, 1], [:LoadInt, 2], [:LoadInt, 3]]])"#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::Code(_)),
        "expected Value::Code, got {:?}",
        result
    );
}

// ast::parse tests

#[test]
fn test_ast_parse_int_literal() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("42")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "Int"),
        "expected [:Int, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_binary_expr() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("1 + 2")"#).unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children.len(), 3);
        // First child is the operator symbol
        assert!(matches!(&children[0], Value::Symbol(s) if s == "+"));
    }
}

#[test]
fn test_ast_parse_lambda() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("\\x x + 1")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "Lambda"),
        "expected [:Lambda, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_let() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("let (x = 1) x + 1")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "Let"),
        "expected [:Let, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_if() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("if true then 1 else 2")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "If"),
        "expected [:If, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_list() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("[1, 2, 3]")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "List"),
        "expected [:List, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_map() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("%{a: 1, b: 2}")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "Map"),
        "expected [:Map, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_call() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("foo(1, 2)")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "Call"),
        "expected [:Call, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_method_call() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"ast::parse("obj.method()")"#).unwrap();
    assert!(
        matches!(result.as_node(), Some((t, _)) if t == "MethodCall"),
        "expected [:MethodCall, ...], got {:?}",
        result
    );
}

#[test]
fn test_ast_parse_pattern_match_on_ast() {
    let mut vm = Vm::new();
    // Parse "1 + 2" and extract the operator and operands using pattern matching
    let result = eval(
        &mut vm,
        r#"
        ast::parse("1 + 2") @ {
            [:Binary, op, [:Int, a], [:Int, b]] => [op, a, b]
        }
    "#,
    )
    .unwrap();
    if let Value::List(items) = result {
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], Value::Symbol(s) if s == "+"));
        assert!(matches!(&items[1], Value::Int(1)));
        assert!(matches!(&items[2], Value::Int(2)));
    } else {
        panic!("expected list, got {:?}", result);
    }
}
