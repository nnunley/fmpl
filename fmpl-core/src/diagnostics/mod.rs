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
    RustString {
        rust_path: PathBuf,
        rust_byte_offset: usize,
    },
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
/// Returns the list of hits in source order. A "hit" is a `Token::Symbol`
/// immediately followed by `Token::LParen` in the lexer's token stream.
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
            hits.push(TaggedSyntaxHit {
                source: source.clone(),
                byte_offset: window[0].span.start,
                tag: s.clone(),
            });
        }
    }
    Ok(hits)
}
