//! Tests for yield injection in loops.
//!
//! These tests verify that YieldCheck instructions are emitted at loop back-edges
//! and that they allow for preemptive multitasking in the future.

use fmpl_core::{Compiler, Instruction, Lexer, Parser};

fn compile(source: &str) -> Vec<Instruction> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let ast = Parser::with_source(&tokens, source).parse().unwrap();
    let code = Compiler::new().compile(&ast).unwrap();
    code.instructions
}

#[test]
fn test_while_loop_emits_yield_check() {
    let instructions = compile("while true do 1 end");
    let yield_check_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::YieldCheck))
        .count();
    assert!(
        yield_check_count > 0,
        "Expected YieldCheck instruction in while loop"
    );
}

#[test]
fn test_do_while_loop_emits_yield_check() {
    let instructions = compile("do 1 while true end");
    let yield_check_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::YieldCheck))
        .count();
    assert!(
        yield_check_count > 0,
        "Expected YieldCheck instruction in do-while loop"
    );
}

#[test]
fn test_straight_line_code_no_yield_check() {
    let instructions = compile("let x = 1 + 2");
    let yield_check_count = instructions
        .iter()
        .filter(|i| matches!(i, Instruction::YieldCheck))
        .count();
    assert_eq!(
        yield_check_count, 0,
        "Straight-line code should have zero YieldCheck instructions"
    );
}
