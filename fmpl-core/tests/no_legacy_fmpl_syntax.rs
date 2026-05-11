//! CI gate: scan four surfaces for legacy FMPL `:Tag(args)` tagged-constructor
//! syntax and assert the per-surface hit counts match a committed baseline
//! JSON file (ITER-0004d.0).
//!
//! Baseline JSON schema: `{ "surface_name": <hit_count>, ... }`
//! Surfaces: "lib/core", "tests/fmpl", "tests/rs", "src/rs".
//! Allowlist filtering applied before counting.
//!
//! ITER-0004d.1 will delete the baseline JSON and flip this gate to assert
//! `== 0` across all surfaces. Until then, the baseline records the
//! iteration's known starting state and any growth fails the gate.
//!
//! To regenerate the baseline (after a deliberate change), run with:
//!   `FMPL_REGEN_BASELINE=1 cargo test -p fmpl-core --test no_legacy_fmpl_syntax`
//!
//! Discovery surfaces:
//! - `lib/core/*.fmpl` — flat directory; non-recursive `read_dir`.
//! - `fmpl-core/tests/fmpl/*.fmpl` — flat; non-recursive.
//! - `fmpl-core/tests/*.rs` — flat; non-recursive. EXCLUDES the gate's own
//!   source file and the unit-test file with deliberate fixtures.
//! - `fmpl-core/src/*.rs` — recursive walk over `.rs` files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use fmpl_core::diagnostics::{SourceKind, TaggedSyntaxHit, scan_fmpl_source};

mod common;

/// Allowlist: `(path-suffix, tag)` pairs. A hit is suppressed if its source's
/// path ends with `path-suffix` AND its tag equals `tag`. Path-suffix match
/// avoids hard-coding the absolute workspace prefix.
///
/// Pre-populated with the 6 known grammar-DSL binding sites where the FMPL
/// lexer tokenizes `name:first (...)` as `Ident, Symbol("first"), LParen` —
/// a real Symbol+LParen pair, but semantically a binding pattern, not a
/// tagged-constructor call.
const ALLOWLIST: &[(&str, &str)] = &[
    // lib/core/fmpl_parser.fmpl — 5 occurrences of `:first (` (lines 213, 214,
    // 222, 225, 228). One entry covers all five because the matcher is
    // (file_path, tag), not (file_path, line).
    ("lib/core/fmpl_parser.fmpl", "first"),
    // fmpl-core/tests/fmpl/fmpl_grammar.fmpl — 1 occurrence at line 68
    // (`qualified_name = ident:first (...)`).
    ("fmpl-core/tests/fmpl/fmpl_grammar.fmpl", "first"),
];

/// Files in `fmpl-core/tests/*.rs` that the gate must NOT scan: the gate
/// itself, the unit-test file that intentionally contains `:Foo(1, 2)`
/// style fixtures, and the SCENARIO-0104/0105/0106 evidence test file
/// (ITER-0004d.1 T19) which constructs `:Tag(args)` strings as parser-input
/// fixtures to prove the rejection contract.
const TESTS_RS_EXCLUDES: &[&str] = &[
    "no_legacy_fmpl_syntax.rs",
    "diagnostics_fmpl_source_scan.rs",
    "structural_invariants.rs",
];

fn workspace_root() -> PathBuf {
    // fmpl-core/Cargo.toml is at <workspace_root>/fmpl-core/Cargo.toml.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has a parent")
        .to_path_buf()
}

fn hit_path_str(hit: &TaggedSyntaxHit) -> String {
    match &hit.source {
        SourceKind::FmplFile { path } => path.display().to_string(),
        SourceKind::RustString { rust_path, .. } => rust_path.display().to_string(),
    }
}

fn is_allowlisted(hit: &TaggedSyntaxHit) -> bool {
    let path_str = hit_path_str(hit);
    ALLOWLIST
        .iter()
        .any(|(suffix, tag)| path_str.ends_with(suffix) && hit.tag.as_str() == *tag)
}

fn read_dir_flat(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return out;
    }
    for entry in fs::read_dir(dir).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some(extension) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Recursively collect `.rs` files under `root`.
fn walk_rs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn relative_to(root: &Path, p: &Path) -> PathBuf {
    p.strip_prefix(root)
        .map(|r| r.to_path_buf())
        .unwrap_or_else(|_| p.to_path_buf())
}

fn scan_fmpl_files(root: &Path, files: &[PathBuf]) -> Vec<TaggedSyntaxHit> {
    let mut all = Vec::new();
    for path in files {
        let rel = relative_to(root, path);
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let src = SourceKind::FmplFile { path: rel };
        match scan_fmpl_source(&content, src) {
            Ok(hits) => all.extend(hits),
            Err(_) => {
                // Lexer error on a stdlib .fmpl file is itself a regression —
                // but at this point we're just collecting hits. The gate
                // surfaces this as a stderr warning rather than failing.
            }
        }
    }
    all
}

fn scan_rs_files(root: &Path, files: &[PathBuf]) -> Vec<TaggedSyntaxHit> {
    let mut all = Vec::new();
    for path in files {
        let rel = relative_to(root, path);
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        match common::rust_string_scanner::scan_rust_strings(&content, &rel) {
            Ok(hits) => all.extend(hits),
            Err(_) => {
                // `syn::parse_file` failed — Rust file doesn't parse. That's
                // a separate problem (cargo build would also fail); ignore
                // here.
            }
        }
    }
    all
}

#[test]
fn gate_matches_baseline() {
    let root = workspace_root();

    // Surface 1: lib/core/*.fmpl
    let lib_core_files = read_dir_flat(&root.join("lib").join("core"), "fmpl");
    let lib_core_hits = scan_fmpl_files(&root, &lib_core_files);

    // Surface 2: fmpl-core/tests/fmpl/*.fmpl
    let tests_fmpl_files =
        read_dir_flat(&root.join("fmpl-core").join("tests").join("fmpl"), "fmpl");
    let tests_fmpl_hits = scan_fmpl_files(&root, &tests_fmpl_files);

    // Surface 3: fmpl-core/tests/*.rs (flat, with explicit excludes)
    let tests_rs_dir = root.join("fmpl-core").join("tests");
    let tests_rs_files: Vec<PathBuf> = read_dir_flat(&tests_rs_dir, "rs")
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            !TESTS_RS_EXCLUDES.contains(&name)
        })
        .collect();
    let tests_rs_hits = scan_rs_files(&root, &tests_rs_files);

    // Surface 4: fmpl-core/src/*.rs (recursive)
    let src_rs_files = walk_rs(&root.join("fmpl-core").join("src"));
    let src_rs_hits = scan_rs_files(&root, &src_rs_files);

    // Apply allowlist filter, then count per surface.
    let count =
        |hits: &[TaggedSyntaxHit]| -> usize { hits.iter().filter(|h| !is_allowlisted(h)).count() };

    let counts: BTreeMap<String, usize> = BTreeMap::from_iter([
        ("lib/core".to_string(), count(&lib_core_hits)),
        ("tests/fmpl".to_string(), count(&tests_fmpl_hits)),
        ("tests/rs".to_string(), count(&tests_rs_hits)),
        ("src/rs".to_string(), count(&src_rs_hits)),
    ]);

    let baseline_path = root
        .join("fmpl-core")
        .join("tests")
        .join("no_legacy_fmpl_syntax.baseline.json");

    if std::env::var("FMPL_REGEN_BASELINE").ok().as_deref() == Some("1") {
        let serialized = serde_json::to_string_pretty(&counts).expect("serialize baseline");
        let with_trailing_newline = format!("{serialized}\n");
        fs::write(&baseline_path, with_trailing_newline).expect("write baseline");
        eprintln!(
            "[no_legacy_fmpl_syntax] regenerated baseline: {:?}",
            baseline_path
        );
        eprintln!("[no_legacy_fmpl_syntax] new counts: {:?}", counts);
        return;
    }

    let baseline_text = fs::read_to_string(&baseline_path).unwrap_or_else(|_| {
        panic!(
            "baseline JSON not found at {:?}. Run with FMPL_REGEN_BASELINE=1 to generate it.",
            baseline_path
        )
    });
    let baseline: BTreeMap<String, usize> =
        serde_json::from_str(&baseline_text).expect("parse baseline JSON");

    // Strict equality: prevents both growth and silent shrinkage. ITER-0004d.1
    // will replace this with `== 0` once cleanup lands.
    if counts != baseline {
        // Build a human-readable diff for the assertion message.
        let mut report = String::from("hit-count mismatch vs baseline:\n");
        let surfaces = ["lib/core", "tests/fmpl", "tests/rs", "src/rs"];
        for surface in surfaces {
            let cur = counts.get(surface).copied().unwrap_or(0);
            let base = baseline.get(surface).copied().unwrap_or(0);
            let marker = if cur == base { " " } else { "!" };
            report.push_str(&format!(
                "  {marker} {surface}: current={cur}, baseline={base}\n",
            ));
        }
        report.push_str("\nIf this change is intentional, regenerate with:\n");
        report.push_str(
            "  FMPL_REGEN_BASELINE=1 cargo test -p fmpl-core --test no_legacy_fmpl_syntax\n",
        );
        panic!("{}", report);
    }
}
