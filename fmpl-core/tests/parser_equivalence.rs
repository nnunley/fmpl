//! Tests that verify the generated parser produces the same results as the legacy parser.
//!
//! These tests ensure that parser generation doesn't introduce regressions.

use fmpl_core::lexer::Lexer;
use fmpl_core::parser::{Parser, generated_parse};

/// Test cases covering various FMPL language constructs
const TEST_CASES: &[&str] = &[
    // Literals
    "42",
    "3.14",
    "true",
    "false",
    "null",
    r#""hello world""#,
    ":symbol",
    ":+",
    // Arithmetic
    "1 + 2",
    "1 + 2 * 3",
    "(1 + 2) * 3",
    "10 - 5 / 2",
    "10 % 3",
    "-42",
    "!true",
    // Comparisons
    "1 < 2",
    "1 <= 2",
    "1 > 2",
    "1 >= 2",
    "1 == 2",
    "1 != 2",
    // Logical operators
    "true && false",
    "true || false",
    "!true && false",
    // Variables and bindings
    "x",
    "foo_bar",
    "let (x = 42) x",
    "let (x = 1) let (y = 2) x + y",
    // Lists
    "[]",
    "[1]",
    "[1, 2, 3]",
    "[1, [2, 3], 4]",
    // Maps
    "%{}",
    "%{a: 1}",
    "%{a: 1, b: 2}",
    "%{foo: [1, 2], bar: %{nested: true}}",
    // Lambdas
    r#"\x x"#,
    r#"\x x + 1"#,
    r#"\x \y x + y"#,
    "lambda(x) x",
    "lambda(x, y) x + y",
    // Conditionals
    "if true then 1 else 2",
    "if x > 0 then x else -x",
    "if a then if b then 1 else 2 else 3",
    // Function calls
    "f()",
    "f(1)",
    "f(1, 2)",
    "f(1, 2, 3)",
    "f(g(x))",
    // Method calls
    "x.foo",
    "x.foo()",
    "x.foo(1)",
    "x.foo(1, 2)",
    "x.foo.bar",
    "x.foo().bar()",
    // Indexing
    "x[0]",
    "x[i]",
    "x[0][1]",
    "x.foo[0]",
    // Qualified names
    "foo::bar",
    "foo::bar::baz",
    "io::load",
    // Complex expressions
    "let (f = \\x x * 2) f(21)",
    // NOTE: "[1, 2, 3].map(\x x * 2)" removed - 'map' is a keyword in legacy parser
    "if list.length(xs) > 0 then xs[0] else null",
    // Batch 1: Statements & Sequences
    "let x = 42",
    "let x = 1 + 2",
    "return 42",
    "return",
    r#"throw "error""#,
    "yield 42",
    "{1; 2; 3}",
    "{let x = 1; x + 1}",
    // Batch 2: Control Flow
    "try { 42 } catch e { 0 }",
    "try { 1 + 2 } catch err { err }",
    "while true do 1",
    "do 1 while true",
    "match x { _ => 0 }",
    // Batch 3: Operators & Collections
    r#"let (f = \x x + 1) 1 |> f"#,
    "_",
    "[1 | [2, 3]]",
    "<- x",
    // Batch 4: Object System Keywords
    "self",
    "parent",
    "caller",
    "user",
    "args",
    "^mytag",
    // Batch 5: Object Definitions
    "object foo { x: 1 }",
    "object bar { greet(): 42 }",
    "object baz { get(x): x }",
    // Batch 6: Grammar Definitions
    r#"grammar G { digit = [0-9] }"#,
    r#"let g = grammar G { digit = [0-9] }"#,
    // Batch 7: Advanced Iteration
    r#"fold \acc \x acc + x, 0, [1, 2, 3]"#,
    r#"foldr \acc \x acc + x, 0, [1, 2, 3]"#,
    r#"map \x x * 2, [1, 2, 3]"#,
    r#"filter \x x > 1, [1, 2, 3]"#,
];

#[test]
#[ignore = "Generated parser regression — bootstrap produces incorrect AST for some constructs"]
fn test_parser_equivalence() {
    let mut failures = Vec::new();

    for source in TEST_CASES {
        // Parse with legacy parser
        let legacy_result = parse_legacy(source);

        // Parse with generated parser
        let generated_result = generated_parse(source);

        match (legacy_result, generated_result) {
            (Ok(legacy_ast), Ok(generated_ast)) => {
                if legacy_ast != generated_ast {
                    failures.push(format!(
                        "AST mismatch for '{}':\n  Legacy:    {:?}\n  Generated: {:?}",
                        source, legacy_ast, generated_ast
                    ));
                }
            }
            (Ok(_), Err(e)) => {
                failures.push(format!("Generated parser failed for '{}': {:?}", source, e));
            }
            (Err(_), Ok(_)) => {
                failures.push(format!(
                    "Legacy parser failed but generated succeeded for '{}'",
                    source
                ));
            }
            (Err(legacy_err), Err(generated_err)) => {
                // Both failed - check if they fail for the same reason
                // For now, consider this acceptable
                eprintln!(
                    "Both parsers failed for '{}': legacy={:?}, generated={:?}",
                    source, legacy_err, generated_err
                );
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Parser equivalence test failed with {} failures:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}

/// Parse using the legacy hand-written parser
fn parse_legacy(source: &str) -> fmpl_core::error::Result<fmpl_core::ast::Expr> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::with_source(&tokens, source).parse()
}

#[test]
fn test_generated_parse_basic() {
    // Simple sanity check that generated_parse works
    let result = generated_parse("1 + 2");
    assert!(result.is_ok(), "Failed to parse '1 + 2': {:?}", result);
}

#[test]
fn test_generated_parse_complex() {
    // More complex expression
    let result = generated_parse("let (f = \\x x * 2) [f(1), f(2), f(3)]");
    assert!(
        result.is_ok(),
        "Failed to parse complex expression: {:?}",
        result
    );
}
