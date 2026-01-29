//! Test named grammar syntax
use fmpl_core::{Value, Vm, eval};

#[test]
fn test_simple_named_grammar() {
    // Named grammars need to be bound with 'let' to be referenced
    let source = r#"
        let Test = grammar Test {
            digit = "0" | "1" | "2" | "3"
        }
        "2" @ Test.digit
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_named_grammar_with_actions() {
    let source = r#"
        let Test = grammar Test {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3
        }
        "2" @ Test.digit
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}
