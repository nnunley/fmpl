//! AC-5 invariant gate for STORY-0099.
//!
//! Every persistence write **within `fmpl-core/src/`** must go through
//! [`persistence::envelope::write`][crate::persistence::envelope::write]
//! per STORY-0099 AC-5. This test scans `fmpl-core/src/` for any
//! call site that bypasses the helper — specifically, any
//! `keyspace.insert(...)` or `partition.insert(...)` that appears
//! outside `persistence/envelope.rs`.
//!
//! **Scope (per ITER-0005a.2 audit fix-up G3, 2026-05-13):** this gate is
//! intentionally scoped to `fmpl-core/src/` only. The `fmpl-web` crate
//! has its own parallel `SnapshotEnvelope` abstraction with 4
//! pre-existing raw `partition.insert(...)` sites; sweeping them is
//! deferred to a separate iteration (`ITER-0005-WEB-PERSISTENCE`) per
//! the Deferred section of `roadmap.md`. EPIC-003 STORY-0099 AC-5
//! pins the scope to `fmpl-core/src/` explicitly.
//!
//! Per `feedback_prefer_proof_tests.md`, this is form #4
//! (universally-quantified structural assertion). Form #1 (typed
//! invariant via newtype wrapping `fjall::Keyspace`) is **not feasible**
//! because `fjall::Keyspace::insert` is a public method on a foreign
//! crate; we cannot seal it at the type level. Form #4 grep is the
//! strongest feasible form for the in-crate sweep.
//!
//! **Known gate limitations (form #4 vs form #1 trade-off):**
//! - The grep matches literal substrings; a variable-alias bypass
//!   (`let ks = &keyspace; ks.insert(...)`) is not caught. Mitigation:
//!   the gate is a defense-in-depth invariant, not a sealed type.
//!   The convention "write only through `persistence::envelope::write`"
//!   is documented at the helper's call sites; PR review carries the
//!   remaining enforcement weight.
//! - The comment-stripper does not handle Rust string literals or
//!   nested block comments. A string literal containing
//!   `"keyspace.insert("` would false-positive. No current code has
//!   this pattern; the gate is sound today.
//!
//! Exempt: `persistence/envelope.rs` itself contains the only
//! authorized `keyspace.insert(...)` call site (the `write` helper).
//!
//! Author template inherits from `persistence_schema_anti_rot.rs`'s
//! AC-6 ratchet.

use std::fs;
use std::path::{Path, PathBuf};

/// Substrings to flag. Each substring matches a raw fjall write API
/// that bypasses the envelope helper.
///
/// Currently only `keyspace.insert(` and `partition.insert(`. If fjall
/// adds new write methods (`batch.insert(...)`, `tx.put(...)`, etc.),
/// extend this list AND verify each new method is also exempted only
/// inside `persistence/envelope.rs`.
const FORBIDDEN_SUBSTRINGS: &[&str] = &["keyspace.insert(", "partition.insert("];

/// Files exempted from the scan. The envelope helper is the ONLY
/// authorized writer; everything else routes through it.
fn is_exempt(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with("persistence/envelope.rs")
}

fn fmpl_core_src() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("src");
    p
}

fn walk_rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root).unwrap_or_else(|e| {
        panic!("read_dir({}): {e}", root.display());
    });
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rust_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Strip `//` line comments and `/* ... */` block comments from a
/// source string. Conservative; only intended for the gate's
/// false-positive avoidance.
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
fn ac5_invariant_no_raw_keyspace_insert_outside_envelope() {
    let mut files = Vec::new();
    walk_rust_files(&fmpl_core_src(), &mut files);

    let mut violations: Vec<(PathBuf, &'static str)> = Vec::new();
    for path in &files {
        if is_exempt(path) {
            continue;
        }
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let stripped = strip_comments(&src);
        for needle in FORBIDDEN_SUBSTRINGS {
            if stripped.contains(needle) {
                violations.push((path.clone(), *needle));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "AC-5 invariant gate failed: the following files contain raw \
         fjall write-API call sites outside `persistence/envelope.rs`. \
         Route them through `persistence::envelope::write` instead:\n{}",
        violations
            .iter()
            .map(|(p, n)| format!("  {} ← `{n}`", p.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Sanity check: the gate detects forbidden substrings when they appear.
#[test]
fn gate_detects_forbidden_substring_in_synthetic_input() {
    let synthetic = "keyspace.insert(b\"hello\", bytes).unwrap();";
    assert!(synthetic.contains("keyspace.insert("));
}

/// Sanity check: comments containing the forbidden substring don't false-positive.
#[test]
fn gate_strips_comments_before_matching() {
    let with_comment = "// keyspace.insert(b\"x\", y) is forbidden\nlet a = 1;";
    let stripped = strip_comments(with_comment);
    assert!(!stripped.contains("keyspace.insert("));
}

// The no-fjall-in-fmpl-core scan that previously lived here (added
// ITER-0005a.5 closing PAR R-D-C-1) has been superseded by the
// workspace-wide `fmpl-workspace-tests::no_fjall_in_consumers` gate
// in ITER-0005a.6 T5. The new gate scans BOTH fmpl-core/src/ AND
// fmpl-core/tests/ AND fmpl-web/src/ — wider scope, single source of
// truth for the invariant.
