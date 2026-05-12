//! Parser-generator epoch / generation number.
//!
//! This constant is the single source of truth for "which version of the
//! parser generator produced the current generated parser." The build script
//! (`fmpl-core/build.rs`) reads this value, compares against the
//! `GENERATED_PARSER_EPOCH` constant embedded in `out/generated_parser.rs`,
//! and forces regeneration on mismatch.
//!
//! The fmpl-bootstrap binary (which links fmpl-core) reads `PARSER_EPOCH` at
//! generator-runtime and emits `pub const GENERATED_PARSER_EPOCH: u32 = N;`
//! into the generated parser source as the first declaration inside the
//! `__generated` module.
//!
//! A compile-time `const _: () = assert!(...)` in `parser.rs` catches any
//! skew that survives build-time regeneration.
//!
//! ## When to bump PARSER_EPOCH
//!
//! Bump the constant in this file (by exactly +1) whenever you make any
//! change that would invalidate the cached `out/generated_parser.rs`. The
//! generator (fmpl-bootstrap) emits Rust source for the parser, so changes
//! to ANYTHING the emitted source depends on require a bump:
//!
//! - **AST surface changes.** Add/remove/rename a variant in
//!   `fmpl-core/src/ast.rs` (`Expr::*`, `ast::Pattern::*`), or change the
//!   shape of a variant's payload.
//! - **Postlude raw-string changes.** Edit the embedded raw-string in
//!   `fmpl-core/src/builtins/ir_to_rust.rs` around line 1141 — that text is
//!   copied verbatim into every generated parser.
//! - **Value-encoding changes.** Change the shape of how the generator
//!   represents AST nodes (e.g., the move from `Value::Tagged` to
//!   `Value::list_node`) since `value_to_expr` depends on the encoding.
//! - **Persisted-instruction changes.** Add/remove an `Instruction` variant
//!   or change its serialized form, since the parser's compiled output is
//!   round-tripped through `Instruction`.
//! - **Helper-function signature changes.** Rename or change the signature
//!   of any FMPL-side helper that grammar-action bodies depend on
//!   (`prepend`, `join`, `symbol`, `reduce`, etc.) — the postlude wires
//!   these into the generated parser.
//!
//! Changes that do NOT require a bump:
//!
//! - Adding tests, fixing documentation, modifying internal helpers that
//!   don't appear in the postlude raw-string.
//! - FMPL stdlib edits that the bootstrap parser already understands.
//! - Renaming non-public Rust functions.
//!
//! When in doubt, bump. The cost of an unnecessary bump is one regeneration;
//! the cost of a missing bump is a confusing `E0599` deep in
//! `out/generated_parser.rs` and lost time.
//!
//! ## Bump history
//!
//! - 1 — initial epoch (ITER-0004d.1, 2026-05-12). Established alongside the
//!   `Expr::Tagged` / `Pattern::Constructor` / `Pattern::TagMatch` deletions
//!   to detect that the generated parser had drifted from the source's
//!   actual AST types.
//! - 2 — ITER-0004d.1 T9+T11 (2026-05-12). Deleted `Expr::Tagged` and
//!   `ast::Pattern::Constructor` variants plus all their consumers / encoders
//!   / decoders. The generated parser must not reference these types.
//! - 3 — ITER-0004d.1 T12+T14 (2026-05-12). Deleted `pattern::Pattern::Tagged`
//!   and `pattern::Pattern::TagMatch` variants. Removed dead grammar-runtime
//!   tagged-value matcher (was reachable only via the deleted
//!   `Pattern::TagMatch` parser productions, removed in F1) plus the now-
//!   orphan `Pattern::contains_repeat` helper.
//! - 4 — ITER-0004d.3a G1 (2026-05-12). Added the
//!   `IS_GENERATED_PARSER: bool` discriminator constant to both the canonical
//!   generator postlude (`ir_to_rust.rs`, set to `true`) and the fallback
//!   parser (`build.rs::write_fallback_parser`, set to `false`). This is the
//!   SCENARIO-0108 fix: it lets `canonical_pipeline_parity::canonical_pipeline_must_be_active`
//!   fire when the fallback parser is silently substituted, preventing every
//!   parity test in that suite from passing trivially. The change is a
//!   postlude raw-string edit, which the bump policy lists as a trigger.
//! - 5 — ITER-0004d.3a G2 (2026-05-12). Added the `PatternLegacyTagCtor` arm
//!   to `value_to_pattern` in the generator postlude (`ir_to_rust.rs`) and
//!   a parallel `pat_legacy_tagged_ctor` rule to `lib/core/fmpl_parser.fmpl`.
//!   This closes the SCENARIO-0108 pattern-position gap: the canonical FMPL
//!   parser now emits the `use [:` canonical-form hint on
//!   `match x { :Pair(a, b) => 1 }` instead of a generic syntactic mismatch
//!   from `pat_symbol` consuming `:Pair` and the trailing `(` being unexpected.
//!   The change is a postlude raw-string edit, which the bump policy lists as
//!   a trigger.

/// Parser-generator epoch. See module-level docs for the bump policy.
pub const PARSER_EPOCH: u32 = 5;
