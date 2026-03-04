//! Tests for CompiledCode persistence to Fjall storage.

#![cfg(feature = "fjall-persistence")]

use fmpl_core::compiler::{CompiledCode, Compiler};
use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;
use fmpl_core::value::Value;
use fmpl_core::vm::Vm;

fn compile(source: &str) -> CompiledCode {
    let tokens = Lexer::new(source).tokenize().expect("lex failed");
    let ast = Parser::with_source(&tokens, source)
        .parse()
        .expect("parse failed");
    Compiler::new().compile(&ast).expect("compile failed")
}

fn eval_code(code: &CompiledCode) -> Value {
    let mut vm = Vm::new();
    vm.run(code).expect("runtime error")
}

/// AC-1: save_bytecode() serializes CompiledCode to Fjall.
/// AC-2: load_bytecode() deserializes and returns usable CompiledCode.
/// AC-3: Restored bytecode executes correctly.
#[test]
fn bytecode_survives_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("bytecode", fjall::KeyspaceCreateOptions::default)
        .unwrap();

    let code = compile("1 + 2");
    code.save_to_fjall(&keyspace, "test_add").unwrap();

    let restored = CompiledCode::load_from_fjall(&keyspace, "test_add")
        .unwrap()
        .expect("should find saved bytecode");

    assert_eq!(eval_code(&restored), Value::Int(3));
}

/// AC-2: load returns None for missing key.
#[test]
fn load_missing_key_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("bytecode", fjall::KeyspaceCreateOptions::default)
        .unwrap();

    let result = CompiledCode::load_from_fjall(&keyspace, "nonexistent").unwrap();
    assert!(result.is_none());
}

/// AC-4: Round-trip works for programs with various instruction types.
#[test]
fn various_instruction_types_survive() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("bytecode", fjall::KeyspaceCreateOptions::default)
        .unwrap();

    // Arithmetic
    let code = compile("10 * 3 + 5");
    code.save_to_fjall(&keyspace, "arith").unwrap();
    let restored = CompiledCode::load_from_fjall(&keyspace, "arith")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(35));

    // String
    let code = compile(r#""hello" + " world""#);
    code.save_to_fjall(&keyspace, "string").unwrap();
    let restored = CompiledCode::load_from_fjall(&keyspace, "string")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::String("hello world".into()));

    // Boolean / conditional
    let code = compile("if true then 1 else 2");
    code.save_to_fjall(&keyspace, "cond").unwrap();
    let restored = CompiledCode::load_from_fjall(&keyspace, "cond")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(1));
}

/// AC-4: Lambdas (nested code) survive round-trip.
#[test]
fn nested_code_survives() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("bytecode", fjall::KeyspaceCreateOptions::default)
        .unwrap();

    let code = compile("let f = \\x x + 1; f(41)");
    code.save_to_fjall(&keyspace, "lambda").unwrap();
    let restored = CompiledCode::load_from_fjall(&keyspace, "lambda")
        .unwrap()
        .unwrap();
    assert_eq!(eval_code(&restored), Value::Int(42));
}

/// Multiple keys coexist in same keyspace.
#[test]
fn multiple_keys_coexist() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("bytecode", fjall::KeyspaceCreateOptions::default)
        .unwrap();

    let code1 = compile("1 + 1");
    let code2 = compile("2 * 3");
    code1.save_to_fjall(&keyspace, "a").unwrap();
    code2.save_to_fjall(&keyspace, "b").unwrap();

    let r1 = CompiledCode::load_from_fjall(&keyspace, "a")
        .unwrap()
        .unwrap();
    let r2 = CompiledCode::load_from_fjall(&keyspace, "b")
        .unwrap()
        .unwrap();

    assert_eq!(eval_code(&r1), Value::Int(2));
    assert_eq!(eval_code(&r2), Value::Int(6));
}
