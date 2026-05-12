//! Test-only helper: scan a Rust source file's string-literal contents
//! for FMPL legacy `:Tag(args)` syntax (ITER-0004d.0).
//!
//! Walks the Rust syntax tree via `syn::visit::Visit`, extracts every
//! `LitStr`, and runs `fmpl_core::diagnostics::scan_fmpl_source` over its
//! decoded value. Errors from the FMPL lexer on individual literals are
//! swallowed silently — many string literals are shell strings, format
//! strings, doc snippets, etc., that cannot lex as FMPL. The worst case is
//! a missed hit, which will resurface the first time someone refactors the
//! file. False positives are unacceptable; missed hits are recoverable.
//!
//! Doc-attribute (`#[doc = "..."]`) `LitStr` nodes — what `///` and `//!`
//! desugar to — are still scanned, but each resulting hit carries
//! `SourceKind::RustString { from_doc_attr: true, .. }` so the gate can
//! suppress them. Doc-attr strings describe FMPL vocabulary in prose;
//! they are documentation, not live FMPL code.
//!
//! `syn` is a `[dev-dependencies]` entry so the runtime crate stays
//! `syn`-free. This module is only compiled as part of integration tests.

use std::path::{Path, PathBuf};

use fmpl_core::diagnostics::{DiagnosticsError, SourceKind, TaggedSyntaxHit, scan_fmpl_source};
use syn::visit::{self, Visit};

#[derive(Debug)]
pub enum RustScanError {
    /// `syn::parse_file` failed — the Rust source doesn't parse.
    SynParseError { path: PathBuf, error: String },
}

/// Scan a Rust source file's string-literal contents for FMPL legacy
/// `:Tag(args)` syntax. Returns the list of hits in source order.
///
/// Behavior notes:
/// - Only `syn::LitStr` nodes are visited (regular AND raw string literals,
///   since `syn` normalizes both into `LitStr`). `LitByteStr`, `LitChar`, etc.
///   are not scanned — they cannot legally encode FMPL `:Tag(args)` syntax in
///   a way that round-trips through the lexer.
/// - FMPL lexer failures on individual literals are swallowed silently
///   (rationale above). Other literals in the same file continue to be
///   scanned.
pub fn scan_rust_strings(
    rust_src: &str,
    rust_path: &Path,
) -> Result<Vec<TaggedSyntaxHit>, RustScanError> {
    let file = syn::parse_file(rust_src).map_err(|e| RustScanError::SynParseError {
        path: rust_path.to_path_buf(),
        error: format!("{e}"),
    })?;

    let mut visitor = LitStrCollector {
        rust_path: rust_path.to_path_buf(),
        hits: Vec::new(),
        in_doc_attr: false,
    };
    visitor.visit_file(&file);
    Ok(visitor.hits)
}

struct LitStrCollector {
    rust_path: PathBuf,
    hits: Vec<TaggedSyntaxHit>,
    /// State flag: `true` while the visitor is descending into a
    /// `#[doc = "..."]` attribute. `visit_lit_str` consults this to
    /// stamp the resulting `SourceKind::RustString.from_doc_attr`.
    in_doc_attr: bool,
}

impl LitStrCollector {
    fn scan_litstr(&mut self, lit: &syn::LitStr) {
        let value = lit.value();
        // Real byte offsets within the enclosing Rust file are unavailable
        // on stable Rust without `proc-macro2/span-locations` (which leaks
        // into the dependency graph and is overkill for a diagnostics gate).
        // We emit `None` rather than a placeholder so future consumers can
        // distinguish "exact location unavailable" from "offset = 0".
        let source = SourceKind::RustString {
            rust_path: self.rust_path.clone(),
            rust_byte_offset: None,
            from_doc_attr: self.in_doc_attr,
        };

        match scan_fmpl_source(&value, source) {
            Ok(mut hits) => self.hits.append(&mut hits),
            Err(DiagnosticsError::LexerError { .. }) => {
                // Swallow — see module-level rationale.
            }
        }
    }
}

impl<'ast> Visit<'ast> for LitStrCollector {
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        // `#[doc = "..."]` is what `///` and `//!` desugar to. The
        // attribute path is a single segment `doc`. We flag the visitor
        // so any `LitStr` reached during attribute descent is recorded
        // as `from_doc_attr: true`.
        let is_doc = attr.path().is_ident("doc");
        let prev = self.in_doc_attr;
        if is_doc {
            self.in_doc_attr = true;
        }
        visit::visit_attribute(self, attr);
        self.in_doc_attr = prev;
    }

    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        self.scan_litstr(lit);
    }
}
