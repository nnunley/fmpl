//! Build script for fmpl-core
//!
//! Generates the parser at build time by running the parser generator through fmpl-bootstrap.
//!
//! ## Bootstrap Strategy
//!
//! To avoid a circular dependency (fmpl-core build.rs -> fmpl-bootstrap -> fmpl-core),
//! we use a two-phase bootstrap:
//!
//! 1. First build: FMPL_BOOTSTRAP_PHASE=1 is set, we skip generation and use fallback
//! 2. After fmpl-bootstrap is built: Normal builds use the pre-built binary
//!
//! During normal development:
//! - `cargo build` will try to use a pre-built fmpl-bootstrap if available
//! - If not available, falls back to the legacy parser
//! - Set FMPL_SKIP_PARSER_GEN=1 to always use legacy parser

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

fn main() {
    // Get directory paths
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let workspace_root = Path::new(&manifest_dir).parent().unwrap();

    // Track dependencies for incremental builds
    println!("cargo::rerun-if-changed=build.rs");

    // Track FMPL source files that the parser generator depends on
    let fmpl_sources = [
        "lib/core/prelude.fmpl",
        "lib/core/fmpl_parser.fmpl",
        "lib/core/parser_generator.fmpl",
        "lib/core/grammar_optimizer.fmpl",
        "lib/core/optimize_grammar.fmpl",
        "lib/core/ast_to_ir.fmpl",
        "lib/core/ir_to_rust.fmpl",
    ];

    for source in &fmpl_sources {
        let path = workspace_root.join(source);
        if path.exists() {
            println!("cargo::rerun-if-changed={}", path.display());
        }
    }

    // Track the fmpl-bootstrap binary itself
    // If it changes, we need to regenerate the parser
    let bootstrap_binary = workspace_root.join("target/debug/fmpl-bootstrap");
    let release_bootstrap = workspace_root.join("target/release/fmpl-bootstrap");

    if bootstrap_binary.exists() {
        println!("cargo::rerun-if-changed={}", bootstrap_binary.display());
    }
    if release_bootstrap.exists() {
        println!("cargo::rerun-if-changed={}", release_bootstrap.display());
    }

    // Skip generation if explicitly requested or during bootstrap
    if env::var("FMPL_SKIP_PARSER_GEN").is_ok() || env::var("FMPL_BOOTSTRAP_PHASE").is_ok() {
        println!("cargo::warning=Parser generation skipped, using legacy parser");
        write_fallback_parser(&out_dir);
        return;
    }

    // Fail stale parser checks in CI (or when explicitly enforced).
    let enforce_freshness =
        env::var("CI").is_ok() || env::var("FMPL_ENFORCE_PARSER_FRESHNESS").is_ok();
    let enforce_determinism =
        env::var("CI").is_ok() || env::var("FMPL_ENFORCE_PARSER_DETERMINISM").is_ok();

    // Look for a pre-built fmpl-bootstrap binary
    // This avoids the circular dependency by using an already-built binary
    let binary_path = if bootstrap_binary.exists() {
        bootstrap_binary
    } else if release_bootstrap.exists() {
        release_bootstrap
    } else {
        // No pre-built binary available
        // This happens on first build or clean build
        let generated_parser_path = Path::new(&out_dir).join("generated_parser.rs");
        let stale = should_regenerate(workspace_root, &fmpl_sources, &generated_parser_path);

        if enforce_freshness && stale {
            panic!(
                "Parser is stale and fmpl-bootstrap is unavailable. Run 'FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap' first."
            );
        }

        println!("cargo::warning=fmpl-bootstrap not found, using legacy parser");
        println!("cargo::warning=Run 'FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap' first");
        write_fallback_parser(&out_dir);
        return;
    };

    // Path to the generator
    let generator_path = workspace_root.join("lib/core/parser_generator.fmpl");

    // Check if generator exists
    if !generator_path.exists() {
        if enforce_freshness {
            panic!("Parser generator not found at {}", generator_path.display());
        }
        println!(
            "cargo::warning=Parser generator not found at {}, using legacy parser",
            generator_path.display()
        );
        write_fallback_parser(&out_dir);
        return;
    }

    // Get the modification time of the bootstrap binary
    // If the binary is newer than the generated parser, regenerate
    let generated_parser_path = Path::new(&out_dir).join("generated_parser.rs");
    let should_regenerate =
        should_regenerate(workspace_root, &fmpl_sources, &generated_parser_path)
            || is_newer_than(&binary_path, &generated_parser_path);

    if !should_regenerate {
        // Parser is up to date, skip generation
        return;
    }

    // Run the parser generator with the pre-built binary
    let output = run_generator(&binary_path, &generator_path, workspace_root);

    match output {
        Ok(output) if output.status.success() => {
            if enforce_determinism {
                let second = run_generator(&binary_path, &generator_path, workspace_root)
                    .expect("Failed to rerun parser generation for determinism check");

                if !second.status.success() {
                    panic!(
                        "Determinism check failed: second parser generation run returned non-zero status"
                    );
                }

                if output.stdout != second.stdout {
                    panic!(
                        "Determinism check failed: repeated parser regeneration produced different bytes"
                    );
                }
            }

            let rust_code =
                String::from_utf8(output.stdout).expect("Generated code is not valid UTF-8");

            let dest_path = Path::new(&out_dir).join("generated_parser.rs");
            fs::write(&dest_path, &rust_code).expect("Failed to write generated parser");

            println!(
                "cargo::warning=Generated parser written to {}",
                dest_path.display()
            );
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if enforce_freshness {
                panic!(
                    "Parser generation failed while freshness is enforced: {}",
                    stderr
                );
            }
            println!("cargo::warning=Parser generation failed: {}", stderr);
            println!("cargo::warning=Using legacy parser as fallback");
            write_fallback_parser(&out_dir);
        }
        Err(e) => {
            if enforce_freshness {
                panic!(
                    "Failed to run fmpl-bootstrap while freshness is enforced: {}",
                    e
                );
            }
            println!("cargo::warning=Failed to run fmpl-bootstrap: {}", e);
            println!("cargo::warning=Using legacy parser as fallback");
            write_fallback_parser(&out_dir);
        }
    }
}

fn should_regenerate(
    workspace_root: &Path,
    fmpl_sources: &[&str],
    generated_parser_path: &Path,
) -> bool {
    let Some(generated_time) = modified_time(generated_parser_path) else {
        return true;
    };

    for source in fmpl_sources {
        let source_path = workspace_root.join(source);
        let Some(source_time) = modified_time(&source_path) else {
            continue;
        };
        if source_time > generated_time {
            return true;
        }
    }

    false
}

fn is_newer_than(lhs: &Path, rhs: &Path) -> bool {
    let Some(lhs_time) = modified_time(lhs) else {
        return true;
    };
    let Some(rhs_time) = modified_time(rhs) else {
        return true;
    };
    lhs_time > rhs_time
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn run_generator(
    binary_path: &Path,
    generator_path: &Path,
    workspace_root: &Path,
) -> std::io::Result<std::process::Output> {
    Command::new(binary_path)
        .arg(generator_path)
        .current_dir(workspace_root)
        .output()
}

/// Write a fallback parser that delegates to the legacy parser
fn write_fallback_parser(out_dir: &str) {
    // Note: This code is included into parser.rs via include!()
    // The parent module already imports:
    // - use crate::ast::*; (includes Expr)
    // - use crate::error::{Error, Result};
    // - use crate::lexer::{SpannedToken, Token};
    // So we must NOT re-import Expr or Result, but we do need Lexer
    let fallback_code = r#"// Fallback generated parser - delegates to legacy parser
// Generated by build.rs when parser generation was skipped or failed

/// Parse FMPL source code (fallback - uses legacy parser)
pub fn generated_parse(source: &str) -> Result<Expr> {
    let tokens = crate::lexer::Lexer::new(source).tokenize()?;
    Parser::with_source(&tokens, source).parse()
}
"#;

    let dest_path = Path::new(out_dir).join("generated_parser.rs");
    fs::write(&dest_path, fallback_code).expect("Failed to write fallback parser");
}
