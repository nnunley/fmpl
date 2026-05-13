//! Schema-format anti-rot ratchet for fmpl-persistence (ITER-0005a.5 T6).
//!
//! `fmpl_persistence::schema` is the single source of truth for the
//! envelope wire-format version, the `PayloadKind` taxonomy, and each
//! kind's current schema version. This test scans `fmpl-persistence/src/`
//! for any file **outside the schema-aware module set** that references
//! the schema-format literals — `ENVELOPE_FORMAT_VERSION`, `PayloadKind::`,
//! `current_schema_version` — and fails if it finds any. Per
//! `feedback_prefer_proof_tests.md` form #4: universally-quantified
//! structural assertion preventing rot.
//!
//! Scope choice: the schema-aware module set comprises `schema.rs` plus
//! its direct schema-aware siblings `envelope.rs` and `loader.rs`. The
//! envelope writer stamps the literals into the header and the loader
//! reads them back for the compatibility check — both legitimately read
//! the constants via `use` and qualified paths. Forbidding bare reads
//! in the sibling modules would force opaque indirection for no real
//! benefit. The ratchet's contract is "nothing OUTSIDE the schema-aware
//! module set redefines or re-derives these"; that's the structural
//! invariant this scan enforces.
//!
//! Future iterations adding persistence consumers (Store impls,
//! migration tooling, etc.) must reference the schema through helper
//! functions exposed by `envelope` / `loader` rather than re-stating
//! `ENVELOPE_FORMAT_VERSION`, naming `PayloadKind` variants directly,
//! or re-deriving `current_schema_version()` per kind. The intent is
//! that the wire-format definition has exactly one source of truth.

use std::fs;
use std::path::{Path, PathBuf};

/// Identifiers that are forbidden outside the schema-aware module set.
/// If any of these appears in another `fmpl-persistence/src/` file, the
/// ratchet fails.
///
/// - `ENVELOPE_FORMAT_VERSION` forbids alternative declarations of the
///   wire-format version (every consumer must route through
///   `schema::ENVELOPE_FORMAT_VERSION`).
/// - `PayloadKind::` forbids re-deriving the payload taxonomy outside
///   the schema-aware modules. The double-colon ensures we only flag
///   actual variant accesses (`PayloadKind::Foo`, `PayloadKind::method`),
///   not the bare type name in trait bounds or function signatures.
/// - `current_schema_version` forbids re-stating per-kind schema versions.
///   Consumers must call `kind.current_schema_version()` rather than
///   inlining version literals.
const FORBIDDEN_LITERALS: &[&str] = &[
    "ENVELOPE_FORMAT_VERSION",
    "PayloadKind::",
    "current_schema_version",
    // Strip-comments + (where applicable) word-boundary matching keeps
    // these from firing on doc-comment text or substrings of unrelated
    // identifiers.
];

/// Files exempted from the scan. The schema-aware module set:
/// `schema.rs` is THE source of truth and the sibling modules
/// `envelope.rs` and `loader.rs` legitimately read the literals via
/// `use` and qualified paths (the envelope writer stamps them into the
/// header; the loader reads them back for compatibility checks). The
/// scope-card contract is that nothing OUTSIDE this set redefines or
/// re-derives the schema-format constants.
fn is_exempt(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.ends_with("/schema.rs") || s.ends_with("/envelope.rs") || s.ends_with("/loader.rs")
}

fn fmpl_persistence_src() -> PathBuf {
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
/// both sides. Used for bare-identifier literals
/// (`ENVELOPE_FORMAT_VERSION`, `current_schema_version`) where a
/// substring match inside a larger identifier should not fire.
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

/// Whether `needle` should be matched with a leading identifier-boundary
/// (false for path-style needles ending in `::`, where the trailing
/// colons are themselves the boundary).
fn needs_word_boundary(needle: &str) -> bool {
    !needle.ends_with("::")
}

/// Plain substring search: returns true if `needle` appears anywhere in
/// `haystack`. Used for path-style literals like `PayloadKind::` where
/// the trailing `::` is the natural identifier boundary.
fn contains_substring(haystack: &str, needle: &str) -> bool {
    haystack.contains(needle)
}

#[test]
fn schema_format_anti_rot_no_literals_outside_schema_aware_modules() {
    let mut files = Vec::new();
    walk_rust_files(&fmpl_persistence_src(), &mut files);

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
            let hit = if needs_word_boundary(needle) {
                contains_as_identifier(&stripped, needle)
            } else {
                contains_substring(&stripped, needle)
            };
            if hit {
                violations.push((path.clone(), *needle));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Schema-format anti-rot ratchet failed: \
         the following files outside the schema-aware module set \
         (`schema.rs`, `envelope.rs`, `loader.rs`) reference \
         schema-format literals (route through `schema` / `envelope` / \
         `loader` instead):\n{}",
        violations
            .iter()
            .map(|(p, n)| format!("  {} ← `{n}`", p.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Sanity check: the ratchet's scanners distinguish identifier-bounded
/// matches from incidental substring matches, and handle the
/// path-style `PayloadKind::` literal via substring search.
#[test]
fn ratchet_identifier_boundary_detection() {
    // Word-boundary matches.
    assert!(contains_as_identifier(
        "ENVELOPE_FORMAT_VERSION",
        "ENVELOPE_FORMAT_VERSION"
    ));
    assert!(contains_as_identifier(
        "let v = ENVELOPE_FORMAT_VERSION;",
        "ENVELOPE_FORMAT_VERSION"
    ));
    assert!(contains_as_identifier(
        "fn current_schema_version() -> u16 { 1 }",
        "current_schema_version"
    ));
    // Substring within a larger identifier should NOT match.
    assert!(!contains_as_identifier(
        "MY_ENVELOPE_FORMAT_VERSION_FOO",
        "ENVELOPE_FORMAT_VERSION"
    ));
    assert!(!contains_as_identifier(
        "compute_current_schema_version_for",
        "current_schema_version"
    ));

    // Path-style needle: substring search.
    assert!(contains_substring(
        "PayloadKind::ObjectRecord",
        "PayloadKind::"
    ));
    assert!(contains_substring(
        "match k { PayloadKind::Grammar => 1, _ => 0 }",
        "PayloadKind::"
    ));
    // Bare `PayloadKind` (no `::`) must NOT match: it's a legal type
    // name in function signatures and trait bounds.
    assert!(!contains_substring(
        "fn handle(k: PayloadKind) {}",
        "PayloadKind::"
    ));
}
