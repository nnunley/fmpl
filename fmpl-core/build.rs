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
    // Source-of-record for the parser-generator epoch (see parser_epoch.rs).
    // A bump invalidates the cached generated parser.
    println!("cargo::rerun-if-changed=src/parser_epoch.rs");
    // The Rust raw-string in this file is the postlude embedded into every
    // generated parser. Edits to it must invalidate the cache.
    println!("cargo::rerun-if-changed=src/builtins/ir_to_rust.rs");
    // Declare the cfg we emit on the successful-generation path so rustc's
    // unexpected_cfgs lint doesn't fire on the const-assert in parser.rs.
    println!("cargo::rustc-check-cfg=cfg(has_generated_parser_epoch)");
    // Allow env-var overrides to invalidate the cached build-script result.
    println!("cargo::rerun-if-env-changed=FMPL_SKIP_PARSER_GEN");
    println!("cargo::rerun-if-env-changed=FMPL_BOOTSTRAP_PHASE");
    println!("cargo::rerun-if-env-changed=FMPL_ENFORCE_PARSER_FRESHNESS");
    println!("cargo::rerun-if-env-changed=FMPL_ENFORCE_PARSER_DETERMINISM");
    println!("cargo::rerun-if-env-changed=CI");

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
    // Source-of-record epoch (single source of truth) vs. whatever the
    // cached generated parser embeds. Mismatch means the cache is stale even
    // if timestamps say otherwise.
    let source_epoch = read_source_parser_epoch(workspace_root);
    let generated_epoch = read_generated_parser_epoch(&generated_parser_path);
    let epoch_mismatch = match (source_epoch, generated_epoch) {
        (Some(src), Some(found)) => src != found,
        // No cached parser yet, or no epoch in it (e.g., fallback parser) —
        // either way, regenerate if we can. The successful regen path emits
        // the cfg flag; if we end up on the fallback path the cfg stays off
        // and the const-assert in parser.rs becomes dormant.
        _ => true,
    };
    let should_regenerate =
        should_regenerate(workspace_root, &fmpl_sources, &generated_parser_path)
            || is_newer_than(&binary_path, &generated_parser_path)
            || epoch_mismatch;

    if !should_regenerate {
        // Parser is up to date — propagate the cfg so parser.rs's
        // const-assert runs.
        println!("cargo::rustc-cfg=has_generated_parser_epoch");
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

            // Wrap generated code in a module with #![allow] for style lints.
            // The generator emits patterns that don't match Rust idioms but
            // are correct — refactoring would obscure the patterns.
            let rust_code = format!(
                concat!(
                    "// Generated code — clippy style lints suppressed.\n",
                    "// SAFETY: this output is machine-generated; re-running the\n",
                    "// generator can reshape these patterns, so suppress at file level.\n",
                    "#[allow(clippy::all)]\n",
                    "#[allow(clippy::pedantic)]\n",
                    "#[allow(unused_parens)]\n",
                    "#[allow(unused_variables)]\n",
                    "#[allow(unused_assignments)]\n",
                    "#[allow(dead_code)]\n",
                    "mod __generated {{\n",
                    "    use super::*;\n",
                    "    {}\n",
                    "}}\n",
                    "pub use __generated::*;\n",
                ),
                rust_code,
            );

            let dest_path = Path::new(&out_dir).join("generated_parser.rs");
            fs::write(&dest_path, &rust_code).expect("Failed to write generated parser");

            println!(
                "cargo::warning=Generated parser written to {}",
                dest_path.display()
            );
            // Generator succeeded — light up the const-assert in parser.rs.
            println!("cargo::rustc-cfg=has_generated_parser_epoch");
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

/// Read `PARSER_EPOCH` from `fmpl-core/src/parser_epoch.rs`.
///
/// Parses the source by scanning for the line `pub const PARSER_EPOCH: u32 = N;`.
/// This is a build-time check; we don't pull in `syn` for one constant.
fn read_source_parser_epoch(workspace_root: &Path) -> Option<u32> {
    let path = workspace_root.join("fmpl-core/src/parser_epoch.rs");
    let text = fs::read_to_string(&path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("pub const PARSER_EPOCH") else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix(':')?.trim_start();
        let rest = rest.strip_prefix("u32")?.trim_start();
        let rest = rest.strip_prefix('=')?.trim_start();
        let value: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return value.parse::<u32>().ok();
    }
    None
}

/// Read the `GENERATED_PARSER_EPOCH` literal from a cached generated parser.
///
/// Returns `None` if the file is missing or doesn't contain the constant
/// (e.g., the fallback parser).
fn read_generated_parser_epoch(generated_parser_path: &Path) -> Option<u32> {
    let text = fs::read_to_string(generated_parser_path).ok()?;
    let marker = "pub const GENERATED_PARSER_EPOCH";
    let idx = text.find(marker)?;
    let after = &text[idx + marker.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix("u32")?.trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    let value: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    value.parse::<u32>().ok()
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
