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
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children.len(), 1);
        assert!(matches!(&children[0], Value::Int(42)));
    }
}

#[test]
fn test_parse_tagged_multiple_args() {
    let mut vm = Vm::new();
    // Note: symbols like :+ don't work yet (lexer limitation)
    // Using :plus instead
    let result = eval(&mut vm, ":Binary(:plus, 1, 2)").unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children.len(), 3);
        assert!(matches!(&children[0], Value::Symbol(s) if s == "plus"));
        assert!(matches!(&children[1], Value::Int(1)));
        assert!(matches!(&children[2], Value::Int(2)));
    }
}

#[test]
fn test_parse_tagged_nested() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, ":Add(:Int(1), :Int(2))").unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Add");
        assert_eq!(children.len(), 2);
        assert!(matches!(&children[0], Value::Tagged(t, _) if t == "Int"));
        assert!(matches!(&children[1], Value::Tagged(t, _) if t == "Int"));
    }
}

#[test]
fn test_tagged_trailing_comma() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, ":Foo(1, 2,)").unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Foo");
        assert_eq!(children.len(), 2);
    }
}

// Pattern matching tests

#[test]
fn test_tagged_pattern_match_simple() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#":Int(42) @ { :Int(n) => n }"#).unwrap();
    assert!(
        matches!(result, Value::Int(42)),
        "expected 42, got {:?}",
        result
    );
}

#[test]
fn test_tagged_pattern_match_nested() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        :Binary(:plus, :Int(1), :Int(2)) @ {
            :Binary(op, :Int(a), :Int(b)) => [op, a, b]
        }
    "#,
    )
    .unwrap();
    if let Value::List(items) = result {
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], Value::Symbol(s) if s == "plus"));
        assert!(matches!(&items[1], Value::Int(1)));
        assert!(matches!(&items[2], Value::Int(2)));
    } else {
        panic!("expected list, got {:?}", result);
    }
}

#[test]
fn test_tagged_pattern_match_choice() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        :Int(42) @ {
            :String(s) => "string"
            :Int(n) => "int"
            _ => "other"
        }
    "#,
    )
    .unwrap();
    assert!(
        matches!(result, Value::String(ref s) if s == "int"),
        "expected \"int\", got {:?}",
        result
    );
}

#[test]
fn test_tagged_let_destructure() {
    let mut vm = Vm::new();
    // Note: FMPL uses `let (bindings) body` not `let (bindings) in body`
    let result = eval(
        &mut vm,
        r#"
        let (:Binary(op, lhs, rhs) = :Binary(:plus, 1, 2))
        [op, lhs, rhs]
    "#,
    )
    .unwrap();
    if let Value::List(items) = result {
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], Value::Symbol(s) if s == "plus"));
        assert!(matches!(&items[1], Value::Int(1)));
        assert!(matches!(&items[2], Value::Int(2)));
    } else {
        panic!("expected list, got {:?}", result);
    }
}
