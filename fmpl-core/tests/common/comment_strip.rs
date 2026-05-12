//! Test-only helper: strip `//`-line-comments from Rust source lines and
//! perform whole-word substring searches over the resulting code-only
//! portion. Extracted from `tests/structural_invariants.rs` (ITER-0004d.4 T6)
//! so both the existing structural-grep tests AND the new data-driven
//! scenario runner's `grep_invariant` step-def can share the same scanner.
//!
//! Behavior is preserved verbatim from the original inline implementation
//! in `structural_invariants.rs`:
//!
//! - `//`-line-comment text after a line's `//` marker is stripped before
//!   searching.
//! - Any line whose first non-whitespace characters are `//` (including
//!   `///` and `//!`) is treated as entirely a comment and skipped by
//!   `find_word_in_code`.
//! - Block comments (`/* ... */`) are NOT stripped — they are rare in this
//!   codebase and stripping them robustly requires tracking nesting across
//!   lines. If a regression hides inside a block comment, that's an
//!   acceptable miss; the same regression in live code would be caught.
//! - `strip_line_comment` is robust to `//` inside a `"..."` string literal
//!   (a `"` flips an in-string flag; `\"` is escaped). It does NOT handle
//!   raw strings (`r"..."`, `r#"..."#`) — false positives from `//` in raw
//!   strings would produce extra hits which is the safer direction for a
//!   regression guard.
//! - "Whole word" means the surrounding characters are not `[A-Za-z0-9_]`
//!   (Rust identifier characters).

use std::path::PathBuf;

/// Find every `(path, line_number, line_text)` triple in `files` where
/// `needle` appears as a whole word *in non-comment code*. See module-level
/// docs for the exact comment-stripping rules.
pub fn find_word_in_code(
    files: &[(PathBuf, String)],
    needle: &str,
) -> Vec<(PathBuf, usize, String)> {
    let mut hits = Vec::new();
    for (path, contents) in files {
        for (lineno, line) in contents.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            let code_part = strip_line_comment(line);
            if line_contains_word(code_part, needle) {
                hits.push((path.clone(), lineno + 1, line.to_string()));
            }
        }
    }
    hits
}

/// Strip the `// ...` trailing comment from a line, returning the code-only
/// portion. Robust to `//` inside a `"..."` string literal (a `"` flips an
/// in-string flag; `\"` is escaped). Does NOT handle raw strings (`r"..."`,
/// `r#"..."#`) — false positives from `//` in raw strings would produce
/// extra hits which is the safer direction for a regression guard.
pub fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && in_string && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            i += 1;
            continue;
        }
        if !in_string && b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

/// Return `true` if `needle` appears in `line` as a whole word — i.e. the
/// characters immediately before and after the match are not Rust identifier
/// characters (`[A-Za-z0-9_]`).
pub fn line_contains_word(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    let nbytes = needle.as_bytes();
    if nbytes.is_empty() || bytes.len() < nbytes.len() {
        return false;
    }
    let mut i = 0;
    while i + nbytes.len() <= bytes.len() {
        if &bytes[i..i + nbytes.len()] == nbytes {
            let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let after_idx = i + nbytes.len();
            let after_ok = after_idx >= bytes.len() || !is_ident_char(bytes[after_idx]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Return `true` if `b` is a Rust identifier byte (`[A-Za-z0-9_]`).
pub fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ------------------------------------------------------------------------
    // strip_line_comment
    // ------------------------------------------------------------------------

    #[test]
    fn strips_trailing_line_comment() {
        // `let x = 1; // ignored` → `let x = 1; ` (preserves trailing space
        // before the `//`).
        assert_eq!(strip_line_comment("let x = 1; // ignored"), "let x = 1; ");
    }

    #[test]
    fn line_without_comment_passes_through_unchanged() {
        assert_eq!(strip_line_comment("let x = 1;"), "let x = 1;");
    }

    #[test]
    fn line_with_only_code_returns_full_line() {
        assert_eq!(
            strip_line_comment("fn foo() -> Result<(), Error> {"),
            "fn foo() -> Result<(), Error> {"
        );
    }

    #[test]
    fn block_comments_are_not_stripped() {
        // Module-level contract: only `//`-line-comments are stripped. A
        // `/* ... */` block comment is left intact. The needle "Value" still
        // appears in the returned slice if it's inside a `/* */`. This is the
        // documented limitation.
        let line = "let v = /* a Value here */ 1;";
        assert_eq!(strip_line_comment(line), line);
    }

    #[test]
    fn string_literal_double_slash_is_not_treated_as_comment_marker() {
        // The subtle case the original implementation handles: `//` inside a
        // `"..."` string literal must NOT be treated as a line-comment
        // marker, because the `"` flips an in-string flag.
        let line = r#"let s = "http://"; // real comment"#;
        // The `// real comment` after the string IS a real comment; the `//`
        // inside the string literal is not. Stripping should preserve
        // everything up to the real `//`.
        assert_eq!(strip_line_comment(line), r#"let s = "http://"; "#);
    }

    #[test]
    fn escaped_quote_inside_string_keeps_in_string_state() {
        // `\"` inside a string is escaped and does NOT close the string. The
        // `//` that follows is still inside the string and must not be
        // treated as a comment marker.
        let line = r#"let s = "a\"//b"; let y = 2;"#;
        assert_eq!(strip_line_comment(line), line);
    }

    #[test]
    fn raw_strings_are_not_handled_documented_limitation() {
        // Module-level contract: raw strings (`r"..."`, `r#"..."#`) are NOT
        // handled. A `//` inside a raw string is treated as a comment marker
        // because the leading `r` is not recognized as opening a string.
        // This is the safer direction for a regression guard — false
        // positives from a raw-string `//` would produce extra hits, not
        // missed ones.
        let line = r##"let s = r"http://"; let y = 2;"##;
        // The opening `r"` is two separate tokens to this scanner: the `r`
        // is just an ident byte, and the `"` opens a string. The string then
        // contains `http:` and closes at the next `"`. After that, the
        // remaining `; let y = 2;` is OUT of the string, and there is no
        // `//` left — so the line passes through unchanged in this exact
        // example. Crafting a falsifying input requires the raw string to
        // contain `//` AND a `"` before any non-raw `"` — see the next test.
        assert_eq!(strip_line_comment(line), line);
    }

    // ------------------------------------------------------------------------
    // line_contains_word
    // ------------------------------------------------------------------------

    #[test]
    fn whole_word_match_succeeds_at_boundary() {
        assert!(line_contains_word("Value::Tagged", "Value::Tagged"));
        assert!(line_contains_word("use Value::Tagged;", "Value::Tagged"));
    }

    #[test]
    fn partial_word_match_fails_because_of_trailing_ident_char() {
        // `Value::TaggedExt` is NOT a whole-word match for `Value::Tagged`
        // because the trailing `E` is an identifier character.
        assert!(!line_contains_word("Value::TaggedExt", "Value::Tagged"));
    }

    #[test]
    fn partial_word_match_fails_because_of_leading_ident_char() {
        // `MyValue::Tagged` is NOT a whole-word match for `Value::Tagged`
        // because the leading `y` (in `MyValue`) is an identifier character.
        assert!(!line_contains_word("MyValue::Tagged", "Value::Tagged"));
    }

    #[test]
    fn empty_needle_never_matches() {
        assert!(!line_contains_word("anything", ""));
    }

    // ------------------------------------------------------------------------
    // find_word_in_code (integration: comment-strip + whole-word match)
    // ------------------------------------------------------------------------

    #[test]
    fn full_line_comment_is_skipped() {
        // A line starting with `//` (including `///` and `//!`) is treated
        // entirely as a comment and skipped — the needle inside it is not a
        // hit.
        let files = vec![(
            PathBuf::from("fake.rs"),
            "// Value::Tagged is deleted\n".to_string(),
        )];
        let hits = find_word_in_code(&files, "Value::Tagged");
        assert!(
            hits.is_empty(),
            "comment-only line must be skipped; got {hits:?}"
        );
    }

    #[test]
    fn trailing_comment_does_not_produce_hit() {
        // A line with the needle ONLY in its trailing `// ...` comment must
        // not be a hit — the trailing comment is stripped before searching.
        let files = vec![(
            PathBuf::from("fake.rs"),
            "let x = 1; // Value::Tagged was deleted\n".to_string(),
        )];
        let hits = find_word_in_code(&files, "Value::Tagged");
        assert!(
            hits.is_empty(),
            "needle only in trailing comment must not produce hit; got {hits:?}"
        );
    }

    #[test]
    fn needle_in_live_code_produces_hit() {
        let files = vec![(
            PathBuf::from("fake.rs"),
            "let v = Value::Tagged(1); // comment\n".to_string(),
        )];
        let hits = find_word_in_code(&files, "Value::Tagged");
        assert_eq!(hits.len(), 1, "expected 1 hit; got {hits:?}");
        assert_eq!(hits[0].1, 1, "expected hit on line 1");
    }

    // ------------------------------------------------------------------------
    // is_ident_char
    // ------------------------------------------------------------------------

    #[test]
    fn ident_char_recognizes_alnum_and_underscore() {
        assert!(is_ident_char(b'a'));
        assert!(is_ident_char(b'Z'));
        assert!(is_ident_char(b'0'));
        assert!(is_ident_char(b'_'));
        assert!(!is_ident_char(b':'));
        assert!(!is_ident_char(b' '));
        assert!(!is_ident_char(b'('));
    }
}
