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

### ITER-0004c — FMPL Stdlib Migration + Optimizer Wiring (Phase A of STORY-0010)

**Stories:** STORY-0010 Phase A (AC-3 through AC-7) plus AC-13 (greppable stdlib invariant — the natural close-out of Phase A's stdlib migration). AC-1, AC-2, AC-8, and AC-15 are already satisfied by ITER-0004b's runtime burn (see verification notes below); they are not re-shipped here. Phase B (AC-9, AC-10, AC-11, AC-12, AC-14) split into ITER-0004d per PAR scope review 2026-05-10. Background EPIC-002.md:154 explicitly identifies Phase B as a "natural pause point"; the line 114 "land together" argument applied when Value::Tagged was still in the runtime, which ITER-0004b already removed.

**Status:** done 2026-05-10. All 5 stdlib files migrated to canonical list-pattern syntax (3 cleanup deletions, then per-file hand-migration of ast_optimizer.fmpl, fmpl_parser.fmpl, ir_to_rust.fmpl, prelude.fmpl, ir_to_execution_tape.fmpl). Optimizer wired into eval_via_fmpl_pipeline. SCENARIO-0103 ships with 4 observables (parity 26 inputs, slot-discriminating, fold-fires-on-real-parse, guards). All 17 optimizer_integration tests un-ignored (INT_MIN test rewritten via direct AST construction; lexer fix scheduled in ITER-0004g). Verification gates: AC-13 CI gate (`stdlib_no_legacy_syntax.rs`), AC-7 runnable check (`ac7_optimizer_pass_through.rs`), ast_optimizer_unit gate (#[ignore]d pending ITER-0004g `++` fix). Workspace tests: 1228 passed (up from 1170 baseline = +58), 183 ignored. See iteration-log.md for full details.

**Already-satisfied AC verification (no re-work needed):**
- AC-1 (`ast::parse` emits list-shaped exclusively): verified at `fmpl-core/src/builtins/ast.rs` — every `expr_to_value` arm returns `Value::list_node(...)`.
- AC-2 (`ir::compile` consumes list-shaped exclusively): verified at `fmpl-core/src/builtins/ir.rs` — `compile_node` dispatches on `Value::as_node()` only.
- AC-8 (`Value::Tagged` enum variant removed): verified — the variant is deleted; `grep -n 'Value::Tagged' fmpl-core/src/value.rs` returns nothing.
- AC-15 (full test suite passes; no `Value::Tagged` source matches): verified at workspace baseline — 1170 passing, no `Value::Tagged` source matches.
**Rationale:** ITER-0004b shipped only the Rust-runtime half of the canonical-representation refactor. The FMPL stdlib files still used legacy `:Tag(args)` syntax. This iteration: (1) hand-migrated 5 stdlib files (transformer attempt abandoned per round-1 PAR + post-implementation spec failure), (2) wired `ast_optimizer.fmpl` into `eval_via_fmpl_pipeline` so the parity corpus actually exercises the optimizer, (3) added SCENARIO-0103, AC-13 CI gate, AC-7 runnable check. Acceptance gate SCENARIO-0103 passes — every parity input matches Rust-compiler output AND folds demonstrably fire AND div-zero/INT_MIN guards prevent unsafe folds. The dual-syntax parser surface (`Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch`, tagged bytecode) survives this iteration unchanged — it's permitted but no longer used by the stdlib. ITER-0004d removes it.

**Impacted scenarios:** SCENARIO-0103 (sentinel — automated `cargo test -p fmpl-core --test scenario_0103_optimizer_pipeline`), SCENARIO-0016 (sentinel — kept optimizer-disabled per round-2 PAR; automated via `cargo test -p fmpl-core --test ast_to_ir_parity`). SCENARIO-0003 and SCENARIO-0039 are ITER-0004d concerns (scenario rewrite + reconfirm).
**Depends on:** ITER-0004b (Rust-runtime burn).
**Look-ahead check:** Unblocks ITER-0005 (persistence) — stdlib representation is now stable. Unblocks ITER-0004d (parser/AST burn). Does NOT yet unblock ITER-0006 (self-compile seed) because the parser still accepts the dual syntax; ITER-0006 needs ITER-0004d's burn to guarantee the seed references exactly one AST shape.

**Files in scope for migration** (verified 2026-05-10 via `grep -cE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl`):
- `lib/core/ast_optimizer.fmpl` (62 legacy lines / 156 occurrences) — also not yet wired into pipeline
- `lib/core/fmpl_parser.fmpl` (96 legacy lines / 101 occurrences)
- `lib/core/ir_to_rust.fmpl` (48 legacy lines / 84 occurrences)
- `lib/core/prelude.fmpl` (41 legacy lines / 45 occurrences) — migrated as-is. Per-PAR-round-2 decision (2026-05-10), the relocation of parser-action helpers (`fold_binary`, `fold_index`, `fold_postfix`, `fold_pipe_at`, `binary_op_to_ir`, `unary_op_to_ir`) into a separate `parser_helpers.fmpl` is deferred to a new iteration (see ITER-0004e — Prelude/Parser-Helper Split, below) to keep ITER-0004c's scope tight on STORY-0010's ACs.
- `lib/core/ir_to_execution_tape.fmpl` (19 legacy lines / 19 occurrences)

**Files deleted in scope item 7** (cleanup, out-of-band of STORY-0010 ACs):
- `lib/core/ast_to_ir_indexed.fmpl` (broken indexed variant; design doc 2026-03-03-self-hosting-bootstrap-design.md:28).
- `lib/core/ir_to_execution_tape_indexed.fmpl` (orphan consumer of the deleted indexed variant).
- `lib/core/pipeline_demo.fmpl` (uncovered demo of the deleted indexed-RPN pipeline).

**Other `lib/core/*.fmpl` files NOT in migration scope but covered by AC-13 CI gate** (verified 2026-05-10 returning 0 legacy hits each): `ast_optimizer_test.fmpl`, `ast_to_ir.fmpl`, `grammar_optimizer.fmpl`, `grammar_optimizer_test.fmpl`, `optimize_grammar.fmpl`, `parser_generator.fmpl`, `test.fmpl`. AC-13's grep applies to ALL files in `lib/core/`; these are listed for completeness so the CI gate scope is unambiguous.

**Build-order dependency (binding, post-pivot 2026-05-10):** Scope items execute in this order: **7 (cleanup) → 1 (ast_optimizer) → 2 (fmpl_parser) → 3 (ir_to_rust + prelude + ir_to_execution_tape + AC-7 TODO) → 4 (lib.rs wiring) → 5 (SCENARIO-0103) → 6 (SCENARIO-0016 binding) → 8 (un-ignore optimizer_integration tests)**. Item 7's deletions go first to reduce the migration set. Items 1–3 then hand-migrate the 5 in-scope files. **Items 1–3 SHOULD precede item 4 (lib.rs wiring)** — note (round-3 PAR correction): functional correctness does NOT strictly require this order. `Pattern::TagMatch` at `fmpl-core/src/grammar/runtime.rs:794` matches list_node shape and `Instruction::MakeTagged` at `fmpl-core/src/vm.rs:877` constructs list_node post-ITER-0004b, so legacy `:Tag(args)` patterns in the un-migrated optimizer would still fold against `Value::list_node` AST input. The migration is needed for AC-13 (greppable invariant) and to keep `ast_optimizer.fmpl` consistent with the canonical syntax. Migration before wiring is preferred for clean bisect (one concern per commit) and AC-13 stability during the wiring window, not for optimizer correctness.

**Design pivot 2026-05-10 (binding):** The originally-planned FMPL transformer was attempted (G1 in the dispatch plan), reviewed by PAR Stage 1, and FAILED spec-compliance: only 3 of 6 transformer rules (d/e/f) could be implemented cleanly; rules (a)(b)(c) for grammar-LHS context required LHS-mode tracking that the FMPL grammar engine cannot express ergonomically without a second pass. Without (a)(b)(c) the transformer would emit semantically broken patterns (binding sites becoming rule references), defeating the migration goal. The `tools/list-transform/list_transform.fmpl` work was abandoned (`jj abandon zwkyzrno` 2026-05-10) per user direction, and ITER-0004c switched to **hand-migration** for the 5 in-scope files. Round-1 PAR review explicitly flagged hand-migration as a viable alternative ("FMPL stdlib has only ~439 occurrences across 7 files — 156+101+84+45+19+10+24 — heavily concentrated in 2 files; hand-migration is feasible"). The build-order is shorter (no transformer build, no dry-run validation), and per-file hand-edits are themselves verified by AC-13 grep + `ast_optimizer_test.fmpl` execution + SCENARIO-0103 + the full workspace test suite.

**Material FMPL-as-a-language verification (per user 2026-05-10):** Even though the transformer is abandoned, this iteration materially demonstrates FMPL works as a programming language through the following gates that exercise non-trivial FMPL programs end-to-end:

1. **`ast_optimizer.fmpl` itself.** This is a non-trivial tree-grammar FMPL program: 88 lines, two grammar blocks (`constant_fold` and `algebraic_simp`), a recursive `optimize` driver with three-iteration fixpoint, and INT_MIN/div-zero guards via `&{...}` predicates. After hand-migration to canonical list-pattern syntax, this file runs as a real optimizer in the bootstrap pipeline (scope item 4) and produces correct folded IR for arbitrary FMPL source input. The fact that this file works end-to-end is itself a material proof of FMPL's expressive power.

2. **`ast_optimizer_test.fmpl` execution gate.** Already in canonical list-pattern syntax (verified 0 legacy hits). This is FMPL test code that exercises the FMPL optimizer's rules. After ITER-0004c migrates `ast_optimizer.fmpl`, this test corpus becomes a fine-grained behavioral net: each fold rule is asserted via FMPL itself, providing a "FMPL writes tests for FMPL" demonstration that does not depend on the Rust runtime for its assertions. Scope item 7 adds this as a verification gate.

3. **SCENARIO-0103 parity sub-observable.** All 55 ast_to_ir parity inputs pass through `ast::parse → ast_optimizer["optimize"] → ast_to_ir.expr → ir::compile → code::eval` and produce results identical to the Rust compiler. The optimizer in the middle is FMPL code; its correctness is verified by the parity match. This proves the FMPL optimizer is at production-quality fidelity against the Rust reference.

4. **`fmpl_parser.fmpl`.** This is the FMPL self-parser (the bootstrap parser written in FMPL). After hand-migration, it continues to parse 100% of the FMPL test corpus (verified by `cargo test --workspace` passing). That 1170-test pass rate post-migration is itself a material demonstration that a non-trivial FMPL grammar program works end-to-end.

These gates are NOT new work — they are the existing iteration gates, framed under the FMPL-self-validation lens. If ANY of them fail post-migration, FMPL-as-a-language is not working at the level this iteration claims; the iteration cannot close.

**Scope:**

1. **Hand-migrate `lib/core/ast_optimizer.fmpl`** — 156 occurrences across 62 lines, the largest file. Workflow: read the file end-to-end first to internalize the structure (88 lines, 3 grammar blocks: `constant_fold`, `algebraic_simp`, plus the top-level `optimize` driver). Then mechanical rewrites:
   - Replace each `:Tag(arg1, arg2, ...)` LHS pattern with `[:Tag, arg1', arg2', ...]` where each `argN'` is `arg` if `arg` is a literal (e.g. `:+`, `:-`, `:Int`, `:Bool`), or `any:argN` if `argN` was a binding identifier (e.g., `:Int(a)` → `[:Int, any:a]`).
   - Replace each `:Tag(arg1, arg2, ...)` RHS expression with `[:Tag, arg1, arg2, ...]` — RHS bare identifiers ARE bindings already (in expression position, not pattern position), so no `any:` wrap is needed.
   - Preserve `&{...}` guard syntax verbatim (it follows the LHS pattern; lexically untouched by the rewrite).
   - Preserve string literals and comments verbatim.
   - **Special-case for trailing-comma:** if any single-element list pattern emerges (e.g., `[:Tag, only_one]`), add a trailing comma `[:Tag, only_one,]` to disambiguate from char-class syntax.
   - **Special-case for `:Pair` sentinel:** `ast_optimizer.fmpl` does NOT have map-pair-emitting rules (verified — no `pair = ...` rule), so the pair sentinel is not relevant for THIS file. (See item 2 for `fmpl_parser.fmpl` if it has them.)
2. **Hand-migrate `lib/core/fmpl_parser.fmpl`** — 101 occurrences across 96 lines. Same workflow as item 1. Honor the **Defer-to-ITER-0004d carve-out** for lines 82-83 and 287-292 (`=> :Tagged(tag, items)`, `=> :PatternTagged(tag, pats)`): migrate them to list shape now (consistent with AC-13 invariant); flag them in the iteration log as "expected churn ITER-0004d will then revisit" because ITER-0004d will delete `Expr::Tagged` entirely. **Cross-iteration coordination:** ITER-0004d scope item 8 references `fmpl_parser.fmpl:82-83, 287-292` by line number; after this iteration's hand-migration those lines hold `[:Tagged, ...]`/`[:PatternTagged, ...]` syntax instead of `:Tagged(...)`/`:PatternTagged(...)` and line numbers may shift. ITER-0004d MUST re-grep at iteration start.
3. **Hand-migrate `lib/core/ir_to_rust.fmpl`, `lib/core/prelude.fmpl`, `lib/core/ir_to_execution_tape.fmpl`** — same workflow. After migration, the comment hand-edits required for AC-13 grep gate (verified 2026-05-10):
   - `lib/core/prelude.fmpl` lines 27, 37, 43–48, 60–63, 72, 89 (~14 comment lines)
   - `lib/core/fmpl_parser.fmpl` line 8 (1 comment line)
   - Re-grep after each file's migration: `grep -nE ':[A-Z][a-zA-Z_]*\(' lib/core/<file>.fmpl` should return 0 matches.

   **Sub-step 3a (AC-7 documentation deliverable):** Add a TODO comment near the catch-all `_:x => x` rules in `lib/core/ast_optimizer.fmpl` (after the migration completes — likely lines ~35 and ~68 post-migration) enumerating the AST node kinds that fall through unchanged: `:Lambda`, `:Let`, `:Match`, `:Call`, `:List`, `:Map`, `:Block`. Form: `-- TODO(AC-7 / ITER-0004c): Pass-through nodes — :Lambda, :Let, :Match, :Call, :List, :Map, :Block. The optimizer does not currently recurse into these; the corresponding scenario_0103 test asserts structural identity for each kind. If recurse-into rules are added, update both the comment and the test enumeration in lockstep.` Without this comment, AC-7's documentation half is not satisfied.

   **Per-file commit discipline:** commit each file's hand-migration as a separate atomic commit so bisect can pinpoint regressions. Commit message form: `refactor(stdlib): hand-migrate <filename>.fmpl to list-pattern syntax (ITER-0004c)`. After each commit, run `cargo test -p fmpl-core --test ast_to_ir_parity` to confirm SCENARIO-0016 still passes (or `cargo test --workspace` if the file is in the bootstrap-load chain).

   **Sub-step 3a (AC-7 documentation deliverable):** Add a TODO comment near the catch-all `_:x => x` rules in `lib/core/ast_optimizer.fmpl` (after the migration completes — likely lines ~35 and ~68 post-migration) enumerating the AST node kinds that fall through unchanged: `:Lambda`, `:Let`, `:Match`, `:Call`, `:List`, `:Map`, `:Block`. Form: `-- TODO(AC-7 / ITER-0004c): Pass-through nodes — :Lambda, :Let, :Match, :Call, :List, :Map, :Block. The optimizer does not currently recurse into these; the corresponding scenario_0103 test asserts structural identity for each kind. If recurse-into rules are added, update both the comment and the test enumeration in lockstep.` Without this comment, AC-7's documentation half is not satisfied (the AC-7 runnable check at the verification gates section catches drift between this comment and behavior; it depends on the comment existing).

   Specifically:
   - **Comment hand-edits required for AC-13 grep gate** — verified 2026-05-10 via `grep -nE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl`:
     - `lib/core/prelude.fmpl` lines 27, 37, 43–48, 60–63, 72, 89 (~14 comment lines)
     - `lib/core/fmpl_parser.fmpl` line 8 (1 comment line; verify via grep at iteration start in case other comment-line hits surface during migration)
   - **Re-grep at iteration start** to enumerate any additional comment / string-literal hits surfaced after migration. The verification gate uses a string-content-blind grep; every false positive must be hand-edited or the gate fails.

   **Defer-to-ITER-0004d carve-out:** `lib/core/fmpl_parser.fmpl` lines 82-83 and 287-292 (`=> :Tagged(tag, items)`, `=> :PatternTagged(tag, pats)`) emit AST node shapes that the legacy parser productions still require. Migrating these RHS expressions to list shape now produces work that ITER-0004d will then re-touch when it deletes `Expr::Tagged` and the parser productions. The transformer SHOULD migrate them (idempotent + consistent with AC-13 invariant); flag them in the iteration log as "expected churn ITER-0004d will then revisit." Do NOT skip them — that would leave AC-13 false-positive matches. **Cross-iteration coordination:** ITER-0004d scope item 8 currently references `fmpl_parser.fmpl:82-83, 287-292` by line number; after this iteration's transformer run those lines will hold `[:Tagged, ...]`/`[:PatternTagged, ...]` syntax instead of `:Tagged(...)`/`:PatternTagged(...)` and line numbers may shift. ITER-0004d MUST re-grep at iteration start; ITER-0004c iteration log should explicitly note this cross-iteration coordination point so ITER-0004d's planning catches it.

4. **Wire `ast_optimizer.fmpl`** into `eval_via_fmpl_pipeline`. **Precondition:** scope items 1-3 are complete (stdlib in canonical syntax). Two edit sites in `fmpl-core/src/lib.rs`:
   - Bootstrap loader: currently lines 121-123 sequence (a) load prelude, (b) load ast_to_ir, (c) set marker. Insert a third load between current line 122 and current line 123 (i.e., before the marker is set, so subsequent invocations skip re-loading). The new call MUST wrap with `let ast_optimizer = io::load(...)` because `lib/core/ast_optimizer.fmpl` ends with a bare module-map literal (`%{ constant_fold: ..., optimize: optimize }`) — there is no internal `let ast_optimizer = ...` binding inside the file. Without the outer let, `io::load` returns the map but no name is bound, and the pipeline at the next step fails with an undefined-name error. (Note: while `ast_optimizer.fmpl` has internal `let constant_fold`, `let optimize_once`, `let optimize` top-level bindings, we still want the *map* under a single name for the bracket-index call style at the pipeline wrapper. Calling `optimize(ast)` directly is also valid but breaks the consistent `module["function"](...)` pattern used at `fmpl-core/tests/optimizer_integration.rs:43`.) Verbatim form: `eval_via_legacy_parser(vm, r#"let ast_optimizer = io::load("lib/core/ast_optimizer.fmpl")"#)?;`. Compare to the existing working pattern at `fmpl-core/tests/optimizer_integration.rs:31-35`.
   - Pipeline wrapper at lines 126-129 (the `pipeline_source` format!) — thread `ast_optimizer["optimize"](ast)` between `ast::parse` and `ast_to_ir.expr`. The bracket-index form `ast_optimizer["optimize"](...)` matches the existing pattern in `fmpl-core/tests/optimizer_integration.rs:43`. **Pre-implementation verification (binding):** `optimizer_integration.rs:43` is currently `#[ignore]`d, so this lookup form is untested. `lib/core/ast_optimizer.fmpl:83-87` returns `%{ optimize: optimize, constant_fold: ..., algebraic_simp: ... }` using ident-key syntax. Before wiring, verify at the FMPL REPL or a quick test that bracket-index with String key `"optimize"` actually retrieves the value (vs. Symbol key `:optimize` which dot-access would use). If bracket-index does NOT work against ident-key maps, switch to the dot-access form `ast_optimizer.optimize(ast)` — both are valid FMPL idioms; pick the one that matches the map's actual key shape. Document the choice in the iteration log. Final order: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval` (or bracket-form, equivalent).
5. **Add SCENARIO-0103 execution.** A new test file `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs` (kept SEPARATE from `ast_to_ir_parity.rs` — see item 6) provides FOUR observables. The 3 guard inputs and the slot-discriminating algebraic-simp fixture do NOT inflate the parity corpus cardinality; they are scenario-specific tests:
   - **(parity)** Run all 55 parity corpus inputs through `eval_via_fmpl_pipeline` (with optimizer wired); assert each result equals the Rust-compiler result for the same input. This is AC-6 evidence.
   - **(slot — discriminating observable, AC-4 evidence)** Use a structural transformation that NO post-IR optimizer would produce: assert `ast_optimizer["optimize"]([:If, [:Bool, true], [:Int, 1], [:Int, 2]])` returns `[:Int, 1]` at the AST level (post-fold, pre-`ast_to_ir`). The `:If(:Bool(true), trans:t, trans:e) => t` rule in `lib/core/ast_optimizer.fmpl:17-18` (which lives in the **`constant_fold`** grammar — verified by inspection) rewrites the conditional to its true arm — this is *branch elimination structurally distinct from arithmetic constant folding* and could only be produced by an AST-stage optimizer (a post-IR optimizer would receive `[:Branch, [:LoadBool, true], ...]` IR and have no semantic license to delete a branch arm). Capture the optimized AST value and compare structurally. Constant-folding (AC-5) is a separate observable: assert `[:Binary, :+, [:Int, 1], [:Int, 2]]` becomes `[:Int, 3]` post-`optimize`. Both are AST-level observables; do not conflate with IR shape.
   - **(fold-fires-on-real-parse, AC-5 evidence)** Run `ast::parse("1 + 2 * 3")` then `ast_optimizer["optimize"]` on its output and assert the optimized AST contains a folded `[:Int, 7]` rather than `[:Binary, ...]`. This ensures the optimizer is exercised on actual `ast::parse` output, not just hand-built AST.
   - **(guards, AC-3 evidence)** Add a separate test (or sub-tests) that exercises the optimizer's existing guards in `lib/core/ast_optimizer.fmpl`:
     - `1 / 0` (source-form, exercises div-zero guard at line 5 `&{ b != 0 }` — guard prevents fold; result must be an unfolded `:Binary` reaching `ir::compile`, which then either evaluates to a runtime division-by-zero error OR matches Rust-compiler behavior — assert parity with Rust-compiler-via-`eval_via_legacy_parser`)
     - `1 % 0` (source-form, exercises mod-zero guard at line 6 — same parity assertion)
     - `1 / (2 - 2)` (source-form, exercises div-zero guard against a *folded-constant denominator*; first-pass fold reduces `(2-2)` to `:Int(0)`, second-pass fold attempts division and the guard prevents it. The optimizer's two-pass `optimize` (`ast_optimizer.fmpl:76-81`, three iterations of `optimize_once`) is the realistic failure mode for guard-vs-folded-input. Assert parity with Rust compiler.)
     - **INT_MIN negation guard** is exercised in scope item 8's `ac3_int_min_negation_does_not_panic` rewrite — NOT in SCENARIO-0103. The Rust-compiler "native baseline" cannot be obtained for `:Int(i64::MIN)` via any source form because the lexer drops `9223372036854775808` (per `fmpl-core/src/lexer.rs:117`). Move this observable to the optimizer_integration.rs test (item 8.a), where the assertion is "compiles without panic + result equals direct-AST-construction native eval" rather than "matches source-form Rust compiler" — the contract that AC-3 actually requires (the guard exists, it fires, no panic).

   Update `behavior-corpus.md` with the execution command (`cargo test -p fmpl-core --test scenario_0103_optimizer_pipeline`) and `behavior-scenarios.md` automation status.
6. **Update SCENARIO-0016 / parity test infrastructure.** `fmpl-core/tests/ast_to_ir_parity.rs:44-67` (`setup_fmpl_pipeline`/`run_full_pipeline`) currently does NOT load `ast_optimizer.fmpl`. **Decision (binding): KEEP SCENARIO-0016 as the optimizer-disabled parity gate AND make AC-6 evidence come from SCENARIO-0103.** Concretely: do NOT modify `ast_to_ir_parity.rs`'s pipeline; SCENARIO-0016 continues to test `ast_to_ir.fmpl` rules in isolation (the contract its name implies). SCENARIO-0103's parity sub-observable (item 5) provides AC-6 evidence ("all 55 parity inputs pass with optimizer enabled"). This avoids the silent-degradation hazard of subsumption: if the optimizer constant-folds `1 + 2 * 3` to `:Int(7)` before `ast_to_ir.expr`, the `Binary(:+, ...) => :Add` rule isn't exercised — but SCENARIO-0016 still exercises it because its pipeline omits the optimizer.
7. **Resolve `ast_to_ir_indexed.fmpl` and `pipeline_demo.fmpl` disposition** (per design doc 2026-03-03-self-hosting-bootstrap-design.md:28 — the indexed variant is flagged broken / wrong approach). Decision: delete `ast_to_ir_indexed.fmpl`; the working `ast_to_ir.fmpl` is the canonical translator. **These deletions are out-of-band of STORY-0010's ACs** (AC-13 reads "All FMPL stdlib files use list-pattern syntax exclusively" — deletion satisfies the invariant vacuously, but isn't *required* by AC-13). The cleanup is included in this iteration anyway — rationale: it removes files from AC-13's invariant set, simplifies migration scope, and the indexed-variant exploration has the same migration cost as deletion. **Iteration log MUST explicitly call out** "deletion of ast_to_ir_indexed.fmpl + ir_to_execution_tape_indexed.fmpl + pipeline_demo.fmpl is cleanup not bound to STORY-0010 AC; rationale: design doc flagged broken (indexed variants); pipeline_demo is a non-CI-tested demo of the indexed-RPN pipeline whose upstream is being deleted." Cascade cleanups:
   - Delete `lib/core/ast_to_ir_indexed.fmpl` (broken indexed variant).
   - Delete `lib/core/ir_to_execution_tape_indexed.fmpl` (orphan consumer of the deleted indexed variant).
   - **Delete `lib/core/pipeline_demo.fmpl`** (per PAR round 2 finding A2#5 / B2#6): the file is a demo of the indexed-RPN-to-execution_tape pipeline whose upstream files are being deleted, has zero CI test coverage (verified via `grep -rn pipeline_demo fmpl-core/`), and migrating its `:Binary(...)` value-constructor on line 5 would change runtime shape with no test catching a misfire. Deleting it removes the migration target entirely AND the comment-hand-edit obligation at line 12 AND the string-literal hand-edit obligation at line 9. If the demo is worth preserving as a teaching artifact, move it to `docs/_archive/`.
   - All three deletions move the corresponding files OUT of scope items 3's migration list. After this scope item completes, `lib/core/*.fmpl`'s migration set is reduced to: `ast_optimizer.fmpl`, `fmpl_parser.fmpl`, `ir_to_rust.fmpl`, `prelude.fmpl`, `ir_to_execution_tape.fmpl` (5 files, not 7). Re-confirm scope item 3's hand-edit list is reduced accordingly: the `prelude.fmpl` comment lines remain (lines 27, 37, 43-48, 60-63, 72, 89), the `fmpl_parser.fmpl:8` comment remains, but `pipeline_demo.fmpl:5/9` are gone (line 12 was a comment with no `:Tag(args)` syntax — included via deletion, not as a hand-edit).

   **Add `ast_optimizer_test.fmpl` to verification gate** (per PAR round 2 finding A2#5): this file already uses list-pattern syntax (verified — 0 legacy hits) and contains unit tests for the optimizer's rules (e.g., `lib/core/ast_optimizer_test.fmpl:13` asserts `[:Binary, :+, [:Int, 1], [:Int, 2]]` becomes `[:Int, 3]`). After ITER-0004c migrates `ast_optimizer.fmpl` to list-pattern syntax, these tests become a free quality gate. Add execution to the verification gates: invoke `ast_optimizer_test.fmpl` from a Rust test harness (e.g., `cargo test -p fmpl-core --test ast_optimizer_unit` if the file structure already supports it, or via `eval(vm, "io::load(\"lib/core/ast_optimizer_test.fmpl\")")` in a new test file). The implementing developer chooses the mechanism; the gate is "the unit tests in `ast_optimizer_test.fmpl` execute and pass against the migrated `ast_optimizer.fmpl`."
8. **Un-ignore optimizer_integration tests.** The 17 `#[ignore = "ITER-0004b: requires lists-everywhere refactor + optimizer wired into eval_via_fmpl_pipeline"]` tests in `fmpl-core/tests/optimizer_integration.rs` un-ignored. **First, update the `#[ignore = ...]` marker text from "ITER-0004b" to "ITER-0004c"** (or remove entirely — the marker is being lifted) on each of the 17 tests at iteration start, then remove the `#[ignore]` attribute as each test is verified passing. All 17 must pass.

   **Sub-task: rewrite `ac3_int_min_negation_does_not_panic`** (`tests/optimizer_integration.rs:104-111`). The current source `"0 - (-9223372036854775808)"` cannot tokenize: the FMPL lexer (`fmpl-core/src/lexer.rs:117`) parses integer literals via `[0-9]+` then `parse::<i64>().ok()`, which returns `None` for `9223372036854775808` (one greater than `i64::MAX`). The lexer drops the token and the source never reaches the optimizer. The test was written incorrectly during ITER-0004b. **Decision (binding): rewrite via direct AST construction (option (a))** — keeps the AC-3 observable contract intact:
   - Construct `[:Unary, :-, [:Int, -9223372036854775808_i64]]` as `Value::list_node("Unary", vec![Value::Symbol(":-"), Value::list_node("Int", vec![Value::Int(i64::MIN)])])` in Rust.
   - Feed through `ast_optimizer["optimize"]` then `ast_to_ir.expr` then `ir::compile` then `code::eval`.
   - Assert: (i) no panic, (ii) the optimizer's INT_MIN guard at `ast_optimizer.fmpl:15` fires (the input passes through unchanged or the result equals `i64::MIN` per Rust's wrapping-negation semantics — pick whichever the Rust-compiler-direct-AST baseline produces).
   - The "native baseline" comparison uses Rust's *direct evaluation* of the same AST through the legacy compiler path (NOT a source-form path, which is unavailable). Document this in the test doc comment.

**Verification gates:**
- `cargo test -p fmpl-core --test ast_to_ir_parity` — 55/55 passing (SCENARIO-0016).
- `cargo test -p fmpl-core --test optimizer_integration` — 17/17 passing (no `--ignored` needed).
- SCENARIO-0103's new execution command passes (`cargo test -p fmpl-core --test scenario_0103_optimizer_pipeline`).
- AC-13 (greppable stdlib invariant): `grep -cE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl` returns 0 across all stdlib files. The grep matches inside both `--`-prefixed comments and double-quoted string literals; both are hand-edited per scope item 3 to avoid false positives.
- **AC-13 CI gate (NEW):** Add `fmpl-core/tests/stdlib_no_legacy_syntax.rs` with a `#[test]` that runs `grep -cE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl` (or equivalent walked manually with `std::fs::read_dir` + `regex`) and panics if any file matches. This locks the AC-13 invariant against future stdlib edits during the window between ITER-0004c and ITER-0004d (when the parser still accepts `:Tag(args)` syntax). Without this, a future stdlib edit can silently re-introduce legacy syntax.
- **AC-7 runnable check (NEW):** AC-7 is *also* exercised by an explicit unit test in `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs` (or a sibling). **Optimizer reality check (verified 2026-05-10 by inspecting `lib/core/ast_optimizer.fmpl:1-88`):** the current `constant_fold` and `algebraic_simp` grammars only have rewrite rules for `:Binary`, `:Unary`, `:If` constructors; every other node kind hits the `_:x => x` catch-all and is returned **structurally unchanged**. The optimizer does NOT recurse into Lambda bodies, Let bindings, Match arms, Call args, List elements, Map values, or Block expressions. AC-7's TODO comment lists these as fall-through-unchanged kinds; the runnable check enforces THAT contract — not the stronger "recurse and fold inside" contract.

  **Test design:** for each AST node kind enumerated in AC-7's TODO comment (Lambda body, Let, Match, Call, List, Map, Block), construct a representative AST whose top-level constructor is one of these kinds and whose interior contains a foldable `:Binary`. Assert that `ast_optimizer["optimize"](input) == input` — i.e., structural identity. Example: `let input = [:Lambda, [], [:Binary, :+, [:Int, 1], [:Int, 2]]]; assert_eq!(optimize(input), input);`. This proves the optimizer treats the kind as opaque (no recursion) and the inner Binary is NOT folded — which is the documented current behavior. If a future change adds recurse-into rules for any of these kinds, this test will fail and force the AC-7 TODO comment to be updated. The test is intentionally a hedge: it locks the enumeration to behavior, so the documentation cannot drift silently.
- Full workspace test suite passes (`cargo test --workspace`).
- **`ast_optimizer_test.fmpl` unit tests pass against migrated `ast_optimizer.fmpl`** (NEW per scope item 7 addendum). Mechanism: either invoke via a Rust test harness or load with `eval(vm, "io::load(\"lib/core/ast_optimizer_test.fmpl\")")` in a new test file. The unit-test corpus is already in list-pattern syntax (verified 0 legacy hits) and provides a fine-grained regression net against optimizer-rule misfires that integration-level SCENARIO-0103 might miss.
- TODO comment in `lib/core/ast_optimizer.fmpl` lists AST node kinds that fall through unchanged (Lambda bodies, Let, Match, Call, List, Map, Block) — AC-7 (documentation half).

**Out of scope (deferred to ITER-0004d):**
- Deleting `Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch`, tagged bytecode instructions, parser productions for `:Tag(args)`.
- Sweeping FMPL source strings inside Rust test files (`tests/parser_equivalence.rs:82-85`, `tests/tagged_values.rs`, `tests/tagged_pattern_match.rs`, `tests/fmpl_interpreter.rs`, `tests/ast_to_ir_parity.rs:88-122`, etc.).
- New parse-rejection scenarios (added in ITER-0004d).
- Reconciling SCENARIO-0039 (uses `:int(n)` value-pattern syntax) and SCENARIO-0066 (references `Value::Tagged`).
- Updating `fmpl-core/src/grammar/parser.rs:2072` internal grammar string `:Tagged(tag, expr*:args) => :MakeTagged(tag, args)` (relevant when MakeTagged is deleted in ITER-0004d).
- Removing `FMPL_USE_FMPL_COMPILER` opt-in.

### ITER-0004d.0 — FMPL-Source-Grep Tooling Precursor

**Stories:** No new STORY-0010 ACs — bottom-up tooling carve-out scheduled 2026-05-10 from ITER-0004d PAR rounds 1 and 2. Round 1 surfaced that the AC-13/AC-14 grep gates produce massive false positives (Rust qualified paths `Pattern::Constructor(`, `Instruction::MakeTagged(` match the naive regex `:[A-Z][a-zA-Z_]*\(`) and that AC-14 has no permanent CI sentinel. Round 2 surfaced that the initial draft of this tool had a foundational tokenization error (it specified `Colon, Ident, LParen` as the FMPL-side token triple, but `fmpl-core/src/lexer.rs:113-115` consumes `:Foo` as a single `Token::Symbol("Foo")` token — colon-absorbed). This iteration ships the corrected tool.

**Rationale:** A precise gate must distinguish FMPL syntax from Rust syntax. The strip-then-scan technique in `stdlib_no_legacy_syntax.rs` works for the `lib/core/*.fmpl` case where every byte is FMPL, but breaks down when scanning `fmpl-core/tests/*.rs` (where `Pattern::Constructor(` is Rust, not FMPL, but the regex matches both). The correct discriminator is "does the token stream — as produced by the existing FMPL lexer — contain a `Token::Symbol(s)` followed immediately by `Token::LParen`?" The lexer at `fmpl-core/src/lexer.rs:113-115` uses regex `:([a-zA-Z_][a-zA-Z0-9_]*|[+\-*/%<>=!|&]+)` and emits `Token::Symbol(SmolStr)` with the leading colon stripped from the slice. Both `:Foo(args)` (uppercase, the legacy tagged-constructor) AND `:foo(args)` (lowercase, also produced by the same legacy syntax) tokenize to `Symbol(name), LParen` — the tool must detect both, since after AC-9 (ITER-0004d.1) deletes the `LParen` arm at `parser.rs:619-640`, both forms become silent reinterpretations as `Call(Symbol, args)` rather than parse errors. Tool's correctness piggybacks on the Rust lexer the codebase already trusts; no parallel implementation.

**Status:** done:ITER-0004d.0 (2026-05-10). Baseline: `lib/core=0, src/rs=43, tests/fmpl=72, tests/rs=625` (post-allowlist). See iteration-log.md for delivered scope, PAR findings, and known limitations carried forward to ITER-0004d.1.
**Depends on:** ITER-0004c (so the stdlib is already in canonical list-pattern syntax — the tool's regression suite needs a clean `lib/core/` baseline to validate against).
**Look-ahead check:** Unblocks ITER-0004d.1 (precise AC-14 sweep targets + permanent CI sentinel). Reusable by ITER-0004d.2 (opcode-name sweep across qualified-path Rust references — though that uses a separate scanner since opcode names are Rust identifiers, not FMPL strings), ITER-0004e (parser-helper relocation gate), ITER-0004f (binary-flatten gate), ITER-0006 (self-compile invariant scans). Replaces `fmpl-core/tests/stdlib_no_legacy_syntax.rs`'s hand-rolled scanner.

**Files in scope:**
- `fmpl-core/src/diagnostics/mod.rs` (NEW module — `fmpl-core/src/diagnostics/` directory does not currently exist; create it) — library module exposing the two scan functions and the `TaggedSyntaxHit` / `SourceKind` types.
- `fmpl-core/Cargo.toml` — add `syn = { version = "2", features = ["full", "extra-traits"] }` as a `[dev-dependencies]` entry (verified absent: `grep syn fmpl-core/Cargo.toml` returns nothing). `syn` is only used by the test scanner and (optionally) the bin; runtime crate stays `syn`-free.
- `fmpl-core/tests/no_legacy_fmpl_syntax.rs` (NEW) — CI gate that calls the library across all configured surfaces and **records** the current hit count (with `== 0` flip deferred to ITER-0004d.1).
- `fmpl-core/tests/stdlib_no_legacy_syntax.rs` — DELETE (superseded by `no_legacy_fmpl_syntax.rs`). The deletion is part of ITER-0004d.0 because the new gate's `lib/core/` surface fully subsumes the old gate's coverage.
- `fmpl-core/tests/diagnostics_fmpl_source_scan.rs` (NEW) — unit tests for the library.
- (Optional, deferred) `xtask/` or `tools/fmpl-grep/` CLI bin — explicitly OUT OF SCOPE for ITER-0004d.0. The CI gate calls the library directly; no CLI consumer exists yet inside this iteration. If a later iteration wants a CLI, it's a small wrapper.

**Scope:**

1. **Library module `diagnostics`.** Public API:
   - `pub struct TaggedSyntaxHit { pub source: SourceKind, pub byte_offset: usize, pub tag: SmolStr }` where `SourceKind` is `FmplFile { path: PathBuf }` or `RustString { rust_path: PathBuf, rust_byte_offset: usize }`.
   - `pub fn scan_fmpl_source(text: &str, source: SourceKind) -> Result<Vec<TaggedSyntaxHit>>` — tokenize via `crate::lexer::Lexer::new(text).tokenize()`. **Note: `Lexer::tokenize` returns `Result` (verified `fmpl-core/src/lexer.rs:264-282`)**; this function propagates the error. Walk the resulting `Vec<SpannedToken>` and for each `Token::Symbol(s)` immediately followed by `Token::LParen`, emit a `TaggedSyntaxHit` with `tag = s.clone()`. Detect BOTH uppercase AND lowercase first-letter — after AC-9 lands, both `:Foo(args)` and `:foo(args)` silently reinterpret as Call expressions, so both must be swept and gated.
   - `pub fn scan_rust_strings(rust_src: &str, rust_path: &Path) -> Result<Vec<TaggedSyntaxHit>>` — parse via `syn::parse_file`, walk the syntax tree with a `syn::visit::Visit` impl, extract every `syn::Lit::Str` (string literals; raw strings handled by `LitStr::value()`). For each literal, attempt `scan_fmpl_source(literal.value(), SourceKind::RustString { rust_path: rust_path.into(), rust_byte_offset: literal.span().byte_range().start })`. **Critical: many string literals are not valid FMPL fragments (shell strings, format strings, doc snippets)**; lexer errors on non-FMPL content are swallowed silently with the rationale that "if it can't even lex as FMPL, it cannot contain valid `:Tag(args)` syntax" (a string containing an unparseable char halfway through still permits the prefix to be scanned — but for the AC-13/14 invariant, the worst case is a missed hit, not a false positive, and a missed hit will resurface the first time someone refactors the file). Document the swallow policy in the function docstring. Rust comments and Rust qualified paths (`Pattern::Constructor(...)`) are not part of the syntax-tree string-literal set, so false positives disappear by construction.
   - `pub enum DiagnosticsError { LexerError { path: PathBuf, message: String }, SynParseError { path: PathBuf, error: syn::Error } }` — error type for cases where the caller wants strict failure (e.g., the CI gate optionally fails on Rust files that don't parse).

2. **CI gate `no_legacy_fmpl_syntax.rs`.** Walks four surfaces:
   - **`lib/core/*.fmpl`** — every `.fmpl` file under `lib/core/`. Calls `scan_fmpl_source` directly on file contents.
   - **`fmpl-core/tests/fmpl/*.fmpl`** — every `.fmpl` file under `fmpl-core/tests/fmpl/`. Same surface as above. Includes the orphan files `ast_to_ir.fmpl` and `fmpl_parser.fmpl` (live test fixtures `apply_operator.fmpl` and `fmpl_grammar.fmpl` will pass; orphans will not).
   - **`fmpl-core/tests/*.rs` string literals** — every `.rs` file under `fmpl-core/tests/`. Calls `scan_rust_strings`.
   - **`fmpl-core/src/*.rs` string literals** — every `.rs` file under `fmpl-core/src/`. Calls `scan_rust_strings`.
   
   At ITER-0004d.0 time, the gate **records the actual hit count per surface** and asserts no growth — implemented as a baseline-stored JSON file at `fmpl-core/tests/no_legacy_fmpl_syntax.baseline.json` that the gate writes once (when `FMPL_REGEN_BASELINE=1` env var is set) and asserts against on every run. This avoids the "fabricated 692 number" problem from PAR round 2 — the baseline is *whatever the tool actually finds*, frozen at the iteration's land time. ITER-0004d.1 deletes the baseline file and changes the gate to assert `== 0` instead.

3. **Allowlist mechanism.** The gate's own source file may contain example FMPL strings inside its test cases (`r#":Foo(1, 2)"#` etc.). An allowlist `&[(file_glob, byte_range_or_substring)]` excludes specific known-OK hits. Allowlist is a small constant in the gate source itself.

4. **Delete `fmpl-core/tests/stdlib_no_legacy_syntax.rs`.** Self-contained test (no callers); the new gate subsumes it.

5. **Bootstrap-mode behavior:** `crate::lexer` is unchanged by ITER-0004d.1; the library compiles and runs against the pre-burn codebase. After ITER-0004d.1, the FMPL files no longer contain `:Tag(args)` syntax — so `scan_fmpl_source` is a no-op on them. After ITER-0004d.1, the `tests/*.rs` strings are swept and the gate flips to `== 0`. The library's API surface stays stable across both iterations.

6. **Unit tests (`diagnostics_fmpl_source_scan.rs`):**
   - `scan_fmpl_source` finds one hit for `r#"let x = :Foo(1, 2)"#`.
   - `scan_fmpl_source` finds one hit for `r#"let x = :foo(1, 2)"#` (lowercase — same legacy form).
   - `scan_fmpl_source` finds zero hits for `r#"let x = [:Foo, 1, 2]"#` (list-pattern syntax, no LParen after Symbol).
   - `scan_fmpl_source` finds zero hits for `r#"let x = :foo"#` (bare symbol).
   - `scan_fmpl_source` finds zero hits for `r#"-- :Foo(1, 2) in a comment"#` (lexer skips comments per `lexer.rs` regex).
   - `scan_rust_strings` finds exactly one hit when fed Rust source containing both `let s = "let x = :Foo(1, 2)";` AND `let p: Pattern = Pattern::Constructor("Foo", vec![]);` — the second is a Rust qualified path, not in any string literal.
   - `scan_rust_strings` finds zero hits when fed `r##"r#"[:Foo, 1, 2]"#"##` (raw-string with list-pattern content).
   - `scan_rust_strings` errors-or-swallows when fed a string with mid-content lexer-illegal chars (`"some \u{0007} bell"`); document the chosen behavior.

7. **CI gate first-run validation:** With `FMPL_REGEN_BASELINE=1`, generate the baseline JSON. Inspect it: expect roughly 14 files in `tests/*.rs` (per round-2 ground truth — *not* a hard assertion in the gate code, just a sanity check that the tool found something close to the expected order of magnitude). Commit the baseline. Subsequent runs without `FMPL_REGEN_BASELINE` assert against the file.

**Verification gates:**
- `cargo test -p fmpl-core --test diagnostics_fmpl_source_scan` passes (unit tests for the library, ~8 cases).
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` passes against the committed baseline JSON.
- `cargo build -p fmpl-core --tests` succeeds (verifies `syn` `[dev-dependencies]` entry works).
- AC-13 invariant from ITER-0004c remains satisfied — the new gate's `lib/core/` baseline records 0 hits (the migration is already complete; if non-zero, that's a regression in ITER-0004c that needs investigation first).

**Out of scope (for ITER-0004d.0):** Scanning markdown / docs / behavior-scenarios.md. Scanning `xtask/`, `examples/`, or third-party crates. Adding a CLI `xtask fmpl-grep` bin — deferred to whichever subsequent iteration first needs a CLI consumer. The `--format json` and `--exclude` CLI flags from round-1 spec are dropped (no consumer named them). Flipping the gate's assertion from "matches baseline" to `== 0` — that's ITER-0004d.1's responsibility.

### ITER-0004d.1 — Parser/AST/Pattern Burn (AC-9, AC-10, AC-12)

**Stories:** STORY-0010 Phase B AC-9, AC-10, AC-12 (the AST/parser surfaces; bytecode opcode rename moves to ITER-0004d.2). Also: SCENARIO-0039 rewrite, SCENARIO-0066 rewrite, STORY-0095/EPIC-032 AC-4 repair, and the CI-gate flip from ITER-0004d.0.

**Rationale:** With the precise tool from ITER-0004d.0 in place, this iteration deletes the AST/parser surfaces that produce tagged-constructor values. Three distinct deletion surfaces, NOT two (round-2 PAR caught this — there are two `Pattern` enums in the codebase plus a tagged variant in each):

| Surface | File | Type produced | Deletion AC |
|---|---|---|---|
| FMPL source-level expression `:Tag(args)` | `fmpl-core/src/parser.rs:619-640` | `ast::Expr::Tagged(SmolStr, Vec<Expr>)` (defined at `ast.rs`) | AC-9 |
| FMPL source-level pattern `:Tag(p1, p2)` | `fmpl-core/src/parser.rs:1849-1871` | `ast::Pattern::Constructor(SmolStr, Vec<Pattern>)` (defined at `fmpl-core/src/ast.rs:116`) | AC-10 |
| Grammar-pattern unified `Pattern::Tagged { tag, patterns }` | (decoder-produced from list-shape AST; consumers walk it) | `pattern::Pattern::Tagged { tag: SmolStr, patterns: Vec<Pattern> }` (defined at `fmpl-core/src/pattern/mod.rs:58-61`) | AC-10/12 |
| Grammar-pattern tree-grammar value match `:Tag(p)` | `fmpl-core/src/grammar/parser.rs:899,1136,1333` | `pattern::Pattern::TagMatch(SmolStr, Vec<Pattern>)` (defined at `fmpl-core/src/pattern/mod.rs:143`) | AC-12 |

(There is a third `Pattern` enum at `fmpl-core/src/tuplespace/mod.rs:77` but it has no `Tagged`/`Constructor`/`TagMatch` variant and is out of scope.)

**Status:** done 2026-05-12 (with **T18 deferred** — the `no_legacy_fmpl_syntax.rs` gate flip to `== 0` mode moved to ITER-0004d.3 because the bootstrap-parse error in `fmpl-bootstrap lib/core/parser_generator.fmpl` would let a green gate silently rely on the fallback parser; the bootstrap follow-up must investigate that first). F1+F2+F9+MF1 parser rejection landed; T7-T14 deleted the four producer/consumer variants; parser-epoch freshness system added; T15-T17 reconciled docs + scenarios + behavior-corpus; T19 added `fmpl-core/tests/structural_invariants.rs` (17 evidence tests, all green) covering SCENARIO-0104/0105/0106.
**Impacted scenarios:** SCENARIO-0104 NEW (implemented), SCENARIO-0105 NEW (implemented), SCENARIO-0106 NEW (implemented), SCENARIO-0039 rewrite (done), SCENARIO-0066 rewrite (done). `no_legacy_fmpl_syntax.rs` CI gate flip moved to ITER-0004d.3. SCENARIO-0103 + SCENARIO-0016 still pass (sentinels). SCENARIO-0003 reconfirms.
**Depends on:** ITER-0004d.0 (precise FMPL-source scanner).
**Look-ahead check:** Unblocks ITER-0004d.2 (opcode rename — the dead match arms in `compiler.rs` consuming `Expr::Tagged` / `Pattern::Constructor` are deleted here, leaving only emit sites that are themselves dead code, simplifying the rename surface). Unblocks ITER-0006 (the parser has exactly one AST shape).

**Binding preconditions:**

- **Explicit parse-rejection logic for AC-9.** Round-2 PAR caught that the original plan was wrong about what happens when the `Token::LParen` arm at `parser.rs:619-640` is deleted. The fallthrough is to `parse_postfix` at `parser.rs:512-515`, which sees `LParen` immediately after `Expr::Symbol` and produces `Expr::Call(Symbol, args)` — a silent reinterpretation, NOT a parse error. To preserve the AC-9 "parse rejection" intent, the deletion must be **replaced with an explicit error**: when `parse_primary` sees `Token::Symbol(s)` immediately followed by `Token::LParen`, emit a clear `Error::Parser { token, message: format!("Legacy tagged-constructor syntax `:{}(...)` was removed in ITER-0004d.1. Use list-pattern syntax `[:{}, ...]` instead.", s, s) }`. Same treatment for the pattern surface at `parser.rs:1849-1871`. SCENARIO-0104 / SCENARIO-0105 observables are stated in terms of "parse error returned and Expr::Tagged value not produced" — observable from a wrapper test, not from the error API's structured fields.
- **Scenario observables reworded to be testable.** SCENARIO-0104/0105 do not require "token range in the error" (the `Error::Parser` type at `fmpl-core/src/error.rs:14-15` only carries `token: usize` + `message: String` — no range field). Observables instead state: "parse() of `:Tag(args)` returns `Err(Error::Parser { .. })`; the error message contains the offending source-side identifier; the parsed AST is not produced." This is testable today without API work.
- **Three tagged-pattern surfaces, not two.** The deletion graph must cover all three (per the table above).

**Scope:**

1. **Baseline confirmation.** Run `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` (from ITER-0004d.0). Confirm the baseline JSON matches the working tree.

2. **Add explicit parse-rejection logic** (binding precondition above). Edit `fmpl-core/src/parser.rs:619-640` to replace the existing `if self.check(&Token::LParen) { ... Ok(Expr::Tagged(s, args)) }` block with an early-error path that emits `Error::Parser` when `LParen` follows `Token::Symbol(s)`. Same for `parser.rs:1849-1871`. Add unit tests for both rejections (these become SCENARIO-0104 / SCENARIO-0105 observables). **Note:** at this step `Expr::Tagged` and `Pattern::Constructor` variants still exist; they're not yet unreachable because other (non-parser) producers may still construct them. The early-error parser change merely closes the *parser* production for these shapes.

3. **Sweep `fmpl-core/tests/*.rs` FMPL source strings.** Use the ITER-0004d.0 scanner output as the precise target list. For each hit, hand-edit the string literal to use list-pattern syntax. After each file, regenerate the baseline (`FMPL_REGEN_BASELINE=1 cargo test ... no_legacy_fmpl_syntax`) and inspect the diff. After all files, regenerate the baseline; expect zero hits across `tests/*.rs`.

   **`parser_equivalence.rs:82-85` resolution (round-5 PAR fix):** that block ("Tagged values (constructors)") is semantically invalidated by AC-9 because the entire concept being tested (legacy hand-rolled parser and generated PEG parser both producing an `Expr::Tagged` AST) no longer applies — both parsers must now reject the syntax. **Decision: delete the block entirely**, not rewrite to list-shape parity. Rationale: the list-shape `[:Tag, args]` syntax is tested by other entries in this same corpus (any list-literal test exercises it). Rewriting this specific block as "both parsers reject `:Tag(args)` with an `Error::Parser`" would be a useful equivalence test but it's structurally a different scenario, not the legacy-vs-generated equivalence the file is named for. Document the deletion in the PR description.

   **Parity-corpus pre-rewrite (round-6 PAR fix — CRITICAL ORDERING):** Round-5 PAR claimed the parity corpus was `:Tag(args)`-free; round-6 PAR caught that this is empirically false. Verified counts (2026-05-10):
   - `fmpl-core/tests/ast_to_ir_parity.rs`: **31 hits** (`:LoadInt(42)`, `:Add(...)`, `:Point(1, 2)`, etc. — used in both Rust-compiler-input and FMPL-compiler-input strings since these tests intentionally exercise the legacy IR-construction syntax).
   - `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs`: **2 scanner-visible hits** (`:Point(1, 2)` at lines 119, 196 inside `assert_optimizer_parity(...)` test strings). Other `:Tag(` matches in this file are inside `//!` / `///` doc comments, which the ITER-0004d.0 syn-based scanner does not flag (it only scans Rust string literals).
   - `fmpl-core/tests/integration_pattern_unification.rs`: **7+ hits** (`:Some(...)`, `:Pair(...)`, etc.).
   - `fmpl-core/tests/generated_parser_correctness.rs`: ~159 hits (many in tests SPECIFICALLY validating the legacy syntax — see step 3.5 below).
   - `fmpl-core/tests/tagged_pattern_match.rs`: ~32 hits (entire file is about `:Tag(args)` matching — see step 3.5 below).
   - Plus many other files captured by the 625-total `tests/rs` count in ITER-0004d.0's baseline.

   **Hard ordering requirement:** step 3 (the sweep) MUST land BEFORE step 2 (the parser early-error logic). Without this ordering, SCENARIO-0016 and SCENARIO-0103 sentinels break the moment step 2 lands and stay broken until step 3 catches up — a multi-task regression window. **Correct order: step 1 (baseline confirm) → step 3 (sweep tests/*.rs) → step 4 (sweep src/*.rs) → step 2 (parser early-error) → step 5+ (orphan delete, stdlib, deletes).** Round-6 PAR formally re-orders.

   Also: SCENARIO-0103 currently runs `eval_via_fmpl_pipeline` against literal source like `:Point(1, 2)` to test list-pattern matching at the AST level. Rewriting these to `[:Point, 1, 2]` exercises a DIFFERENT internal code path (Value::list_node literal in source becomes a list-literal AST, not a tagged-constructor AST). Verify after rewrite that SCENARIO-0103's "fold-fires" observable (line 1993) still fires — the parity assertion is unchanged but the IR shape passing through `ir::compile` changes.

3.5. **Legacy-syntax-validation tests — DELETE entirely (not sweep).** Three test files exist solely to validate that the legacy `:Tag(args)` syntax parses and matches correctly. Sweeping these to `[:Tag, ...]` would make them duplicates of existing list-literal tests, providing no incremental coverage:
   - `fmpl-core/tests/generated_parser_correctness.rs:423-450` (3 tests: `test_empty_tagged`, `test_tagged_with_args`, `test_nested_tagged`) — delete these three test functions.
   - `fmpl-core/tests/tagged_pattern_match.rs` — delete the entire file (whose name is itself a marker of deprecated functionality). The behaviors it covers (tagged-pattern matching against tagged values) are subsumed by list-pattern matching tests elsewhere.
   - `fmpl-core/tests/tagged_values.rs` (153 lines, 10 `#[test]` functions all evaluating `:Tag(args)` source strings — round-7 PAR addition) — delete the entire file. Same rationale as `tagged_pattern_match.rs`: the file exists solely to verify legacy syntax parses+evaluates; post-AC-9, this whole concern is gone.

   Add brief PR comment: "Deleted with AC-9/10; subsumed by list-pattern matching tests."

3.6. **Stdlib `:TagMatch` rewrite arms are already dead.** `lib/core/grammar_optimizer.fmpl:61, 94, 129, 177` each contain `| [:TagMatch, any:tag, trans*:children] => prepend(:TagMatch, prepend(tag, children))` rewrite arms. **Verification (round-7 PAR correction):** these arms are ALREADY dead code today, not "dead after step 11" — no Rust code in `fmpl-core/src/` produces `[:TagMatch, ...]`-shaped Value nodes (verified 2026-05-10 by `grep -rnE '"TagMatch"|"\bTagMatch\b"' fmpl-core/src/` returning no encoder sites). The earlier round-6 wording incorrectly claimed `grammar_to_ir.rs:234` was an encoder; it is an UNSUPPORTED-pattern `Err` branch, not an encoder. Step 11's deletion of `Pattern::TagMatch` reinforces this but is not a precondition. **Action: delete the four arms.** Verify by running `cargo test -p fmpl-core` after the deletion; no test should break (the arms were identity rewrites of an unproduced input shape).

4. **Sweep `fmpl-core/src/*.rs` FMPL source strings.** Same approach. Most src files don't contain FMPL strings, but some doctest examples might.

5. **Delete orphan `fmpl-core/tests/fmpl/{ast_to_ir,fmpl_parser}.fmpl`.** Verified spike solutions from commit `66c42665` (2026-01-29); zero references (`grep tests/fmpl/` finds only `fmpl_runner.rs:50` for `apply_operator.fmpl` and `grammar/parser.rs:2046` for `fmpl_grammar.fmpl` — neither references the orphans). Closes the coherence gap.

6. **Update `lib/core/fmpl_parser.fmpl`** — delete `tagged_arg_rest` (line 80), `tagged_args` (line 81), `tagged_with_args` (line 82), `tagged_empty` (line 83), `tagged` (line 84), and `pat_constructor` (line 292). **PRESERVE** `pat_arg_rest` (line 294) and `pat_args` (line 295) — they are shared with `pat_list` (line 297) which is the canonical list-pattern syntax that this iteration preserves; deleting them would break list patterns. Update the `primary` rule (line 178) to drop the `tagged` alternative. Update the `pat_primary` rule (line 303) to drop `pat_constructor` from the alternation. Bare `:foo` symbol literals (`pat_symbol` rule, a different rule) remain. **Also delete the now-orphan `tag_name` rule** (`fmpl_parser.fmpl:79`) — after `tagged_*` and `pat_constructor` deletions, `tag_name` has no consumers; round-6 PAR caught it as dead code. **Line numbers verified 2026-05-10 post-ITER-0004c migration; `pat_args` shared-use confirmed by PAR round 5; `tag_name` orphan-status confirmed by round 6.**

7. **Delete `Expr::Tagged` AST variant** (AC-9). **Note (round-6/7 PAR addition):** after this step lands, `fmpl-core/src/ir_builder.rs:239 fn tagged(&mut self, tag: SmolStr, args: &[InstrIndex]) -> InstrIndex` becomes a zero-caller helper (verified: `grep -rnE '\.tagged\(' fmpl-core/src/ fmpl-bootstrap/src/` returns no caller sites; only the definition at `ir_builder.rs:239` remains). The function emits `Instruction::MakeTagged` which ITER-0004d.2 will rename. **Action: delete `fn tagged` in this iteration to keep the dead-code surface contained.** Producer + consumer sites (verified 2026-05-10):
   - Producer: `fmpl-core/src/parser.rs:619-640` — step 2 already removed the `LParen` arm. The variant itself remains constructable from other paths until those are deleted.
   - Decoder: `fmpl-core/src/value_to_ast.rs:358` (decoder constructs `Expr::Tagged`); `fmpl-core/src/builtins/ir_to_rust.rs:1440`; `fmpl-core/src/builtins/ast.rs:27` (`expr_to_value` arm — delete the arm; the encoder no longer needs to encode `Tagged`); `fmpl-core/src/builtins/grammar_to_ir.rs:311`.
   - Display impl arm: `fmpl-core/src/repr.rs:225`.
   - Compiler consumer at `fmpl-core/src/compiler.rs:869-874` (the `Expr::Tagged(tag, args) =>` arm emitting `Instruction::MakeTagged`) — delete the arm. **Note on `Instruction::MakeTagged`:** the opcode variant is NOT renamed in this iteration (ITER-0004d.2 handles the rename). After this iteration, `MakeTagged` continues to be emitted at `fmpl-core/src/ir_builder.rs:240` (the `tagged()` helper) and `fmpl-core/src/builtins/ir.rs:344` (the `"MakeTagged"` IR dispatch arm). Deleting the compiler.rs:869-874 arm only removes ONE of three emit sites; the opcode is not emission-less, just narrower. ITER-0004d.2's rename map at roadmap.md:509 already accounts for these surviving emit sites.
   - Variant definition: `fmpl-core/src/ast.rs:157` (pinned 2026-05-10).
   - Order: delete consumers → delete the variant. Verify with `grep -rn 'Expr::Tagged' fmpl-core/src/` returning zero matches.

8. **Update `lib/core/ast_to_ir.fmpl:21`** — delete the rule `[:Tagged, any:tag, exprs:xs] => [:MakeTagged, tag, xs]` entirely. This step lands AFTER step 7 (producer-consumer deletion of `Expr::Tagged`) so that no intermediate state can route AST `[:Tagged, ...]` values through `ast_to_ir.expr` with no matching rule. Round-5 PAR caught this ordering: the rule deletion must follow producer removal, not precede it.

9. **Delete `ast::Pattern::Constructor` variant** (AC-10). Producer + consumer sites:
   - Producer: `fmpl-core/src/parser.rs:1849-1871` — step 2 already removed the `LParen` arm.
   - Decoder/encoder: `fmpl-core/src/value_to_ast.rs:1241`, `fmpl-core/src/builtins/ir_to_rust.rs:1873`, `fmpl-core/src/builtins/ast.rs:398`.
   - Display impl: `fmpl-core/src/repr.rs:101`.
   - **Test fixture** (round-5/7 PAR addition): `fmpl-core/tests/diagnostics_fmpl_source_scan.rs:140` uses `Pattern::Constructor("Foo".to_string(), vec![])` inside a `r##"..."##` string literal as fixture Rust source for testing the scan_rust_strings parser. **Round-7 PAR scope clarification:** SCENARIO-0106 grep #5 is scoped to `fmpl-core/src/` (NOT `fmpl-core/tests/`), so the fixture would not actually be caught by the structural invariant. Nevertheless, rewrite the fixture to use a synthetic enum name (e.g., `MyPattern::Constructor(...)`) to (a) eliminate stale documentation referencing a deleted type, and (b) avoid confusing a future implementer who reads the fixture and thinks `Pattern::Constructor` still exists.
   - Compiler consumer arms at `fmpl-core/src/compiler.rs:2540-2548` (emits `MatchTag` for outer constructor — PRESERVED opcode), `:2646-2670` (emits nested `MatchTag` + `ExtractTaggedChild` for nested constructors), `:2958-2967` (emits `ExtractTaggedChild` for unification). Delete the arms entirely; their consumer `ast::Pattern::Constructor` is gone. The opcodes those arms emitted (`MatchTag`, `ExtractTaggedChild`) lose these specific emit sites but `MatchTag` is still emitted by `Pattern::Symbol` (preserved). **Clarification (round-5 PAR):** `compiler.rs:3117, 3342` are NOT governed by this step — they are inside `UP::Tagged` arms (where `UP = pattern::Pattern` via `use crate::pattern::Pattern as UP`), which are governed by step 10 (delete `pattern::Pattern::Tagged`). Do not conflate the two `Pattern` types' arms when sequencing deletes. `ExtractTaggedChild` is also emitted from `builtins/ir.rs:776` which IS reachable from a different code path; verify at iteration start that this site survives this iteration (it's renamed in ITER-0004d.2).
   - `is_symbol_with_paren()` helper at `parser.rs:1963` and its caller at `parser.rs:1599` (`parse_let` destructuring) — become dead code; delete.
   - Variant definition: `fmpl-core/src/ast.rs:116`.

10. **Delete `pattern::Pattern::Tagged` variant** (AC-10/12, the unified-pattern surface). The variant is decoded from list-shape AST values by `value_to_ast.rs` (somewhere — search) and consumed by:
    - `fmpl-core/src/grammar/runtime.rs:1149`
    - `fmpl-core/src/grammar/trampoline.rs:1183`
    - `fmpl-core/src/grammar/optimizer.rs:215`
    - `fmpl-core/src/builtins/grammar_to_ir.rs:250`
    - `fmpl-core/src/repr.rs:687` (`GrammarPattern::Tagged { tag, patterns }` arm — `GrammarPattern` is a re-export of `pattern::Pattern`)
    - Compiler arms in `compiler.rs:3111` (UP::Tagged → MatchTagged emit), `:3333` (UP::Tagged → MatchTagged emit), and **`compiler.rs:3803`** (GP::Tagged → MatchTagged emit in `compile_grammar_pattern`; round-5 PAR addition — `GP = grammar::Pattern` via `use ... as GP`). These emit the legacy opcode names. After this iteration, the arms are deleted, but the opcode definitions remain until ITER-0004d.2.
    - Variant definition: `fmpl-core/src/pattern/mod.rs:58-61`.
    - **Test-side construction sites** (round-5 PAR addition; not picked up by step 3's FMPL-string sweep because these are Rust constructor calls): `fmpl-core/tests/pattern_unification.rs:39, 43, 176, 279, 284` (5 sites) and `fmpl-core/tests/context_aware_compilation.rs:95, 340, 549` (3 sites). Rewrite each test to use `Pattern::ListMatch` (the list-pattern equivalent) constructed via `Pattern::list_match(...)` or its equivalent helper, OR delete the test if the underlying behavior (constructor-pattern unification semantics) no longer applies. The plan is to rewrite, not delete — the unification semantics still apply to list-patterns. Verify by running `cargo test -p fmpl-core --test pattern_unification` after edits.

    **Note (round-5 PAR fix):** `compiler.rs:4380` was incorrectly attributed to this step in earlier drafts. That site is inside a `GP::TagMatch(tag, child_patterns)` arm starting at `:4360`, i.e., it consumes `pattern::Pattern::TagMatch`, not `pattern::Pattern::Tagged`. It is correctly placed in step 11 below.

11. **Delete `pattern::Pattern::TagMatch` variant** (AC-12). Producer + consumer sites:
    - Grammar-DSL parser productions: `fmpl-core/src/grammar/parser.rs:899, 1136, 1333` — delete.
    - Consumers: `fmpl-core/src/grammar/runtime.rs:784`, `fmpl-core/src/grammar/trampoline.rs:999`, `fmpl-core/src/grammar/optimizer.rs:221`, `fmpl-core/src/pattern/mod.rs:281`, `fmpl-core/src/builtins/grammar_to_ir.rs:234`, `fmpl-core/src/repr.rs:616` (`GrammarPattern::TagMatch(tag, pats)` arm).
    - **Compiler arms** (round-5 PAR addition):
        - `compiler.rs:3638` — `UP::TagMatch(_, _)` appears as one branch of a multi-arm `MatchAny` pattern in `compile_pattern_full`. Delete the branch (the surrounding `|` chain remains valid with the branch removed).
        - `compiler.rs:4360-4392` — full `GP::TagMatch(tag, child_patterns) => { … emit MatchTaggedWithBindings at :4380, MatchTagged at :4389 … }` arm in `compile_grammar_pattern`. Delete the arm entirely.
    - Variant definition: `fmpl-core/src/pattern/mod.rs:143`. Also delete the doc comment immediately preceding the variant (currently at `pattern/mod.rs:140-142`) which references the deleted `Value::Tagged` type. **Also** (round-7 PAR correction): `pattern/mod.rs:169` `contains_repeat` doc comment says "Used by TagMatch to decide whether to unwrap list-valued children". The only callers are inside `Pattern::TagMatch` arms (`grammar/runtime.rs:784, 821`). After step 11 deletes those arms, `contains_repeat` has zero callers and becomes orphan. **Action: delete `fn contains_repeat` entirely** along with its doc comment. Verify with `grep -rn 'contains_repeat' fmpl-core/src/` returning only the definition site post-deletion (or zero sites after deletion).

12. **Update grammar-DSL test fixtures**. Round-2 PAR identified THREE test functions in `fmpl-core/src/grammar/parser.rs` using grammar-DSL `:Tag(args)` patterns:
    - `:2067-2084` `test_parse_star_quantifier_in_tag_child` — tests `*` quantifier inside a tag-child position. The "tag-child" concept refers to children of a `:Tag(...)` pattern. After step 11 deletes that pattern form, the feature no longer exists. **Resolution: delete this test entirely** (note in PR: "feature deleted with AC-12; star-in-list-position is already covered by other tests").
    - `:2086-2101` `test_parse_rule_binding_in_tag_child` — same reasoning. **Resolution: delete entirely.**
    - `:2103-2118` `test_parse_tag_in_list_pattern` — tests that a `:Tag(args)` pattern can appear as a child of a list pattern. After step 11, the inner `:Tag(args)` no longer parses. The TEST INTENT (nested patterns inside list patterns) is still relevant. **Resolution: rewrite to use a list-pattern child** — e.g., the inner pattern becomes `[:Tag, ...]`. Rename to `test_parse_nested_list_pattern`.

    **Ordering (round-5 PAR clarification):** this step lands BEFORE step 11 so that the tests-being-deleted don't fail in between. Specifically: delete the two doomed tests (or rewrite the third) → THEN delete the grammar-DSL `Pattern::TagMatch` productions at step 11 → run `cargo test -p fmpl-core grammar::parser` and confirm no failures. The rewritten third test must NOT depend on `Pattern::Tagged` (the unified one) — verify it parses to `Pattern::ListMatch` after the rewrite, which `grammar/parser.rs:1098` confirms is the list-pattern production target.

13. **Repair STORY-0095/AC-4 in `docs/superpowers/iterations/requirements/EPIC-032.md:21`.** Current text references the deleted `Value::Tagged` type. Rewrite to: `AC-4: Constructor-shape values are represented as Value::List(Arc<Vec<Value>>) whose first element is Value::Symbol(tag) and whose remaining elements are the children; this shape is constructed via Value::list_node(tag, children) and inspected via Value::as_node() -> Option<(&SmolStr, &[Value])> · impact:local · seam:unit · scenario:SCENARIO-0066`. (Round-6 PAR fix: precise about the Arc layout — the whole `[Symbol(tag), ...children]` is wrapped in ONE `Arc<Vec<Value>>`, not two.)

14. **Update EPIC-002.md STORY-0010 AC tags.** AC-9, AC-10, AC-12 currently lack `· scenario:` tags; this iteration's scenarios pin them:
    - AC-9: `· scenario:SCENARIO-0104, SCENARIO-0106`
    - AC-10: `· scenario:SCENARIO-0105, SCENARIO-0106`
    - AC-12: `· scenario:SCENARIO-0105, SCENARIO-0106`
    (AC-11 gets its scenario tag in ITER-0004d.2; AC-14 is covered by the CI gate.)

15. **Reconcile scenarios in `behavior-scenarios.md`:**
    - **Rewrite SCENARIO-0039** (currently uses `:int(n)` value-pattern syntax in grammar definitions): translate to list-pattern syntax `[:int, any:n]`. The test stays a tree-grammar-constant-folding test; the syntax updates. Preserves STORY-0057/0054/0053 owning stories.
    - **Rewrite SCENARIO-0066** per scope item 13 (drop `Value::Tagged` references; assert list-node shape via `Value::as_node()`).
    - **Add SCENARIO-0104** — parser rejects `:Tag(args)` value-constructor syntax at the FMPL source surface. Observable A: `parse(":Foo(1, 2)")` (FMPL parser) returns `Err(Error::Parser { .. })` with a message containing the substring `Foo` and the substring `Use list-pattern syntax`. Observable B: bare `:foo` symbol literals continue to parse as `Expr::Symbol`. Observable C (grammar-DSL surface, round-5 PAR addition): parsing a grammar source string containing a `:Tag(args)` value-match pattern returns a parser-level error from `GrammarParser::new(source).parse()` (entry point at `fmpl-core/src/grammar/parser.rs:27, 32`; round-6 PAR corrected the misnamed `parse_grammar` reference). The grammar-DSL error message is NOT required to match the FMPL surface's message — only that an error is returned (the productions at `grammar/parser.rs:899, 1136, 1333` are deleted, so the parse fails by missing-production rather than by explicit-rejection). The test asserts non-empty error, not error-message content, for observable C.
    - **Add SCENARIO-0105** — parser rejects `:Tag(p1, p2)` pattern syntax at the FMPL source surface. Observable A: in `match x { :Foo(a, b) => 1 }` source, the embedded `:Foo(a, b)` pattern triggers `Err(Error::Parser { .. })` whose message contains the substring `Foo` and the substring `Use list-pattern syntax`. Observable B: `match x { :foo => 1 }` (bare symbol pattern) continues to parse as `Pattern::Symbol`-equivalent (i.e., a symbol-literal pattern, not a constructor pattern).
    - **Add SCENARIO-0106** — Rust-side greppable invariant. **Round-5/6 PAR fix:** the originally-drafted qualified-path grep (`ast::Pattern::Constructor` etc.) was broken because the code uses unqualified `Pattern::Constructor` / `Pattern::Tagged` / `Pattern::TagMatch` with `use ... as UP` / `use ... as GP` / `use ... as GrammarPattern` aliases. Round-6 also caught that POSIX `grep` is line-oriented and cannot match across-newline patterns. Correct invariants (use `rg --multiline` OR `grep -P` with PCRE-style `(?s)` modes for any cross-newline pattern):
        1. `grep -rnE '\bExpr::Tagged\b' fmpl-core/src/` returns no matches.
        2. `grep -rnE '^\s*Constructor\(SmolStr,\s*Vec<Pattern>\),' fmpl-core/src/ast.rs` returns no matches (single-line variant definition; locks AC-10's `ast::Pattern::Constructor` deletion).
        3. `grep -rnE '^\s*Tagged\s*\{\s*$' fmpl-core/src/pattern/mod.rs` returns no matches — the variant definition opens with `Tagged {` on its own line at `pattern/mod.rs:58`. Single-line anchor avoids the multi-line POSIX grep issue. Locks AC-10/12's `pattern::Pattern::Tagged` deletion.
        4. `grep -rnE 'TagMatch\(SmolStr,\s*Vec<Pattern>\)' fmpl-core/src/pattern/mod.rs` returns no matches (single-line variant definition at `pattern/mod.rs:143`).
        5. `grep -rnE '\bPattern::Constructor\b' fmpl-core/src/` returns no matches (no remaining consumer call sites for `ast::Pattern::Constructor`).
        6. `grep -rnE '\bUP::Tagged\b|\bGP::Tagged\b|\bGrammarPattern::Tagged\b|\bPattern::Tagged\s*\{' fmpl-core/src/` returns no matches — covers all aliases (`UP` in compiler.rs, `GP` in compile_grammar_pattern, `GrammarPattern` in repr.rs).
        7. `grep -rnE '\bUP::TagMatch\b|\bGP::TagMatch\b|\bGrammarPattern::TagMatch\b|\bPattern::TagMatch\b' fmpl-core/src/` returns no matches — same alias coverage.
        Automate via a Rust test that runs the seven grep commands and asserts each is empty. **Validation gate (round-6 PAR addition):** before this iteration starts, run each grep against the current source and confirm it returns at least one match TODAY. If any returns zero pre-deletion, the grep is broken and provides false confidence — fix the grep before proceeding.

16. **Flip the `no_legacy_fmpl_syntax.rs` CI gate** from "asserts against baseline" to "asserts `== 0`". This requires TWO edits:
    (a) Edit `fmpl-core/tests/no_legacy_fmpl_syntax.rs` to remove the baseline-loading code path and replace it with `assert_eq!(total_hits, 0, ...)`. Confirm the test still honors the allowlist for non-syntax `name:first (...)` bindings noted in the ITER-0004d.0 scope.
    (b) After (a) compiles and the test passes, delete the `no_legacy_fmpl_syntax.baseline.json` file. Do NOT delete the JSON file before (a) — the existing baseline-reading code path would crash on missing file before the test logic could assert.

**Verification:**
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` returns 0 hits across all four surfaces (permanent CI sentinel; locks AC-9, AC-10, AC-13).
- SCENARIO-0106's seven greps all return zero matches (see scope item 15 for the corrected pattern list).
- SCENARIO-0104, SCENARIO-0105 pass (parse rejection + bare-symbol preservation).
- SCENARIO-0103 + SCENARIO-0016 still pass (parity sentinels).
- Full workspace test suite passes (a few `Instruction::MakeTagged` / `MatchTagged` references in compiler.rs, vm.rs, ir_builder.rs, and builtins/ir.rs survive — they are renamed in ITER-0004d.2).
- `fmpl-core/tests/fmpl/{ast_to_ir,fmpl_parser}.fmpl` no longer present on disk.
- EPIC-032.md:21 STORY-0095/AC-4 rewritten.
- EPIC-002.md AC-9, AC-10, AC-12 have `· scenario:` tags.
- Explicit-rejection observable from binding precondition: `parse(":Foo(1, 2)")` returns `Error::Parser` whose message contains the substring `Foo` and the substring `Use list-pattern syntax`. Same observable holds at the pattern surface for `parse_pattern(":Foo(1, 2)")`.

**AC-14 scope clarification (round-5 PAR fix):** AC-14 (EPIC-002.md:134) says "All Rust tests use `Value::list_node` for construction and `value.as_node()` for shape assertions — no `Value::Tagged(...)` literals remain in test code". This was already satisfied by ITER-0004b's runtime burn (verified: `grep -rn 'Value::Tagged(' fmpl-core/` returns 0 in test code as of 2026-05-10). This iteration does NOT re-do AC-14. What scope items 3 + 4 sweep is FMPL-source-string `:Tag(args)` *syntax* inside `r#"..."#` literals — that is the test-side coverage of AC-9, AC-10, and AC-13, NOT AC-14. The two sweeps are disjoint: AC-14 was a Rust-level `Value::Tagged` rewrite; AC-9/10/13 sweep is FMPL-source `:Tag(args)` rewrite.

**Out of scope (deferred to ITER-0004d.2):** AC-11 bytecode opcode rename. ITER-0004d.2's "qualified-path sweep" handles `Instruction::MatchTagged` → `Instruction::MatchListNode` in Rust assertion code, which is NOT a `:Tag(args)` syntax issue.

**Out of scope (other iterations):** `Type::Tagged` cleanup → ITER-0004h. Removing `FMPL_USE_FMPL_COMPILER` opt-in.

### ITER-0004d.2 — Bytecode Opcode Rename (AC-11)

**Stories:** STORY-0010 Phase B AC-11. Adds SCENARIO-0107.

**Rationale:** With the AST/Pattern surfaces deleted (ITER-0004d.1), the tagged-bytecode opcodes have only emit sites in dead/about-to-be-deleted code. This iteration renames the surviving list-node-shaped opcodes for clarity (`MakeTagged` → `MakeListNode`, etc.) and sweeps qualified-path references in Rust test files. **`MatchTag` is preserved unchanged** — it backs `Pattern::Symbol` matching which AC-9 explicitly preserves.

**Status:** pending
**Depends on:** ITER-0004d.1 (AST/Pattern variants deleted; consumer arms in compiler.rs deleted with them; the remaining emit sites for the renamed opcodes are limited and traceable).
**Look-ahead check:** Unblocks ITER-0005 (persistence: the renamed opcodes are what gets serialized to Fjall via `CompiledCode::save_to_fjall`). Forward-compatibility: Serde wire-format preservation handled via `#[serde(rename = ...)]` attributes (binding precondition below).

**Binding preconditions:**

- **Rename map** (verified 2026-05-10 against `fmpl-core/src/compiler.rs` and `fmpl-core/src/vm.rs`):
  - `Instruction::MakeTagged { tag, args }` → `Instruction::MakeListNode { tag, args }`. Definition: `compiler.rs:260`. VM handler: `vm.rs:877`. Emit sites: `compiler.rs:874` (post-ITER-0004d.1 this arm may be deleted; verify), `ir_builder.rs:238-240`, plus `builtins/ir.rs:336` string-to-instruction lookup (`"MakeTagged"` → `"MakeListNode"`).
  - `Instruction::ExtractTaggedChild { source, index }` → `Instruction::ExtractListChild { source, index }`. Definition: `compiler.rs:363`. VM handler: `vm.rs:1182`. Emit sites (post-ITER-0004d.1): `compiler.rs:2965, 3117, 3342` (verify reachability after AC-10 deletes Pattern::Constructor arms) and `builtins/ir.rs:776` (separate code path, reachable independently).
  - `Instruction::MatchTagged { tag_idx, patterns }` → `Instruction::MatchListNode { tag_idx, patterns }`. Definition: `compiler.rs:505`. VM handler: `vm.rs:2567` (Reviewer A round 2 caught that the original plan omitted this line). Emit sites: `compiler.rs:3346, 3809` and possibly `:4389` — verify which survive ITER-0004d.1.
  - `Instruction::MatchTaggedWithBindings { tag_idx, bindings }` → `Instruction::MatchListNodeWithBindings { tag_idx, bindings }`. Definition: `compiler.rs:510`. VM handler: `vm.rs:2521`. Emit sites: `compiler.rs:4380` (verify survives ITER-0004d.1). Cross-reference: `vm.rs:2609` is inside `MatchTagged`'s nested dispatch, not an emit site — it's renamed if the surrounding match arm is renamed.
  - `Instruction::MatchTag { tag_idx, fail_target, expected_arity }` — **PRESERVED unchanged**. Definition: `compiler.rs:369`. VM handler: `vm.rs:1204` (single handler dispatches both `expected_arity: None` for Pattern::Symbol and `expected_arity: Some(n)` for Pattern::Constructor — but post-ITER-0004d.1, only the `None` case has live emit sites because Pattern::Constructor is deleted). Emit sites post-ITER-0004d.1: `compiler.rs:2533, 2665` (the `Pattern::Symbol`-emitting sites; `:2542, 2654` are deleted with Pattern::Constructor arms).

- **Serde wire-format compatibility.** `Instruction` derives `Serialize, Deserialize` (verify via `fmpl-core/src/compiler.rs` enum-attribute lines). `serde_json::to_vec(self)` serializes variants by **variant name**, so renaming changes the wire format. Two options:
  - **Option A: bump format version, drop legacy names.** Cleanest. Requires ITER-0005's STORY-0099 versioned envelope to handle the migration. Doesn't apply here because ITER-0005 hasn't shipped.
  - **Option B (binding): preserve wire-format compat via `#[serde(rename = "MakeTagged")]`** on each renamed variant. Wire format stays identical; Rust code uses the new names. The rename map above is implemented purely as a Rust-level cosmetic rename plus a `serde(rename)` attribute per variant. No persisted-bytecode migration needed.
  - **Decision: Option B.** Defer wire-format breakage to ITER-0005's envelope. Document the `#[serde(rename)]` attributes as the explicit forward-compat surface; ITER-0005 may choose to drop them when bumping the envelope version.

- **MatchTag inclusion check.** `compiler.rs:369` and `vm.rs:1204` MUST NOT appear in the rename map's edit targets. Round-2 PAR caught these as accidentally-included in an earlier draft.

**Scope:**

1. **Rename variants in `compiler.rs:260, 363, 505, 510`** (NOT line 369, which is `MatchTag`). Add `#[serde(rename = "MakeTagged")]` etc. to each renamed variant.

2. **Rename VM handler arms in `vm.rs:877, 1182, 2567, 2521`** (NOT line 1204, which is `MatchTag`). Verify with `grep -nE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)' fmpl-core/src/vm.rs` returns no matches after edit.

3. **Rename emit sites in `compiler.rs`** for whichever sites survived ITER-0004d.1. Run `grep -nE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)' fmpl-core/src/compiler.rs` at iteration start to enumerate. Expect 3-6 surviving sites.

4. **Rename emit sites in `ir_builder.rs:238-240`** for `MakeTagged` → `MakeListNode`.

5. **Rename `builtins/ir.rs:336`** string lookup (`"MakeTagged"` → `"MakeListNode"`). Verify other tagged-opcode string lookups in the same dispatch table get renamed too.

6. **Rename emit site in `builtins/ir.rs:776`** for `ExtractTaggedChild` → `ExtractListChild`.

7. **Sweep Rust test qualified-path references.** Round-2 PAR enumerated:
   - `fmpl-core/tests/context_aware_compilation.rs` — 10 matches at lines 4, 105, 108, 112, 115, 118, 121, 347, 350, 353 (using `Instruction::MatchTagged` / `Instruction::ExtractTaggedChild` in `matches!()` macros).
   - `fmpl-core/tests/stream_coercion.rs` — verify legacy opcode references.
   - Other files: run `grep -lE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)\b' fmpl-core/tests/` at iteration start to enumerate authoritatively. Round-2 ground truth: `grep -rn 'MakeTagged\|MatchTagged\|ExtractTaggedChild' src/ tests/` returns 60+ references; enumerate before sweeping.

8. **Update `lib/core/ast_to_ir.fmpl`** if any rule still references `:MakeTagged` as an emit target. (ITER-0004d.1 already deleted the `[:Tagged, ..., => [:MakeTagged, tag, xs]` rule at line 21.) Verify by re-scanning.

9. **Add SCENARIO-0107** — bytecode-opcode invariant: 
   - `grep -rnE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)\b' fmpl-core/src/` returns no matches (legacy gone).
   - `grep -rnE 'Instruction::(MakeListNode|MatchListNode|MatchListNodeWithBindings|ExtractListChild)\b' fmpl-core/src/` returns matches (new names present).
   - `grep -nE 'Instruction::MatchTag\b' fmpl-core/src/compiler.rs` returns matches at lines 369 (definition), 2533, 2665 (Pattern::Symbol emit sites; post-ITER-0004d.1).
   - **Behavioral observable (not just structural):** SCENARIO-0103 + SCENARIO-0016 (sentinels) still pass — verifies that the rename preserves semantics. Round-2 PAR flagged that a grep-only observable wouldn't catch a VM handler dispatch bug; the sentinel pass is the behavioral check.

10. **Update EPIC-002.md STORY-0010 AC-11** to add `· scenario:SCENARIO-0107`.

**Verification:**
- `cargo test --workspace` passes.
- SCENARIO-0103 + SCENARIO-0016 still pass (behavioral assurance for the rename).
- SCENARIO-0107 passes (structural invariant).
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` still returns 0 (preserved from ITER-0004d.1).
- `grep -rnE 'Instruction::MakeTagged\b' fmpl-core/` returns 0 matches (sanity).
- Persisted bytecode round-trip test (whichever currently exists, e.g., `bytecode_persistence.rs`) still passes — Serde wire-format unchanged due to `#[serde(rename)]` attributes.

**Out of scope:** `Type::Tagged` cleanup (ITER-0004h). Wire-format version bump (ITER-0005's STORY-0099).

### ITER-0004d.3 — Bootstrap-Parse Investigation + `no_legacy_fmpl_syntax` Gate Flip (T18)

**Stories:** STORY-0010 Phase B AC-9/AC-10/AC-12 final CI-gate ratchet — the deferred T18 from ITER-0004d.1.

**Rationale:** ITER-0004d.1 deferred the `no_legacy_fmpl_syntax` gate flip to `== 0` mode because the metacircular pipeline is currently broken: `fmpl-bootstrap lib/core/parser_generator.fmpl` fails with `Parser error at token 20: expected Comma` when three or more `io::load(...)` calls are chained. The failure reproduces with the parent-commit source (it pre-dates the T-task work). Test sentinels stay green because `FMPL_SKIP_PARSER_GEN=1` falls back to the legacy parser, but a green `== 0` gate would silently mean "the fallback parser doesn't see legacy syntax", not "the metacircular pipeline doesn't see legacy syntax". DESIGN-001 (metacircular bootstrap) requires the latter.

**Status:** done 2026-05-12.

T1 reproduced and identified the root cause: `is_inline_pattern_block` heuristic at `parser.rs:1089-1182` misclassified `g @ { [:Tag, any:name, ...] => ... }` blocks as AST inline pattern blocks because it only inspected 3-4 tokens. The "three-load failure" label was incidental — it was actually a content-based misclassification that surfaced on the third load (the file containing `any:name` patterns). T1a discovered all 30 residual `no_legacy_fmpl_syntax` hits originated from Rust doc comments (`///` / `//!` desugar to `#[doc = "..."]` LitStr nodes that syn visits). T3 fixed `is_inline_pattern_block` (added `contains_grammar_bind_in_outer_brackets` helper detecting `Ident Symbol` adjacency inside brackets). T4 added `from_doc_attr: bool` to `SourceKind::RustString` and suppressed doc-attr origin hits in the gate. T5 (eliminate residual hits) was subsumed by T4's doc-attr fix — hit counts went 30→0 directly. T5a was a no-op (the ALLOWLIST entries cover non-doc-attr grammar-DSL sites in actual `.fmpl` files, still load-bearing). T6 flipped the gate to `== 0` mode and deleted the baseline JSON. T7a added SCENARIO-0108 + `canonical_pipeline_parity.rs` evidence tests (per the PAR-revised scope). T7b discovered via SCENARIO-0108's first run that the FMPL stdlib parser silently accepted `:Foo(1)` while the source-tree parser rejected it — fixed by adding a `legacy_tagged_ctor` rule to `lib/core/fmpl_parser.fmpl` using a poison-AST-node pattern (the FMPL grammar runtime lacks a `fail()` primitive — that limitation is a documented follow-up).

**Impacted scenarios:** SCENARIO-0104, SCENARIO-0105, SCENARIO-0106 re-pass under the canonical metacircular pipeline. `no_legacy_fmpl_syntax` is now in `== 0` mode (baseline JSON deleted). SCENARIO-0108 NEW provides positive evidence that the canonical pipeline is behaviorally equivalent to the source-tree parser.

**Depends on:** ITER-0004d.1 (the T-task variant deletions; the parser-rejection contract scenarios; the parser-epoch freshness signal that would catch a regressing generator).

**Look-ahead check:** Unblocks any iteration whose proof obligations include "via the canonical pipeline" (currently only the metacircular sentinels, which are passing via fallback today). Unblocks confidently flipping the gate, which downstream iterations rely on as the syntactic-cleanliness signal.

**Binding preconditions:**

- **Bisect the three-load failure.** From progress.md session 1: a single `io::load(...)` works; two consecutive loads work; three consecutive loads fail with `expected Comma`. Reproduce against parent-commit source (confirmed pre-existing); confirm it's not a parser-epoch staleness issue (the epoch system is dormant under fallback — should not affect bootstrap). Likely candidates: cross-file scope leak; bootstrap parser bug in the third load; side-effect ordering in `io::load`'s evaluation.
- **Root-cause fix, not symptomatic.** Per CLAUDE.md `investigate` skill discipline — reproduce in a minimal failing case first, then identify the line/expression that triggers the failure, then fix the underlying mechanism. Workarounds that mask the problem (e.g., merging the three grammar files into one) do NOT close this iteration.
- **Inspect the 4 + 26 residual hits BEFORE designing the scanner discriminator (PAR-revised).** The original roadmap description called these "`module:function(args)`" patterns and proposed a single-token lookback that checks for `Ident COLON Symbol LParen`. PAR-B raised a real concern: if the FMPL lexer tokenizes `module:function(args)` as `Ident COLON Ident LParen` (not `... Symbol LParen`), the existing scanner could not be firing on them, and the description has the wrong direction. Step 1a-NEW below mandates dumping the actual hit contents first; the discriminator design follows from observation, not speculation.
- **No new allowlist entries (PAR-revised).** The existing two entries (`fmpl_parser.fmpl:first`, `fmpl_grammar.fmpl:first`) cover documented grammar-DSL binding sites. After the scanner refinement, both entries should become unreachable (the scanner stops flagging those sites without an allowlist). The iteration MUST NOT add any new allowlist entries; if a hit cannot be cleared by the discriminator, the iteration scope re-opens before flipping the gate.
- **Grammar-runtime fix consequence: PARSER_EPOCH bump (PAR-revised).** If step 2's bisect lands the root cause in `fmpl-core/src/grammar/runtime.rs` (e.g., scope leak in `io::load` evaluation), the fix changes value-encoding or grammar-action semantics per the parser-epoch bump policy in `src/parser_epoch.rs:21`. In that case the iteration scope explicitly includes: bump `PARSER_EPOCH`, complete one full bootstrap-rebuild loop, and verify the regenerated parser passes the canonical-pipeline sentinel (step 7a-NEW below) before flipping the gate.
- **Canonical-pipeline scenario (PAR-revised).** Both reviewers flagged a real coverage gap: no existing sentinel routes through `parser::generated_parse`. The "all sentinels pass without `FMPL_SKIP_PARSER_GEN=1`" claim is weaker than implied because `ast_to_ir_parity` explicitly calls `eval_via_legacy_parser`, `scenario_0103` uses `eval_via_fmpl_pipeline` which internally uses the legacy parser, and `structural_invariants.rs` uses `Parser::with_source`. **SCENARIO-0108 NEW** is added to this iteration: assert that the generated parser produces the same parse result (rejection-equivalent for SCENARIO-0104/0105 inputs; identical AST for a representative successful input from `ast_to_ir_parity`) as the source-tree Rust parser. Evidence: a new test file `fmpl-core/tests/canonical_pipeline_parity.rs` that calls `Parser::with_source` and `generated_parse` against the same inputs and asserts equality.

**Scope:**

1. **Reproduce the three-load failure** in a minimal test harness (`fmpl-bootstrap` invocation with a synthetic 3-load file). Confirm it's not env-dependent.
1a. **Dump the 4 + 26 residual hits (PAR-revised).** Print the actual file:line + surrounding text + tag value for each hit. Categorize by token-sequence (which lexer token sequence produced the Symbol+LParen). This is observation work; the discriminator design follows from what's actually present, not from speculation about `module:function(args)`.
2. **Bisect the failure mechanism.** Compare 2-load (works) vs 3-load (fails) parser state. Determine whether the issue is in the bootstrap-binary parser, the grammar runtime, or the `io::load` builtin's evaluation order.
3. **Land the fix.** Add a unit test for the formerly-failing minimal case.
3a. **If step 2 identified `grammar/runtime.rs` or similar value-encoding change as the root cause (PAR-revised):** bump `PARSER_EPOCH` per `src/parser_epoch.rs` policy. Run the bootstrap rebuild loop. Verify `cargo check -p fmpl-core` succeeds with the regenerated parser (i.e., the parser-epoch assertion at compile time passes).
4. **Refine the legacy-syntax scanner** based on step 1a's observations. Add unit tests in `diagnostics_fmpl_source_scan.rs` for both the surviving form (kept as hit) and the false-positive form (now cleared).
5. **Eliminate the residual hits.** With the refined scanner, the 4 + 26 should drop to 0. **The "near-zero with a small allowlist" hedge from the previous scope is removed — no new allowlist entries are permitted (binding precondition above).** If hits remain that the scanner cannot cleanly classify, stop and revise scope before continuing.
5a. **Remove now-redundant allowlist entries (PAR-revised).** After the scanner correctly classifies grammar-DSL binding sites, the two existing entries (`fmpl_parser.fmpl:first`, `fmpl_grammar.fmpl:first`) become dead code. Remove them. Re-run the gate to confirm hits remain at 0 without the allowlist.
6. **Flip the gate.** Edit `fmpl-core/tests/no_legacy_fmpl_syntax.rs` to drop the baseline-JSON loading and assert `total_hits == 0`. Delete `fmpl-core/tests/no_legacy_fmpl_syntax.baseline.json`. Remove the `FMPL_REGEN_BASELINE` regen path (it is no longer meaningful in `== 0` mode). Update the file's module docs.
7. **Re-run all sentinels** under the canonical pipeline (no `FMPL_SKIP_PARSER_GEN=1` env var, fresh bootstrap rebuild). All scenarios must pass. **NOTE (PAR-revised):** this only verifies the canonical generator runs to completion — it does NOT prove the generated parser is behaviorally equivalent to the fallback. That proof comes from step 7a.
7a. **SCENARIO-0108 evidence: canonical-pipeline parity (PAR-revised, NEW).** Add `fmpl-core/tests/canonical_pipeline_parity.rs`:
    - For each SCENARIO-0104 / SCENARIO-0105 rejection input: call `Parser::with_source(...).parse()` AND `parser::generated_parse(...)`. Assert both return `Err`. Assert the error messages contain the same canonical-form hint substring (`use [:`).
    - For a representative successful input (`1 + 2` from `ast_to_ir_parity` baseline): call both parsers; assert the produced `Expr` trees are equal under `PartialEq`.
    - This is the proof that the metacircular pipeline produces semantically-equivalent output to the source-tree parser, which is what DESIGN-001 requires and what scope step 7's "all sentinels pass" alone does NOT prove.

**Acceptance (PAR-revised — split into syntactic vs behavioral gates):**

*Syntactic-cleanliness gate (the `no_legacy_fmpl_syntax == 0` proof):*
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` passes in `== 0` mode (baseline JSON deleted; regen path removed).
- The allowlist has zero entries (or only documented exceptions explicitly approved during this iteration).
- The scanner has unit-test coverage for the false-positive forms identified in step 1a.

*Behavioral-correctness gate (the canonical-pipeline proof — distinct from the syntactic gate):*
- `fmpl-bootstrap lib/core/parser_generator.fmpl` succeeds end-to-end (no env-skip; the canonical generator regenerates `out/generated_parser.rs`).
- The minimal-reproducer test for the three-load case passes.
- `cargo test -p fmpl-core --test canonical_pipeline_parity` passes (SCENARIO-0108 evidence).
- All sentinels still green WITHOUT `FMPL_SKIP_PARSER_GEN=1`: ast_to_ir_parity, scenario_0103_optimizer_pipeline, tavern_demo, structural_invariants, no_legacy_fmpl_syntax, canonical_pipeline_parity.
- If a PARSER_EPOCH bump was needed (step 3a), the regenerated parser's embedded `GENERATED_PARSER_EPOCH` matches the new source `PARSER_EPOCH` value.

**Out of scope:** ITER-0004d.2's opcode rename (separately scheduled). The data-driven scenario runner (ITER-0004d.4 below). The bootstrap-parse fix is bounded to the three-load issue; broader bootstrap refactoring is not in scope. Migrating SCENARIO-0108 into the scenario-runner card format (deferred to ITER-0004d.4 along with the others).

### ITER-0004d.3a — SCENARIO-0108 audit-fix-up (G1+G2+G3)

**Stories:** STORY-0010 Phase B AC-9/AC-10/AC-12 evidence strengthening. Direct response to ITER-0004d.3's three-tier audit findings.

**Rationale:** The ITER-0004d.3 audit (PAR, two parallel auditors) returned GAPS FOUND with three high-confidence findings — two CRITICAL (raised by BOTH auditors) and one SERIOUS (raised by both). The findings are bounded and concrete; rather than ship ITER-0004d.3 with known gaps or fold them into the unrelated ITER-0004d.2 (opcode rename), this fix-up iteration closes the gaps surgically.

**Status:** done 2026-05-12. G1 added `IS_GENERATED_PARSER: bool` to both real and fallback parser binaries + `canonical_pipeline_must_be_active` assertion test (PARSER_EPOCH 3→4). G2 strengthened the two remaining SCENARIO-0108 rejection tests with `use [:` hint assertions and added `pat_legacy_tagged_ctor` rule + `PatternLegacyTagCtor` postlude arm (PARSER_EPOCH 4→5). G3 added structural grep `scenario_0106_grep_8_legacy_tag_ctor_coupling` (asserts both magic strings in both files) + isolated postlude test `g3_postlude_arms_fire_on_poison_nodes`. Focused re-audit caught a residual gap (G3 behavioral test lacked the IS_GENERATED_PARSER guard, making it trivially passable under fallback — exactly the failure mode G1 prevented). Fixed by adding the guard. Final sentinel sweep: 140 passed, 3 ignored across 7 suites (+3 net tests vs end-of-ITER-0004d.3 baseline; 0 regressions).

**The three gaps:**

- **G1 (CRITICAL).** SCENARIO-0108's `canonical_pipeline_parity` tests are unfalsifiable when the fallback parser is active. The fallback's `generated_parse` delegates to `Parser::with_source(...).parse()` — identical to the source-tree path — so all 7 tests pass trivially in fallback mode. The test designed to catch fallback substitution cannot itself detect it. The iteration's manual verification doesn't survive into CI.
- **G2 (CRITICAL).** SCENARIO-0108's contract states "both error messages MUST contain `use [:`" for all three rejection inputs. Only `parity_rejects_value_constructor_single_arg` actually checks this; the other two only assert `is_err()`. The pattern-position case can't satisfy the hint requirement at all — `legacy_tagged_ctor` is only in `primary` (expression position), not `pat_primary`. The canonical parser rejects pattern-position `:Tag(a, b)` via a generic syntactic mismatch, not the canonical-form hint.
- **G3 (SERIOUS).** The T7b workaround couples a magic string ("LegacyTagCtor") between `lib/core/fmpl_parser.fmpl` (the grammar rule's emitted tag) and `fmpl-core/src/builtins/ir_to_rust.rs` (the postlude match arm). A rename in one without the other silently breaks the rejection. No isolated unit test exercises the postlude arm directly; no structural invariant catches the name-coupling.

**Impacted scenarios:** SCENARIO-0108 evidence strengthened (no new scenario added; the existing scenario gets tighter tests). No other scenarios change.

**Depends on:** ITER-0004d.3 (the surface that needs strengthening).

**Look-ahead check:** ITER-0004d.2 (opcode rename) is unaffected. The G2 pattern-position rejection edit to `pat_primary` may interact with ITER-0004d.4's scenario-runner if that ever lands — the runner would presumably check `legacy_tagged_ctor` rejection via the scenario-card format, and the pattern-position case becoming a real rejection (not generic mismatch) makes the card more cleanly expressible.

**Scope:**

1. **G1 fix.** Add `pub const IS_GENERATED_PARSER: bool` to both the real generated parser (emitted by `fmpl-core/src/builtins/ir_to_rust.rs` postlude → set to `true`) and the fallback parser (emitted by `fmpl-core/build.rs::write_fallback_parser` → set to `false`). At the top of `fmpl-core/tests/canonical_pipeline_parity.rs`, add an assertion that `fmpl_core::parser::IS_GENERATED_PARSER` is `true` with a panic message explaining how to rebuild fmpl-bootstrap and unset `FMPL_SKIP_PARSER_GEN`/`FMPL_BOOTSTRAP_PHASE`. Document the contract in the test file's module docs.

2. **G2 fix.** Two sub-edits:
    - Add `assert!(st_msg.contains("use [:"))` and `assert!(cn_msg.contains("use [:"))` to `parity_rejects_value_constructor_multi_arg` and `parity_rejects_pattern_constructor_in_match_arm`, mirroring the existing `parity_rejects_value_constructor_single_arg` pattern.
    - Add a `legacy_tagged_ctor`-equivalent to `pat_primary` in `lib/core/fmpl_parser.fmpl` so the canonical FMPL parser also rejects pattern-position `:Tag(p1, p2)` via the `LegacyTagCtor` postlude path (producing the same `use [:` hint as the source-tree parser).

3. **G3 fix.** Two sub-edits:
    - Add a unit test (in `fmpl-core/src/builtins/ir_to_rust.rs` or a sibling test) that directly invokes `value_to_expr` with `Value::list_node("LegacyTagCtor", vec![Value::Symbol("Foo".into())])` and asserts `Err(Error::Parser{...})` with message containing `use [:`. This catches a regression where the postlude arm is renamed without updating the grammar (or vice versa).
    - Add a structural invariant grep to `structural_invariants.rs` (or a similar test) that confirms the literal `"LegacyTagCtor"` appears in BOTH `lib/core/fmpl_parser.fmpl` AND `fmpl-core/src/builtins/ir_to_rust.rs`. The grep can use the existing `find_word_in_code`-style helper.

4. **Re-run sentinels.** Full sweep (canonical pipeline active, no env-skip) — expected ≥137 passed (the new G1/G2/G3 tests add net ≥3 tests; nothing removed).

5. **Focused re-audit (single round).** Confirm the three flagged gaps are closed — not a full three-tier audit, just a targeted check on G1/G2/G3.

**Acceptance:**

- `cargo test -p fmpl-core --test canonical_pipeline_parity` fails immediately with a clear error if run against the fallback parser (G1 closed).
- All three SCENARIO-0108 rejection tests assert the `use [:` hint on BOTH parsers' errors (G2 closed).
- `cargo test -p fmpl-core --test structural_invariants` includes the new name-coupling grep (G3 closed).
- A unit test directly exercises `value_to_expr` with the `LegacyTagCtor` poison node and asserts the rejection (G3 closed).
- Full sentinel sweep: ≥137 passed, 3 ignored across 7 suites, 0 regressions.
- Focused re-audit reports CLEAN on the three gap surfaces.

**Out of scope:** Anything not in G1/G2/G3. The structural_invariants.rs migration to the scenario runner (deferred to ITER-0004d.4). Opcode rename (ITER-0004d.2). Broader audit findings beyond the three gaps.

### ITER-0004d.4 — Data-Driven Scenario Runner (cucumber/SLIM-style)

**Stories:** New story TBD — register one under EPIC-002 once the iteration starts. Migration of `fmpl-core/tests/structural_invariants.rs` is the first consumer; further migration is opportunistic per iteration.

**Rationale:** User feedback from 2026-05-12 (ITER-0004d.1 T19 review): the per-scenario Rust test pattern (e.g., `scenario_0104_rejects_single_arg_value_constructor`) is stylish but makes the test file harder to read at a glance — each test mostly restates structure already in the scenario card. A cucumber/FitNesse-SLIM-style data-driven runner where the scenario card IS the test spec, and the Rust side is a thin step-definition driver, would (a) make scenario contracts directly executable, (b) collapse boilerplate, and (c) make "add a new scenario" a primarily card-authoring activity.

**Status:** pending

**Impacted scenarios:** No new scenarios; this is infrastructure. Existing scenarios `SCENARIO-0104`, `SCENARIO-0105`, `SCENARIO-0106` migrate from Rust-per-test to data-driven (their `Execution command` entries in `behavior-corpus.md` will change). Future scenarios benefit; existing free-form scenarios stay free-form unless explicitly migrated.

**Depends on:** ITER-0004d.1 (the three example scenarios that prove the pattern). Does NOT depend on ITER-0004d.3 (gate flip); they can ship in either order.

**Look-ahead check:** Could simplify ITER-0005 (persistence) and downstream iterations that have structural-invariant proof obligations. Risk: over-architecting the step-def registry too early — three scenarios is a small sample.

**Binding preconditions:**

- **Scenario card structure must be parseable.** Today `behavior-scenarios.md` is free-form markdown with conventional headings (Preconditions, Action, Expected observables, etc.). Either (a) tighten the convention so a regex/markdown parser can extract typed fields, or (b) add a sibling structured file (`behavior-scenarios.yaml` or similar) that mirrors the cards. Option (a) preserves the human-readable narrative; option (b) avoids parser fragility. Decide at iteration kickoff.
- **Step-definition registry.** A registry mapping `(action_type, expected_observable_pattern) → step_def_fn`. The registry must support both general step-defs (`parse_rejection(source: &str) -> Result<...>`) and scenario-specific ones (e.g., `grep_invariant_pattern_constructor()`). Step-def names must be discoverable from the scenario card text.
- **Thin driver test.** A single `tests/scenario_runner.rs` that iterates the scenario corpus, dispatches each card to its step-defs, and emits a `[scenario_id]` test name so `cargo test scenario_0104` still works.
- **Cucumber/SLIM conventions, not invention.** Lean on prior art: cucumber/Gherkin uses Given/When/Then; FitNesse SLIM uses tables and fixtures. Pick one model and stick to it — don't invent a third.

**Scope:**

1. **Decide scenario card structure.** Either tighten markdown conventions or add a structured sibling file. Land the decision in `docs/design-principles.md` (DESIGN-006 or similar).
2. **Parse the corpus.** Walk `behavior-scenarios.md`, extract per-scenario `(id, kind, seam, preconditions, action, expected_observables, execution_status)`. Validate on the existing corpus.
3. **Build the step-definition registry.** Start with the three step types needed by SCENARIO-0104/0105/0106: `parse_rejection`, `parse_success_control`, `grep_invariant`. Make the registry extensible.
4. **Write the driver test.** `fmpl-core/tests/scenario_runner.rs` invokes the registry per card.
5. **Migrate SCENARIO-0104/0105/0106 from `structural_invariants.rs` to the driver.** Verify the same 17 evidence tests pass. Delete `structural_invariants.rs` once the driver covers it (or keep it as a legacy bridge for one iteration).
6. **Document the step-def API** so future scenario authors can add cards without touching Rust until a genuinely new action type appears.

**Acceptance:**

- `cargo test -p fmpl-core --test scenario_runner scenario_0104` runs and passes (same evidence as `structural_invariants.rs` produces today).
- Same for SCENARIO-0105, SCENARIO-0106.
- Adding a new scenario card with existing step-defs requires zero Rust code changes.
- `behavior-corpus.md` execution commands for migrated scenarios point at `scenario_runner` instead of `structural_invariants`.

**Out of scope:** Migrating scenarios that have no step-def coverage today (e.g., SCENARIO-0001..0077 which are largely TBD). FMPL-grammar-based scenario parsing (a possible ITER-0005x successor — metacircular scenario evaluation is on-brand with DESIGN-001 but bigger scope than this iteration).

### ITER-0004e — Prelude / Parser-Helper Split

**Stories:** No new STORY-0010 ACs — this iteration is structural cleanup with its own scenario evidence. Created 2026-05-10 as a deferred carve-out from ITER-0004c (PAR round 2, both reviewers flagged ITER-0004c as plausibly two iterations' worth; user directed splitting the relocation work into its own iteration).

**Rationale:** `lib/core/prelude.fmpl` should be like Haskell's Prelude — the **minimal set of definitions required to use the higher-level FMPL language, expressible directly and cleanly in FMPL** — not a bootstrap dump-ground. Today it carries 6 helpers (`fold_binary`, `fold_index`, `fold_postfix`, `fold_pipe_at`, `binary_op_to_ir`, `unary_op_to_ir`) that exist solely to support bootstrap parser grammar actions, encoding compiler-specific AST/IR node shapes (`:Binary`, `:If`, `:Add`, `:Neg`, etc.). Their consumers are exclusively `lib/core/fmpl_parser.fmpl`, `fmpl-core/tests/fmpl/fmpl_parser.fmpl`, `fmpl-core/tests/core_prelude.rs`, and Rust mirrors at `fmpl-core/src/value_to_ast.rs:179` and `fmpl-core/src/builtins/ir_to_rust.rs:134-218`. Relocating these into `lib/core/parser_helpers.fmpl` (a) eliminates conceptual coupling between `prelude` and bootstrap-parser internals, (b) leaves `prelude.fmpl` containing only general-purpose helpers (`reduce`, `join`, `to_int`, `map_list`, `prepend`, `symbol`, `digit_grammar`), (c) prepares for ITER-0006 self-compile, where prelude semantics become a user-facing surface.

**Status:** pending
**Depends on:** ITER-0004c (so the helpers being relocated are already in canonical list-pattern syntax — avoids migration churn during the move).
**Look-ahead check:** Unblocks future stdlib growth where third-party FMPL programs would naturally `use` prelude without inheriting compiler-internal AST helpers.

**Files in scope:**
- `lib/core/prelude.fmpl` — remove 6 helper definitions (and their doc comments). Add header comment establishing the binding contract: "Minimal high-level FMPL vocabulary; no compiler-internal AST/IR helpers — those belong in `parser_helpers.fmpl` or another bootstrap-specific file."
- `lib/core/parser_helpers.fmpl` (NEW) — receives the 6 helpers + doc comments verbatim.
- `fmpl-core/src/lib.rs` (bootstrap loader) — insert `parser_helpers.fmpl` load call between the prelude load and the `fmpl_parser.fmpl` initialization. The load mechanism MUST mirror the existing `prelude.fmpl` pattern — top-level `let` bindings inside the loaded file are global-bound by the FMPL VM, so no outer `let parser_helpers = ...` wrapper is needed (the helpers self-bind as global names). Verify by reading `lib/core/prelude.fmpl`'s loader call at `fmpl-core/src/lib.rs:121` and copying that pattern.
- `fmpl-core/tests/core_prelude.rs` — modify the `load_prelude(vm)` setup helper at line 11-14 to ALSO load `parser_helpers.fmpl`. This single change benefits all tests in the file (the 6 fold-helper tests AND the ~100 `test_fmpl_parser_*` tests that transitively depend on the helpers via `fmpl_parser.fmpl` consumption). Do NOT split tests into a new file — keeping them in `core_prelude.rs` minimizes rewrite.

**Out of scope (for `fmpl_parser.fmpl`):** Do NOT add `io::load("lib/core/parser_helpers.fmpl")` inside `fmpl_parser.fmpl`. The bootstrap-loader-driven pattern in `lib.rs` is the existing convention; an in-file `io::load` would be inconsistent and would create new dependency surfaces ITER-0005/ITER-0006 would need to know about.

**Scope:**

1. Read `lib/core/prelude.fmpl` and `lib/core/fmpl_parser.fmpl` post-ITER-0004c-migration (so the file state is canonical list-pattern syntax) to confirm the helpers' final shape.
2. Create `lib/core/parser_helpers.fmpl`. Header doc-comment: "Bootstrap parser grammar-action helpers; not for general FMPL programs. These functions encode compiler-specific AST/IR node shapes and are consumed only by `lib/core/fmpl_parser.fmpl` and Rust mirrors at `fmpl-core/src/value_to_ast.rs` / `fmpl-core/src/builtins/ir_to_rust.rs`."
3. Move `fold_binary`, `fold_index`, `fold_postfix`, `fold_pipe_at`, `binary_op_to_ir`, `unary_op_to_ir` (and their preceding doc-comments) from `prelude.fmpl` to `parser_helpers.fmpl`. Delete the corresponding lines from `prelude.fmpl`.
4. Add a header doc-comment to `lib/core/prelude.fmpl` codifying the contract: "Minimal high-level FMPL vocabulary; no compiler-internal AST/IR helpers — those belong in `parser_helpers.fmpl` or another bootstrap-specific file. Subsequent stdlib edits should honor this contract."
5. Update `fmpl-core/src/lib.rs` bootstrap loader: insert a `parser_helpers.fmpl` load call between the existing prelude load and the `ast_to_ir.fmpl` load (or before the `fmpl_parser.fmpl` load if the parser ever gains its own bootstrap line). Use the same form as the prelude load — no outer `let` wrapper.
6. Update `fmpl-core/tests/core_prelude.rs` `load_prelude(vm)` helper to load `parser_helpers.fmpl` after `prelude.fmpl`.
7. Run `cargo test --workspace` and verify all tests still pass. Specifically the 6 fold-helper tests and the ~100 `test_fmpl_parser_*` tests must continue passing.
8. **Scenario evidence:** Add a single small test (or extend a sentinel) that asserts `prelude.fmpl` no longer references AST/IR-shape constructors. Mechanical observable: `grep -E '(fold_binary|fold_index|fold_postfix|fold_pipe_at|binary_op_to_ir|unary_op_to_ir)' lib/core/prelude.fmpl` returns no matches. This locks the relocation.

**Verification gates:**
- `cargo test --workspace` passes (no regressions).
- `grep -E '(fold_binary|fold_index|fold_postfix|fold_pipe_at|binary_op_to_ir|unary_op_to_ir)' lib/core/prelude.fmpl` returns no matches.
- `grep -E '(fold_binary|fold_index|fold_postfix|fold_pipe_at|binary_op_to_ir|unary_op_to_ir)' lib/core/parser_helpers.fmpl` returns matches for all 6.
- AC-13 invariant from ITER-0004c remains satisfied: `grep -cE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl` returns 0.
- The CI gate added in ITER-0004c (`fmpl-core/tests/stdlib_no_legacy_syntax.rs`) still passes after the new file is added.

### ITER-0004f — Flatten Binary/Unary AST Nodes

**Stories:** No new STORY-0010 ACs — this is an architectural cleanup proposed by user 2026-05-10 during ITER-0004c hand-migration. Created in response to seeing the redundant `[:Binary, :+, l, r]` shape in the freshly-migrated ast_optimizer.fmpl.

**Rationale:** Currently the AST encodes binary/unary operators as `[:Binary, :+, l, r]` and `[:Unary, :-, e]` — a `:Binary`/`:Unary` "kind tag" plus the operator-symbol child. Flattening to `[:+, l, r]` and `[:-, e]` (operator symbol AS the tag) produces:
- ~14% smaller AST per binary node (3 elements vs 4)
- Cleaner pattern code in optimizers (no `:Binary` prefix to repeat)
- A natural lowercase-AST / Capitalized-IR distinction (e.g., AST `[:+, l, r]` vs IR `[:Add, l, r]`)
- Closer alignment with how grammar-action helpers like `fold_binary` would express "binary op trees"
- **Closer alignment with OMeta/Ohm conventions** — see `~/development/ometa/ometa-js/bs-ometa-compiler.txt`. OMeta uses operator-as-head list shape (`[#App, ruleName, ...args]` — head is the constructor, not a kind tag) for tree pattern matching. FMPL's parser/grammar idioms intentionally follow OMeta; flattening Binary brings AST shape into closer alignment with this lineage.

**Status:** pending
**Depends on:** ITER-0004c (so the stdlib is already in canonical list-pattern syntax before the AST shape changes).
**Look-ahead check:** Unblocks no critical path; could ship before or after ITER-0004d. Most ergonomic placement: between ITER-0004d (parser/AST burn) and ITER-0005 (persistence) so that the flattened shape is what gets persisted. Alternative: between ITER-0004c and ITER-0004d so that ITER-0004d's deletions land against the flattened shape.

**Files in scope:**
- `fmpl-core/src/ast.rs` (or wherever `Expr::Binary(lhs, op, rhs)` and `Expr::Unary(op, e)` are defined) — change variant or change `expr_to_value` encoding
- `fmpl-core/src/builtins/ast.rs` — `expr_to_value` arms for Binary/Unary become `Value::list_node(op_str, vec![l, r])` instead of `Value::list_node("Binary", vec![Symbol(op), l, r])`
- `fmpl-core/src/value_to_ast.rs` — decoder arms for Binary/Unary
- `lib/core/ast_to_ir.fmpl` — 13 binary-rule entries collapse from `[:Binary, :+, expr:l, expr:r] => [:Add, l, r]` to `[:+, expr:l, expr:r] => [:Add, l, r]`; 2 unary rules similarly
- `lib/core/ast_optimizer.fmpl` — substantial pattern rewrite; ~150 sites
- `lib/core/parser_helpers.fmpl` (post-ITER-0004e) or `lib/core/prelude.fmpl` (pre-ITER-0004e) — `fold_binary` produces `[:Binary, ...]` AST nodes; reshape to produce `[op, ...]`
- `fmpl-core/src/builtins/ir_to_rust.rs` — Rust mirror of `fold_binary`
- `fmpl-core/tests/ast_to_ir_parity.rs` — any inputs that introspect `Expr::Binary` shape need updating
- AC-13 grep regex update — currently `:[A-Z][a-zA-Z_]*\(` requires capitalized tags; `[:+, ...]` does NOT match this. The grep stays correct (it would still detect legacy `:Tag(args)` re-introduction), but a separate gate may be needed for the new lowercase-symbol AST convention.

**Scope:**

1. Define the encoding contract: which operator symbols become AST tags? Arithmetic (`:+`, `:-`, `:*`, `:/`, `:%`), comparison (`:==`, `:!=`, `:<`, `:>`, `:<=`, `:>=`), boolean (`:&&`, `:||`), pipe (`:|>`). Unary: `:-`, `:!`. Document in `fmpl-core/src/ast.rs` or a dedicated `docs/conventions/ast-shape.md`.
2. Update `expr_to_value` (Binary and Unary arms) to emit the flat shape.
3. Update `value_to_ast.rs` decoder arms.
4. Update `lib/core/ast_to_ir.fmpl` Binary and Unary rules.
5. Update `lib/core/ast_optimizer.fmpl` Binary and Unary rules (the largest chunk of work).
6. Update `lib/core/parser_helpers.fmpl` (or `prelude.fmpl` pre-ITER-0004e) `fold_binary` and `unary_op_to_ir`.
7. Update Rust mirrors in `builtins/ir_to_rust.rs`.
8. Update parity tests if they assert AST shape.
9. Add a verification gate that asserts the new shape: e.g., `expr_to_value(parse("1 + 2"))` returns `[:+, [:Int, 1], [:Int, 2]]`, NOT `[:Binary, :+, [:Int, 1], [:Int, 2]]`.

**Verification gates:**
- `cargo test --workspace` passes (no regressions).
- `grep -nE '\[:Binary,\s*:[+\-*/%<>=&|!]+,' lib/core/*.fmpl` returns no matches (no legacy Binary-prefixed shape in stdlib).
- SCENARIO-0103 still passes against the new shape.
- AC-13 invariant from ITER-0004c remains satisfied (grep for `:[A-Z][a-zA-Z_]*\(` returns 0).

### ITER-0004g — Lexer: Handle INT_MIN Literal in Negation Context

**Stories:** No new STORY-0010 ACs — surfaced 2026-05-10 during ITER-0004c item 8 implementation. The user pushed back on the AST-construction workaround in `ac3_int_min_negation_does_not_panic`: source-form `"0 - (-9223372036854775808)"` should be lexable, but currently the FMPL lexer (`fmpl-core/src/lexer.rs:117`) silently drops the `9223372036854775808` token because `parse::<i64>().ok()` returns `None` for any value greater than `i64::MAX`. The negation rewrite happens at the AST stage, so the lexer never sees the value as INT_MIN — only as an out-of-range positive integer.

**Rationale:** A user who writes `let x = -9223372036854775808` reasonably expects FMPL to interpret this as `i64::MIN`. Currently the lexer drops the literal. This is a parser-surface bug that affects any program touching INT_MIN, not just the optimizer test. Two fix approaches:

1. **Two-token approach (simpler):** Lex the leading `-` as a separate `Minus` token (which the lexer already does). When the integer-parse fails because the value equals `i64::MAX + 1`, check the previous token in the lexer (or in a post-pass) — if it's `Minus`, replace both tokens with a single `Int(i64::MIN)` token. This requires lookback in the lexer or a token post-processing pass.

2. **Negative-aware integer regex (cleaner):** Rewrite the integer regex to optionally consume a leading `-`, then parse the slice as `i64`. Requires care because `1 - 2` should still be three tokens (`Int(1)`, `Minus`, `Int(2)`), not two (`Int(1)`, `Int(-2)`). Disambiguation: only consume the `-` when the previous token is not `Int`/`Var`/`)`/`]`/`}`/etc. (i.e., the `-` is unary, not binary). This is essentially context-sensitive lexing.

3. **String-then-coerce approach (deferred-fix):** Store integer tokens as their literal source string, defer the i64 conversion to the parser/AST stage where unary-negation context is visible. Largest change but most flexible (also opens the door to `i128` literals later).

**Status:** pending
**Depends on:** None (touches lexer only); ITER-0004c item 8 currently works around this via direct AST construction.
**Look-ahead check:** Unblocks rewriting `ac3_int_min_negation_does_not_panic` to use source-form input (`"0 - (-9223372036854775808)"` becomes parseable). Does not block any other iteration.

**Files in scope:**
- `fmpl-core/src/lexer.rs` (line 117 region) — integer regex / parse logic
- `fmpl-core/tests/lexer_*.rs` (or `tests/lexer.rs` if such a file exists) — add tests for INT_MIN literal handling
- `fmpl-core/tests/optimizer_integration.rs` — once the lexer fix lands, rewrite `ac3_int_min_negation_does_not_panic` to use source-form `"0 - (-9223372036854775808)"` and remove the TODO(ITER-0004g) comment

**Scope:**

1. Decide between approaches (1), (2), (3) — recommend (2) negative-aware regex as a balance of simplicity and correctness.
2. Implement the chosen approach in `fmpl-core/src/lexer.rs`.
3. Add direct lexer tests:
   - `lex("9223372036854775807")` succeeds (i64::MAX)
   - `lex("-9223372036854775808")` succeeds as `Int(i64::MIN)` OR as two-token `Minus, Int(i64::MAX+1?)` depending on chosen approach
   - `lex("9223372036854775808")` (one over i64::MAX, no leading `-`) returns a clear error
   - `lex("1 - 2")` returns three tokens (not `Int(1), Int(-2)`)
   - `lex("(- 5)")` returns four tokens (`LParen, Minus, Int(5), RParen`) — unary negation with whitespace
4. Update `ac3_int_min_negation_does_not_panic` in `optimizer_integration.rs` to use the source-form `"0 - (-9223372036854775808)"`. Remove the AST-construction workaround. Remove the `TODO(ITER-0004g)` comment.
5. Run `cargo test --workspace` and verify no regressions.

**Verification gates:**
- All new lexer tests pass.
- `ac3_int_min_negation_does_not_panic` passes with source-form input.
- `cargo test --workspace` passes overall.
- No new ignored or failing tests.

**Optional companion fix (also surfaced 2026-05-10):** `lib/core/ast_optimizer_test.fmpl` uses `++` for string concatenation in its print-summary section (lines 184-186 + 194), but the FMPL parser does not support `++` as a binary operator (it lexes as two consecutive `Plus` tokens, leading to "unexpected token: Plus"). This blocks the AC-13 companion gate `fmpl-core/tests/ast_optimizer_unit.rs` from un-ignoring. Either add `++` to the FMPL operator vocabulary, or rewrite the test file's print-summary section to use `string.join`. If addressed in this iteration, also un-ignore the `ast_optimizer_unit` test.

### ITER-0004h — Type::Tagged Cleanup (post-burn)

**Stories:** No new STORY-0010 ACs — orphan cleanup carve-out scheduled 2026-05-10 from ITER-0004d PAR round 1 findings. PAR reviewer B observed that after AC-9 deletes the parser productions producing `Expr::Tagged`, no parser path constructs `Type::Tagged` values; the type-system variant becomes dead code that no scenario or AC explicitly references.

**Rationale:** `fmpl-core/src/types.rs:30` defines `Type::Tagged(SmolStr, Vec<Type>)` as a constructor type. The variant survives ITER-0004d because the iteration's deletion graph is scoped to AST/Pattern/Bytecode, not the type system. Leaving `Type::Tagged` dead-but-defined is an orphan that violates the iteration's "one shape" coherence claim at the type-system layer. Two paths: (a) delete the variant entirely (if no surviving Rust code constructs it after ITER-0004d) or (b) repurpose it for typed-list-shape values (e.g., `Type::ListNode(SmolStr, Vec<Type>)`) so future static analysis can talk about constructor shapes by name. Decision made at iteration start based on the post-ITER-0004d codebase.

**Status:** pending
**Depends on:** ITER-0004d (the AST/parser deletions must land first so the dead-code claim can be verified).
**Look-ahead check:** Unblocks no critical path; could ship before or after ITER-0005. Recommended placement: after ITER-0004d but before ITER-0005, so the type-system surface that ITER-0005's persisted bytecode references is in its final shape.

**Files in scope:**
- `fmpl-core/src/types.rs` (variant definition + Display/PartialEq/Hash impls)
- Any remaining `Type::Tagged` consumers (enumerate via `grep -rn 'Type::Tagged' fmpl-core/src/` at iteration start)
- Type-system test corpus under `fmpl-core/tests/type_inference.rs` (if it asserts `Type::Tagged` shape)

**Scope:**

1. Re-grep `Type::Tagged` consumers after ITER-0004d lands. If zero remain, delete the variant and update any tests that assert its absence. If consumers exist (likely zero, but check), evaluate whether each is reachable from any post-burn code path.
2. Optionally rename to `Type::ListNode` if static-analysis use cases exist for typed-list-shape values. Otherwise delete.
3. Update `behavior-scenarios.md` if any scenario references `Type::Tagged` (likely none, since the type system is largely unobservable from FMPL programs today).

**Verification gates:**
- `grep -rn 'Type::Tagged' fmpl-core/src/` returns no matches (if deletion path chosen).
- `cargo test --workspace` passes.
- No new ignored or failing tests.

**Out of scope:** Broader type-system refactor; this is a single-variant cleanup.

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
**Depends on:** ITER-0004 (compiler cutover), ITER-0004b + ITER-0004c + ITER-0004d (full canonical-representation refactor — runtime burn + FMPL stdlib migration + parser/AST burn), and ITER-0005 (persistence). The fmpl_compiler.fmpl pipeline `fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile` requires that *every* stdlib file in that chain be in the canonical list-pattern syntax (delivered by ITER-0004c) AND that the parser accepts only one AST shape (delivered by ITER-0004d).
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
