//! FMPL source diagnostics — scans FMPL source text for legacy
//! `:Tag(args)` tagged-constructor syntax (ITER-0004d.0).
//!
//! The detector piggybacks on the production FMPL lexer
//! (`crate::lexer::Lexer`). A "hit" is defined precisely as a
//! `Token::Symbol(s)` token immediately followed by `Token::LParen`. This
//! covers both uppercase (`:Foo(args)`) and lowercase (`:foo(args)`) forms,
//! both of which become silent `Call(Symbol, args)` reinterpretations after
//! AC-9 lands in ITER-0004d.1.
//!
//! This module is `syn`-free; it works on plain FMPL text. The Rust-source
//! scanner that walks string literals via `syn` is a test-only helper kept
//! under `fmpl-core/tests/common/rust_string_scanner.rs` so the runtime
//! crate stays free of the `syn` dependency.

use std::path::PathBuf;

use smol_str::SmolStr;

use crate::lexer::{Lexer, Token};

/// Origin of a scanned region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    /// A standalone `.fmpl` file.
    FmplFile { path: PathBuf },
    /// A string literal embedded in a Rust source file.
    ///
    /// `rust_byte_offset` is `Some(n)` only when the caller can compute it
    /// (stable Rust does not expose `proc_macro2::Span` byte offsets without
    /// the `span-locations` feature, so the current test helper passes
    /// `None`). Future consumers must treat `None` as "exact location
    /// unavailable" rather than zero.
    ///
    /// `from_doc_attr` is `true` when the `LitStr` originated from a
    /// `#[doc = "..."]` attribute (which is what Rust doc comments —
    /// `///` and `//!` — desugar into). Doc-attr origins describe FMPL
    /// vocabulary in prose, not live FMPL code, so the
    /// `no_legacy_fmpl_syntax` gate filters them out before counting.
    /// `false` indicates a regular string literal in code (e.g.
    /// `let s = ":Foo(1)";`), which still counts as a live legacy hit.
    RustString {
        rust_path: PathBuf,
        rust_byte_offset: Option<usize>,
        from_doc_attr: bool,
    },
}

impl SourceKind {
    /// Convenience accessor: `true` iff this source is a `RustString`
    /// whose `LitStr` came from a `#[doc = "..."]` attribute (i.e. a
    /// Rust doc comment). Returns `false` for `FmplFile` and for regular
    /// `RustString` literals.
    pub fn from_doc_attr(&self) -> bool {
        match self {
            SourceKind::RustString { from_doc_attr, .. } => *from_doc_attr,
            SourceKind::FmplFile { .. } => false,
        }
    }
}

/// One occurrence of legacy `:Tag(args)` syntax in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedSyntaxHit {
    pub source: SourceKind,
    /// Byte offset of the `Symbol` token within the scanned text. For
    /// `RustString` sources this is the offset within the *string literal's
    /// value*, not within the enclosing Rust file.
    pub byte_offset: usize,
    /// The tag (the part of `:Tag` after the colon).
    pub tag: SmolStr,
}

/// Errors raised while scanning source.
#[derive(Debug)]
pub enum DiagnosticsError {
    /// The FMPL lexer failed on the input. Wraps the lexer's message and the
    /// source it came from.
    LexerError { source: SourceKind, message: String },
}

/// Scan a chunk of FMPL source text for legacy `:Tag(args)` syntax.
///
/// Returns the list of hits in source order. A "hit" is an identifier-style
/// `Token::Symbol` (one starting with `[a-zA-Z_]`) immediately followed by
/// `Token::LParen`. Operator-style symbols (`:+`, `:==`, etc.) are excluded
/// — they were never legacy tagged-constructor syntax.
///
/// Errors: propagates lexer failures as `DiagnosticsError::LexerError`. The
/// production lexer already skips comments and string literals, so they do
/// not produce false hits.
pub fn scan_fmpl_source(
    text: &str,
    source: SourceKind,
) -> Result<Vec<TaggedSyntaxHit>, DiagnosticsError> {
    let tokens = Lexer::new(text)
        .tokenize()
        .map_err(|e| DiagnosticsError::LexerError {
            source: source.clone(),
            message: format!("{e}"),
        })?;
    let mut hits = Vec::new();
    for window in tokens.windows(2) {
        if let (Token::Symbol(s), Token::LParen) = (&window[0].token, &window[1].token) {
            if !is_identifier_tag(s) {
                continue;
            }
            hits.push(TaggedSyntaxHit {
                source: source.clone(),
                byte_offset: window[0].span.start,
                tag: s.clone(),
            });
        }
    }
    Ok(hits)
}

fn is_identifier_tag(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}
