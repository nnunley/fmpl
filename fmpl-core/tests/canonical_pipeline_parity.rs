//! Evidence tests for SCENARIO-0108 (ITER-0004d.3 T7a).
//!
//! Asserts that the source-tree Rust parser (`Parser::with_source(...).parse()`)
//! and the canonical FMPL-generated parser (`parser::generated_parse(...)`)
//! agree semantically on representative inputs. This is the proof that the
//! metacircular pipeline (DESIGN-001) produces output equivalent to the
//! source-tree parser — which the sentinel corpus alone does NOT establish
//! because all other sentinels route exclusively through one parser or the
//! other.
//!
//! Two equivalence classes are checked:
//!
//! 1. **Rejection equivalence** — for SCENARIO-0104 / SCENARIO-0105 inputs
//!    (legacy `:Tag(args)` syntax in value and pattern positions), both
//!    parsers must return `Err` and both error messages must contain the
//!    canonical-form hint substring (`use [:`).
//!
//! 2. **AST equivalence** — for representative successful inputs, both
//!    parsers must produce structurally-equal `Expr` trees (under
//!    `PartialEq`). The representative-input set is intentionally small;
//!    `ast_to_ir_parity` already covers depth-of-coverage parity at the
//!    full-pipeline seam.
//!
//! ## Falsifiability contract
//!
//! Every test in this file calls `generated_parse(...)`. When the
//! fallback parser is in use (because `FMPL_SKIP_PARSER_GEN=1`,
//! `FMPL_BOOTSTRAP_PHASE=1`, or `fmpl-bootstrap` is unavailable), that
//! function delegates to `Parser::with_source(...).parse()` — the SAME
//! function `parse_source_tree` calls — making every parity assertion
//! true by definition. To prevent silent fallback substitution from
//! making this suite unfalsifiable, the leading
//! `canonical_pipeline_must_be_active` test asserts on the
//! `IS_GENERATED_PARSER` discriminator emitted by both parser binaries.
//! When the fallback is active that test fails loudly with remediation
//! steps; the remaining seven tests can only run when the canonical
//! FMPL-generated parser is the binary under test.

use fmpl_core::ast::Expr;
use fmpl_core::lexer::Lexer;
use fmpl_core::parser::{Parser, generated_parse};

/// SCENARIO-0108 contract: these tests MUST run against the canonical
/// FMPL-generated parser. The fallback parser delegates `generated_parse`
/// to `Parser::with_source`, which would make every test in this file
/// pass trivially even when the canonical pipeline is absent. The
/// fallback-detection constant IS_GENERATED_PARSER discriminates the
/// two parser binaries; this assertion fires on any test invocation
/// so a future regression cannot silently substitute the fallback.
#[test]
#[allow(clippy::assertions_on_constants)]
fn canonical_pipeline_must_be_active() {
    // The `clippy::assertions_on_constants` lint is suppressed because the
    // constancy is the point: `IS_GENERATED_PARSER` is `true` in the real
    // generated parser and `false` in the fallback. The assertion's value
    // changes between the two parser binaries even though it's compile-time
    // constant within a single build. A `const { assert!(...) }` form would
    // refuse to compile under fallback (catching the issue at build time)
    // but the runtime form gives the user the actionable remediation message.
    assert!(
        fmpl_core::parser::IS_GENERATED_PARSER,
        "SCENARIO-0108 requires the canonical FMPL-generated parser, but \
         the fallback parser is in use. To fix:\n  \
         1. Ensure FMPL_SKIP_PARSER_GEN and FMPL_BOOTSTRAP_PHASE are unset.\n  \
         2. Build fmpl-bootstrap: FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap\n  \
         3. Rebuild fmpl-core: touch fmpl-core/build.rs && cargo build -p fmpl-core\n  \
         4. Re-run: cargo test -p fmpl-core --test canonical_pipeline_parity"
    );
}

/// Parse `source` with the source-tree Rust parser. Returns the same
/// `Result<Expr>` shape `generated_parse` does, for direct comparison.
fn parse_source_tree(source: &str) -> fmpl_core::error::Result<Expr> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::with_source(&tokens, source).parse()
}

// ============================================================================
// Rejection equivalence (SCENARIO-0104 / SCENARIO-0105 inputs)
// ============================================================================

/// Both parsers must reject value-position `:Tag(args)` with the canonical-form
/// hint. Input mirrors SCENARIO-0104 case 0.
#[test]
fn parity_rejects_value_constructor_single_arg() {
    let source = ":Foo(1)";
    let source_tree = parse_source_tree(source);
    let canonical = generated_parse(source);

    assert!(
        source_tree.is_err(),
        "source-tree parser: expected rejection of `{source}`, got: {source_tree:?}"
    );
    assert!(
        canonical.is_err(),
        "canonical (generated) parser: expected rejection of `{source}`, got: {canonical:?}"
    );

    let st_msg = format!("{:?}", source_tree.unwrap_err());
    let cn_msg = format!("{:?}", canonical.unwrap_err());

    assert!(
        st_msg.contains("use [:"),
        "source-tree error must point to canonical form; got: {st_msg}"
    );
    assert!(
        cn_msg.contains("use [:"),
        "canonical error must point to canonical form; got: {cn_msg}"
    );
}

/// Both parsers must reject value-position `:Tag(a, b, c)`. Input mirrors
/// SCENARIO-0104 case 1.
#[test]
fn parity_rejects_value_constructor_multi_arg() {
    let source = ":Bar(1, 2, 3)";
    let source_tree = parse_source_tree(source);
    let canonical = generated_parse(source);

    assert!(
        source_tree.is_err(),
        "source-tree parser: expected rejection of `{source}`, got: {source_tree:?}"
    );
    assert!(
        canonical.is_err(),
        "canonical (generated) parser: expected rejection of `{source}`, got: {canonical:?}"
    );

    let st_msg = format!("{:?}", source_tree.unwrap_err());
    let cn_msg = format!("{:?}", canonical.unwrap_err());

    assert!(
        st_msg.contains("use [:"),
        "source-tree error must point to canonical form; got: {st_msg}"
    );
    assert!(
        cn_msg.contains("use [:"),
        "canonical error must point to canonical form; got: {cn_msg}"
    );
}

/// Both parsers must reject pattern-position `:Tag(p1, p2)` in a match arm.
/// Input mirrors SCENARIO-0105 case 0.
#[test]
fn parity_rejects_pattern_constructor_in_match_arm() {
    let source = "match x { :Pair(a, b) => 1 }";
    let source_tree = parse_source_tree(source);
    let canonical = generated_parse(source);

    assert!(
        source_tree.is_err(),
        "source-tree parser: expected rejection of `{source}`, got: {source_tree:?}"
    );
    assert!(
        canonical.is_err(),
        "canonical (generated) parser: expected rejection of `{source}`, got: {canonical:?}"
    );

    let st_msg = format!("{:?}", source_tree.unwrap_err());
    let cn_msg = format!("{:?}", canonical.unwrap_err());

    assert!(
        st_msg.contains("use [:"),
        "source-tree error must point to canonical form; got: {st_msg}"
    );
    assert!(
        cn_msg.contains("use [:"),
        "canonical error must point to canonical form; got: {cn_msg}"
    );
}

// ============================================================================
// AST equivalence (successful inputs)
// ============================================================================

/// The simplest successful input: a literal integer. If this fails, the two
/// parsers disagree at the most fundamental level. Picked as the canary because
/// every other AST shape builds on `Expr::Int`.
#[test]
fn parity_ast_int_literal() {
    let source = "42";
    let st = parse_source_tree(source).expect("source-tree parse");
    let cn = generated_parse(source).expect("canonical parse");
    assert_eq!(
        st, cn,
        "source-tree and canonical produced different ASTs for `{source}`:\n  source-tree: {st:?}\n  canonical:   {cn:?}"
    );
}

/// Representative arithmetic: precedence + associativity. From `ast_to_ir_parity`
/// baseline. If this passes, the binary-op AST shape is parity-equivalent.
#[test]
fn parity_ast_arithmetic_with_precedence() {
    let source = "1 + 2 * 3";
    let st = parse_source_tree(source).expect("source-tree parse");
    let cn = generated_parse(source).expect("canonical parse");
    assert_eq!(
        st, cn,
        "source-tree and canonical produced different ASTs for `{source}`:\n  source-tree: {st:?}\n  canonical:   {cn:?}"
    );
}

/// Bare `:Symbol` literal — the carve-out from SCENARIO-0104. Should parse
/// successfully on BOTH parsers (control case for the rejection tests above).
#[test]
fn parity_ast_bare_symbol_literal() {
    let source = ":Foo";
    let st = parse_source_tree(source).expect("source-tree parse");
    let cn = generated_parse(source).expect("canonical parse");
    assert_eq!(
        st, cn,
        "source-tree and canonical produced different ASTs for `{source}`:\n  source-tree: {st:?}\n  canonical:   {cn:?}"
    );
}

/// Canonical list-form `[:Foo, 1, 2]` — the replacement syntax. Should parse
/// successfully on BOTH parsers. Establishes parity on the migration target.
#[test]
fn parity_ast_canonical_list_form() {
    let source = "[:Foo, 1, 2]";
    let st = parse_source_tree(source).expect("source-tree parse");
    let cn = generated_parse(source).expect("canonical parse");
    assert_eq!(
        st, cn,
        "source-tree and canonical produced different ASTs for `{source}`:\n  source-tree: {st:?}\n  canonical:   {cn:?}"
    );
}
