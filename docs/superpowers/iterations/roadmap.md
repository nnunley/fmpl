# Roadmap

## Walking skeleton (ITER-0000)

**Intent:** Establish the bootstrap parity test harness and sentinel corpus, verifying the currently-passing subset of the FMPL compilation pipeline (source → ast::parse → ast_to_ir.fmpl → ir::compile → code::eval) produces correct results for basic expressions.

**Design rationale:** Phase 1 (parser cutover) is already complete. ITER-0000 formalizes the test harness, confirms the passing tests as the sentinel corpus baseline, and establishes the regression gate for all subsequent iterations.

**Journey scenario:** SCENARIO-0016 (parity contract)

**Stories committed:**
- STORY-0007 (EPIC-002)

**Status:** done

---

## Iteration list

### Completed iterations

### ITER-0001 — Parity: Core Expression Coverage

**Stories:** STORY-0043, STORY-0044, STORY-0045, STORY-0046, STORY-0047, STORY-0048
**Status:** done
**Result:** 36/55 parity tests passing. IR compilation layer fully verified.

### ITER-0002 — Parity: Control Flow and Bindings

**Stories:** STORY-0006, STORY-0008, STORY-0049a
**Status:** done
**Result:** Fixed grammar engine binding scoping bug (sub-runtime rule_depth). Unblocked arithmetic, string, let, if, sequence parity.

### ITER-0003 — Parity: Advanced Language Features + ir::compile Gaps

**Stories:** STORY-0049b, STORY-0009
**Status:** done
**Result:** Fixed Star-in-TagMatch list unwrapping, `args` keyword collision, map pair binding, vacuous all() check, added ir::compile handlers for While/For/Block/Pipe/Slice/Match/TryCatch, added tagged value introspection, added InlinePatternBlock→Match in ast::parse. **55/55 parity tests passing, 0 ignored.**

---

## Remaining iterations (critical path to self-hosting)

### ITER-0004 — Optimizer Integration and Compiler Retirement

**Stories:** STORY-0010, STORY-0012, STORY-0011, STORY-0005
**Rationale:** Integrate ast_optimizer.fmpl (constant folding, algebraic simplification) into the bootstrap compilation pipeline between ast_to_ir.fmpl and ir::compile. Verify parity is preserved with optimization enabled. Then retire the Rust compiler from the main compilation path — the FMPL pipeline becomes the default, with the Rust compiler retained only in fmpl-bootstrap as the stage 0 fallback. This is the **compiler cutover milestone**.
**Status:** done (compiler cutover wired; optimizer integration deferred — ast_optimizer.fmpl uses list-based patterns and needs the AST refactor first)
**Impacted scenarios:** SCENARIO-0003, SCENARIO-0016
**Depends on:** ITER-0003 (complete)
**Look-ahead check:** Completes Phase 2. Unblocks self-compile (ITER-0006). Persistence (ITER-0005) must follow ITER-0004b for representation stability.

**Delivered:**
- `eval_via_fmpl_pipeline()` — runs source through ast::parse → ast_to_ir.fmpl → ir::compile → code::eval
- `eval_via_rust_compiler()` — explicit Rust-compiler path (formerly the only `eval`)
- `FMPL_USE_FMPL_COMPILER=1` opt-in flag for FMPL pipeline as default
- 11 E2E tests (`fmpl_pipeline_compiler.rs`) verify identical results: integer, arithmetic with precedence, string, let, if, lambda, list, nested arith, boolean logic, comparison, plus bootstrap caching
- Rust compiler is now an explicit fallback, not the only path

**Deferred to ITER-0004b:**
- STORY-0010 (ast_optimizer.fmpl integration — needs list-based AST refactor; STORY-0012 consolidated into STORY-0010 as duplicate)
- Removing `FMPL_USE_FMPL_COMPILER` opt-in and making FMPL pipeline the default everywhere

### ITER-0004b — Single Canonical Representation (Lists Everywhere + Burn the Bridge)

**Stories:** STORY-0010 (consolidated: STORY-0012 absorbed as duplicate scope; STORY-0010c absorbed because the cutover and the cleanup are one refactor)
**Rationale:** Today FMPL has two interchangeable shapes for tagged/structured data: `Value::Tagged(tag, children)` and `Value::List([Symbol(tag), ...children])`, plus two parser surfaces: `:Tag(args)` and `[:Tag, args]`. This iteration collapses both axes to a single canonical representation: list-shaped values, list-shaped patterns. After this iteration there is exactly one way to represent and pattern-match structured data, no parallel codepaths, no parser ambiguity, no runtime ambiguity. **This iteration MUST land before ITER-0005** — see "Why before persistence" below.

The cutover (make `ast::parse` emit lists; FMPL pipeline consume lists; integrate the optimizer) and the cleanup (delete `Value::Tagged`, `Expr::Tagged`, `Pattern::Constructor`, the `:Tag(args)` parser productions, tagged bytecode, `Pattern::TagMatch`) are one refactor. Splitting them was attempted in the 2026-05-08 session and produced a worse interim state (parallel representations, dual codepaths, more code to maintain) than either before or after the full cleanup. They land together or not at all.

**Status:** partially shipped 2026-05-08 — Rust runtime canonicalized; FMPL stdlib + AST/parser surfaces deferred to ITER-0004c. See iteration-log.md ("ITER-0004b — Single Canonical Representation (partial)") for details.
**Impacted scenarios:** SCENARIO-0003, SCENARIO-0016, SCENARIO-0039 (touched, ongoing); SCENARIO-0103 (NEW — blocked, optimizer not yet wired)
**Depends on:** ITER-0004 (compiler cutover wired)
**Look-ahead check (revised):** **Partial — does NOT yet fully lock in a single representation.** `Value::Tagged` is gone, but `Expr::Tagged`, `Pattern::Constructor`, the `:Tag(args)` parser production, and `Pattern::TagMatch` are still present (the parser silently translates `:Tag(args)` to list-shaped values at compile time via the surviving AST nodes). 5 stdlib files (`ast_optimizer.fmpl`, `fmpl_parser.fmpl`, `ir_to_rust.fmpl`, `prelude.fmpl`, `ir_to_execution_tape.fmpl`) still hold legacy `:Tag(args)` syntax. ITER-0005 (persistence) is technically unblocked because snapshots will only see `Value::List` — the runtime variant is the one that lives in serialized bytes — but the AST and parser ambiguity remains until ITER-0004c lands. ITER-0006 (self-compile seed) is **blocked** because the FMPL transformer was never built, so the stdlib can't be regenerated mechanically from source.

**What actually shipped (Rust side only):**
- Phase A items 1–2 and 5: `Value::list_node` + `Value::as_node` helpers, ast-grep rule files at `tools/list-transform/rust-rules/`, hand-tested.
- Phase B items 6, 9: Ran ast-grep over fmpl-core (229 mechanical rewrites). Updated `expr_to_value` and `ir::compile_node` for list-only dispatch (commit `qworqxrm`).
- Phase B item 7 partial: `lib/core/ast_to_ir.fmpl` was rewritten **by hand** (no FMPL transformer was ever built — items 3, 4, 7 in the original plan never executed).
- Phase C item 13: `Value::Tagged` enum variant deleted. The Rust type-system burn is complete.

**What was deferred (now ITER-0004c):**
- Phase A item 3 (FMPL transformer build) — never started.
- Phase B item 7 (FMPL transformer applied to all stdlib files) — only `ast_to_ir.fmpl` got rewritten, by hand. 5 files still in legacy syntax.
- Phase B item 10 (optimizer wired into `eval_via_fmpl_pipeline`) — `ast_optimizer.fmpl` is still in legacy syntax and not called by any pipeline. The 16 `#[ignore]`'d tests in `optimizer_integration.rs` remain ignored.
- Phase B item 12 (SCENARIO-0103) — added but blocked.
- Phase C items 14–18 (delete `Expr::Tagged`, `Pattern::Constructor`, tagged bytecode, `Pattern::TagMatch`, the `:Tag(args)` parser production) — not started. The Rust type system permits both shapes; the parser still accepts both syntaxes; the runtime value layer is the only place that's truly canonicalized.

**Scope:**

**Strategy:** Transformer-driven rewrite, not hand-edit-driven. The 2026-05-08 attempt confirmed that hand-editing ~349 sites mid-session burns context faster than it converges. The plan below uses two structural code transformers that do the bulk of the work mechanically, leaving only the irreducibly novel work (helper additions, deletions, optimizer integration) for hand-editing.

- **Rust side:** [ast-grep](https://ast-grep.github.io/) (already installed at `~/.cargo/bin/ast-grep`). Pattern-based structural rewrite using YAML rule files. Idempotent — re-running on its output yields no diff.
- **FMPL side:** A small FMPL-in-FMPL transformer (a tree grammar) that rewrites `:Tag(args)` → `[:Tag, args]` for both expressions and patterns. Lives at `tools/list-transform/list_transform.fmpl`. Built on FMPL's own parser; dogfoods the language. Also idempotent.

Both transformers are rules + driver, not from-scratch tools. Total transformer code is well under a few hundred lines; the win is having the rewrite be mechanical and re-runnable, not having a fancy tool.

---

**Phase A — Build and validate the transformers:**

1. **Add helpers on `Value`.** `Value::list_node(tag, children) -> Value` constructor producing `Value::List([Symbol(tag), ...children])`. `Value::as_node(&self) -> Option<(&str, &[Value])>` accessor that destructures it. Both transformer outputs depend on these existing first.

2. **Write ast-grep rule files at `tools/list-transform/rust-rules/`.** One YAML file per pattern:

   - `producer-with-args.yml` — matches `Value::Tagged(SmolStr::new($TAG), Arc::new(vec![$$$ARGS]))` → rewrites to `Value::list_node($TAG, vec![$$$ARGS])`. Verified working in the 2026-05-08 session.
   - `producer-empty.yml` — matches `Value::Tagged(SmolStr::new($TAG), Arc::new(Vec::new()))` → rewrites to `Value::list_node($TAG, vec![])`.
   - `producer-non-literal-vec.yml` — matches `Value::Tagged(SmolStr::new($TAG), Arc::new($EXPR))` (where `$EXPR` is not `vec![...]` or `Vec::new()`) → rewrites to `Value::list_node($TAG, $EXPR.to_vec())` or similar. Edge cases captured in `manual-review.md` for human review.
   - `consumer-iflet.yml` — matches `if let Value::Tagged($TAG, $CHILDREN) = $V { $$$BODY }` → rewrites to `if let Some(($TAG, $CHILDREN)) = $V.as_node() { $$$BODY }` (with appropriate `&str`/`&[Value]` reference adjustment).
   - `consumer-match-arm-guard.yml` — matches `match $V { Value::Tagged($T, $C) if $T.as_str() == $TAG => $BODY, $$$REST }` → rewrites to use `$V.as_node()` then if-let-chain on the literal tag.
   - `consumer-match-arm-bind.yml` — matches `Value::Tagged($T, $C) => $BODY` arms in matches → rewrites to use `as_node()`.
   - `display-tagged-string.yml` — matches `format!("{:?}", Value::Tagged(...))` and similar formatter assumptions → flagged in `manual-review.md` because Display output changes.

   Run with `ast-grep scan --rule tools/list-transform/rust-rules/*.yml --update-all` repeatedly until idempotent (no further diffs).

3. **Write the FMPL transformer at `tools/list-transform/list_transform.fmpl`.** A tree grammar that rewrites FMPL ASTs. Two rules:

   ```
   let list_transform = grammar list_transform {
       expr = :Tagged(any:tag, expr*:args) => [tag, args]
            | :Pattern(:Constructor(any:tag, pattern*:pats)) => [:Pattern, [:List, [tag, pats]]]
            | any:other => other  -- recurse via descend rule

       pattern = :Constructor(any:tag, pattern*:pats) => [:List, [tag, pats]]
               | any:other => other
   }
   ```

   Driver: a CLI (`tools/list-transform/transform.rs`, ~50 lines) that walks `lib/**/*.fmpl`, parses each file, applies the transformer, pretty-prints back. Comment-preserving by working at AST-trivia level rather than reformatting.

   **Special-case rules** the transformer applies after the basic rewrite:
   - **Trailing comma for single-element list patterns.** `[expr*:xs] => xs` → `[expr*:xs,] => xs`. Required to disambiguate from char classes in the grammar parser.
   - **Pair sentinel wrap.** `[_:k, expr:v] => [k_ir, v_ir]` (where both children of the result are list-shaped) → `[_:k, expr:v] => [:Pair, k_ir, v_ir]`. Required to prevent the runtime "list-of-lists ⇒ spread" collapse.
   - **List-pattern binding repair.** Bare identifiers in tag-child position become bindings; in list-pattern position they're rule references. Where the input was `:Tag(name)` (binding `name`), the output `[:Tag, name]` would be a rule reference. Rewrite to `[:Tag, any:name]`.

   The special-case rules can be expressed as additional grammar rules in `list_transform.fmpl` (recommended) or as post-processing passes in the driver.

4. **Validate dry-runs.** Both transformers run in `--check` mode and produce:
   - Diff stats: files changed, sites rewritten per rule
   - `tools/list-transform/manual-review.md` listing sites that need human attention
   - Idempotency confirmation: a second run produces zero diffs

5. **Hand-test the transformers** on a small subset (`fmpl-core/src/builtins/ir.rs` for ast-grep; `lib/core/ast_to_ir.fmpl` for the FMPL transformer). Verify the output compiles and tests pass for those files.

---

**Phase B — Apply the transformers; integrate the optimizer:**

6. **Run the Rust transformer for real.** `ast-grep scan --rule tools/list-transform/rust-rules/*.yml --update-all` over the workspace. Expected: ~349 mechanical rewrites land in one pass. Cargo build still works because `Value::list_node` and `Value::as_node` work alongside the still-defined `Value::Tagged` variant.

7. **Run the FMPL transformer for real.** Rewrites `lib/**/*.fmpl` and any inline FMPL string literals in Rust tests (the FMPL transformer's driver scans for FMPL string literals via tree-sitter or a simpler regex). Output: list-pattern syntax everywhere.

8. **Hand-edit the manual-review sites.** The transformer's `manual-review.md` lists sites it couldn't safely rewrite — typically: complex nested patterns, comments referencing Tagged, Display assertions. Walk through these.

9. **Update `expr_to_value` and `ir::compile_node` for list-only dispatch.** The transformer already converted producers and most consumers; this step removes the now-dead Tagged code paths in these two specific files. Use the `ast_node!` and `ast_match!` macros from `fmpl-core/src/macros.rs`.

10. **Wire the optimizer into `eval_via_fmpl_pipeline`** at the correct slot: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval`. The optimizer (`lib/core/ast_optimizer.fmpl`) is already in list-pattern form post-transformer; this step adds the call site.

11. **Verify Phase B is green.** Full `cargo test --workspace` passes. The 55 ast_to_ir parity tests pass with optimizer enabled. Tree is in a stable state (lists everywhere, but `Value::Tagged` variant still defined and unused). **This is a natural pause point** — if a session ends here, Phase C is a follow-on, not a redo.

12. **Add SCENARIO-0103: full parity corpus passes with optimizer enabled.**

---

**Phase C — Burn the bridge (delete the dual representation):**

After Phase B the transformer has eliminated almost all `Value::Tagged` references. Now delete the variants, AST nodes, parser productions, and bytecode. Each deletion surfaces a small number of remaining sites the transformer missed; fix those by hand.

13. **Delete `Value::Tagged` enum variant** in `fmpl-core/src/value.rs`, plus its `Display`, `equals`, `index`, `is_truthy`, `type_name`, and unit-test arms. Cargo errors will surface any sites the transformer missed (rare; should be near zero after a clean transformer pass).

14. **Delete `Expr::Tagged`** AST variant. Drop the parser production for `:Tag(args)` value-constructor syntax. Update `compile_expr` and `expr_to_value` to remove the `Expr::Tagged` arms.

15. **Delete `Pattern::Constructor`** AST variant. Drop the parser production for `:Tag(p1, p2)` pattern syntax. Update `compile_match_bindings` to remove the `Pattern::Constructor` arms.

16. **Delete tagged bytecode**: `Instruction::MakeTagged`, `MatchTag`, `ExtractTaggedChild`, `MatchTagged`, `MatchTaggedWithBindings`. The compiler currently emits `MatchTag` for `Pattern::Symbol` and `Pattern::Constructor` — switch the `Pattern::Symbol` case to use list-head dispatch, or rename `MatchTag` to `MatchListNode`.

17. **Delete `Pattern::TagMatch`** from `fmpl-core/src/pattern/mod.rs`. Delete its handlers in `fmpl-core/src/grammar/runtime.rs:784` and `fmpl-core/src/grammar/trampoline.rs:999`. `Pattern::ListMatch` already covers the shape.

18. **Delete grammar parser's `:Tag(args)` pattern production** in `fmpl-core/src/grammar/parser.rs::parse_value_pattern`.

19. **Document optimizer coverage gap** (TODO in `ast_optimizer.fmpl`): Lambda bodies, Let, Match, Call, List, Map, Block fall through unchanged — constants inside them don't fold. Tracked for a future iteration.

20. **Final verification.** Full `cargo test --workspace` passes, zero `Value::Tagged` references in source (`grep -r "Value::Tagged" .` returns no source matches; only doc references in `docs/` remain).

**Explicitly OUT OF SCOPE:**

- **Removing `FMPL_USE_FMPL_COMPILER` opt-in flag.** Default `eval()` still uses `eval_via_native`. Promotion to default is a separate iteration.

**Implementation discipline:**

- **Phases A and B can land independently.** Phase A (transformers) is a small, reviewable, self-contained tool. Phase B (apply transformers + optimizer integration) lands on top and produces a coherent state (lists everywhere; Tagged variant still defined but unused). If a session ends after Phase B, Phase C is a clean follow-on, not a redo. **This is the key benefit of the transformer approach** — it converts a "single huge atomic refactor" into "two reviewable artifacts" without producing an incoherent interim state.
- **Phase C should still be atomic.** Deleting `Value::Tagged` and the parser productions is a single coordinated change.
- **Don't try to keep tests green during Phase C deletions.** Get the build green first (drive cargo error count to zero), then run tests.
- **Tooling first.** Build the transformers fully before running them in anger. A dry-run with manual review of the diff is the cheap insurance.

**Why before persistence:**
ITER-0005 will serialize `ObjectDb`, `CompiledCode`, `GrammarRegistry`, and the full VM image — all of which transitively contain `Value`. With `Value::Tagged` still present, snapshots taken now would be locked to a shape we want to abandon. Landing the canonical-representation refactor first means ITER-0005 persists `Value::List`, the only shape going forward.

### ITER-0004c — FMPL Stdlib + Parser Burn (deferred from ITER-0004b)

**Stories:** STORY-0010 (continued — ast_optimizer wiring is the canonical acceptance gate)
**Rationale:** ITER-0004b shipped only the Rust-runtime half of the canonical-representation refactor. Five stdlib files still use legacy `:Tag(args)` syntax, and the AST/parser surfaces (`Expr::Tagged`, `Pattern::Constructor`, the `:Tag(args)` parser production, `Pattern::TagMatch`) are still present. This iteration completes the work: build the FMPL transformer that ITER-0004b's plan called for, apply it to the remaining stdlib files, wire the optimizer into `eval_via_fmpl_pipeline`, then delete the AST/parser surfaces. After this iteration there is genuinely one shape and one syntax.
**Status:** pending
**Impacted scenarios:** SCENARIO-0103 (the SCENARIO-0103 sentinel created in ITER-0004b can finally be unblocked here), plus reconfirms SCENARIO-0003/0016/0039 with optimizer enabled.
**Depends on:** ITER-0004b (the Rust-runtime half).
**Look-ahead check:** Closes the `Value::Tagged` look-ahead obligation. ITER-0006 (self-compile seed) is unblocked because the stdlib can be regenerated mechanically from source via the transformer, and the seed references exactly one AST shape with no parser ambiguity.

**Files still on legacy syntax (post-ITER-0004b, verified 2026-05-09):**
- `lib/core/ast_optimizer.fmpl` (62 legacy hits, 0 list-pattern) — also not yet wired into pipeline
- `lib/core/fmpl_parser.fmpl` (96 legacy hits)
- `lib/core/ir_to_rust.fmpl` (48 legacy hits)
- `lib/core/prelude.fmpl` (41 legacy hits)
- `lib/core/ir_to_execution_tape.fmpl` (19 legacy hits)
- `lib/core/pipeline_demo.fmpl` (2 legacy hits — mostly already migrated)

**Scope (picks up ITER-0004b's deferred items):**

1. **Build the FMPL transformer** (`tools/list-transform/list_transform.fmpl` + driver). Was Phase A item 3 in ITER-0004b's plan. Tree-grammar rules + special-case rules (trailing comma, pair sentinel wrap, list-pattern binding repair) per the original spec.
2. **Validate dry-runs** on a small subset (`prelude.fmpl` is the cheapest target — most `:Tag(args)` are simple constructor calls).
3. **Apply the FMPL transformer** to all 6 files. Hand-edit transformer-flagged exceptions.
4. **Wire `ast_optimizer.fmpl`** into `eval_via_fmpl_pipeline` at the slot the original plan specified: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval`.
5. **Delete the AST/parser surfaces** that ITER-0004b's Phase C left behind: `Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch`, `Instruction::MakeTagged`/`MatchTag`/`ExtractTaggedChild`/`MatchTagged`/`MatchTaggedWithBindings` (or rename `MatchTag` to `MatchListNode`), the grammar parser's `:Tag(args)` pattern production at `fmpl-core/src/grammar/parser.rs::parse_value_pattern`, and the parser production for `:Tag(args)` value-constructor expressions.
6. **Verification:** the 16 `#[ignore]`'d tests in `fmpl-core/tests/optimizer_integration.rs` un-ignored and passing; `grep -r ':[A-Z][a-zA-Z]*(' lib/core/` returns no matches (only list-pattern syntax in stdlib); `grep -r 'Expr::Tagged\|Pattern::Constructor\|Pattern::TagMatch' fmpl-core/src/` returns no matches.

**Out of scope:** Removing `FMPL_USE_FMPL_COMPILER` opt-in (still deferred — promotion to default is its own iteration once the FMPL pipeline path has soak time).

### ITER-0005 — Image Persistence (Consolidated)

**Stories:** STORY-0099, STORY-0100, STORY-0013, STORY-0014, STORY-0015, STORY-0019, STORY-0021, STORY-0069, STORY-0016, STORY-0017, STORY-0018, STORY-0020
**Rationale:** Consolidated from old ITER-0007/0008/0009. Persist all compiler state to Fjall in one iteration: ObjectDb (objects already derive Serialize/Deserialize), compiled bytecode (rkyv support exists), grammar definitions and memo tables (hardest — semantic actions contain AST expressions), and full VM snapshot/restore. Enable fjall-persistence feature flag. Verify full image survives process restart.
**Status:** pending
**Impacted scenarios:** SCENARIO-0007, SCENARIO-0008, SCENARIO-0009, SCENARIO-0010, SCENARIO-0011, SCENARIO-0099, SCENARIO-0100, SCENARIO-0101, SCENARIO-0102
**Depends on:** ITER-0004b (single canonical representation — see ITER-0004b "Why before persistence")
**Look-ahead check:** Unblocks self-compile seed creation (ITER-0006).

**Build order within iteration (STORY-0099 and STORY-0100 are foundational):**
1. **STORY-0099 first** — versioned envelope is the schema all other persistence callers will write through. Land it before any single payload writer is plumbed in, so no caller is ever rewritten away from raw `serde_json::to_vec`.
2. **STORY-0100 second** — content-addressed source store. The envelope (STORY-0099) carries a `source_hash` field; nothing populates it until this story lands. Constructor synthesis for sourceless artifacts (objects, anonymous lambdas, runtime grammars) is the hard part — implement and test in isolation before the per-payload stories depend on it.
3. **STORY-0013/0014/0015/0019/0021** — per-payload writers (objects, bytecode, grammars, memo tables) all built on the STORY-0099 envelope and STORY-0100 source store. None should bypass the envelope.
4. **STORY-0016/0017/0018/0020** — VM snapshot, full-image roundtrip, normal-startup loading. Depend on the per-payload writers above.
5. **STORY-0069** — feature flag wiring; ship last so the default-disabled path is well-defined.

### ITER-0006 — Self-Compile and Seed

**Stories:** STORY-0024, STORY-0025, STORY-0027, STORY-0028
**Rationale:** Create seed snapshot from current Rust compiler (Stage 0). Add --snapshot and --from-seed flags to fmpl-bootstrap. Write fmpl_compiler.fmpl — the FMPL compiler driver that orchestrates the full pipeline (fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile). Verify round-trip: snapshot → restore → compile "1 + 2" → get 3.
**Status:** pending
**Impacted scenarios:** SCENARIO-0020
**Depends on:** ITER-0004 (compiler cutover), ITER-0004b + ITER-0004c (full canonical-representation refactor — both the runtime burn and the FMPL stdlib + parser burn), and ITER-0005 (persistence). The fmpl_compiler.fmpl pipeline `fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile` requires that *every* stdlib file in that chain be in the canonical list-pattern syntax; today, three of the four are still in legacy syntax (only ast_to_ir.fmpl was migrated in ITER-0004b).
**Look-ahead check:** Unblocks fixpoint verification.

### ITER-0007 — Fixpoint Verification

**Stories:** STORY-0022, STORY-0023, STORY-0026
**Rationale:** The capstone. Stage 0: Rust compiler compiles FMPL compiler pipeline into bytecode seed. Stage 1: load seed, feed FMPL compiler source to itself, produce new bytecode. Verify fixpoint: Stage 1 output == Stage 0 output (byte-identical or semantically equivalent). Verify cold bootstrap from seed produces a working compiler. After this, the bootstrap is stable.
**Status:** pending
**Impacted scenarios:** SCENARIO-0021
**Depends on:** ITER-0006
**Look-ahead check:** After this, self-hosting is achieved.

---

## Deferred

### Parser Cutover Completion (was ITER-0012)

**Stories:** STORY-0001, STORY-0002, STORY-0003, STORY-0004, STORY-0038, STORY-0089
**Rationale:** Phase 1 parser cutover is functionally complete and verified by the 900+ test suite. Formal evidence gathering for AST bridge, parse_with_grammar path, and legacy parser retirement is nice-to-have but not on the critical path.
**Status:** deferred

### Grammar/VM Verification (was ITER-0005/0006)

**Stories:** STORY-0050, STORY-0053, STORY-0057, STORY-0066, STORY-0062, STORY-0070, STORY-0071, STORY-0076, STORY-0077, STORY-0082, STORY-0085, STORY-0086
**Rationale:** Absorbed by ITER-0001 through ITER-0003. The 55/55 parity tests provide stronger evidence that the grammar engine and VM work correctly than the planned formalization stories. These were evidence-gathering, not implementation.
**Status:** absorbed

### Grammar Advanced Features (was ITER-0013)

**Stories:** STORY-0051, STORY-0052, STORY-0054, STORY-0055, STORY-0058, STORY-0059, STORY-0060
**Rationale:** Grammar inheritance, anonymous grammars, PEG combinators, backtracking, memoization, trampolining. Not on the bootstrap critical path. Pursue if stability issues arise.
**Status:** deferred

### VM Advanced Features (was ITER-0014)

**Stories:** STORY-0073, STORY-0074, STORY-0075, STORY-0078, STORY-0079, STORY-0080, STORY-0083, STORY-0084, STORY-0087, STORY-0088
**Rationale:** Pipe operator, name resolution, scoping, nested compiled bodies, object properties, async. Not on the bootstrap critical path. Pursue if needed during self-compile.
**Status:** deferred

### Supporting Infrastructure (was ITER-0015)

**Stories:** STORY-0056, STORY-0061, STORY-0063, STORY-0064, STORY-0065, STORY-0067, STORY-0068, STORY-0081, STORY-0090, STORY-0091, STORY-0092, STORY-0093, STORY-0094, STORY-0095, STORY-0096, STORY-0097, STORY-0098
**Status:** pruned

### MLIR Backend (was ITER-FUTURE)

**Stories:** STORY-0029 through STORY-0042
**Rationale:** Post-self-hosting initiative. Not in scope for bootstrap stabilization.
**Status:** deferred
