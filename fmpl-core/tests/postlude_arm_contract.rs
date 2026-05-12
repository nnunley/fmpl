//! Evidence that the FMPL grammar's `legacy_tagged_ctor` poison-AST-node
//! pattern correctly propagates through the postlude `value_to_expr` /
//! `value_to_pattern` arms.
//!
//! This test lives in its own file rather than being migrated to the
//! data-driven scenario runner (ITER-0004d.4) because:
//!
//! 1. It asserts `fmpl_core::parser::IS_GENERATED_PARSER == true` as a
//!    falsifiability precondition (added by ITER-0004d.3a). This is a
//!    precondition check, not a scenario case.
//!
//! 2. It exercises `parser::generated_parse` (the canonical FMPL-generated
//!    parser), not `Parser::with_source` (the source-tree parser). The
//!    existing `parse_rejection` step-def in `tests/steps/parse_rejection.rs`
//!    uses the source-tree parser. Adding a separate `generated_parse_rejection`
//!    step-def is possible but out of scope for ITER-0004d.4; a future
//!    iteration can migrate this test to a card if that step-def exists.
//!
//! 3. The test asserts not just rejection but also that the error message
//!    contains the OFFENDING TAG NAME (`Foo`, `Pair`), proving the tag was
//!    threaded through the poison-AST-node path and not lost in some
//!    generic syntactic fallthrough.
//!
//! Source of the original test: `fmpl-core/tests/structural_invariants.rs`
//! before ITER-0004d.4's migration (T10 deletes the original after T9 lands).

/// G3 postlude-arm contract — the `"LegacyTagCtor"` and `"PatternLegacyTagCtor"`
/// arms in the generated `value_to_expr` / `value_to_pattern` functions MUST
/// fire when a legacy-syntax input reaches them, returning `Err(Parser)` with
/// a message containing the canonical-form hint `use [:`.
///
/// Why this exists alongside `canonical_pipeline_parity::parity_rejects_*`:
/// the parity tests assert that BOTH parsers reject the input, with the focus
/// on cross-parser agreement. This test is scoped to the postlude arms alone
/// — its single failure mode is "the postlude arm did not fire" (renamed,
/// removed, or branch-collapsed). A regression here points directly at the
/// `ir_to_rust.rs` postlude, not at a parity divergence.
///
/// The test cannot reach `value_to_expr` / `value_to_pattern` directly because
/// they are private to the generator-emitted `__generated` module (see
/// `structural_invariants.rs` module-level comment for details), so we route
/// through `generated_parse` with the minimal triggering inputs.
#[test]
#[allow(clippy::assertions_on_constants)]
fn g3_postlude_arms_fire_on_poison_nodes() {
    use fmpl_core::parser::generated_parse;

    // Falsifiability guard (mirrors `canonical_pipeline_must_be_active` in
    // `canonical_pipeline_parity.rs`). Under the fallback parser,
    // `generated_parse` delegates to `Parser::with_source`, which ALSO
    // rejects `:Foo(1)` and `match x { :Pair(a, b) => 1 }` with messages
    // containing `use [:` and the offending tag name — every assertion
    // below would pass trivially without proving the postlude arm fired.
    // Asserting `IS_GENERATED_PARSER` here makes the test unfalsifiable
    // under fallback. The lint is suppressed because the constancy is the
    // point: the value differs between the two parser binaries.
    assert!(
        fmpl_core::parser::IS_GENERATED_PARSER,
        "G3: this test requires the canonical FMPL-generated parser to verify \
         the postlude arms fire. Under the fallback parser, `generated_parse` \
         delegates to `Parser::with_source`, which would make every assertion \
         pass trivially without exercising the postlude. To fix:\n  \
         1. Ensure FMPL_SKIP_PARSER_GEN and FMPL_BOOTSTRAP_PHASE are unset.\n  \
         2. Build fmpl-bootstrap: FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap\n  \
         3. Rebuild fmpl-core: touch fmpl-core/build.rs && cargo build -p fmpl-core\n  \
         4. Re-run: cargo test -p fmpl-core --test postlude_arm_contract \
         g3_postlude_arms_fire_on_poison_nodes"
    );

    // Expression-position poison-node arm: `:Foo(1)` triggers
    // `legacy_tagged_ctor` in the grammar, which emits
    // `[:LegacyTagCtor, "Foo"]`. The postlude arm rejects with the canonical
    // hint.
    let expr_result = generated_parse(":Foo(1)");
    let expr_err = expr_result.expect_err(
        "G3: postlude `LegacyTagCtor` arm must reject `:Foo(1)`. \
         If parse succeeded, the arm was deleted or the grammar emit-site \
         was renamed without updating the postlude.",
    );
    let expr_msg = format!("{expr_err:?}");
    assert!(
        expr_msg.contains("use [:"),
        "G3: postlude `LegacyTagCtor` arm must return an error containing the \
         canonical-form hint `use [:`. Actual error: {expr_msg}"
    );
    // The hint must reference the offending tag name so the user knows what
    // to rewrite.
    assert!(
        expr_msg.contains("Foo"),
        "G3: postlude `LegacyTagCtor` arm error message must reference the \
         tag name `Foo` from the input `:Foo(1)`. Actual error: {expr_msg}"
    );

    // Pattern-position poison-node arm: `:Pair(a, b)` inside a match arm
    // triggers `pat_legacy_tagged_ctor` in the grammar, which emits
    // `[:PatternLegacyTagCtor, "Pair"]`. The postlude arm rejects with the
    // canonical hint.
    let pat_result = generated_parse("match x { :Pair(a, b) => 1 }");
    let pat_err = pat_result.expect_err(
        "G3: postlude `PatternLegacyTagCtor` arm must reject \
         `match x { :Pair(a, b) => 1 }`. If parse succeeded, the arm was \
         deleted or the grammar emit-site was renamed without updating the \
         postlude.",
    );
    let pat_msg = format!("{pat_err:?}");
    assert!(
        pat_msg.contains("use [:"),
        "G3: postlude `PatternLegacyTagCtor` arm must return an error \
         containing the canonical-form hint `use [:`. Actual error: {pat_msg}"
    );
    assert!(
        pat_msg.contains("Pair"),
        "G3: postlude `PatternLegacyTagCtor` arm error message must reference \
         the tag name `Pair` from the input `:Pair(a, b)`. Actual error: \
         {pat_msg}"
    );
}
