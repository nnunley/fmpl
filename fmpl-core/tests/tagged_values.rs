//! Tests for tagged/constructor values.

use fmpl_core::{Value, Vm, eval};

#[test]
fn test_parse_tagged_no_args() {
    let mut vm = Vm::new();
    // :Null() is a tagged value with no children
    let result = eval(&mut vm, ":Null()").unwrap();
    assert!(
        matches!(result, Value::Tagged(ref tag, ref children) if tag == "Null" && children.is_empty()),
        "expected :Null(), got {:?}",
        result
    );
}

#[test]
fn test_symbol_without_parens() {
    let mut vm = Vm::new();
    // :foo without parens is just a symbol
    let result = eval(&mut vm, ":foo").unwrap();
    assert!(
        matches!(result, Value::Symbol(ref s) if s == "foo"),
        "expected :foo symbol, got {:?}",
        result
    );
}

#[test]
fn test_parse_tagged_single_arg() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, ":Int(42)").unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children.len(), 1);
        assert!(matches!(&children[0], Value::Int(42)));
    } else {
        panic!("expected Tagged, got {:?}", result);
    }
}

#[test]
fn test_parse_tagged_multiple_args() {
    let mut vm = Vm::new();
    // Note: symbols like :+ don't work yet (lexer limitation)
    // Using :plus instead
    let result = eval(&mut vm, ":Binary(:plus, 1, 2)").unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children.len(), 3);
        assert!(matches!(&children[0], Value::Symbol(s) if s == "plus"));
        assert!(matches!(&children[1], Value::Int(1)));
        assert!(matches!(&children[2], Value::Int(2)));
    } else {
        panic!("expected Tagged, got {:?}", result);
    }
}

#[test]
fn test_parse_tagged_nested() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, ":Add(:Int(1), :Int(2))").unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Add");
        assert_eq!(children.len(), 2);
        assert!(matches!(&children[0], Value::Tagged(t, _) if t == "Int"));
        assert!(matches!(&children[1], Value::Tagged(t, _) if t == "Int"));
    } else {
        panic!("expected Tagged, got {:?}", result);
    }
}

#[test]
fn test_tagged_trailing_comma() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, ":Foo(1, 2,)").unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Foo");
        assert_eq!(children.len(), 2);
    } else {
        panic!("expected Tagged, got {:?}", result);
    }
}
