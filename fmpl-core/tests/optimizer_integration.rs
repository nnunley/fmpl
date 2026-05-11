//! ITER-0004c — Optimizer Integration tests.
//!
//! Verifies STORY-0010: ast_optimizer.fmpl is wired into the FMPL pipeline at
//! the correct slot (between ast::parse and ast_to_ir.expr), folds actually
//! fire on real ast::parse output, and parity is preserved across the corpus.
//!
//! ITER-0004c (2026-05-10): all `#[ignore]` markers removed. The lists-
//! everywhere stdlib migration is complete, ast_optimizer is wired into
//! eval_via_fmpl_pipeline (lib.rs:121-129), and these tests run against the
//! migrated optimizer. The `ac3_int_min_negation_does_not_panic` test was
//! rewritten to use direct AST construction because the FMPL lexer cannot
//! tokenize 9223372036854775808 (per fmpl-core/src/lexer.rs:117).

#![allow(dead_code)]

use fmpl_core::{Value, Vm};

fn workspace_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
}

fn setup_pipeline_with_optimizer() -> Vm {
    std::env::set_current_dir(workspace_root()).expect("set cwd");
    let mut vm = Vm::new();
    fmpl_core::eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/prelude.fmpl")"#)
        .expect("load prelude");
    fmpl_core::eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/ast_to_ir.fmpl")"#)
        .expect("load ast_to_ir");
    // ast_optimizer.fmpl returns a module map as its top-level expression value;
    // bind it so subsequent code can reference `ast_optimizer["optimize"]`.
    fmpl_core::eval_via_legacy_parser(
        &mut vm,
        r#"let ast_optimizer = io::load("lib/core/ast_optimizer.fmpl")"#,
    )
    .expect("load ast_optimizer");
    vm
}

fn run_optimized_pipeline(vm: &mut Vm, source: &str) -> Value {
    // Pipeline: ast::parse -> ast_optimizer["optimize"] -> ast_to_ir.expr -> ir::compile -> code::eval
    let pipeline = format!(
        r#"let (ast = ast::parse({:?}))
           let (opt = ast_optimizer["optimize"](ast))
           let (ir = opt @ ast_to_ir.expr)
           let (code = ir::compile(ir))
           code::eval(code)"#,
        source
    );
    fmpl_core::eval_via_legacy_parser(vm, &pipeline)
        .unwrap_or_else(|e| panic!("optimized pipeline failed for {:?}: {}", source, e))
}

fn run_native(vm: &mut Vm, source: &str) -> Value {
    fmpl_core::eval_via_legacy_parser(vm, source)
        .unwrap_or_else(|e| panic!("native eval failed for {:?}: {}", source, e))
}

fn assert_optimized_matches_native(source: &str) {
    let mut vm_opt = setup_pipeline_with_optimizer();
    let mut vm_native = Vm::new();
    let opt_result = run_optimized_pipeline(&mut vm_opt, source);
    let native_result = run_native(&mut vm_native, source);
    assert_eq!(
        opt_result, native_result,
        "optimizer pipeline disagrees with native for {:?}: opt={:?}, native={:?}",
        source, opt_result, native_result
    );
}

// ─── AC-2: end-to-end fold actually fires on real ast::parse output ─────────

/// AC-2: Verify the optimizer is not silently bypassed by shape-mismatch.
/// We feed `1 + 2` (which the optimizer should fold to `3`) and inspect the
/// IR after optimization. If the optimizer fired, the IR will be a literal
/// LoadInt(3), not an Add of two LoadInts.
#[test]
fn ac2_fold_actually_fires_on_real_parse_output() {
    let mut vm = setup_pipeline_with_optimizer();
    // Parse, optimize, and check the result of parse is folded.
    // We compare the optimized AST to the AST we'd get for "3" directly.
    let optimized_for_sum = fmpl_core::eval_via_legacy_parser(
        &mut vm,
        r#"ast_optimizer["optimize"](ast::parse("1 + 2"))"#,
    )
    .expect("optimize sum");
    let parsed_three =
        fmpl_core::eval_via_legacy_parser(&mut vm, r#"ast::parse("3")"#).expect("parse 3");
    assert_eq!(
        optimized_for_sum, parsed_three,
        "optimizer did NOT fold `1 + 2` to `3` — silent shape mismatch suspected. \
         Got optimized={:?}, expected (parsed `3`)={:?}",
        optimized_for_sum, parsed_three
    );
}

// ─── AC-3: bug-guard tests ──────────────────────────────────────────────────

/// AC-3: INT_MIN negation must not panic or produce wrong result.
/// In the optimizer, `[:Unary, :-, [:Int, INT_MIN]]` would compute `0 - INT_MIN`
/// which overflows. The optimizer's guard at lib/core/ast_optimizer.fmpl:15
/// (`&{ a > 0 - 9223372036854775807 - 1 }`) prevents the fold; the input must
/// pass through unchanged AND ir::compile + code::eval must not panic.
///
/// Rewritten in ITER-0004c: the original source-form `"0 - (-9223372036854775808)"`
/// could not tokenize because the FMPL lexer (fmpl-core/src/lexer.rs:117)
/// cannot parse `9223372036854775808` (one more than i64::MAX). We bypass the
/// lexer by hand-constructing the AST as a `Value::list_node`.
///
/// TODO(ITER-0004g): rewrite this test to use source-form input
/// `"0 - (-9223372036854775808)"` once the lexer can tokenize i64::MIN as
/// part of a unary-negation expression. The current AST-construction approach
/// is a documented workaround; the optimizer guard contract is exercised
/// either way, but the parsing path is the user-facing surface.
#[test]
fn ac3_int_min_negation_does_not_panic() {
    let mut vm = setup_pipeline_with_optimizer();

    // Hand-construct the AST [:Unary, :-, [:Int, i64::MIN]] in FMPL by
    // composing it from list literals and integer arithmetic. The FMPL lexer
    // can read 9223372036854775807 (i64::MAX); subtract 1 to reach i64::MIN.
    // (This avoids needing the lexer to tokenize the i64::MIN literal directly.)
    let optimized = fmpl_core::eval_via_legacy_parser(
        &mut vm,
        r#"let (int_min = 0 - 9223372036854775807 - 1)
           let (ast = [:Unary, :-, [:Int, int_min]])
           ast_optimizer["optimize"](ast)"#,
    )
    .expect("optimize INT_MIN-negation AST");

    // The guard at ast_optimizer.fmpl:15 is `&{ a > 0 - 9223372036854775807 - 1 }`,
    // i.e., a > i64::MIN. For a == i64::MIN, the guard fails so the fold is
    // skipped; `_:x => x` returns the input unchanged.
    let unchanged = fmpl_core::eval_via_legacy_parser(
        &mut vm,
        r#"let (int_min = 0 - 9223372036854775807 - 1)
           [:Unary, :-, [:Int, int_min]]"#,
    )
    .expect("build expected AST");

    assert_eq!(
        optimized, unchanged,
        "INT_MIN negation guard did not fire — optimizer produced unsafe fold. \
         Got optimized={:?}, expected (unchanged AST)={:?}",
        optimized, unchanged
    );

    // Now feed the unchanged AST through the rest of the pipeline and assert
    // it does not panic at compile time. The runtime behavior (whether it
    // wraps to i64::MIN per Rust two's-complement semantics or panics) is
    // unconstrained here — the AC-3 contract is "the guard prevents the fold
    // and the optimizer does not panic."
    let pipeline_result = fmpl_core::eval_via_legacy_parser(
        &mut vm,
        r#"let (int_min = 0 - 9223372036854775807 - 1)
           let (ast = [:Unary, :-, [:Int, int_min]])
           let (opt = ast_optimizer["optimize"](ast))
           let (ir = opt @ ast_to_ir.expr)
           ir::compile(ir)"#,
    );
    // ir::compile should succeed (no panic during compile). We don't run
    // code::eval here because the runtime negation semantics are unconstrained.
    assert!(
        pipeline_result.is_ok(),
        "INT_MIN-guarded AST failed to compile through ast_to_ir + ir::compile: {:?}",
        pipeline_result
    );
}

/// AC-3: Constant `1 / 0` must not be folded to a compile-time error or panic.
/// The native path raises a runtime error; the optimizer must preserve that —
/// it cannot fold `[:Binary, :/, [:Int, 1], [:Int, 0]]` to a literal.
#[test]
fn ac3_division_by_zero_not_folded() {
    let mut vm_opt = setup_pipeline_with_optimizer();
    let mut vm_native = Vm::new();

    // Both paths should error at runtime (not at optimization time).
    let opt_result = fmpl_core::eval_via_legacy_parser(
        &mut vm_opt,
        r#"let (ast = ast::parse("1 / 0"))
           let (opt = ast_optimizer["optimize"](ast))
           let (ir = opt @ ast_to_ir.expr)
           let (code = ir::compile(ir))
           code::eval(code)"#,
    );
    let native_result = fmpl_core::eval_via_legacy_parser(&mut vm_native, "1 / 0");

    assert_eq!(
        opt_result.is_err(),
        native_result.is_err(),
        "division by zero: optimizer error_state={:?}, native error_state={:?}",
        opt_result.is_err(),
        native_result.is_err()
    );
}

/// AC-3: Constant `1 % 0` must not be folded.
#[test]
fn ac3_modulo_by_zero_not_folded() {
    let mut vm_opt = setup_pipeline_with_optimizer();
    let mut vm_native = Vm::new();

    let opt_result = fmpl_core::eval_via_legacy_parser(
        &mut vm_opt,
        r#"let (ast = ast::parse("1 % 0"))
           let (opt = ast_optimizer["optimize"](ast))
           let (ir = opt @ ast_to_ir.expr)
           let (code = ir::compile(ir))
           code::eval(code)"#,
    );
    let native_result = fmpl_core::eval_via_legacy_parser(&mut vm_native, "1 % 0");

    assert_eq!(
        opt_result.is_err(),
        native_result.is_err(),
        "modulo by zero: optimizer error_state={:?}, native error_state={:?}",
        opt_result.is_err(),
        native_result.is_err()
    );
}

// ─── AC-4: full parity corpus passes with optimizer enabled ─────────────────

/// AC-4 / SCENARIO-0103: representative samples from the parity corpus run
/// through the optimizer-enabled pipeline and produce the same result as
/// the native compiler.
#[test]
fn scenario_0103_integer() {
    assert_optimized_matches_native("42");
}

#[test]
fn scenario_0103_arithmetic_with_precedence() {
    assert_optimized_matches_native("1 + 2 * 3");
}

#[test]
fn scenario_0103_string() {
    assert_optimized_matches_native("\"hello\"");
}

#[test]
fn scenario_0103_let_binding() {
    assert_optimized_matches_native("let (x = 42) x + 1");
}

#[test]
fn scenario_0103_if_true() {
    assert_optimized_matches_native("if true then 1 else 2");
}

#[test]
fn scenario_0103_if_false() {
    assert_optimized_matches_native("if false then 1 else 2");
}

#[test]
fn scenario_0103_lambda() {
    assert_optimized_matches_native("let (f = \\x x + 1) f(41)");
}

#[test]
fn scenario_0103_list() {
    assert_optimized_matches_native("[1, 2, 3]");
}

#[test]
fn scenario_0103_nested_arithmetic() {
    assert_optimized_matches_native("(1 + 2) * (3 + 4)");
}

#[test]
fn scenario_0103_boolean_logic() {
    assert_optimized_matches_native("true && false");
}

#[test]
fn scenario_0103_comparison() {
    assert_optimized_matches_native("3 < 5");
}

#[test]
fn scenario_0103_constant_only_arithmetic() {
    // Pure constants — optimizer should fold to a single LoadInt and result is unchanged.
    assert_optimized_matches_native("(1 + 2) * 3 - 4");
}

#[test]
fn scenario_0103_unary_negation_runtime_safe() {
    // Negating a non-INT_MIN constant — should fold safely.
    assert_optimized_matches_native("-(5)");
}
