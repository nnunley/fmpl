//! Workspace-wide no-fjall invariant.
//!
//! After ITER-0005a.6 closed, fjall is named ONLY inside
//! `fmpl-persistence` (the Store trait's only backend impl) and the
//! `fmpl-core` dev-deps (integration tests that probe `FjallStore`
//! semantics directly). Every other consumer must go through
//! `fmpl_persistence::Store` / `FjallStore` and must NOT name
//! `fjall::*` or `use fjall` in source.
//!
//! This gate replaces ITER-0005a.5's per-crate `no_fjall_in_fmpl_core`
//! check (which lived at `fmpl-core/tests/persistence_envelope_invariant.rs`)
//! with a single workspace-wide scan. The old gate's writer-bypass
//! invariant (`keyspace.insert(` / `partition.insert(`) is unchanged
//! and stays where it is — different scope.
//!
//! Per `feedback_prefer_proof_tests.md` form #4 (universally-
//! quantified structural assertion).

use std::fs;
use std::path::{Path, PathBuf};

/// Consumer crate source roots, relative to the workspace root.
/// Listing them explicitly (rather than walking the workspace) keeps
/// the contract readable and makes adding new consumers an explicit
/// edit, not an accidental capture.
///
/// `fmpl-core/tests` is included because the previous in-crate gate
/// (per ITER-0005a.5 R-D-C-1) scanned the test surface — dev-deps
/// permit fjall direct use at compile time, and the gate exists
/// specifically to prevent that pattern. This workspace-wide gate
/// preserves that coverage while extending it cross-crate.
const CONSUMER_CRATES: &[&str] = &["fmpl-core/src", "fmpl-core/tests", "fmpl-web/src"];

/// Substrings that mean fjall has been named in source. Both forms
/// must be absent from every CONSUMER_CRATES root.
///
/// `fjall::` and `use fjall` catch direct fjall-crate references.
/// `: FjallStore` catches type-position uses of the concrete backend
/// (struct fields, function signatures, generic bounds) that would
/// re-leak the backend identity that ITER-0005a.6 R-H-C-1 closed.
/// Constructor call sites (`FjallStore::open(...)`) are NOT caught
/// because `FjallStore::` doesn't match `: FjallStore` — that
/// intentional asymmetry lets consumers construct the type for
/// immediate boxing into `Box<dyn Store + Send + Sync>` fields.
const FJALL_NAME_SUBSTRINGS: &[&str] = &["fjall::", "use fjall", ": FjallStore"];

/// Resolve the workspace root from this crate's manifest dir.
/// `env!("CARGO_MANIFEST_DIR")` points at `fmpl-workspace-tests/`;
/// `.parent()` once gives the workspace root. Centralized here so
/// the cross-crate path math lives in exactly one place.
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("workspace root above CARGO_MANIFEST_DIR")
        .to_path_buf()
}

fn walk_rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(it) => it,
        Err(e) => panic!("read_dir({}): {e}", root.display()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rust_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Strip `//` line comments and `/* ... */` block comments so the
/// scanner doesn't false-positive on historical narratives.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

#[test]
fn no_fjall_in_consumers() {
    let root = workspace_root();
    let mut files = Vec::new();
    for relative in CONSUMER_CRATES {
        let crate_root = root.join(relative);
        walk_rust_files(&crate_root, &mut files);
    }

    let mut violations: Vec<(PathBuf, &'static str)> = Vec::new();
    for path in &files {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let stripped = strip_comments(&src);
        for needle in FJALL_NAME_SUBSTRINGS {
            if stripped.contains(needle) {
                violations.push((path.clone(), *needle));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "no-fjall-in-consumers gate failed: the following consumer-crate \
         files name `fjall` directly. Storage must be consumed through \
         `fmpl_persistence::Store` / `FjallStore`; fjall MUST NOT be \
         named in {CONSUMER_CRATES:?}:\n{}",
        violations
            .iter()
            .map(|(p, n)| format!("  {} ← `{n}`", p.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Sanity check: the scanner correctly detects the forbidden patterns.
#[test]
fn gate_detects_forbidden_patterns() {
    assert!(strip_comments("use fjall::Keyspace;").contains("use fjall"));
    assert!(strip_comments("let x: fjall::Error = e;").contains("fjall::"));
    // Type-position FjallStore use (per R-H-C-1):
    assert!(strip_comments("struct S { f: FjallStore }").contains(": FjallStore"));
    assert!(strip_comments("fn f(s: FjallStore) {}").contains(": FjallStore"));
    // But constructor calls should NOT trip the gate:
    assert!(!strip_comments("FjallStore::open(p).unwrap()").contains(": FjallStore"));
}

/// Sanity check: comments don't false-positive.
#[test]
fn gate_strips_comments_before_matching() {
    let with_comment = "// use fjall::Keyspace; — forbidden\nlet a = 1;";
    let stripped = strip_comments(with_comment);
    assert!(!stripped.contains("use fjall"));
    assert!(!stripped.contains("fjall::"));
}
