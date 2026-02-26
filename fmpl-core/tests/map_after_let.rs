//! Tests for map literal parsing after let statements.
//!
//! Validates that %{...} is not parsed as modulo when following a let binding.

use fmpl_core::{Compiler, Lexer, Parser, Result, Value, Vm};

fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

#[test]
fn map_after_let_no_semicolon() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "let x = 42\n%{k: x}").unwrap();
    let s = format!("{}", result);
    assert!(
        s.contains("k") && s.contains("42"),
        "expected map with k:42, got: {}",
        s
    );
}

#[test]
fn map_after_two_lets() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "let x = 1\nlet y = 2\n%{a: x, b: y}").unwrap();
    let s = format!("{}", result);
    assert!(
        s.contains("a") && s.contains("1") && s.contains("b") && s.contains("2"),
        "expected map with a:1, b:2, got: {}",
        s
    );
}

#[test]
fn modulo_still_works() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "10 % 3").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn modulo_with_parens() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "(10 % 3)").unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn map_after_lambda() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "let f = \\x (x + 1)\nlet r = f(1)\n%{a: r}").unwrap();
    let s = format!("{}", result);
    assert!(
        s.contains("a") && s.contains("2"),
        "expected map with a:2, got: {}",
        s
    );
}
