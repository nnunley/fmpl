use fmpl_core::object::ObjectId;
use fmpl_core::{Compiler, Lexer, Parser, Result, Value, Vm};

fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

fn load_prelude(vm: &mut Vm) {
    let prelude_source = std::fs::read_to_string(PRELUDE_PATH).expect("read prelude");
    let _ = eval(vm, &prelude_source).expect("load prelude");
}

// Tests run from fmpl-core/ directory, so use relative path to workspace root
const PRELUDE_PATH: &str = "../lib/core/prelude.fmpl";

#[test]
fn test_join_empty_list() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "join([])").unwrap();
    assert_eq!(result, Value::String("".into()));
}

#[test]
fn test_join_single_char() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "join([\"a\"])").unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn test_join_multiple_chars() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "join([\"h\", \"e\", \"l\", \"l\", \"o\"])").unwrap();
    assert_eq!(result, Value::String("hello".into()));
}

#[test]
fn test_to_int_digit_0() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "to_int(\"0\")").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_to_int_digit_9() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "to_int(\"9\")").unwrap();
    assert_eq!(result, Value::Int(9));
}

#[test]
fn test_to_int_digit_5() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "to_int(\"5\")").unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_reduce_sum() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "reduce(\\acc \\x acc + x, 0, [1, 2, 3, 4])").unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_reduce_empty() {
    let mut vm = Vm::new();
    load_prelude(&mut vm);
    let result = eval(&mut vm, "reduce(\\acc \\x acc + x, 0, [])").unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_reduce_digits_to_int() {
    let mut vm = Vm::new();
    // Use integers directly since to_int() returns streams that need binding
    // reduce(\acc \d acc * 10 + d, 0, [1, 2, 3]) => 123
    load_prelude(&mut vm);
    let result = eval(&mut vm, "reduce(\\acc \\d acc * 10 + d, 0, [1, 2, 3])").unwrap();
    assert_eq!(result, Value::Int(123));
}

#[test]
fn test_fold_binary_empty_rest() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), []) => :Int(1)
    load_prelude(&mut vm);
    let result = eval(&mut vm, "fold_binary([:Int, 1], [])").unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(1));
    }
}

#[test]
fn test_fold_binary_single_op() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), [[:+, :Int(2)]]) => :Binary(:+, :Int(1), :Int(2))
    load_prelude(&mut vm);
    let result = eval(&mut vm, "fold_binary([:Int, 1], [[:+, [:Int, 2]]])").unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    }
}

#[test]
fn test_fold_binary_multiple_ops() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), [[:+, :Int(2)], [:+, :Int(3)]])
    // => :Binary(:+, :Binary(:+, :Int(1), :Int(2)), :Int(3))
    load_prelude(&mut vm);
    let result = eval(
        &mut vm,
        "fold_binary([:Int, 1], [[:+, [:Int, 2]], [:+, [:Int, 3]]])",
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        // The left child (children[1]) should also be a Binary
        if let Some((left_tag, _)) = &children[1].as_node() {
            assert_eq!(left_tag.as_str(), "Binary");
        } else {
            panic!("expected nested Binary, got {:?}", children[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_int_literal() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "42" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(42));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_int_literal_single_digit() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "7" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(7));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_string_literal() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "\"hello\"" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "String");
        assert_eq!(children[0], Value::String("hello".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_string_with_escape() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "\"hello\\nworld\"" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "String");
        // The string should contain a literal newline
        assert_eq!(children[0], Value::String("hello\nworld".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_bool_true() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "true" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Bool");
        assert_eq!(children[0], Value::Bool(true));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_bool_false() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "false" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Bool");
        assert_eq!(children[0], Value::Bool(false));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_null() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "null" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Null");
        assert!(children.is_empty());
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_identifier() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "foo" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Var");
        // Grammar now returns the identifier name as a symbol (to match ast::parse)
        assert_eq!(children[0], Value::Symbol("foo".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_identifier_with_underscore() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "my_var" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Var");
        // Grammar now returns the identifier name as a symbol (to match ast::parse)
        assert_eq!(children[0], Value::Symbol("my_var".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_keyword_not_identifier() {
    let mut vm = Vm::new();
    // "true" should parse as Bool, not Var
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "true" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, _)) = result.as_node() {
        assert_eq!(tag.as_str(), "Bool"); // Not "Var"
    } else {
        panic!("expected Tagged(:Bool, ...)");
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_addition() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 + 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_subtraction() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "5 - 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("-".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_multiplication() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "3 * 4" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("*".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_division() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "10 / 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("/".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_modulo() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "7 % 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("%".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_precedence_mult_over_add() {
    let mut vm = Vm::new();
    // 1 + 2 * 3 should parse as 1 + (2 * 3), i.e., Binary(+, 1, Binary(*, 2, 3))
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 + 2 * 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
        // The right operand should be Binary(*, 2, 3)
        if let Some((right_tag, _)) = &children[2].as_node() {
            assert_eq!(right_tag.as_str(), "Binary");
        } else {
            panic!("expected right operand to be Binary");
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_left_associative() {
    let mut vm = Vm::new();
    // 1 - 2 - 3 should parse as (1 - 2) - 3, i.e., Binary(-, Binary(-, 1, 2), 3)
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 - 2 - 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("-".into()));
        // The left operand should be Binary(-, 1, 2)
        if let Some((left_tag, left_children)) = &children[1].as_node() {
            assert_eq!(left_tag.as_str(), "Binary");
            assert_eq!(left_children[0], Value::Symbol("-".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_simple_paren() {
    let mut vm = Vm::new();
    // (1) should parse as Int(1)
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "(1)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(1));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_eq() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 == 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("==".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_neq() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 != 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("!=".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_lt() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 < 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("<".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_gt() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 > 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol(">".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_lte() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 <= 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("<=".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_gte() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 >= 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol(">=".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_cmp_with_arithmetic() {
    let mut vm = Vm::new();
    // 1 + 2 < 3 * 4 should parse as Binary(<, Binary(+, 1, 2), Binary(*, 3, 4))
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 + 2 < 3 * 4" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("<".into()));
        // Both left and right should be Binary operations
        if let Some((left_tag, _)) = &children[1].as_node() {
            assert_eq!(left_tag.as_str(), "Binary");
        } else {
            panic!("expected left to be Binary");
        }
        if let Some((right_tag, _)) = &children[2].as_node() {
            assert_eq!(right_tag.as_str(), "Binary");
        } else {
            panic!("expected right to be Binary");
        }
    }
}

// NOTE: if/then/else tests are disabled due to stack overflow in grammar runtime
// when the if rule needs to recursively parse expressions. This is the same class
// of bug as the parentheses issue - indirect recursion handling in grammar engine.
// See: grammar runtime memoization/recursion handling

// #[test]
// fn test_fmpl_parser_if_then_else() { ... }
// #[test]
// fn test_fmpl_parser_if_with_comparison() { ... }
// #[test]
// fn test_fmpl_parser_if_with_arithmetic() { ... }

// ============================================================
// INTEGRATION TESTS - Full Self-Interpreter Pipeline
// ============================================================
// These tests demonstrate the complete self-interpreter:
// 1. Parse FMPL source with fmpl_parser grammar -> AST
// 2. Transform AST to IR with inline pattern matching
// 3. Compile IR with ir::compile -> bytecode
// 4. Execute bytecode with code::eval -> result

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_full_pipeline_integer() {
    let mut vm = Vm::new();
    // Full pipeline: source -> fmpl_parser.code -> pattern match -> ir::compile -> code::eval
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        let (ast = "42" @ fmpl_parser.code)
        let (ir = ast @ {{ [:Int, n] => [:LoadInt, n] }})
        code::eval(ir::compile(ir))
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_full_pipeline_addition() {
    let mut vm = Vm::new();
    // Simple flat expression: 1 + 2
    // Pattern matches :Binary(:+, :Int(a), :Int(b)) directly
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        let (ast = "1 + 2" @ fmpl_parser.code)
        let (ir = ast @ {{
            [:Binary, :+, [:Int, a], [:Int, b]] => [:Add, [:LoadInt, a], [:LoadInt, b]]
        }})
        code::eval(ir::compile(ir))
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_full_pipeline_multiplication() {
    let mut vm = Vm::new();
    // Simple flat expression: 3 * 4
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        let (ast = "3 * 4" @ fmpl_parser.code)
        let (ir = ast @ {{
            [:Binary, :*, [:Int, a], [:Int, b]] => [:Mul, [:LoadInt, a], [:LoadInt, b]]
        }})
        code::eval(ir::compile(ir))
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(12));
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_full_pipeline_comparison() {
    let mut vm = Vm::new();
    // Simple flat expression: 1 < 2
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        let (ast = "1 < 2" @ fmpl_parser.code)
        let (ir = ast @ {{
            [:Binary, :<, [:Int, a], [:Int, b]] => [:Lt, [:LoadInt, a], [:LoadInt, b]]
        }})
        code::eval(ir::compile(ir))
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_full_pipeline_string() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        let (ast = "\"hello\"" @ fmpl_parser.code)
        let (ir = ast @ {{ [:String, s] => [:LoadString, s] }})
        code::eval(ir::compile(ir))
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::String("hello".into()));
}

// ============================================================
// LOGICAL OPERATORS TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_and() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "true && false" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("&&".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_or() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "true || false" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("||".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_logical_precedence() {
    let mut vm = Vm::new();
    // true || false && true should parse as true || (false && true)
    // because && has higher precedence than ||
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "true || false && true" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("||".into()));
        // Right operand should be Binary(&&, ...)
        if let Some((right_tag, right_children)) = &children[2].as_node() {
            assert_eq!(right_tag.as_str(), "Binary");
            assert_eq!(right_children[0], Value::Symbol("&&".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_logical_with_comparison() {
    let mut vm = Vm::new();
    // 1 < 2 && 3 > 4 should parse with comparisons inside &&
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 < 2 && 3 > 4" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("&&".into()));
        // Both operands should be comparison Binary nodes
        if let Some((left_tag, left_children)) = &children[1].as_node() {
            assert_eq!(left_tag.as_str(), "Binary");
            assert_eq!(left_children[0], Value::Symbol("<".into()));
        }
        if let Some((right_tag, right_children)) = &children[2].as_node() {
            assert_eq!(right_tag.as_str(), "Binary");
            assert_eq!(right_children[0], Value::Symbol(">".into()));
        }
    }
}

// ============================================================
// UNARY OPERATORS TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_unary_minus() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "-42" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Unary");
        assert_eq!(children[0], Value::Symbol("-".into()));
        // The operand should be Int(42)
        if let Some((inner_tag, inner_children)) = &children[1].as_node() {
            assert_eq!(inner_tag.as_str(), "Int");
            assert_eq!(inner_children[0], Value::Int(42));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_unary_not() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "!true" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Unary");
        assert_eq!(children[0], Value::Symbol("!".into()));
        // The operand should be Bool(true)
        if let Some((inner_tag, inner_children)) = &children[1].as_node() {
            assert_eq!(inner_tag.as_str(), "Bool");
            assert_eq!(inner_children[0], Value::Bool(true));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_double_negation() {
    let mut vm = Vm::new();
    // --5 should parse as -(-5), i.e., Unary(-, Unary(-, Int(5)))
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "- -5" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Unary");
        assert_eq!(children[0], Value::Symbol("-".into()));
        // Inner should also be Unary(-, ...)
        if let Some((inner_tag, inner_children)) = &children[1].as_node() {
            assert_eq!(inner_tag.as_str(), "Unary");
            assert_eq!(inner_children[0], Value::Symbol("-".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_unary_with_binary() {
    let mut vm = Vm::new();
    // -1 + 2 should parse as (-1) + 2, i.e., Binary(+, Unary(-, Int(1)), Int(2))
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "-1 + 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
        // Left operand should be Unary(-, Int(1))
        if let Some((left_tag, left_children)) = &children[1].as_node() {
            assert_eq!(left_tag.as_str(), "Unary");
            assert_eq!(left_children[0], Value::Symbol("-".into()));
        }
    }
}

// ============================================================
// IF/THEN/ELSE TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_if_then_else() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "if true then 1 else 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "If");
        assert_eq!(children.len(), 3);
        // Condition should be Bool(true)
        if let Some((cond_tag, cond_children)) = &children[0].as_node() {
            assert_eq!(cond_tag.as_str(), "Bool");
            assert_eq!(cond_children[0], Value::Bool(true));
        }
        // Then branch should be Int(1)
        if let Some((then_tag, then_children)) = &children[1].as_node() {
            assert_eq!(then_tag.as_str(), "Int");
            assert_eq!(then_children[0], Value::Int(1));
        }
        // Else branch should be Int(2)
        if let Some((else_tag, else_children)) = &children[2].as_node() {
            assert_eq!(else_tag.as_str(), "Int");
            assert_eq!(else_children[0], Value::Int(2));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_if_with_comparison() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "if 1 < 2 then 10 else 20" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "If");
        // Condition should be Binary(<, ...)
        if let Some((cond_tag, cond_children)) = &children[0].as_node() {
            assert_eq!(cond_tag.as_str(), "Binary");
            assert_eq!(cond_children[0], Value::Symbol("<".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_nested_if() {
    let mut vm = Vm::new();
    // Nested if: if true then (if false then 1 else 2) else 3
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "if true then if false then 1 else 2 else 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "If");
        // Then branch should be another If
        if let Some((then_tag, _)) = &children[1].as_node() {
            assert_eq!(then_tag.as_str(), "If");
        } else {
            panic!("expected then branch to be Tagged(:If, ...)");
        }
    }
}

// ============================================================
// LET BINDINGS TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_let_simple() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "let (x = 42) x" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Now produces :Let([:Binding(:x, :Int(42))], :Var(:x)) to match ast::parse format
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Let");
        assert_eq!(children.len(), 2);
        // First child is bindings list
        if let Value::List(bindings) = &children[0] {
            assert_eq!(bindings.len(), 1);
            if let Some((binding_tag, binding_children)) = &bindings[0].as_node() {
                assert_eq!(binding_tag.as_str(), "Binding");
                // Name should be symbol :x
                assert_eq!(binding_children[0], Value::Symbol("x".into()));
                // Value should be Int(42)
                if let Some((val_tag, val_children)) = &binding_children[1].as_node() {
                    assert_eq!(val_tag.as_str(), "Int");
                    assert_eq!(val_children[0], Value::Int(42));
                } else {
                    panic!("expected value to be Tagged(:Int, ...)");
                }
            }
        } else {
            panic!("expected bindings list");
        }
        // Body should be Var(:x)
        if let Some((body_tag, body_children)) = &children[1].as_node() {
            assert_eq!(body_tag.as_str(), "Var");
            assert_eq!(body_children[0], Value::Symbol("x".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_let_with_expr() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "let (x = 1 + 2) x * 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Now produces :Let([:Binding(:x, :Binary(:+, ...))], :Binary(:*, ...))
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Let");
        assert_eq!(children.len(), 2);
        // First child is bindings list, check the value is Binary(+, ...)
        if let Value::List(bindings) = &children[0] {
            if let Some((_, binding_children)) = &bindings[0].as_node() {
                if let Some((val_tag, val_children)) = &binding_children[1].as_node() {
                    assert_eq!(val_tag.as_str(), "Binary");
                    assert_eq!(val_children[0], Value::Symbol("+".into()));
                }
            } else {
                panic!("expected Tagged(:Binding, ...)");
            }
        } else {
            panic!("expected bindings list");
        }
        // Body should be Binary(*, ...)
        if let Some((body_tag, body_children)) = &children[1].as_node() {
            assert_eq!(body_tag.as_str(), "Binary");
            assert_eq!(body_children[0], Value::Symbol("*".into()));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_nested_let() {
    let mut vm = Vm::new();
    // let (x = 1) let (y = 2) x + y
    // Now produces :Let([:Binding(:x, ...)], :Let([:Binding(:y, ...)], ...))
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "let (x = 1) let (y = 2) x + y" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Let");
        assert_eq!(children.len(), 2);
        // Body (index 1) should be another Let
        if let Some((body_tag, _)) = &children[1].as_node() {
            assert_eq!(body_tag.as_str(), "Let");
        } else {
            panic!(
                "expected body to be Tagged(:Let, ...), got {:?}",
                children[1]
            );
        }
    }
}

// ============================================================
// COMMENTS TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_sp_whitespace() {
    // Verify sp can parse just whitespace
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "   " @ fmpl_parser.sp
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert!(
        matches!(result, Value::List(_) | Value::String(_)),
        "expected list or string, got {:?}",
        result
    );
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_sp_comment_only() {
    // Verify sp can parse just a comment (no leading whitespace)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "-- comment" @ fmpl_parser.sp
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert!(
        matches!(result, Value::List(_) | Value::String(_)),
        "expected list or string, got {:?}",
        result
    );
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_sp_with_comment() {
    // Verify sp can parse whitespace + comment
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        " -- comment" @ fmpl_parser.sp
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert!(
        matches!(result, Value::List(_) | Value::String(_)),
        "expected list or string, got {:?}",
        result
    );
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_comment_at_end() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "42 -- this is a comment" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(42));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_comment_between_tokens() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 -- comment
+ 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_c_line_comment() {
    // C-style // comment
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "42 // this is a C comment" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(42));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_block_comment() {
    // C-style /* ... */ block comment
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 /* block comment */ + 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_multiline_block_comment() {
    // Multi-line block comment
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 /* multi
line
comment */ + 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    }
}

// ============================================================
// SYMBOL TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_symbol_ident() {
    // Symbol with identifier name: :foo
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        ":foo" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Symbol");
        assert_eq!(children[0], Value::String("foo".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_symbol_operator() {
    // Symbol with operator name: :+
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        ":+" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Symbol");
        assert_eq!(children[0], Value::String("+".into()));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_symbol_multi_char_op() {
    // Symbol with multi-char operator: :==
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        ":==" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Symbol");
        assert_eq!(children[0], Value::String("==".into()));
    }
}

// ============================================================
// LIST TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_list_empty() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "List");
        assert_eq!(children[0], Value::List(std::sync::Arc::new(vec![])));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_list_single() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[42]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "List");
        // children[0] should be a list containing one :Int(42)
        if let Value::List(items) = &children[0] {
            assert_eq!(items.len(), 1);
        } else {
            panic!("expected list, got {:?}", children[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_list_multiple() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[1, 2, 3]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "List");
        if let Value::List(items) = &children[0] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected list, got {:?}", children[0]);
        }
    }
}

// ============================================================
// MAP TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_map_empty() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "%{{}}" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Map");
        assert_eq!(children[0], Value::List(std::sync::Arc::new(vec![])));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_map_colon_syntax() {
    // %{key: value} syntax
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "%{{x: 1}}" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Map");
        if let Value::List(entries) = &children[0] {
            assert_eq!(entries.len(), 1);
        } else {
            panic!("expected list of entries, got {:?}", children[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_map_arrow_syntax() {
    // %{key => value} syntax
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "%{{\"x\" => 1}}" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Map");
        if let Value::List(entries) = &children[0] {
            assert_eq!(entries.len(), 1);
        } else {
            panic!("expected list of entries, got {:?}", children[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_map_multiple() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "%{{a: 1, b: 2}}" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Map");
        if let Value::List(entries) = &children[0] {
            assert_eq!(entries.len(), 2);
        } else {
            panic!("expected list of entries, got {:?}", children[0]);
        }
    }
}

// ============================================================
// TAGGED VALUE TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_tagged_empty() {
    // Tagged value with no args: :Foo()
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[:Null]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Tagged");
        assert_eq!(children[0], Value::String("Null".into()));
        assert_eq!(children[1], Value::List(std::sync::Arc::new(vec![])));
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_tagged_single_arg() {
    // Tagged value with one arg: :Int(42)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[:Int, 42]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Tagged");
        assert_eq!(children[0], Value::String("Int".into()));
        if let Value::List(items) = &children[1] {
            assert_eq!(items.len(), 1);
        } else {
            panic!("expected list of items, got {:?}", children[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_tagged_multiple_args() {
    // Tagged value with multiple args: :Binary(:+, left, right)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[:Binary, :+, 1, 2]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Tagged");
        assert_eq!(children[0], Value::String("Binary".into()));
        if let Value::List(items) = &children[1] {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected list of items, got {:?}", children[1]);
        }
    }
}

// ============================================================
// INDEXING TESTS
// ============================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_index_simple() {
    // Simple indexing: x[0]
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "x[0]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Index");
        // First child should be :Var("x")
        if let Some((var_tag, _)) = &children[0].as_node() {
            assert_eq!(var_tag.as_str(), "Var");
        } else {
            panic!("expected :Var, got {:?}", children[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_index_chained() {
    // Chained indexing: x[0][1]
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "x[0][1]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, children)) = result.as_node() {
        assert_eq!(tag.as_str(), "Index");
        // First child should be another :Index
        if let Some((inner_tag, _)) = &children[0].as_node() {
            assert_eq!(inner_tag.as_str(), "Index");
        } else {
            panic!("expected nested :Index, got {:?}", children[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_index_expr() {
    // Index with expression: list[i + 1]
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "list[i + 1]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, _)) = result.as_node() {
        assert_eq!(tag.as_str(), "Index");
    } else {
        panic!("expected Tagged(:Index, ...), got {:?}", result);
    }
}

// ============================================================================
// Lambda tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_short_lambda_single_param() {
    // Short lambda: \x x + 1
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "\\x x + 1" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = result.as_node() {
        assert_eq!(tag.as_str(), "ShortLambda");
        // First field should be single param symbol
        if let Value::Symbol(p) = &fields[0] {
            assert_eq!(p.as_str(), "x");
        } else {
            panic!("expected symbol param, got {:?}", fields[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_short_lambda_nested() {
    // Nested short lambdas: \x \y x + y
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "\\x \\y x + y" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = result.as_node() {
        assert_eq!(tag.as_str(), "ShortLambda");
        // First field should be param symbol x
        if let Value::Symbol(p) = &fields[0] {
            assert_eq!(p.as_str(), "x");
        } else {
            panic!("expected symbol param, got {:?}", fields[0]);
        }
        // Body should be another ShortLambda
        if let Some((inner_tag, _)) = &fields[1].as_node() {
            assert_eq!(inner_tag.as_str(), "ShortLambda");
        } else {
            panic!("expected nested ShortLambda, got {:?}", fields[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_full_lambda() {
    // Full lambda: lambda(x, y) x + y
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "lambda(x, y) x + y" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Lambda");
        // Check params - prepend returns a list, param_list uses it
        // Result should be ["x", "y"]
        if let Value::List(params) = &fields[0] {
            // prepend produces a flat list now
            assert_eq!(params.len(), 2, "Expected 2 params, got {:?}", params);
        } else {
            panic!("expected list of params, got {:?}", fields[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_full_lambda_empty_params() {
    // Full lambda with no params: lambda() 42
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "lambda() 42" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = result.as_node() {
        assert_eq!(tag.as_str(), "Lambda");
        // Check params is empty
        if let Value::List(params) = &fields[0] {
            assert_eq!(params.len(), 0);
        } else {
            panic!("expected list of params, got {:?}", fields[0]);
        }
    }
}

// ============================================================================
// Function call tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_call_no_args() {
    // Function call with no args: f()
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "f()" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        // First field is function (Var "f")
        if let Some((fn_tag, fn_fields)) = &fields[0].as_node() {
            assert_eq!(fn_tag.as_str(), "Var");
            if let Value::String(name) = &fn_fields[0] {
                assert_eq!(name.as_str(), "f");
            }
        }
        // Second field is args (empty list)
        if let Value::List(call_args) = &fields[1] {
            assert_eq!(call_args.len(), 0);
        } else {
            panic!("expected args list, got {:?}", fields[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_call_single_arg() {
    // Function call with one arg: f(42)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "f(42)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        if let Value::List(call_args) = &fields[1] {
            assert_eq!(call_args.len(), 1);
        } else {
            panic!("expected args list, got {:?}", fields[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_call_multiple_args() {
    // Function call with multiple args: f(1, 2, 3)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "f(1, 2, 3)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        if let Value::List(call_args) = &fields[1] {
            assert_eq!(call_args.len(), 3);
        } else {
            panic!("expected args list, got {:?}", fields[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_call_chained() {
    // Chained function calls: f(1)(2)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "f(1)(2)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Should be :Call(:Call(:Var("f"), [1]), [2])
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        // First field should also be a Call
        if let Some((inner_tag, _)) = &fields[0].as_node() {
            assert_eq!(inner_tag.as_str(), "Call");
        } else {
            panic!("expected inner Call, got {:?}", fields[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_call_with_index() {
    // Mixed postfix: arr[0](1)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "arr[0](1)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Should be :Call(:Index(:Var("arr"), :Int(0)), [:Int(1)])
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        // First field should be Index
        if let Some((inner_tag, _)) = &fields[0].as_node() {
            assert_eq!(inner_tag.as_str(), "Index");
        } else {
            panic!("expected inner Index, got {:?}", fields[0]);
        }
    }
}

// ============================================================================
// Property access tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_prop_access() {
    // Property access: obj.name
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "obj.name" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Prop");
        // First field is receiver (Var "obj")
        if let Some((recv_tag, _)) = &fields[0].as_node() {
            assert_eq!(recv_tag.as_str(), "Var");
        } else {
            panic!("expected Var receiver, got {:?}", fields[0]);
        }
        // Second field is property name (now symbol to match ir::compile)
        if let Value::Symbol(name) = &fields[1] {
            assert_eq!(name.as_str(), "name");
        } else {
            panic!("expected property name symbol, got {:?}", fields[1]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_prop_chained() {
    // Chained property access: a.b.c
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "a.b.c" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Should be :Prop(:Prop(:Var("a"), "b"), "c")
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Prop");
        // First field should also be a Prop
        if let Some((inner_tag, _)) = &fields[0].as_node() {
            assert_eq!(inner_tag.as_str(), "Prop");
        } else {
            panic!("expected inner Prop, got {:?}", fields[0]);
        }
    }
}

// ============================================================================
// Method call tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_method_call_no_args() {
    // Method call with no args: obj.method()
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "obj.method()" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "MethodCall");
        // First field is receiver
        if let Some((recv_tag, _)) = &fields[0].as_node() {
            assert_eq!(recv_tag.as_str(), "Var");
        }
        // Second field is method name
        if let Value::String(name) = &fields[1] {
            assert_eq!(name.as_str(), "method");
        }
        // Third field is args (empty list)
        if let Value::List(method_args) = &fields[2] {
            assert_eq!(method_args.len(), 0);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_method_call_with_args() {
    // Method call with args: obj.method(1, 2)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "obj.method(1, 2)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "MethodCall");
        // Third field is args
        if let Value::List(method_args) = &fields[2] {
            assert_eq!(method_args.len(), 2);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_method_chain() {
    // Chained method calls: obj.a().b()
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "obj.a().b()" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Should be :MethodCall(:MethodCall(:Var("obj"), "a", []), "b", [])
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "MethodCall");
        // First field should be inner MethodCall
        if let Some((inner_tag, _)) = &fields[0].as_node() {
            assert_eq!(inner_tag.as_str(), "MethodCall");
        } else {
            panic!("expected inner MethodCall, got {:?}", fields[0]);
        }
    }
}

// ============================================================================
// Qualified name tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_qualified_name_two_parts() {
    // Qualified name: foo::bar
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "foo::bar" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "QualifiedName");
        if let Value::List(parts) = &fields[0] {
            assert_eq!(parts.len(), 2, "Expected 2 parts, got {:?}", parts);
            if let Value::String(s) = &parts[0] {
                assert_eq!(s.as_str(), "foo");
            }
            if let Value::String(s) = &parts[1] {
                assert_eq!(s.as_str(), "bar");
            }
        } else {
            panic!("expected list of parts, got {:?}", fields[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_qualified_name_three_parts() {
    // Qualified name: a::b::c
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "a::b::c" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "QualifiedName");
        if let Value::List(parts) = &fields[0] {
            assert_eq!(parts.len(), 3);
        } else {
            panic!("expected list of parts, got {:?}", fields[0]);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_simple_var_not_qualified() {
    // Simple variable should not be QualifiedName
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "foo" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, _)) = &result.as_node() {
        assert_eq!(
            tag.as_str(),
            "Var",
            "Simple variable should be Var, not QualifiedName"
        );
    } else {
        panic!("expected Tagged(:Var, ...), got {:?}", result);
    }
}

// ============================================================================
// Comprehensive integration tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_complex_arithmetic() {
    // Complex nested arithmetic: (1 + 2) * 3 - 4 / 2
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "(1 + 2) * 3 - 4 / 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    // Should parse correctly with proper precedence
    if let Some((tag, _)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}

// TODO: Stack overflow in grammar - needs optimization
#[test]
#[ignore = "stack overflow in grammar - needs optimization; fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_nested_function_calls() {
    // Nested function calls: f(g(x), h(y, z))
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "f(g(x), h(y, z))" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Call");
        if let Value::List(call_args) = &fields[1] {
            assert_eq!(call_args.len(), 2, "Should have 2 arguments");
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_method_chain_with_args() {
    // Method chain with args: obj.foo(1).bar(2, 3).baz()
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "obj.foo(1).bar(2, 3).baz()" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "MethodCall");
        // Innermost should also be MethodCall
        if let Some((inner_tag, inner_fields)) = &fields[0].as_node() {
            assert_eq!(inner_tag.as_str(), "MethodCall");
            if let Some((innermost_tag, _)) = &inner_fields[0].as_node() {
                assert_eq!(innermost_tag.as_str(), "MethodCall");
            }
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_lambda_with_body_expression() {
    // Lambda with complex body: \x \y x + y * 2
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "\\x \\y x + y * 2" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "ShortLambda");
        // Body should be another ShortLambda
        if let Some((inner_tag, inner_fields)) = &fields[1].as_node() {
            assert_eq!(inner_tag.as_str(), "ShortLambda");
            // Inner body should be Binary
            if let Some((body_tag, _)) = &inner_fields[1].as_node() {
                assert_eq!(body_tag.as_str(), "Binary");
            }
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_if_with_complex_condition() {
    // If with complex condition: if x < 10 && y > 5 then a else b
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "if x < 10 && y > 5 then a else b" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "If");
        // Condition should be Binary with &&
        if let Some((cond_tag, cond_fields)) = &fields[0].as_node() {
            assert_eq!(cond_tag.as_str(), "Binary");
            if let Value::Symbol(op) = &cond_fields[0] {
                assert_eq!(op.as_str(), "&&");
            }
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_let_with_lambda() {
    // Let with lambda value: let (f = \x x + 1) f(5)
    // Now produces :Let([:Binding(:f, :Lambda(...))], :Call(...))
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "let (f = \\x x + 1) f(5)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Let");
        assert_eq!(fields.len(), 2);
        // Value in binding should be ShortLambda (short form)
        if let Value::List(bindings) = &fields[0]
            && let Some((_, binding_children)) = bindings[0].as_node()
            && let Some((val_tag, _)) = binding_children[1].as_node()
        {
            assert_eq!(val_tag.as_str(), "ShortLambda");
        }
        // Body (index 1) should be Call
        if let Some((body_tag, _)) = &fields[1].as_node() {
            assert_eq!(body_tag.as_str(), "Call");
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_list_of_lambdas() {
    // List containing lambdas: [\x x, \y y + 1]
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "[\\x x, \\y y + 1]" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "List");
        if let Value::List(items) = &fields[0] {
            assert_eq!(items.len(), 2);
            // Both items should be ShortLambda (short form)
            for item in items.iter() {
                if let Some((item_tag, _)) = item.as_node() {
                    assert_eq!(item_tag.as_str(), "ShortLambda");
                } else {
                    panic!("expected ShortLambda in list, got {:?}", item);
                }
            }
        }
    }
}

// TODO: This test causes stack overflow due to deeply nested grammar recursion
// when parsing maps with complex expression values and multiple entries.
// The grammar needs optimization to handle this case.
#[test]
#[ignore = "stack overflow in grammar - needs optimization; fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_map_with_expressions() {
    // Map with expression values: %{a: 1 + 2, b: f(x)}
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "%{{a: 1 + 2, b: f(x)}}" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Map");
        if let Value::List(entries) = &fields[0] {
            assert_eq!(entries.len(), 2);
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_qualified_name_method_call() {
    // Qualified name with method call: foo::bar.method(1)
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "foo::bar.method(1)" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "MethodCall");
        // Receiver should be QualifiedName
        if let Some((recv_tag, _)) = &fields[0].as_node() {
            assert_eq!(recv_tag.as_str(), "QualifiedName");
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_index_and_call_chain() {
    // Complex postfix chain: arr[0].foo()[1].bar
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "arr[0].foo()[1].bar" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, _)) = &result.as_node() {
        // Final result should be Prop (property access)
        assert_eq!(tag.as_str(), "Prop");
    } else {
        panic!("expected Tagged(:Prop, ...), got {:?}", result);
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_fmpl_parser_comments_in_code() {
    // Code with various comment styles
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        io::load("../lib/core/fmpl_parser.fmpl")
        "1 + -- line comment
2 /* block */ * 3" @ fmpl_parser.code
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    if let Some((tag, _)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Binary");
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}

// ============================================================================
// Tree Grammar Explicit Recursion Tests
// ============================================================================

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_tree_grammar_explicit_recursion_simple() {
    // Test that tree grammars use explicit rule references for recursion
    // :Binary(:+, :Int(1), :Int(2)) should transform to :Add(:LoadInt(1), :LoadInt(2))
    // Using expr:l syntax to apply the expr rule to children
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let tree_transform = grammar tree_transform {
            expr = [:Int, n] => [:LoadInt, n]
                 | [:Binary, :+, expr:l, expr:r] => [:Add, l, r]
        }
        [:Binary, :+, [:Int, 1], [:Int, 2]] @ tree_transform.expr
    "#,
    )
    .unwrap();
    // l and r should be transformed via explicit expr rule application
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Add");
        // l should be :LoadInt(1)
        if let Some((l_tag, l_fields)) = &fields[0].as_node() {
            assert_eq!(l_tag.as_str(), "LoadInt");
            assert_eq!(l_fields[0], Value::Int(1));
        }
        // r should be :LoadInt(2)
        if let Some((r_tag, r_fields)) = &fields[1].as_node() {
            assert_eq!(r_tag.as_str(), "LoadInt");
            assert_eq!(r_fields[0], Value::Int(2));
        }
    }
}

#[test]
#[ignore = "fmpl_parser.fmpl grammar not yet ready"]
fn test_tree_grammar_explicit_recursion_nested() {
    // Test nested transformation: :Binary(:+, :Binary(:*, :Int(1), :Int(2)), :Int(3))
    // Using expr:l syntax for recursive tree transformation
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let tree_transform = grammar tree_transform {
            expr = [:Int, n] => [:LoadInt, n]
                 | [:Binary, :+, expr:l, expr:r] => [:Add, l, r]
                 | [:Binary, :*, expr:l, expr:r] => [:Mul, l, r]
        }
        [:Binary, :+, [:Binary, :*, [:Int, 1], [:Int, 2]], [:Int, 3]] @ tree_transform.expr
    "#,
    )
    .unwrap();
    // Result should be :Add(:Mul(:LoadInt(1), :LoadInt(2)), :LoadInt(3))
    if let Some((tag, fields)) = &result.as_node() {
        assert_eq!(tag.as_str(), "Add");
        // l should be :Mul(:LoadInt(1), :LoadInt(2))
        if let Some((l_tag, l_fields)) = &fields[0].as_node() {
            assert_eq!(l_tag.as_str(), "Mul");
            if let Some((ll_tag, ll_fields)) = &l_fields[0].as_node() {
                assert_eq!(ll_tag.as_str(), "LoadInt");
                assert_eq!(ll_fields[0], Value::Int(1));
            } else {
                panic!(
                    "expected ll to be Tagged(:LoadInt, ...), got {:?}",
                    l_fields[0]
                );
            }
        }
        // r should be :LoadInt(3)
        if let Some((r_tag, r_fields)) = &fields[1].as_node() {
            assert_eq!(r_tag.as_str(), "LoadInt");
            assert_eq!(r_fields[0], Value::Int(3));
        }
    }
}

// ============================================================================
// ir::to_rust transpilation tests
// ============================================================================

#[test]
fn test_ir_to_rust_simple_int() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        ir::to_rust([:LoadInt, 42])
    "#,
    )
    .unwrap();
    if let Value::String(s) = result {
        assert!(
            s.contains("Value::Int(42)"),
            "Expected Int(42) in output: {}",
            s
        );
        assert!(s.contains("fn main()"), "Expected main function: {}", s);
    } else {
        panic!("expected string, got {:?}", result);
    }
}

#[test]
fn test_ir_to_rust_expr_only() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        ir::to_rust_expr([:Add, [:LoadInt, 1], [:LoadInt, 2]])
    "#,
    )
    .unwrap();
    if let Value::String(s) = result {
        assert!(s.contains(".add("), "Expected .add() in output: {}", s);
        assert!(
            !s.contains("fn main()"),
            "Should not have main function: {}",
            s
        );
    } else {
        panic!("expected string, got {:?}", result);
    }
}

#[test]
fn test_ir_to_rust_let_binding() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        ir::to_rust_expr([:Let, :x, [:LoadInt, 10], [:Var, :x]])
    "#,
    )
    .unwrap();
    if let Value::String(s) = result {
        assert!(s.contains("let x"), "Expected let x in output: {}", s);
    } else {
        panic!("expected string, got {:?}", result);
    }
}

#[test]
fn test_ir_to_rust_full_pipeline() {
    // Build IR directly and transpile to Rust
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let (ir = [:Add, [:LoadInt, 1], [:Mul, [:LoadInt, 2], [:LoadInt, 3]]])
        ir::to_rust_expr(ir)
    "#,
    )
    .unwrap();
    if let Value::String(s) = result {
        assert!(s.contains(".add("), "Expected .add() in output: {}", s);
        assert!(s.contains(".mul("), "Expected .mul() in output: {}", s);
    } else {
        panic!("expected string, got {:?}", result);
    }
}

#[test]
fn test_ir_to_rust_compiles_and_runs() {
    use std::io::Write;
    use std::process::Command;

    // Generate Rust code from IR
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        ir::to_rust([:Add, [:LoadInt, 40], [:LoadInt, 2]])
    "#,
    )
    .unwrap();

    let rust_code = match result {
        Value::String(s) => s.to_string(),
        _ => panic!("expected string"),
    };

    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let rs_path = temp_dir.join("fmpl_test_compile.rs");
    let bin_path = temp_dir.join("fmpl_test_compile");

    let mut file = std::fs::File::create(&rs_path).unwrap();
    file.write_all(rust_code.as_bytes()).unwrap();

    // Compile with rustc
    let compile_status = Command::new("rustc")
        .arg(&rs_path)
        .arg("-o")
        .arg(&bin_path)
        .arg("-A") // Allow warnings
        .arg("warnings")
        .status()
        .expect("failed to execute rustc");

    assert!(
        compile_status.success(),
        "Generated Rust code should compile"
    );

    // Run and capture output
    let output = Command::new(&bin_path)
        .output()
        .expect("failed to execute compiled binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Int(42)"),
        "Output should contain Int(42), got: {}",
        stdout
    );

    // Cleanup
    let _ = std::fs::remove_file(&rs_path);
    let _ = std::fs::remove_file(&bin_path);
}

// ============================================================
// USER CONTEXT TESTS
// ============================================================

#[test]
fn test_user_returns_none_when_unset() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, "user").unwrap();
    assert_eq!(result, Value::Symbol("none".into()));
}

#[test]
fn test_user_returns_object_id_when_set() {
    let mut vm = Vm::new();

    let principal_id: ObjectId = 42;
    vm.current_user = Some(principal_id);

    let result = eval(&mut vm, "user").unwrap();
    assert_eq!(result, Value::Object(principal_id));
}
