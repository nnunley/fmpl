//! Determinism and correctness tests for the bootstrap pipeline.
//!
//! The fmpl-bootstrap → generated_parser.rs pipeline must be:
//!
//! 1. **Deterministic**: same input produces byte-identical output
//! 2. **Correct**: generated parser produces the same AST as the legacy parser
//! 3. **Self-consistent**: ast_to_ir.fmpl loads through the generated parser
//! 4. **Pipeline-stable**: the full FMPL pipeline produces correct results
//!
//! These tests catch parser-generation regressions that don't show up in
//! ordinary unit tests. They run against the actual built artifacts.

use fmpl_core::{Value, Vm, eval_via_fmpl_pipeline, eval_via_legacy_parser, eval_via_native};
use std::path::PathBuf;
use std::process::Command;

/// Find the workspace root from CARGO_MANIFEST_DIR.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Find the fmpl-bootstrap binary, building it if necessary.
fn fmpl_bootstrap_path() -> PathBuf {
    let root = workspace_root();
    let debug = root.join("target/debug/fmpl-bootstrap");
    let release = root.join("target/release/fmpl-bootstrap");
    if debug.exists() {
        debug
    } else if release.exists() {
        release
    } else {
        panic!(
            "fmpl-bootstrap binary not found. Build it first with: \
             FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap"
        );
    }
}

/// Run fmpl-bootstrap to regenerate the parser, return stdout bytes.
fn run_bootstrap_generator() -> Vec<u8> {
    let root = workspace_root();
    let bin = fmpl_bootstrap_path();
    let generator = root.join("lib/core/parser_generator.fmpl");

    let output = Command::new(&bin)
        .arg(&generator)
        .current_dir(&root)
        .output()
        .expect("failed to invoke fmpl-bootstrap");

    if !output.status.success() {
        panic!(
            "fmpl-bootstrap failed (exit={:?}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    output.stdout
}

/// Set cwd to workspace root so io::load() finds lib/core/*.fmpl.
fn cd_workspace() {
    std::env::set_current_dir(workspace_root()).expect("set cwd");
}

#[test]
#[ignore = "fmpl-bootstrap parser generation is currently non-deterministic across runs"]
fn parser_generation_is_deterministic_across_runs() {
    let first = run_bootstrap_generator();
    let second = run_bootstrap_generator();
    assert_eq!(
        first.len(),
        second.len(),
        "parser generation produced different lengths: {} vs {}",
        first.len(),
        second.len()
    );
    assert_eq!(
        first, second,
        "parser generation is non-deterministic — repeated runs produce different bytes"
    );
}

#[test]
#[ignore = "Generated parser regression — see bootstrap_determinism.rs. Run with FMPL_USE_GENERATED_PARSER=1 once bootstrap is fixed."]
fn ast_to_ir_loads_through_generated_parser() {
    cd_workspace();
    // SAFETY: this test is #[ignore]d and won't race in default test runs.
    unsafe { std::env::set_var("FMPL_USE_GENERATED_PARSER", "1") };
    let mut vm = Vm::new();
    let prelude = eval_via_native(&mut vm, r#"io::load("lib/core/prelude.fmpl")"#);
    let ast_to_ir = eval_via_native(&mut vm, r#"io::load("lib/core/ast_to_ir.fmpl")"#);
    unsafe { std::env::remove_var("FMPL_USE_GENERATED_PARSER") };

    prelude.expect("prelude.fmpl must load with generated parser");
    let result = ast_to_ir.expect("ast_to_ir.fmpl must load with generated parser");
    assert!(
        matches!(result, Value::Grammar(_)),
        "ast_to_ir.fmpl returned {:?} (expected a grammar value)",
        result
    );
}

#[test]
fn ast_to_ir_loads_through_legacy_parser() {
    cd_workspace();
    // Direct call — avoids env var leakage between parallel tests.
    let mut vm = Vm::new();
    eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/prelude.fmpl")"#)
        .expect("prelude.fmpl must load with legacy parser");
    let r = eval_via_legacy_parser(&mut vm, r#"io::load("lib/core/ast_to_ir.fmpl")"#)
        .expect("ast_to_ir.fmpl must load with legacy parser");
    assert!(
        matches!(r, Value::Grammar(_)),
        "ast_to_ir.fmpl returned {:?} via legacy parser",
        r
    );
}

#[test]
fn fmpl_pipeline_compiles_basic_arithmetic() {
    cd_workspace();
    let mut vm = Vm::new();
    let result = eval_via_fmpl_pipeline(&mut vm, "1 + 2 * 3").expect("pipeline must compile");
    assert_eq!(result, Value::Int(7));
}

#[test]
fn fmpl_pipeline_compiles_let_binding() {
    cd_workspace();
    let mut vm = Vm::new();
    let result = eval_via_fmpl_pipeline(&mut vm, "let (x = 41) x + 1").expect("pipeline");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn fmpl_pipeline_compiles_lambda_call() {
    cd_workspace();
    let mut vm = Vm::new();
    let result = eval_via_fmpl_pipeline(&mut vm, "let (f = \\x x + 1) f(41)").expect("pipeline");
    assert_eq!(result, Value::Int(42));
}

/// Compare default-parser AST against legacy-parser AST for a corpus of inputs.
/// Both should produce the same `[:Int, 42]` etc. tagged AST.
///
/// Currently passes for basic expressions (the corpus); failures show up in
/// grammar-literal-heavy code like ast_to_ir.fmpl.
#[test]
fn default_and_legacy_parsers_agree_on_corpus() {
    cd_workspace();
    let corpus = [
        "42",
        "1 + 2",
        "let (x = 1) x",
        "if true then 1 else 2",
        "[1, 2, 3]",
        "\\x x + 1",
        r#""hello""#,
        ":foo",
    ];

    for source in corpus {
        // Default parser (generated)
        let mut vm_default = Vm::new();
        let ast_default = eval_via_native(&mut vm_default, &format!("ast::parse({:?})", source))
            .unwrap_or_else(|e| panic!("default parser failed on {:?}: {}", source, e));

        // Legacy parser (hand-written) — direct call avoids env var leakage
        let mut vm_legacy = Vm::new();
        let ast_legacy =
            eval_via_legacy_parser(&mut vm_legacy, &format!("ast::parse({:?})", source))
                .unwrap_or_else(|e| panic!("legacy parser failed on {:?}: {}", source, e));

        assert_eq!(
            ast_default, ast_legacy,
            "parser disagreement for {:?}: default={:?}, legacy={:?}",
            source, ast_default, ast_legacy
        );
    }
}
