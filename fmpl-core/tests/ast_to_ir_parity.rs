//! Parity tests for FMPL IR compilation.
//!
//! These tests verify that the ir::compile() builtin correctly compiles
//! IR tagged values to bytecode that produces identical results
//! to the Rust compiler.

//!
//! NOTE: These tests use the ir::compile() builtin directly by constructing
//! IR tagged values in FMPL code. The ir::compile() builtin expects:
//! raw FMPL values as children (not tagged values).

use fmpl_core::{Value, Vm, eval};

fn run_ir_pipeline(vm: &mut Vm, ir_source: &str) -> Value {
    eval(vm, ir_source).expect(&format!("IR pipeline failed for: {}", ir_source))
}

fn run_rust_compiler(vm: &mut Vm, source: &str) -> Value {
    eval(vm, source).expect(&format!("Rust compiler failed for: {}", source))
}

fn assert_ir_parity(rust_source: &str, ir_source: &str) {
    let mut vm_ir = Vm::new();
    let mut vm_rust = Vm::new();

    let ir_result = run_ir_pipeline(&mut vm_ir, ir_source);
    let rust_result = run_rust_compiler(&mut vm_rust, rust_source);

    assert_eq!(
        ir_result, rust_result,
        "IR parity mismatch for '{}': IR={:?}, Rust={:?}",
        rust_source, ir_result, rust_result
    );
}

mod literals {
    use super::*;

    #[test]
    fn integer() {
        assert_ir_parity("42", r#"code::eval(ir::compile(:LoadInt(42)))"#);
    }

    #[test]
    fn bool_true() {
        assert_ir_parity("true", r#"code::eval(ir::compile(:LoadBool(true)))"#);
    }

    #[test]
    fn bool_false() {
        assert_ir_parity("false", r#"code::eval(ir::compile(:LoadBool(false)))"#);
    }

    #[test]
    fn null_value() {
        assert_ir_parity("null", r#"code::eval(ir::compile(:LoadNull()))"#);
    }

    #[test]
    fn string_literal() {
        assert_ir_parity(
            "\"hello world\"",
            r#"code::eval(ir::compile(:LoadString("hello world")))"#,
        );
    }
}

mod arithmetic {
    use super::*;

    #[test]
    fn addition() {
        assert_ir_parity(
            "1 + 2",
            r#"code::eval(ir::compile(:Add(:LoadInt(1), :LoadInt(2))))"#,
        );
    }

    #[test]
    fn subtraction() {
        assert_ir_parity(
            "10 - 3",
            r#"code::eval(ir::compile(:Sub(:LoadInt(10), :LoadInt(3))))"#,
        );
    }

    #[test]
    fn multiplication() {
        assert_ir_parity(
            "4 * 5",
            r#"code::eval(ir::compile(:Mul(:LoadInt(4), :LoadInt(5))))"#,
        );
    }

    #[test]
    fn division() {
        assert_ir_parity(
            "20 / 4",
            r#"code::eval(ir::compile(:Div(:LoadInt(20), :LoadInt(4))))"#,
        );
    }

    #[test]
    fn modulo() {
        assert_ir_parity(
            "17 % 5",
            r#"code::eval(ir::compile(:Mod(:LoadInt(17), :LoadInt(5))))"#,
        );
    }

    #[test]
    fn negation() {
        assert_ir_parity("-42", r#"code::eval(ir::compile(:Neg(:LoadInt(42))))"#);
    }
}

mod comparisons {
    use super::*;

    #[test]
    fn equality() {
        assert_ir_parity(
            "5 == 5",
            r#"code::eval(ir::compile(:Eq(:LoadInt(5), :LoadInt(5))))"#,
        );
    }

    #[test]
    fn inequality() {
        assert_ir_parity(
            "5 != 3",
            r#"code::eval(ir::compile(:NotEq(:LoadInt(5), :LoadInt(3))))"#,
        );
    }

    #[test]
    fn less_than() {
        assert_ir_parity(
            "3 < 5",
            r#"code::eval(ir::compile(:Lt(:LoadInt(3), :LoadInt(5))))"#,
        );
    }

    #[test]
    fn greater_than() {
        assert_ir_parity(
            "5 > 3",
            r#"code::eval(ir::compile(:Gt(:LoadInt(5), :LoadInt(3))))"#,
        );
    }

    #[test]
    fn less_than_equal() {
        assert_ir_parity(
            "5 <= 5",
            r#"code::eval(ir::compile(:LtEq(:LoadInt(5), :LoadInt(5))))"#,
        );
    }

    #[test]
    fn greater_than_equal() {
        assert_ir_parity(
            "5 >= 5",
            r#"code::eval(ir::compile(:GtEq(:LoadInt(5), :LoadInt(5))))"#,
        );
    }
}

mod logical {
    use super::*;

    #[test]
    fn and_operator() {
        assert_ir_parity(
            "true && true",
            r#"code::eval(ir::compile(:And(:LoadBool(true), :LoadBool(true))))"#,
        );
    }

    #[test]
    fn or_operator() {
        assert_ir_parity(
            "false || true",
            r#"code::eval(ir::compile(:Or(:LoadBool(false), :LoadBool(true))))"#,
        );
    }

    #[test]
    fn not_operator() {
        assert_ir_parity("!true", r#"code::eval(ir::compile(:Not(:LoadBool(true))))"#);
    }
}

mod control_flow {
    use super::*;

    #[test]
    fn if_true() {
        assert_ir_parity(
            "if true then 1 else 2",
            r#"code::eval(ir::compile(:If(:LoadBool(true), :LoadInt(1), :LoadInt(2))))"#,
        );
    }

    #[test]
    fn if_false() {
        assert_ir_parity(
            "if false then 1 else 2",
            r#"code::eval(ir::compile(:If(:LoadBool(false), :LoadInt(1), :LoadInt(2))))"#,
        );
    }
}

mod let_bindings {
    use super::*;

    #[test]
    fn simple_let() {
        assert_ir_parity(
            "let (x = 5) x",
            r#"code::eval(ir::compile(:Let(:x, :LoadInt(5), :Var(:x))))"#,
        );
    }

    #[test]
    fn let_with_arithmetic() {
        assert_ir_parity(
            "let (x = 5) x + 3",
            r#"code::eval(ir::compile(:Let(:x, :LoadInt(5), :Add(:Var(:x), :LoadInt(3)))))"#,
        );
    }
}

mod data_structures {
    use super::*;

    #[test]
    fn empty_list() {
        assert_ir_parity("[]", r#"code::eval(ir::compile(:MakeList([])))"#);
    }

    #[test]
    fn list_of_ints() {
        assert_ir_parity(
            "[1, 2, 3]",
            r#"code::eval(ir::compile(:MakeList([:LoadInt(1), :LoadInt(2), :LoadInt(3)])))"#,
        );
    }

    #[test]
    fn empty_map() {
        assert_ir_parity("%{}", r#"code::eval(ir::compile(:MakeMap([])))"#);
    }

    #[test]
    fn map_literal() {
        assert_ir_parity(
            "%{a: 1}",
            r#"code::eval(ir::compile(:MakeMap([[:LoadString("a"), :LoadInt(1)]])))"#,
        );
    }
}

mod functions {
    use super::*;

    #[test]
    fn lambda_call() {
        assert_ir_parity(
            "(\\x x + 1)(5)",
            r#"code::eval(ir::compile(:Call(:Lambda([:x], :Add(:Var(:x), :LoadInt(1))), [:LoadInt(5)])))"#,
        );
    }
}
