//! Tests for tagged pattern matching in compile_match().
//!
//! Validates that bare symbol patterns (:Look), constructor patterns (:Look(place)),
//! and mixed cases work correctly through the inline Match compilation path.

use fmpl_core::{Compiler, Lexer, Parser, Result, Value, Vm};

fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

#[test]
fn bare_symbol_matches_tagged() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#":Look("here") @ { :Look => "found", _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("found".into()));
}

#[test]
fn bare_symbol_falls_through() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#":Talk("bob") @ { :Look => "found", _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("nope".into()));
}

#[test]
fn constructor_binds_children() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#":Look("here") @ { :Look(place) => "saw " + place, _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("saw here".into()));
}

#[test]
fn constructor_multi_children() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#":Move("north", "fast") @ { :Move(dir, speed) => dir + ":" + speed, _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("north:fast".into()));
}

#[test]
fn constructor_wildcard_child() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#":Look("here") @ { :Look(_) => "matched", _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("matched".into()));
}

#[test]
fn constructor_arity_mismatch_falls_through() {
    let mut vm = Vm::new();
    // :Look("here") has 1 child, pattern :Look(a, b) expects 2
    let result = eval(
        &mut vm,
        r#":Look("here") @ { :Look(a, b) => "two", _ => "nope" }"#,
    )
    .unwrap();
    assert_eq!(result, Value::String("nope".into()));
}

#[test]
fn mixed_cases() {
    let mut vm = Vm::new();
    // Test :Look bare symbol, :Talk constructor, and wildcard in one block
    let code = r#"
        let dispatch = \cmd cmd @ {
            :Look => "look"
            :Talk(npc) => "talk to " + npc
            _ => "unknown"
        }
        dispatch(:Look("here"))
    "#;
    let result = eval(&mut vm, code).unwrap();
    assert_eq!(result, Value::String("look".into()));

    let result2 = eval(&mut vm, r#"dispatch(:Talk("bob"))"#).unwrap();
    assert_eq!(result2, Value::String("talk to bob".into()));

    let result3 = eval(&mut vm, r#"dispatch(:Foo("bar"))"#).unwrap();
    assert_eq!(result3, Value::String("unknown".into()));
}

#[test]
fn wildcard_in_inline_block() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#":Anything("x") @ { _ => "caught" }"#).unwrap();
    assert_eq!(result, Value::String("caught".into()));
}

#[test]
fn nested_constructor_patterns() {
    let mut vm = Vm::new();
    // Matches :Binary(:+, :Int(1), :Int(2)) with nested constructors
    let result = eval(
        &mut vm,
        r#":Binary(:+, :Int(1), :Int(2)) @ { :Binary(:+, :Int(a), :Int(b)) => a + b, _ => 0 }"#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn nested_constructor_tag_mismatch_falls_through() {
    let mut vm = Vm::new();
    // :Binary(:-, ...) doesn't match pattern :Binary(:+, ...)
    let result = eval(
        &mut vm,
        r#":Binary(:-, :Int(1), :Int(2)) @ { :Binary(:+, :Int(a), :Int(b)) => a + b, _ => 0 }"#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn grammar_at_as_call_arg_to_pattern_dispatch() {
    // Issue 3: grammar @ as function argument, dispatched via pattern @
    let mut vm = Vm::new();
    let code = r#"
        let g = grammar G {
            cmd = "look" => :Look("here")
        }
        let dispatch = \cmd cmd @ {
            :Look(place) => "saw " + place
            _ => "unknown"
        }
        let parsed = "look" @ g.cmd
        dispatch(parsed)
    "#;
    let result = eval(&mut vm, code).unwrap();
    assert_eq!(result, Value::String("saw here".into()));
}
