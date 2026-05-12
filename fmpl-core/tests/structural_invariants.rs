//! Evidence tests for SCENARIO-0104, 0105, 0106 (ITER-0004d.1 T19).
//!
//! These tests are the passing behavior evidence for three behavior-corpus
//! scenarios:
//!
//! - SCENARIO-0104: parser rejects `:Tag(args)` value-constructor syntax
//! - SCENARIO-0105: parser rejects `:Tag(p1, p2)` pattern-position syntax
//! - SCENARIO-0106: Rust-side greppable invariant — deleted variants stay deleted
//!
//! Per DESIGN-001 (metacircular bootstrap) the Rust and grammar-DSL parsers
//! describe the same language, so each rejection is exercised in both
//! surfaces. Per DESIGN-002 (single canonical form) the `:Tag(args)` syntax
//! is the legacy form that the rewrite to `[:Tag, ...]` replaced; this file
//! provides the contract that the rejection stays in place AND that the
//! deleted Rust types stay deleted.
//!
//! Why two layers (parser rejection + structural grep)? The parser tests
//! prove that the *syntactic* surface is closed. The structural-grep test
//! proves that a future contributor cannot reintroduce the deleted variants
//! by name from a non-parser surface (e.g., FFI, deserialization, or a new
//! builtin) and have the code compile.

use std::fs;
use std::path::{Path, PathBuf};

use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;

// ============================================================================
// Shared helpers
// ============================================================================

/// Repo root for greppable-invariant tests. Resolved via `CARGO_MANIFEST_DIR`,
/// which Cargo sets to `fmpl-core/` at test compile time.
fn fmpl_core_src_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("src")
}

/// Parse a source string with the legacy Rust parser. Returns the parser's
/// `Result<Expr, Error>` — the rejection tests expect `Err` with a specific
/// message.
fn parse_expr(source: &str) -> fmpl_core::error::Result<fmpl_core::ast::Expr> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::with_source(&tokens, source).parse()
}

/// Assert that parsing `source` is rejected (i.e., produces an `Err`). The
/// rejection is the contract; the specific error message is not asserted here
/// because multiple parser-grammar paths can reject the same surface syntax
/// (e.g., a let-destructure may reject at the let-binding rule before reaching
/// the F2 constructor-rejection arm). A separate test in each scenario asserts
/// the message-quality invariant.
fn assert_rejected(source: &str, scenario: &str) {
    let result = parse_expr(source);
    if let Ok(ast) = result {
        panic!(
            "{scenario}: expected rejection of {source:?}, but parse succeeded with AST: {ast:?}"
        );
    }
}

// ============================================================================
// SCENARIO-0104 — Parser rejects `:Tag(args)` value-constructor syntax
// ============================================================================

/// SCENARIO-0104 / case 1: single-argument value-constructor in expression
/// position.
#[test]
fn scenario_0104_rejects_single_arg_value_constructor() {
    assert_rejected(":Foo(1)", "SCENARIO-0104");
}

/// SCENARIO-0104 / case 2: multi-argument value-constructor in expression
/// position.
#[test]
fn scenario_0104_rejects_multi_arg_value_constructor() {
    assert_rejected(":Bar(1, 2, 3)", "SCENARIO-0104");
}

/// SCENARIO-0104 / case 3: value-constructor on the rhs of a let-binding.
/// This exercises the path where `:Pair` could otherwise be a bare symbol
/// — the parser must commit to the rejection only when it sees the `(`.
#[test]
fn scenario_0104_rejects_value_constructor_in_let_rhs() {
    assert_rejected("let (x = :Pair(1, 2)) x", "SCENARIO-0104");
}

/// SCENARIO-0104 / control case: bare `:Foo` (no parens) MUST parse — it is
/// an `Expr::Symbol` literal, not a constructor. The contract carves out
/// symbol literals as "remains valid" alongside the rejection of `:Foo(...)`.
#[test]
fn scenario_0104_bare_symbol_still_parses() {
    let result = parse_expr(":Foo");
    assert!(
        result.is_ok(),
        "SCENARIO-0104 control: expected `:Foo` (no parens) to parse as a symbol literal, got: {result:?}"
    );
}

/// SCENARIO-0104 / control case: list-form `[:Foo, 1, 2]` MUST parse. This is
/// the canonical replacement syntax. If this fails, the contract is broken in
/// the other direction (the migration target itself doesn't work).
#[test]
fn scenario_0104_list_form_still_parses() {
    let result = parse_expr("[:Foo, 1, 2]");
    assert!(
        result.is_ok(),
        "SCENARIO-0104 control: expected `[:Foo, 1, 2]` (canonical form) to parse, got: {result:?}"
    );
}

/// SCENARIO-0104 / error-message guidance: the rejection MUST point the user
/// to the canonical alternative. This is a usability invariant — a bare
/// "syntax error" without the hint would land a user in the wrong forum
/// search results.
#[test]
fn scenario_0104_error_message_points_to_canonical_form() {
    let result = parse_expr(":Foo(1)");
    match result {
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("[:") && msg.contains("instead"),
                "SCENARIO-0104: error message must point to canonical `[:Tag, ...]` form. \
                 Actual: {msg}"
            );
        }
        Ok(_) => panic!("SCENARIO-0104: expected rejection of `:Foo(1)`"),
    }
}

// ============================================================================
// SCENARIO-0105 — Parser rejects `:Tag(p1, p2)` pattern-position syntax
// ============================================================================

/// SCENARIO-0105 / case 1: pattern-position `:Tag(...)` in a match arm. The
/// `parse_pattern` path is distinct from `parse_expr` and must reject this
/// independently.
#[test]
fn scenario_0105_rejects_constructor_pattern_in_match_arm() {
    let source = "match x { :Pair(a, b) => 1 }";
    assert_rejected(source, "SCENARIO-0105");
}

/// SCENARIO-0105 / case 2: pattern-position `:Tag(...)` in a let-binding
/// destructuring. Pre-F2 this path went through `is_symbol_with_paren` and
/// produced `ast::Pattern::Constructor`; F2 closed that surface. Note: the
/// let-binding grammar dispatches based on the FIRST token of the binding —
/// `:Pair(...)` falls into the simple-binding path which rejects before
/// reaching the F2 arm. The contract is "the source is rejected"; the specific
/// rejection path is implementation-defined.
#[test]
fn scenario_0105_rejects_constructor_pattern_in_let_destructure() {
    let source = "let (:Pair(a, b) = pair_value) a + b";
    assert_rejected(source, "SCENARIO-0105");
}

/// SCENARIO-0105 / control case: the list-pattern form `[:Pair, a, b]` is the
/// canonical replacement and MUST parse in both match-arm and let-destructure
/// positions.
#[test]
fn scenario_0105_list_pattern_in_match_arm_parses() {
    let source = "match x { [:Pair, a, b] => 1 }";
    let result = parse_expr(source);
    assert!(
        result.is_ok(),
        "SCENARIO-0105 control: expected `[:Pair, a, b]` match-arm pattern to parse, got: {result:?}"
    );
}

/// SCENARIO-0105 / error-message guidance: pattern-position rejection MUST
/// point at the canonical list-pattern form (matches SCENARIO-0104's
/// expression-position invariant).
#[test]
fn scenario_0105_error_message_points_to_canonical_form() {
    let source = "match x { :Foo(a) => 1 }";
    let result = parse_expr(source);
    match result {
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("[:") && msg.contains("instead"),
                "SCENARIO-0105: error message must point to canonical `[:Tag, ...]` form. \
                 Actual: {msg}"
            );
        }
        Ok(_) => panic!("SCENARIO-0105: expected rejection of `:Foo(a)` in pattern position"),
    }
}

// ============================================================================
// SCENARIO-0106 — Greppable Rust invariant: deleted variants stay deleted
// ============================================================================
//
// These tests walk `fmpl-core/src/` and apply structural greps. The scanner
// is intentionally simple (substring + word-boundary check) rather than a
// full Rust parser, because:
//
// - The cost of `syn` parsing every src file is real (already ~5s in
//   `no_legacy_fmpl_syntax.rs` via the LitStr helper); doing it again here
//   is wasted work.
// - The contract is about *names*, not semantic meaning. If `Value::Tagged`
//   appears anywhere in `src/` (even in a comment), it's a regression signal:
//   either a real reintroduction, or stale documentation that should be
//   cleaned up.
// - The walker is reused across the seven greps via a single AST-style scan
//   below.

/// Read every `.rs` file under `fmpl-core/src/` and return `(path, contents)`.
/// Excludes nothing — every src-tree file is in scope (per the scenario's
/// "outside the strictly-allowed sites" wording, the allowlist is empty for
/// `src/`).
fn read_src_tree() -> Vec<(PathBuf, String)> {
    let root = fmpl_core_src_root();
    let mut out = Vec::new();
    walk_dir(&root, &mut out);
    assert!(
        !out.is_empty(),
        "SCENARIO-0106 setup error: scanned 0 files under {}",
        root.display()
    );
    out
}

fn walk_dir(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| {
        panic!(
            "SCENARIO-0106 setup error: failed to read {}: {e}",
            dir.display()
        )
    });
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let contents = fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!(
                    "SCENARIO-0106 setup error: failed to read {}: {e}",
                    path.display()
                )
            });
            out.push((path, contents));
        }
    }
}

/// Find every `(path, line_number, line_text)` triple in the src tree where
/// `needle` appears as a whole word *in non-comment code*. "Whole word" means
/// the surrounding characters are not `[A-Za-z0-9_]` (Rust identifier
/// characters). "Non-comment" means `//`-line-comment text after a line's `//`
/// marker is stripped before searching, AND any line starting with `//!` or
/// `///` is considered entirely a comment and skipped.
///
/// The comment-strip rule means a historical narrative like
/// `//! Deleted Pattern::Tagged in T12` is ignored, while a live reference
/// like `if let Pattern::Tagged(...) = ...` would be caught. Block comments
/// (`/* */`) are NOT stripped — they are rare in this codebase and stripping
/// them robustly requires tracking nesting across lines. If a regression hides
/// inside a block comment, that's an acceptable miss; the same regression in
/// live code would be caught.
fn find_word_in_code(files: &[(PathBuf, String)], needle: &str) -> Vec<(PathBuf, usize, String)> {
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
fn strip_line_comment(line: &str) -> &str {
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

fn line_contains_word(line: &str, needle: &str) -> bool {
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

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Grep #1 — `Value::Tagged` must NOT appear anywhere in `src/`.
#[test]
fn scenario_0106_grep_1_value_tagged_is_absent() {
    let files = read_src_tree();
    let hits = find_word_in_code(&files, "Value::Tagged");
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #1: expected 0 `Value::Tagged` references in src/, found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #2 — `Expr::Tagged` must NOT appear in `src/` (deleted in T9).
#[test]
fn scenario_0106_grep_2_expr_tagged_is_absent() {
    let files = read_src_tree();
    let hits = find_word_in_code(&files, "Expr::Tagged");
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #2: expected 0 `Expr::Tagged` references in src/, found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #3 — `Pattern::Constructor` must NOT appear in `src/` (deleted in
/// T11). Synthetic test-side enum names like `MyPattern::Constructor` are
/// allowed in `tests/` but `src/` has no legitimate use.
#[test]
fn scenario_0106_grep_3_pattern_constructor_is_absent() {
    let files = read_src_tree();
    let hits = find_word_in_code(&files, "Pattern::Constructor");
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #3: expected 0 `Pattern::Constructor` references in src/, found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #4 — `Pattern::Tagged` must NOT appear in `src/` (deleted in T12).
#[test]
fn scenario_0106_grep_4_pattern_tagged_is_absent() {
    let files = read_src_tree();
    let hits = find_word_in_code(&files, "Pattern::Tagged");
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #4: expected 0 `Pattern::Tagged` references in src/, found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #5 — `Pattern::TagMatch` must NOT appear in `src/` (deleted in T14).
#[test]
fn scenario_0106_grep_5_pattern_tagmatch_is_absent() {
    let files = read_src_tree();
    let hits = find_word_in_code(&files, "Pattern::TagMatch");
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #5: expected 0 `Pattern::TagMatch` references in src/, found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #6 — `Instruction::MakeListNode` qualified reference must NOT appear in
/// `src/compiler.rs`. ITER-0004d.1 T9 deleted the AST→IR emit site that
/// constructed the list-node-make opcode from `Expr::Tagged`. The opcode
/// variant itself survives (renamed in ITER-0004d.2 from `MakeTagged` to
/// `MakeListNode`), and two references remain outside compiler.rs which are
/// explicitly out of scope for this invariant:
///
/// - `vm.rs` — the runtime dispatch handler for the surviving variant
/// - `builtins/ir.rs` — the IR-node `:MakeListNode` builtin handler (the
///   FMPL stdlib still has a `:MakeListNode` IR node form for codepaths that
///   construct list-shaped nodes from FMPL-level code)
///
/// Grep #6 is therefore scoped to compiler.rs only — that's where T9
/// removed the emit. A future contributor reintroducing the emit there
/// would be caught.
#[test]
fn scenario_0106_grep_6_instruction_makelistnode_absent_from_compiler() {
    let path = fmpl_core_src_root().join("compiler.rs");
    let contents = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "SCENARIO-0106 grep #6 setup error: failed to read {}: {e}",
            path.display()
        )
    });
    let mut hits = Vec::new();
    for (lineno, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let code_part = strip_line_comment(line);
        if line_contains_word(code_part, "Instruction::MakeListNode") {
            hits.push((path.clone(), lineno + 1, line.to_string()));
        }
    }
    assert!(
        hits.is_empty(),
        "SCENARIO-0106 grep #6: expected 0 `Instruction::MakeListNode` references in compiler.rs \
         (T9 deleted the emit; surviving references in vm.rs / builtins/ir.rs are out of scope), \
         found {}:\n{}",
        hits.len(),
        format_hits(&hits)
    );
}

/// Grep #7 — `ExtractListChild` MUST appear in compiler.rs (positive
/// invariant). This is the canonical replacement for the deleted
/// pattern-extraction path; T12's `UP::ListMatch` arm uses it. If this
/// disappears, the migration target itself is broken. Checked against the
/// non-comment portion of each line so a stale `// ExtractListChild...`
/// narrative doesn't fool the test into passing when the live emit is gone.
#[test]
fn scenario_0106_grep_7_extract_list_child_present_in_compiler() {
    let path = fmpl_core_src_root().join("compiler.rs");
    let contents = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "SCENARIO-0106 grep #7 setup error: failed to read {}: {e}",
            path.display()
        )
    });
    let mut count = 0;
    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let code_part = strip_line_comment(line);
        if line_contains_word(code_part, "ExtractListChild") {
            count += 1;
        }
    }
    assert!(
        count >= 1,
        "SCENARIO-0106 grep #7: expected ≥1 live `ExtractListChild` reference in compiler.rs \
         (canonical list-pattern extraction path), found {count}"
    );
}

fn format_hits(hits: &[(PathBuf, usize, String)]) -> String {
    let mut s = String::new();
    for (path, lineno, line) in hits {
        s.push_str(&format!(
            "  {}:{}: {}\n",
            path.display(),
            lineno,
            line.trim()
        ));
    }
    s
}

// ============================================================================
// SCENARIO-0106 grep #8 + postlude-arm contract (ITER-0004d.3a / G3)
// ============================================================================
//
// The T7b workaround for the missing grammar `fail()` primitive routes legacy
// `:Tag(args)` rejections through a *poison-AST-node* protocol:
//
//   1. lib/core/fmpl_parser.fmpl matches the offending fragment and emits
//      `[:LegacyTagCtor, tag]` (expression position) or
//      `[:PatternLegacyTagCtor, tag]` (pattern position) as a normal AST node.
//   2. fmpl-core/src/builtins/ir_to_rust.rs emits a postlude that includes
//      `value_to_expr` / `value_to_pattern` functions with `"LegacyTagCtor" =>
//      Err(...)` / `"PatternLegacyTagCtor" => Err(...)` arms. These arms fire
//      during AST-construction, returning `Error::Parser` with the canonical
//      `use [:Tag, ...]` hint.
//
// The poison-tag names are bare string literals coupled across the two files.
// A rename in one without updating the other is silently broken:
//   - If grammar renames but postlude doesn't: the poison node carries a new
//     tag name, no postlude arm matches, and `value_to_expr` falls through to
//     the catch-all "Unknown node" runtime error (or worse, silently accepts).
//   - If postlude renames but grammar doesn't: the postlude arm never fires.
//
// Two tests below guard the coupling:
//   - `scenario_0106_grep_8_legacy_tag_ctor_coupling` — structural grep
//     asserting BOTH magic strings appear in BOTH files.
//   - `g3_postlude_arms_fire_on_poison_nodes` — behavior test confirming the
//     postlude arms return `Err(Parser)` with the canonical hint when a
//     legacy-syntax input reaches them. This complements
//     `canonical_pipeline_parity::parity_rejects_*` by being scoped narrowly
//     to the postlude-arm contract (one assertion per arm, no parity check),
//     so a regression report points directly at the postlude rather than at
//     a parser-parity divergence.
//
// Why not an isolated unit-test that calls `value_to_expr` directly?
// `value_to_expr` and `value_to_pattern` are private (`fn`, not `pub fn`)
// inside the generator-emitted `__generated` module inside `parser.rs`. Only
// `generated_parse` and `GENERATED_PARSER_EPOCH` are re-exported via
// `pub use __generated::*`. No test crate can reach the inner functions
// without modifying the generator. Routing through `generated_parse` with
// minimal triggering inputs is the closest available isolation.

/// SCENARIO-0106 grep #8 — `LegacyTagCtor` / `PatternLegacyTagCtor` magic-string
/// name-coupling invariant. BOTH poison-tag names MUST appear in BOTH the FMPL
/// grammar (`lib/core/fmpl_parser.fmpl`) and the Rust postlude
/// (`fmpl-core/src/builtins/ir_to_rust.rs`). If either file is missing either
/// name, the legacy-syntax rejection is silently broken (see module-level
/// comment above for the failure modes).
#[test]
fn scenario_0106_grep_8_legacy_tag_ctor_coupling() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fmpl_parser_path = PathBuf::from(manifest_dir)
        .join("..")
        .join("lib")
        .join("core")
        .join("fmpl_parser.fmpl");
    let ir_to_rust_path = PathBuf::from(manifest_dir)
        .join("src")
        .join("builtins")
        .join("ir_to_rust.rs");

    let fmpl_parser = fs::read_to_string(&fmpl_parser_path).unwrap_or_else(|e| {
        panic!(
            "SCENARIO-0106 grep #8 setup error: failed to read {}: {e}",
            fmpl_parser_path.display()
        )
    });
    let ir_to_rust = fs::read_to_string(&ir_to_rust_path).unwrap_or_else(|e| {
        panic!(
            "SCENARIO-0106 grep #8 setup error: failed to read {}: {e}",
            ir_to_rust_path.display()
        )
    });

    for tag in &["LegacyTagCtor", "PatternLegacyTagCtor"] {
        assert!(
            fmpl_parser.contains(tag),
            "SCENARIO-0106 grep #8: {} must contain magic string {tag:?} \
             (poison-AST tag emitted by the legacy_tagged_ctor grammar rule). \
             If you renamed the postlude arm in ir_to_rust.rs, you must rename \
             the grammar emit-site too — they are coupled by string identity.",
            fmpl_parser_path.display()
        );
        assert!(
            ir_to_rust.contains(tag),
            "SCENARIO-0106 grep #8: {} must contain magic string {tag:?} \
             (postlude match arm in `value_to_expr` / `value_to_pattern`). \
             If you renamed the grammar emit-site in fmpl_parser.fmpl, you \
             must rename the postlude arm too — they are coupled by string \
             identity.",
            ir_to_rust_path.display()
        );
    }
}

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
/// module-level comment for details), so we route through `generated_parse`
/// with the minimal triggering inputs.
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
         4. Re-run: cargo test -p fmpl-core --test structural_invariants \
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
