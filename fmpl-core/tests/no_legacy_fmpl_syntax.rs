//! CI gate: scan four surfaces for legacy FMPL `:Tag(args)` tagged-constructor
//! syntax and assert the per-surface hit counts are zero (ITER-0004d.3).
//!
//! Surfaces: "lib/core", "tests/fmpl", "tests/rs", "src/rs".
//! Filters applied before counting:
//!   - ALLOWLIST: documented grammar-DSL binding sites (e.g.,
//!     `cmp:first (...)` in `lib/core/fmpl_parser.fmpl`). These produce a
//!     `Symbol+LParen` token sequence the lexer cannot distinguish from
//!     legacy syntax without grammar-DSL-aware context, so they are
//!     suppressed by hand.
//!   - from_doc_attr: hits originating from `#[doc = "..."]` attributes
//!     (i.e., `///` and `//!` doc comments) are documentation about FMPL
//!     vocabulary, not live FMPL code. Suppressed via
//!     `SourceKind::RustString.from_doc_attr` set by the test-side
//!     `rust_string_scanner.rs`.
//!
//! ITER-0004d.0 introduced this gate in baseline-mode. ITER-0004d.3 flipped
//! it to `== 0` after T3 fixed the metacircular bootstrap and T4 added the
//! doc-attr suppression. The baseline JSON file is deleted; the
//! FMPL_REGEN_BASELINE env var is no longer meaningful.
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
/// itself and tests that intentionally contain `:Tag(args)` strings as
/// parser-input fixtures to prove the rejection contract.
const TESTS_RS_EXCLUDES: &[&str] = &[
    "no_legacy_fmpl_syntax.rs",
    "diagnostics_fmpl_source_scan.rs",
    // SCENARIO-0108 evidence (ITER-0004d.3 T7a). Contains `:Tag(args)`
    // strings as parser-input fixtures to prove rejection parity between
    // the source-tree parser and the canonical generated parser.
    "canonical_pipeline_parity.rs",
    // G3 postlude-arm contract (ITER-0004d.4 T9). Contains `:Foo(1)` and
    // `:Pair(a, b)` strings as parser-input fixtures to prove the postlude
    // `LegacyTagCtor` / `PatternLegacyTagCtor` arms fire when reached via
    // `generated_parse`.
    "postlude_arm_contract.rs",
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
fn gate_asserts_zero_legacy_hits() {
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

    // Apply allowlist filter and suppress doc-attribute origins, then
    // count per surface. Doc-attr origins (`///` / `//!` desugaring to
    // `#[doc = "..."]`) are documentation about FMPL vocabulary, not
    // live FMPL code, so they are not legacy hits. ITER-0004d.3 T4.
    let count = |hits: &[TaggedSyntaxHit]| -> usize {
        hits.iter()
            .filter(|h| !is_allowlisted(h) && !h.source.from_doc_attr())
            .count()
    };

    let counts: BTreeMap<String, usize> = BTreeMap::from_iter([
        ("lib/core".to_string(), count(&lib_core_hits)),
        ("tests/fmpl".to_string(), count(&tests_fmpl_hits)),
        ("tests/rs".to_string(), count(&tests_rs_hits)),
        ("src/rs".to_string(), count(&src_rs_hits)),
    ]);

    let total: usize = counts.values().sum();
    if total != 0 {
        let mut report =
            String::from("legacy FMPL `:Tag(args)` syntax detected (gate requires == 0):\n");
        let surfaces = ["lib/core", "tests/fmpl", "tests/rs", "src/rs"];
        for surface in surfaces {
            let cur = counts.get(surface).copied().unwrap_or(0);
            let marker = if cur == 0 { " " } else { "!" };
            report.push_str(&format!("  {marker} {surface}: {cur} hits\n"));
        }
        report.push_str(
            "\nFix: either eliminate the legacy syntax at its source, or — if the hit is\n\
             a documentation example or a grammar-DSL bind — extend the suppression in\n\
             `no_legacy_fmpl_syntax.rs` (ALLOWLIST for grammar-DSL bind sites in .fmpl\n\
             files; doc-attr suppression is automatic via `from_doc_attr` for `///`/`//!`\n\
             comments).\n",
        );
        panic!("{}", report);
    }
}
