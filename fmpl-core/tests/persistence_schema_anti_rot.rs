//! AC-6 anti-rot ratchet for STORY-0099.
//!
//! `fmpl_core::vm_version` is the single source of truth for the VM
//! version constants stamped into every persisted record. This test
//! scans `fmpl-core/src/` for any file **outside `vm_version.rs`**
//! that references the version-derivation literals — `CARGO_PKG_VERSION`,
//! `VM_VERSION_MAJOR`, `VM_VERSION_MINOR`, `VM_VERSION_PATCH` — and
//! fails if it finds any. Per `feedback_prefer_proof_tests.md` form #4:
//! universally-quantified structural assertion preventing rot.
//!
//! Scope choice (ITER-0005a.5 T6 update): the VM-version constants
//! moved from `persistence/schema.rs` to a top-level `vm_version.rs`
//! module per T0.5, because fmpl-persistence is now its own crate and
//! must stay version-agnostic. The exemption tracks the constants:
//! `vm_version.rs` itself (where they are defined) and `lib.rs` (which
//! `pub use`s them as part of the crate's public surface) are exempt.
//! Every other consumer must route through the canonical helper paths
//! exposed by `fmpl_core::VM_VERSION` / `fmpl_persistence::envelope`
//! rather than re-deriving the constants from
//! `env!("CARGO_PKG_VERSION")` or re-reading the bare identifiers.
//!
//! Future iterations adding persistence consumers must either (a) live
//! inside `vm_version.rs` (in scope for the exemption), or (b)
//! reference the constants through helper functions exposed by
//! `fmpl_persistence::envelope` (the canonical pattern — see
//! `EnvelopeHeader::new_for_current_vm`).

use std::fs;
use std::path::{Path, PathBuf};

/// Identifiers that are forbidden outside `vm_version.rs`. If any of
/// these appears in another `fmpl-core/src/` file, the ratchet fails.
///
/// - `CARGO_PKG_VERSION` forbids alternative version-derivation sites
///   (anyone outside `vm_version.rs` re-deriving the VM version from
///   the env var directly).
/// - `VM_VERSION_MAJOR` / `VM_VERSION_MINOR` / `VM_VERSION_PATCH`
///   forbid bare-identifier reads outside `vm_version.rs`. Downstream
///   consumers must route through the `fmpl_core::VM_VERSION` value
///   (or the `fmpl_persistence::envelope` helpers that take a
///   `VmVersion` parameter) rather than reading the per-component
///   constants directly.
const FORBIDDEN_LITERALS: &[&str] = &[
    "CARGO_PKG_VERSION",
    "VM_VERSION_MAJOR",
    "VM_VERSION_MINOR",
    "VM_VERSION_PATCH",
    // Strip-comments + word-boundary matching ensures these only fire
    // on actual identifiers, not on substrings of unrelated text.
];

/// Files exempted from the scan. Two exemptions:
///
/// 1. `vm_version.rs` — the source of truth where these constants are
///    defined.
/// 2. `lib.rs` — `pub use`s the constants as part of fmpl-core's public
///    surface so that downstream crates (notably fmpl-persistence
///    tests) can name `fmpl_core::VM_VERSION_MAJOR`. Without this
///    re-export the constants would not be reachable from outside
///    fmpl-core. The exemption is narrow — `lib.rs` only re-exports;
///    it must not consume or re-derive the constants.
fn is_exempt(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with("/vm_version.rs") || s.ends_with("/lib.rs")
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
/// source string. Conservative; only intended for the ratchet's
/// false-positive avoidance. Doesn't handle string literals containing
/// `//` (rare; not a real concern for the forbidden identifiers here).
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // line comment; skip to newline
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // block comment; skip past `*/`
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

/// Word-boundary substring search: returns true if `needle` appears in
/// `haystack` with non-identifier characters (or string boundaries) on
/// both sides.
fn contains_as_identifier(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let nb = needle.as_bytes();
    let mut i = 0;
    while i + nb.len() <= bytes.len() {
        if &bytes[i..i + nb.len()] == nb {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + nb.len() == bytes.len() || !is_ident_byte(bytes[i + nb.len()]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[test]
fn ac6_anti_rot_no_version_derivation_outside_schema() {
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
        for needle in FORBIDDEN_LITERALS {
            if contains_as_identifier(&stripped, needle) {
                violations.push((path.clone(), *needle));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "AC-6 anti-rot ratchet failed: \
         the following files outside `vm_version.rs` reference \
         version-derivation literals (use `fmpl_core::VM_VERSION` or \
         the `fmpl_persistence::envelope` helpers instead):\n{}",
        violations
            .iter()
            .map(|(p, n)| format!("  {} ← `{n}`", p.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Sanity check: the ratchet's substring scanner correctly distinguishes
/// identifier-bounded matches from incidental substring matches.
#[test]
fn ratchet_identifier_boundary_detection() {
    assert!(contains_as_identifier(
        "CARGO_PKG_VERSION",
        "CARGO_PKG_VERSION"
    ));
    assert!(contains_as_identifier(
        "env!(\"CARGO_PKG_VERSION\")",
        "CARGO_PKG_VERSION"
    ));
    assert!(contains_as_identifier(
        "let x = CARGO_PKG_VERSION;",
        "CARGO_PKG_VERSION"
    ));
    // Substring within a larger identifier should NOT match.
    assert!(!contains_as_identifier(
        "MY_CARGO_PKG_VERSION_FOO",
        "CARGO_PKG_VERSION"
    ));
}
