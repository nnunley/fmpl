//! Unit tests for `fmpl_core::diagnostics::scan_fmpl_source` and the
//! `scan_rust_strings` test helper (ITER-0004d.0).
//!
//! These fixtures deliberately contain `:Foo(1, 2)` style strings — that's
//! the point. The CI gate (`no_legacy_fmpl_syntax.rs`) explicitly excludes
//! this file from its `tests/*.rs` scan surface to avoid flagging the
//! fixtures as real legacy hits.

use std::path::PathBuf;

use fmpl_core::diagnostics::{SourceKind, scan_fmpl_source};

mod common;

fn fmpl_source(path: &str) -> SourceKind {
    SourceKind::FmplFile {
        path: PathBuf::from(path),
    }
}

#[test]
fn scan_fmpl_source_uppercase_tagged_constructor_has_one_hit() {
    let src = r#"let x = :Foo(1, 2)"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert_eq!(hits[0].tag.as_str(), "Foo");
}

#[test]
fn scan_fmpl_source_lowercase_tagged_constructor_has_one_hit() {
    let src = r#"let x = :foo(1, 2)"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert_eq!(hits[0].tag.as_str(), "foo");
}

#[test]
fn scan_fmpl_source_list_pattern_has_zero_hits() {
    // `[:Foo, 1, 2]` — Symbol("Foo") is followed by Comma, not LParen.
    let src = r#"let x = [:Foo, 1, 2]"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_fmpl_source_bare_symbol_has_zero_hits() {
    let src = r#"let x = :foo"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_fmpl_source_comment_has_zero_hits() {
    let src = "-- :Foo(1, 2) in a comment\nlet x = 1";
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_fmpl_source_lexer_error_is_propagated() {
    // BELL char (\x07) outside any string literal triggers a lexer error,
    // which `scan_fmpl_source` surfaces as `DiagnosticsError::LexerError`.
    let src = "let x = \x07";
    let result = scan_fmpl_source(src, fmpl_source("test.fmpl"));
    assert!(
        result.is_err(),
        "expected lexer error to propagate, got {:?}",
        result
    );
}

#[test]
fn scan_fmpl_source_multiple_hits_preserves_order_and_offsets() {
    let src = r#":Foo(1) :Bar(2)"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].tag.as_str(), "Foo");
    assert_eq!(hits[1].tag.as_str(), "Bar");
    assert!(
        hits[0].byte_offset < hits[1].byte_offset,
        "offsets must preserve source order"
    );
}

#[test]
fn scan_fmpl_source_inside_fmpl_string_literal_has_zero_hits() {
    // The production FMPL lexer skips string-literal contents, so a
    // `:Tag(args)` embedded *inside* a quoted FMPL string never tokenizes
    // as Symbol+LParen — it's a single StringLit token. This invariant is
    // load-bearing for `scan_rust_strings`, which feeds FMPL-source-shaped
    // Rust string literals here and must not double-count their contents.
    let src = r#"let x = "fake :Foo(1, 2) inside a string""#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_fmpl_source_operator_symbol_call_has_zero_hits() {
    // Operator-style symbols (`:+`, `:==`, etc.) are not legacy
    // tagged-constructor syntax — they are operator references. A site like
    // `:+(args)` is exotic but legal under the lexer and must not be flagged.
    let src = r#"let f = :+; f(1, 2)"#;
    let hits = scan_fmpl_source(src, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);

    // Direct call form: `:+(args)` tokenizes as Symbol("+") + LParen but the
    // scanner filters non-identifier tags.
    let src2 = r#":+(1, 2)"#;
    let hits2 = scan_fmpl_source(src2, fmpl_source("test.fmpl")).expect("lex ok");
    assert!(hits2.is_empty(), "expected 0 hits, got {:?}", hits2);
}

// --- scan_rust_strings (test-only helper) ---

#[test]
fn scan_rust_strings_finds_hit_in_string_literal_only() {
    // Rust source: a string literal containing `:Foo(1, 2)` (one real hit) AND
    // a Rust qualified path `Pattern::Constructor("Foo", vec![])` (NOT a hit —
    // qualified paths are syntax-tree-path nodes, not string literals).
    let rust_src = r##"
fn example() {
    let s = "let x = :Foo(1, 2)";
    let _ = s;
}
"##;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert_eq!(hits[0].tag.as_str(), "Foo");
}

#[test]
fn scan_rust_strings_ignores_rust_qualified_paths() {
    // No string literal contains `:Foo(`. The `MyPattern::Constructor(` is a
    // Rust qualified path — never reaches scan_fmpl_source. Uses a synthetic
    // enum name to avoid referencing the deleted ast::Pattern::Constructor.
    let rust_src = r##"
enum MyPattern { Constructor(String, Vec<i32>) }
fn example() {
    let _p = MyPattern::Constructor("Foo".to_string(), vec![]);
}
"##;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_rust_strings_raw_string_with_list_pattern_has_zero_hits() {
    let rust_src = r####"
fn example() {
    let _r = r#"[:Foo, 1, 2]"#;
}
"####;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert!(hits.is_empty(), "expected 0 hits, got {:?}", hits);
}

#[test]
fn scan_rust_strings_swallows_lexer_errors_silently() {
    // String literal containing a BELL char — FMPL lexer fails. Per the
    // documented swallow policy (roadmap line 349), scan_rust_strings returns
    // Ok with the hits it COULD find from other literals (here: none).
    let rust_src = "fn example() { let s = \"bell \\x07 char\"; let _ = s; }";
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert!(
        hits.is_empty(),
        "expected 0 hits (swallow policy), got {:?}",
        hits
    );
}

// --- doc-attribute discrimination (ITER-0004d.3 T4) ---
//
// `///` and `//!` Rust doc comments desugar to `#[doc = "..."]` attributes
// during parsing. The scanner must distinguish doc-attr-origin `LitStr`s
// from regular string literals so the gate can suppress doc hits without
// silencing genuine legacy syntax in code.

#[test]
fn scan_doc_attr_outer_litstr_is_marked_from_doc_attr() {
    // `///` doc comment in front of an item — desugars to
    // `#[doc = "..."]`. The string contains `:Foo(1, 2)` which scans as
    // one hit; that hit's `SourceKind::RustString.from_doc_attr` must be
    // `true`.
    let rust_src = r#"
/// Convert :Foo(1, 2) to bar.
pub fn x() {}
"#;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert!(
        hits[0].source.from_doc_attr(),
        "doc-attr LitStr should be flagged: {:?}",
        hits[0]
    );
}

#[test]
fn scan_doc_attr_inner_litstr_is_marked_from_doc_attr() {
    // `//!` inner doc comment — desugars to `#![doc = "..."]`. Same
    // discrimination requirement as outer doc comments.
    let rust_src = r#"
//! Module doc with :Bar(42) example.

pub fn x() {}
"#;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert!(
        hits[0].source.from_doc_attr(),
        "inner doc-attr LitStr should be flagged: {:?}",
        hits[0]
    );
    assert_eq!(hits[0].tag.as_str(), "Bar");
}

#[test]
fn scan_regular_litstr_is_not_marked_from_doc_attr() {
    // A plain string literal in code — `from_doc_attr` must be `false`.
    let rust_src = r#"
pub fn x() {
    let s = ":Foo(1, 2)";
    let _ = s;
}
"#;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert_eq!(hits.len(), 1, "expected 1 hit, got {:?}", hits);
    assert!(
        !hits[0].source.from_doc_attr(),
        "regular LitStr must not be flagged as doc-attr: {:?}",
        hits[0]
    );
}

#[test]
fn scan_mixed_doc_and_regular_litstrs_discriminates_correctly() {
    // Both kinds in one file. Ensure flag flips per-hit rather than
    // leaking across the visitor traversal.
    let rust_src = r#"
/// Doc with :Foo(1) inside.
pub fn x() {
    let s = ":Bar(2)";
    let _ = s;
}
"#;
    let hits =
        common::rust_string_scanner::scan_rust_strings(rust_src, &PathBuf::from("example.rs"))
            .expect("syn parses example.rs");
    assert_eq!(hits.len(), 2, "expected 2 hits, got {:?}", hits);
    // Order: doc attr is visited before the function body in syn's
    // default traversal. The doc hit should be flagged, the body hit not.
    let doc_hit = hits.iter().find(|h| h.tag.as_str() == "Foo").expect("Foo");
    let body_hit = hits.iter().find(|h| h.tag.as_str() == "Bar").expect("Bar");
    assert!(
        doc_hit.source.from_doc_attr(),
        "Foo hit (in doc attr) must be flagged: {:?}",
        doc_hit
    );
    assert!(
        !body_hit.source.from_doc_attr(),
        "Bar hit (in code literal) must NOT be flagged: {:?}",
        body_hit
    );
}
