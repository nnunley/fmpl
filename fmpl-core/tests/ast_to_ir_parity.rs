//! Parity tests for FMPL IR compilation.
//!
//! These tests verify that the ir::compile() builtin correctly compiles
//! IR tagged values to bytecode that produces identical results
//! to the Rust compiler.

//!
//! NOTE: These tests use the ir::compile() builtin directly by constructing
//! IR tagged values in FMPL code. The ir::compile() builtin expects:
//! raw FMPL values as children (not tagged values).

use fmpl_core::{Value, Vm};

fn run_ir_pipeline(vm: &mut Vm, ir_source: &str) -> Value {
    // Use legacy parser to avoid generated-parser regressions affecting these
    // bootstrap-pipeline tests.
    fmpl_core::eval_via_legacy_parser(vm, ir_source)
        .unwrap_or_else(|_| panic!("IR pipeline failed for: {}", ir_source))
}

fn run_rust_compiler(vm: &mut Vm, source: &str) -> Value {
    fmpl_core::eval_via_legacy_parser(vm, source)
        .unwrap_or_else(|_| panic!("Rust compiler failed for: {}", source))
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

/// Setup helper: loads prelude and ast_to_ir into VM, returns VM ready for pipeline.
/// Uses `eval_via_legacy_parser` for library loads since ast_to_ir.fmpl
/// contains grammar definitions that don't parse cleanly through the
/// generated parser during bootstrap drift.
fn setup_fmpl_pipeline() -> Vm {
    // cargo test sets cwd to the crate directory (fmpl-core/), but lib/ is at workspace root
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    std::env::set_current_dir(workspace_root).expect("failed to set cwd to workspace root");

    let mut vm = Vm::new();
    fmpl_core::eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/prelude.fmpl")"#)
        .expect("failed to load prelude");
    fmpl_core::eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/ast_to_ir.fmpl")"#)
        .expect("failed to load ast_to_ir");
    vm
}

/// Run the source through the FMPL compiler pipeline (ast:: ast_to_ir -> ir::compile -> code::eval)
fn run_full_pipeline(vm: &mut Vm, source: &str) -> Value {
    let compile_code = format!(
        r#"let (ast = ast::parse({:?})) let (ir = ast @ ast_to_ir.expr) let (code = ir::compile(ir)) code::eval(code)"#,
        source
    );
    fmpl_core::eval_via_legacy_parser(vm, &compile_code)
        .unwrap_or_else(|_| panic!("FMPL pipeline failed for: {}", source))
}

fn assert_pipeline_parity(source: &str) {
    let mut vm_fmpl = setup_fmpl_pipeline();
    let mut vm_rust = Vm::new();

    let fmpl_result = run_full_pipeline(&mut vm_fmpl, source);
    let rust_result = run_rust_compiler(&mut vm_rust, source);

    assert_eq!(
        fmpl_result, rust_result,
        "Pipeline parity mismatch for '{}': FMPL={:?}, Rust={:?}",
        source, fmpl_result, rust_result
    );
}

mod literals {
    use super::*;

    #[test]
    fn integer() {
        assert_ir_parity("42", r#"code::eval(ir::compile([:LoadInt, 42]))"#);
    }

    #[test]
    fn bool_true() {
        assert_ir_parity("true", r#"code::eval(ir::compile([:LoadBool, true]))"#);
    }

    #[test]
    fn bool_false() {
        assert_ir_parity("false", r#"code::eval(ir::compile([:LoadBool, false]))"#);
    }

    #[test]
    fn null_value() {
        assert_ir_parity("null", r#"code::eval(ir::compile([:LoadNull]))"#);
    }

    #[test]
    fn string_literal() {
        assert_ir_parity(
            "\"hello world\"",
            r#"code::eval(ir::compile([:LoadString, "hello world"]))"#,
        );
    }
}

mod arithmetic {
    use super::*;

    #[test]
    fn addition() {
        assert_ir_parity(
            "1 + 2",
            r#"code::eval(ir::compile([:Add, [:LoadInt, 1], [:LoadInt, 2]]))"#,
        );
    }

    #[test]
    fn subtraction() {
        assert_ir_parity(
            "10 - 3",
            r#"code::eval(ir::compile([:Sub, [:LoadInt, 10], [:LoadInt, 3]]))"#,
        );
    }

    #[test]
    fn multiplication() {
        assert_ir_parity(
            "4 * 5",
            r#"code::eval(ir::compile([:Mul, [:LoadInt, 4], [:LoadInt, 5]]))"#,
        );
    }

    #[test]
    fn division() {
        assert_ir_parity(
            "20 / 4",
            r#"code::eval(ir::compile([:Div, [:LoadInt, 20], [:LoadInt, 4]]))"#,
        );
    }

    #[test]
    fn modulo() {
        assert_ir_parity(
            "17 % 5",
            r#"code::eval(ir::compile([:Mod, [:LoadInt, 17], [:LoadInt, 5]]))"#,
        );
    }

    #[test]
    fn negation() {
        assert_ir_parity("-42", r#"code::eval(ir::compile([:Neg, [:LoadInt, 42]]))"#);
    }
}

mod comparisons {
    use super::*;

    #[test]
    fn equality() {
        assert_ir_parity(
            "5 == 5",
            r#"code::eval(ir::compile([:Eq, [:LoadInt, 5], [:LoadInt, 5]]))"#,
        );
    }

    #[test]
    fn inequality() {
        assert_ir_parity(
            "5 != 3",
            r#"code::eval(ir::compile([:NotEq, [:LoadInt, 5], [:LoadInt, 3]]))"#,
        );
    }

    #[test]
    fn less_than() {
        assert_ir_parity(
            "3 < 5",
            r#"code::eval(ir::compile([:Lt, [:LoadInt, 3], [:LoadInt, 5]]))"#,
        );
    }

    #[test]
    fn greater_than() {
        assert_ir_parity(
            "5 > 3",
            r#"code::eval(ir::compile([:Gt, [:LoadInt, 5], [:LoadInt, 3]]))"#,
        );
    }

    #[test]
    fn less_than_equal() {
        assert_ir_parity(
            "5 <= 5",
            r#"code::eval(ir::compile([:LtEq, [:LoadInt, 5], [:LoadInt, 5]]))"#,
        );
    }

    #[test]
    fn greater_than_equal() {
        assert_ir_parity(
            "5 >= 5",
            r#"code::eval(ir::compile([:GtEq, [:LoadInt, 5], [:LoadInt, 5]]))"#,
        );
    }
}

mod logical {
    use super::*;

    #[test]
    fn and_operator() {
        assert_ir_parity(
            "true && true",
            r#"code::eval(ir::compile([:And, [:LoadBool, true], [:LoadBool, true]]))"#,
        );
    }

    #[test]
    fn or_operator() {
        assert_ir_parity(
            "false || true",
            r#"code::eval(ir::compile([:Or, [:LoadBool, false], [:LoadBool, true]]))"#,
        );
    }

    #[test]
    fn not_operator() {
        assert_ir_parity(
            "!true",
            r#"code::eval(ir::compile([:Not, [:LoadBool, true]]))"#,
        );
    }
}

mod control_flow {
    use super::*;

    #[test]
    fn if_true() {
        assert_ir_parity(
            "if true then 1 else 2",
            r#"code::eval(ir::compile([:If, [:LoadBool, true], [:LoadInt, 1], [:LoadInt, 2]]))"#,
        );
    }

    #[test]
    fn if_false() {
        assert_ir_parity(
            "if false then 1 else 2",
            r#"code::eval(ir::compile([:If, [:LoadBool, false], [:LoadInt, 1], [:LoadInt, 2]]))"#,
        );
    }
}

mod let_bindings {
    use super::*;

    #[test]
    fn simple_let() {
        assert_ir_parity(
            "let (x = 5) x",
            r#"code::eval(ir::compile([:Let, :x, [:LoadInt, 5], [:Var, :x]]))"#,
        );
    }

    #[test]
    fn let_with_arithmetic() {
        assert_ir_parity(
            "let (x = 5) x + 3",
            r#"code::eval(ir::compile([:Let, :x, [:LoadInt, 5], [:Add, [:Var, :x], [:LoadInt, 3]]]))"#,
        );
    }
}

mod data_structures {
    use super::*;

    #[test]
    fn empty_list() {
        assert_ir_parity("[]", r#"code::eval(ir::compile([:MakeList, []]))"#);
    }

    #[test]
    fn list_of_ints() {
        assert_ir_parity(
            "[1, 2, 3]",
            r#"code::eval(ir::compile([:MakeList, [[:LoadInt, 1], [:LoadInt, 2], [:LoadInt, 3]]]))"#,
        );
    }

    #[test]
    fn empty_map() {
        assert_ir_parity("%{}", r#"code::eval(ir::compile([:MakeMap, []]))"#);
    }

    #[test]
    fn map_literal() {
        assert_ir_parity(
            "%{a: 1}",
            r#"code::eval(ir::compile([:MakeMap, [[[:LoadString, "a"], [:LoadInt, 1]]]]))"#,
        );
    }
}

mod functions {
    use super::*;

    #[test]
    fn lambda_call() {
        assert_ir_parity(
            "(\\x x + 1)(5)",
            r#"code::eval(ir::compile([:Call, [:Lambda, [:x], [:Add, [:Var, :x], [:LoadInt, 1]]], [[:LoadInt, 5]]]))"#,
        );
    }
}

/// Full pipeline parity tests: FMPL parser + ast_to_ir vs Rust compiler
///
/// These tests verify that the complete FMPL compilation pipeline produces
/// identical results to the Rust compiler:
/// 1. ast::parse(source) -> AST tagged values
/// 2. ast @ ast_to_ir.expr -> IR tagged values
/// 3. ir::compile(ir) -> CompiledCode
/// 4. code::eval(code) -> result
///
/// NOTE: These tests are currently ignored because ast_to_ir.fmpl is incomplete.
/// See AGENTS.md line 230: "21 parity tests track progress" on incomplete features.
mod full_pipeline {
    use super::*;

    #[test]
    fn parity_integer() {
        assert_pipeline_parity("42");
    }

    #[test]
    fn parity_arithmetic() {
        assert_pipeline_parity("1 + 2 * 3");
    }

    #[test]
    fn parity_string() {
        assert_pipeline_parity("\"hello\"");
    }

    #[test]
    fn parity_let_binding() {
        assert_pipeline_parity("let (x = 42) x + 1");
    }

    #[test]
    fn parity_if_expr() {
        assert_pipeline_parity("if true then 1 else 2");
    }

    #[test]
    fn parity_lambda() {
        assert_pipeline_parity("let (f = \\x x + 1) f(41)");
    }

    #[test]
    fn parity_list() {
        assert_pipeline_parity("[1, 2, 3]");
    }

    #[test]
    fn parity_map() {
        assert_pipeline_parity("%{a: 1, b: 2}");
    }

    #[test]
    fn parity_symbol() {
        assert_pipeline_parity(":hello");
    }

    #[test]
    fn parity_tagged() {
        assert_pipeline_parity("[:Point, 1, 2]");
    }

    #[test]
    fn parity_index() {
        assert_pipeline_parity("[1, 2, 3][0]");
    }

    #[test]
    fn parity_prop_access() {
        assert_pipeline_parity("%{a: 1, b: 2}.a");
    }

    #[test]
    fn parity_nested_lambda() {
        assert_pipeline_parity("let (add = \\x \\y x + y) add(2)(3)");
    }

    #[test]
    fn parity_closure() {
        assert_pipeline_parity(
            "let (make_adder = \\n \\x x + n) let (add5 = make_adder(5)) add5(10)",
        );
    }

    #[test]
    fn parity_method_call() {
        assert_pipeline_parity("[1, 2, 3].len()");
    }
}

mod full_pipeline_loops {
    use super::*;

    #[test]
    fn parity_while_simple() {
        assert_pipeline_parity("while false do 1");
    }

    #[test]
    fn parity_for_simple() {
        assert_pipeline_parity("for x in [1, 2, 3] { x * 2 }");
    }
}

mod full_pipeline_sequences {
    use super::*;

    #[test]
    fn parity_sequence() {
        assert_pipeline_parity("let (a = 1) let (b = 2) a + b");
    }

    #[test]
    fn parity_block() {
        assert_pipeline_parity("{ let (x = 1) x + 1 }");
    }
}

mod full_pipeline_advanced {
    use super::*;

    #[test]
    fn parity_return_value() {
        assert_pipeline_parity("let (f = \\x return x + 1) f(41)");
    }

    #[test]
    fn parity_pipe_simple() {
        assert_pipeline_parity("1 |> \\x x + 1");
    }

    #[test]
    fn parity_slice_open() {
        assert_pipeline_parity("[1, 2, 3][1..]");
    }

    #[test]
    fn parity_slice_closed() {
        assert_pipeline_parity("[1, 2, 3][0..2]");
    }

    #[test]
    fn parity_match_simple() {
        assert_pipeline_parity("match 42 { _ => 0 }");
    }

    #[test]
    fn parity_match_tagged() {
        // ITER-0004d.1 T2b: list-pattern syntax `[:Tag, ...]` is now
        // recognized in match arms (parser heuristic) and let destructuring
        // (compiler arm). Bytecode is identical to the legacy
        // tagged-constructor form (same MatchTag + ExtractTaggedChild).
        assert_pipeline_parity("[:Point, 1, 2] @ { [:Point, x, y] => [x, y], _ => [] }");
    }

    #[test]
    fn parity_match_tagged_empty_constructor() {
        // ITER-0004d.1 T2b: [:Tag] (no children) must match a zero-arity tagged value.
        assert_pipeline_parity("[:None] @ { [:None] => 0, _ => 1 }");
    }

    #[test]
    fn parity_match_tagged_tag_mismatch_routes_to_fallthrough() {
        // ITER-0004d.1 T2b: tag mismatch falls through to next arm.
        assert_pipeline_parity("[:Bar, 1] @ { [:Foo, x] => 99, [:Bar, x] => x, _ => 0 }");
    }

    // ITER-0004d.1 T2b: arity-check semantics diverge between legacy compiler
    // (strict-arity MatchTag) and FMPL ir::compile (tag-only). Pre-existing
    // divergence; same gap for legacy tagged-constructor patterns too.
    // Follow-up tracked in task #30.
    #[test]
    #[ignore = "pipeline divergence — see task #30"]
    fn parity_match_tagged_arity_mismatch_routes_to_fallthrough() {
        assert_pipeline_parity("[:Point, 1, 2, 3] @ { [:Point, x, y] => 99, _ => 0 }");
    }

    // ITER-0004d.1 T2b: nested list-pattern children unsupported by FMPL
    // ir::compile pipeline. Pre-existing gap; same for nested legacy patterns.
    // Follow-up tracked in task #30.
    #[test]
    #[ignore = "ir::compile lacks nested pattern support — see task #30"]
    fn parity_match_tagged_nested() {
        assert_pipeline_parity("[:Outer, [:Inner, 7]] @ { [:Outer, [:Inner, n]] => n, _ => -1 }");
    }

    #[test]
    fn parity_try_catch() {
        assert_pipeline_parity("try { 42 } catch e { 0 }");
    }
}
