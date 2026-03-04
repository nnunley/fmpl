use fmpl_core::ast::Expr;
use fmpl_core::{Lexer, Parser, Vm, eval};

/// AC-1: Parse `^name` constructor syntax — `is_constructor` field is set
#[test]
fn test_caret_name_sets_is_constructor() {
    let source = r#"
object ^cell(bcom, val) {
  get(): val
}
"#;
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let ast = Parser::with_source(&tokens, source).parse().expect("parse");

    // The top-level expression should be an ObjectDef with is_constructor = true
    match &ast {
        Expr::ObjectDef(obj) => {
            assert!(
                obj.is_constructor,
                "^cell should have is_constructor = true"
            );
            assert_eq!(obj.name.to_string(), "cell");
            assert_eq!(obj.params.len(), 2);
            assert_eq!(obj.params[0].as_str(), "bcom");
            assert_eq!(obj.params[1].as_str(), "val");
        }
        other => panic!("Expected ObjectDef, got {:?}", other),
    }
}

/// AC-1: Regular object (no ^) should have is_constructor = false
#[test]
fn test_regular_object_not_constructor() {
    let source = r#"
object counter {
  get(): self.count
  count: 0
}
"#;
    let tokens = Lexer::new(source).tokenize().expect("lex");
    let ast = Parser::with_source(&tokens, source).parse().expect("parse");

    match &ast {
        Expr::ObjectDef(obj) => {
            assert!(
                !obj.is_constructor,
                "regular object should have is_constructor = false"
            );
            assert_eq!(obj.name.to_string(), "counter");
        }
        other => panic!("Expected ObjectDef, got {:?}", other),
    }
}

/// AC-1: bcom constructor can be used at runtime (end-to-end smoke test)
#[test]
fn test_bcom_constructor_parses_and_evaluates() {
    let mut vm = Vm::new();
    // For now, just verify it parses. The bcom runtime behavior is AC-2+.
    // The constructor should parse and create an object that can be spawned.
    let result = eval(
        &mut vm,
        r#"
object ^cell(bcom, val) {
  .#public
  get(): val
}
"#,
    );
    // Parsing should succeed even if runtime behavior is not yet implemented
    assert!(
        result.is_ok(),
        "bcom constructor should parse: {:?}",
        result.err()
    );
}
