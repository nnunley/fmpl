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

**Status:** done 2026-05-12. T2-T6 mechanical rename across 5 files: 4 variants in compiler.rs with `#[serde(rename)]` (Option B for wire-format preservation); 5 VM handler arms in vm.rs (1204 MatchTag preserved); 3 emit sites in compiler.rs + 2 in builtins/ir.rs (lines 344, 983) + the IR dispatcher arm key `"MakeTagged"` → `"MakeListNode"`; test references in context_aware_compilation.rs + stream_coercion.rs; SCENARIO-0106 grep needles flipped in structural_invariants.rs. T7 added `opcode_rename_evidence.rs` (7 tests) covering the two PAR-flagged gaps: dead-code variant reachability + Serde round-trip catching missing `serde(rename)` attributes. Final sentinel sweep: 147 passed, 3 ignored across 8 suites (+7 net tests vs end-of-ITER-0004d.3a). PARSER_EPOCH unchanged (Instruction enum lives in compiler.rs, not the ir_to_rust.rs postlude).
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

**Scope (PAR-revised):**

The original scope's emit-site inventory was stale; the orchestrator re-verified current sites before approving the iteration. Authoritative current state (post-ITER-0004d.1):

- `MakeTagged` (compiler.rs:260, vm.rs:877, builtins/ir.rs:344) — 1 live emit site (FMPL-side, `builtins/ir.rs:344`).
- `ExtractTaggedChild` (compiler.rs:363, vm.rs:1182, builtins/ir.rs:983, compiler.rs emit sites at 2654/2968/3132) — 4 live emit sites total.
- `MatchTagged` (compiler.rs:505, vm.rs:2567, nested ref at vm.rs:2609) — **ZERO live emit sites** (PAR finding #2 from both reviewers).
- `MatchTaggedWithBindings` (compiler.rs:510, vm.rs:2521) — **ZERO live emit sites** (PAR finding #2).
- `MatchTag` (compiler.rs:369, vm.rs:1204, 4 emit sites at compiler.rs:2523/2539/2663/2673) — **PRESERVED**.

The MatchTagged/MatchTaggedWithBindings handlers are dead code post-ITER-0004d.1. The roadmap's earlier line-number references (3346, 3809, 4380, 4389) were stale from pre-T-task source.

1. **Rename variants in `compiler.rs:260, 363, 505, 510`** (NOT line 369, which is `MatchTag`). Add `#[serde(rename = "MakeTagged")]` etc. to each renamed variant.

2. **Rename VM handler arms in `vm.rs:877, 1182, 2521, 2567`** (NOT line 1204, which is `MatchTag`; the nested ref at vm.rs:2609 is inside MatchTagged's arm scope and renames with it). Verify with `grep -nE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)' fmpl-core/src/vm.rs` returns no matches after edit.

3. **Rename live compiler emit sites in `compiler.rs:2654, 2968, 3132`** — all three are `ExtractTaggedChild` emits in `Pattern::Symbol` matching paths. The previously-listed `compiler.rs:874` (MakeTagged emit) was deleted in ITER-0004d.1 T9. MatchTagged/MatchTaggedWithBindings compiler emits are likewise gone post-ITER-0004d.1.

4. **(REMOVED)** — `ir_builder.rs:238-240` referenced an `ir_builder::tagged` helper that was deleted in ITER-0004d.1 T9 (zero callers confirmed pre-deletion). No edit needed here.

5. **(REORDERED) Rename `builtins/ir.rs:344, 983`** — the FMPL-IR dispatcher arm names. The arm key string `"MakeTagged"` is dispatched on by `compile_ir` (line 336 area) when an FMPL-side IR value `[:MakeTagged, ...]` is encountered. Step 8 (`ast_to_ir.fmpl` scan) MUST complete first to confirm no FMPL stdlib emits `:MakeTagged`. **Current state:** verified by orchestrator that `lib/core/ast_to_ir.fmpl` has zero `MakeTagged` references (the post-T10 state). Safe to rename both the arm key AND the emit construction:
   - `builtins/ir.rs:344`: `Ok(self.emit(Instruction::MakeTagged { tag, args }))` → `Ok(self.emit(Instruction::MakeListNode { tag, args }))`. Match arm key `"MakeTagged"` → `"MakeListNode"`.
   - `builtins/ir.rs:983`: `Instruction::ExtractTaggedChild { ... }` → `Instruction::ExtractListChild { ... }`.

6. **(MERGED INTO 5)** — handled in step 5 above.

7. **Sweep Rust test qualified-path references.** The orchestrator's pre-iteration grep enumerated:
   - `fmpl-core/tests/context_aware_compilation.rs:109, 119, 340` — `matches!(i, Instruction::ExtractTaggedChild { .. })` and `Instruction::MatchTagged { .. }` plus one comment reference.
   - `fmpl-core/tests/stream_coercion.rs:254, 371` — **CONSTRUCT `Instruction::MakeTagged { tag, args }` directly in test code** (PAR finding #3). MUST be renamed or tests fail to compile.
   - `fmpl-core/tests/structural_invariants.rs:400, 402, 431, 437` — these are SCENARIO-0106 greps that use the LITERAL string `"Instruction::MakeTagged"` as the search needle. See step 7a below for the explicit grep-flip.

7a. **(NEW PER PAR finding #1) Update SCENARIO-0106 greps #6 and #7 in `fmpl-core/tests/structural_invariants.rs`** — the grep needles MUST change to the new names; otherwise grep #7 (positive-presence assertion for `ExtractTaggedChild` in compiler.rs) will fail immediately after step 3 lands.
   - Grep #6: change the needle from `"Instruction::MakeTagged"` to `"Instruction::MakeListNode"`. The semantic invariant (absent from compiler.rs) is preserved because the renamed variant also has no compiler-emit sites — only the builtins/ir.rs emitter survives. Update the test's docstring to reflect the post-rename status (the original parenthetical "rename scheduled for ITER-0004d.2" is now stale and should be removed).
   - Grep #7: change the needle from `"ExtractTaggedChild"` to `"ExtractListChild"`. The positive-presence assertion (≥1 reference in compiler.rs) holds because the three emit sites at compiler.rs:2654/2968/3132 are renamed in step 3.

8. **(REORDERED, NOW BEFORE step 5) Update `lib/core/ast_to_ir.fmpl`** if any rule still references `:MakeTagged` as an emit target. **Pre-verification: orchestrator confirmed zero references.** If new references somehow appeared mid-iteration, this step catches them. Step ordering: 8 → 5 → 3 → 1 → 2 → 7 → 7a chronologically; numbering preserved for traceability.

9. **(PAR-EXPANDED) Add SCENARIO-0107** — bytecode-opcode invariant. Original three observables retained:
   - `grep -rnE 'Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)\b' fmpl-core/src/` returns no matches (legacy gone).
   - `grep -rnE 'Instruction::(MakeListNode|MatchListNode|MatchListNodeWithBindings|ExtractListChild)\b' fmpl-core/src/` returns matches (new names present).
   - `grep -nE 'Instruction::MatchTag\b' fmpl-core/src/compiler.rs` returns matches at lines 369 (definition), 2523, 2539, 2663, 2673 (Pattern::Symbol emit sites; post-ITER-0004d.1 — line numbers refreshed by orchestrator).

   **NEW per PAR finding #2** — behavioral observable beyond sentinel-pass:
   - **Direct VM-handler exercise for the dead-code opcodes.** Add a test (`fmpl-core/tests/opcode_rename_evidence.rs` or appended to `stream_coercion.rs`) that DIRECTLY constructs `Instruction::MatchListNode { ... }` and `Instruction::MatchListNodeWithBindings { ... }` bytecode, executes them via the VM (or a minimal harness), and asserts behavioral correctness. Pattern: same as `stream_coercion.rs:251-257` which constructs `MakeTagged` directly today. Without this, a typo in either handler ships undetected because no live emit path reaches them.

   **NEW per PAR finding #4** — wire-format round-trip evidence:
   - **Serde round-trip test for renamed opcodes.** Add a test that serializes a `CompiledCode` containing each of the four renamed opcodes via the FMPL pipeline, captures the JSON, and asserts the wire-format strings are still `"MakeTagged"` / `"ExtractTaggedChild"` / `"MatchTagged"` / `"MatchTaggedWithBindings"` (the `#[serde(rename)]` targets). For the two dead-code opcodes (MatchListNode, MatchListNodeWithBindings), construct the bytecode directly (same as the dead-code VM test above) and verify serialization works.

10. **Update EPIC-002.md STORY-0010 AC-11** to add `· scenario:SCENARIO-0107`.

**Verification (PAR-revised):**
- `cargo test --workspace` passes.
- SCENARIO-0103 + SCENARIO-0016 still pass (behavioral assurance for the rename's live-emit opcodes: MakeListNode + ExtractListChild).
- SCENARIO-0107 passes including:
  - Structural greps (legacy absent, new names present, MatchTag preserved).
  - Direct VM-handler exercise for MatchListNode + MatchListNodeWithBindings (the dead-code opcodes whose handlers no live emit reaches).
  - Serde round-trip for each renamed opcode confirms wire-format compatibility.
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` still returns 0.
- `grep -rnE 'Instruction::MakeTagged\b' fmpl-core/` returns 0 matches.
- `cargo clippy --all-targets --quiet -- -D warnings` clean (pre-commit hook will block otherwise).
- Canonical-pipeline sentinel `canonical_pipeline_parity` still passes (the rename is internal to `Instruction` enum; doesn't touch parser surface).

**Out of scope:** `Type::Tagged` cleanup (ITER-0004h — distinct type-system surface, no `Instruction` overlap). Wire-format version bump (ITER-0005's STORY-0099 — the `serde(rename)` attributes are the forward-compat surface ITER-0005 may choose to drop).

### ITER-0004d.2a — Opcode-rename audit fix-up (G1+G2+G3+G4)

**Stories:** STORY-0010 AC-11 evidence strengthening. Direct response to ITER-0004d.2's three-tier audit findings.

**Rationale:** ITER-0004d.2 audit returned GAPS FOUND with three SERIOUS findings (one from BOTH auditors, one from each) plus minor stale-comment drift. Pattern matches ITER-0004d.3 → 0004d.3a: surgical fix-up keeps the original iteration cleanly closed.

**Status:** done 2026-05-12. G1 synced ir_to_rust.rs transpiler dispatcher (1 arm key rename in live Rust code, no PARSER_EPOCH bump needed). G2 updated SCENARIO-0106 card bullets #6 and #7 to use the post-rename needle strings. G3 added 8 VM-execution tests covering MatchListNode/MatchListNodeWithBindings handlers + the inlined nested-dispatch path at vm.rs:2610 — handler-body logic now exercised through `ParsePush`-driven bytecode. G4 swept 8 stale comments across compiler.rs/vm.rs/context_aware_compilation.rs/ast_to_ir_parity.rs/progress.md. Focused re-audit verified all four gaps CLOSED. Final sentinel sweep: 155 passed, 3 ignored across 8 suites (+8 net from G3's VM tests; 0 regressions). Clippy clean.

**The four gaps:**

- **G1 (SERIOUS, BOTH auditors).** `fmpl-core/src/builtins/ir_to_rust.rs:543` has a `"MakeTagged"` arm in the Rust transpiler's IR dispatcher but no `"MakeListNode"` arm. ITER-0004d.2 updated the bytecode IR dispatcher at `builtins/ir.rs:336` but missed this parallel dispatcher in the Rust transpiler. The two dispatchers are now divergent for the same semantic operation. Currently latent (no FMPL stdlib emits `:MakeListNode` IR nodes that flow through this path), but represents inconsistency + dead code. The same gap likely exists for `ExtractTaggedChild` / `MatchTagged` / `MatchTaggedWithBindings` arm keys; investigate all four.

- **G2 (SERIOUS, Auditor A).** `behavior-scenarios.md` SCENARIO-0106 card expected observables describe pre-rename grep needles (`Instruction::MakeTagged` must NOT appear, `ExtractTaggedChild` MUST appear). After ITER-0004d.2 T6 flipped the test code's needles, the card text diverged from the test contract. Scenario contract integrity violation — future readers see wrong invariants.

- **G3 (SERIOUS, Auditor B).** `MatchListNode` and `MatchListNodeWithBindings` VM handlers have ZERO execution coverage. The `opcode_rename_evidence::renamed_variants_are_constructible` test constructs the variants but never runs them through the VM. A handler-body logic bug (arity check at vm.rs:2590, nested dispatch at vm.rs:2608) would compile and ship undetected. The PAR pre-iteration finding #2 motivated the construction test but stopped short of execution.

- **G4 (MINOR, BOTH auditors).** Stale comments referencing old opcode names in `compiler.rs:2649/2965/3033/3128`, `vm.rs:2523`, `context_aware_compilation.rs:4`, `ast_to_ir_parity.rs:482`, `progress.md:15`. Update for consistency.

**Impacted scenarios:** SCENARIO-0107 evidence strengthened (G3 adds execution coverage); SCENARIO-0106 card text corrected (G2). No new scenarios.

**Depends on:** ITER-0004d.2 (the rename surface that needs strengthening).

**Look-ahead check:** ITER-0004h (Type::Tagged) is unaffected — distinct surface. ITER-0005 (persistence) gets a cleaner forward-compat surface because the ir_to_rust dispatcher is consistent.

**Scope:**

1. **G1.** Edit `fmpl-core/src/builtins/ir_to_rust.rs`. Find all `transpile_tagged` (or similar) arms with legacy opcode names; rename each to its new-name counterpart. Check whether this file is the postlude raw-string (would require PARSER_EPOCH bump per `parser_epoch.rs:27-29`) or live Rust code (no bump). The IR-to-Rust transpiler in this file is a Rust function the bootstrap binary calls — the file itself is NOT the postlude. But verify carefully: there's also a postlude raw-string inside this file that gets emitted into generated parsers, and the audit found dispatcher arms in the live Rust code (line 543). The dispatcher is live Rust, not postlude — no bump needed.

2. **G2.** Edit `docs/superpowers/iterations/behavior-scenarios.md` SCENARIO-0106 card. Update the "Expected observables" entries #6 and #7 to use the new needle strings (`Instruction::MakeListNode` for grep #6, `ExtractListChild` for grep #7). Update any other card text (Note field, Sources) that references old names. The card's "Note" field has historical content that's OK to leave — only update the active contract description.

3. **G3.** Add `fmpl-core/tests/opcode_rename_evidence.rs` execution tests for MatchListNode + MatchListNodeWithBindings. Construct bytecode containing each opcode, set up parse_state with appropriate input values, run via VM, assert match/no-match behavior. Cover at least: (a) MatchListNode success — input matches expected tag, all child patterns succeed; (b) MatchListNode failure — input has wrong tag, handler returns null; (c) MatchListNodeWithBindings success — bindings correctly assigned; (d) MatchListNodeWithBindings arity-mismatch failure (the audit specifically flagged the arity check at vm.rs:2590 as untested). Model on stream_coercion.rs's make_code/execute helpers.

4. **G4.** Sweep stale comments. List of sites (from the audit):
    - `compiler.rs` lines 2649, 2965, 3033, 3128 — `ExtractTaggedChild` in inline comments
    - `vm.rs:2523` — `Value::Tagged("Int", [v])` comment in MatchListNodeWithBindings handler
    - `context_aware_compilation.rs:4` — module docstring `ExtractTaggedChild`
    - `ast_to_ir_parity.rs:482` — comment `ExtractTaggedChild`
    - `progress.md:15` — Ratchet status section `canonical replacement ExtractTaggedChild`

5. **Re-run full sentinel sweep + clippy.** Expected count: 147 + new G3 tests; 0 regressions.

6. **Focused re-audit.** Confirm G1-G4 are genuinely closed (mirror of ITER-0004d.3a re-audit pattern). If re-audit finds residual gaps, fix inline.

**Acceptance:**

- `fmpl-core/src/builtins/ir_to_rust.rs` has no legacy opcode name as a live dispatcher arm key (G1).
- SCENARIO-0106 card text matches the actual test contract (G2).
- `opcode_rename_evidence.rs` has execution tests for MatchListNode + MatchListNodeWithBindings handlers that exercise success + failure paths (G3).
- All stale comments updated (G4).
- Full sentinel sweep: ≥147 passed + ≥4 new G3 tests, 3 ignored, 0 regressions.
- Clippy clean.
- Focused re-audit reports CLEAN.

**Out of scope:** Anything not in G1-G4. ITER-0004h's Type::Tagged work.

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

**Status:** done 2026-05-12 (PAR-revised scope: Rust runner only; FMPL-side runner deferred to ITER-0004d.5). T1 scaffolded the `fmpl-scenario-runner` workspace crate (deps: inventory 0.3). T2 implemented error.rs with Display impls for StepError/DispatchError/CorpusError (PAR-required for codegen `{}` format). T3 implemented the markdown corpus parser (689 lines, fixture-driven TDD, parses 87 cards from the real corpus). T4 implemented StepDef trait + inventory dispatch (4 integration tests for registration/dispatch/Unknown/Display). T5 extended fmpl-core/build.rs with codegen for `OUT_DIR/scenarios_generated.rs` (uses `env!("CARGO_MANIFEST_DIR")` baked at test-binary compile time per PAR finding; cargo::rerun-if-changed for the corpus markdown). T6 moved comment_strip helper to `tests/common/comment_strip.rs` (preserved verbatim; +15 unit tests added). T7 implemented 3 step-defs in `tests/steps/`: parse_rejection, parse_success, grep_invariant (handles both expect_absent and expect_present). T8 migrated SCENARIO-0104 (6 cases), SCENARIO-0105 (4 cases), SCENARIO-0106 (12 cases incl. NEW grep #9 for `Type::Tagged` from ITER-0004h audit) to the structured `**Action type:**` + `**Cases:**` card format. T9 created `tests/scenario_runner.rs` (3-line glue: mod common; mod steps; include!) and `tests/postlude_arm_contract.rs` (relocated `g3_postlude_arms_fire_on_poison_nodes` as a special-case test that doesn't fit the card format). T10 deleted `tests/structural_invariants.rs` entirely (all 19 evidence tests migrated; 15 comment_strip tests live in tests/common now). T11 updated behavior-corpus.md execution commands. T12 was subsumed by T9/T10 inline edits. Final sentinel sweep: 218 passed, 3 ignored across 10 fmpl-core suites + 27 passed across 5 fmpl-scenario-runner suites = **245 total tests passing**, 0 regressions. Clippy clean. **SCENARIO-0106 grep #9 ratchet for `Type::Tagged` is now authored as a scenario card (not a Rust test), closing the ITER-0004h audit gap via the new data-driven infrastructure.**

**Impacted scenarios:** No new scenarios; this is infrastructure. SCENARIO-0104/0105/0106 migrated from Rust-per-test to data-driven (execution commands updated in behavior-corpus.md). Future scenarios benefit; existing free-form scenarios stay free-form unless explicitly migrated.

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

**Out of scope (PAR-revised):**
- Migrating scenarios that have no step-def coverage today (e.g., SCENARIO-0001..0077 which are largely TBD).
- FMPL-grammar-based scenario parsing (a possible follow-on — metacircular scenario evaluation is on-brand with DESIGN-001).
- **FMPL-side runner / bootstrap-durability surface (deferred to ITER-0004d.5).** PAR scope review's primary finding was that the originally-bundled FMPL-side runner (`lib/tests/scenarios/scenarios.fmpl`, `fmpl_emit.rs`, `scenario_runner_bootstrap.rs`) is a separate substantial deliverable. Rationale: `grep_invariant` cannot be implemented FMPL-side until `io::read_dir` lands, so v1 FMPL-side runner would ship as a partial stub. ITER-0004d.5 (added below) tracks the deferred work.

**PAR-revised acceptance criteria** (full set in the design spec at `docs/superpowers/specs/2026-05-12-scenario-runner-design.md`): see the spec's Acceptance criteria section for the post-revision list. Key updates: ≥20 passing tests (was ≥17 — original spec undercounted), explicit grep #9 (`Type::Tagged` absent) requirement, `g3_postlude_arms_fire_on_poison_nodes` relocates to `fmpl-core/tests/postlude_arm_contract.rs` (special-case test that doesn't fit the scenario card format).

**Authoritative design:** `docs/superpowers/specs/2026-05-12-scenario-runner-design.md` (PAR-revised 2026-05-12).

### ITER-0004d.5 — FMPL-side Scenario Runner + Bootstrap Durability

**Stories:** New story TBD — register under EPIC-002 once the iteration starts. Completes the bootstrap-durability surface that ITER-0004d.4 PAR-deferred.

**Rationale:** ITER-0004d.4's PAR scope review deferred the FMPL-side runner (originally bundled into 0004d.4's spec at brainstorming time) because: (a) `grep_invariant` can't be implemented FMPL-side until `io::read_dir` exists, so v1 ships as a partial stub; (b) the Rust runner alone is a complete, well-bounded iteration. ITER-0004d.5 picks up the FMPL-side work once 0004d.4 lands.

**Status:** pending (blocked on ITER-0004d.4 + on `io::read_dir` builtin landing).

**Files in scope (per the design spec's deferred section):**
- `fmpl-scenario-runner/src/fmpl_emit.rs` — compile `Vec<Card>` → list-shape FMPL value.
- `fmpl-core/build.rs` — extend codegen to also emit `lib/tests/scenarios/scenarios.fmpl` (alongside the existing Rust output from 0004d.4).
- `fmpl-core/tests/scenario_runner_bootstrap.rs` — FMPL-surface test target that drives the bootstrap-rebuild and re-executes the corpus against the regenerated parser.
- `lib/tests/scenarios/scenarios.fmpl` — compiled corpus artifact (git-tracked).
- `lib/tests/scenarios/dispatch.fmpl` — FMPL-side dispatcher (initial coverage: parse_rejection + parse_success; grep_invariant deferred until `io::read_dir` lands or coverage stays Rust-only).

**Depends on:** ITER-0004d.4 (Rust runner + corpus parser). `io::read_dir` builtin (status: not currently scoped; may need its own precursor iteration).

**Look-ahead check:** Completes the DESIGN-001 "scenarios as durable artifacts" claim from the brainstorming spec. Architecturally compatible with ITER-0005 (Fjall persistence) — `scenarios.fmpl` is a regular FMPL value.

**Out of scope:** Self-compile cycle durability (ITER-0006 — requires that iteration to land first). FMPL-grammar-based markdown parsing (a separate follow-on — the runner's markdown parser stays Rust in v1).

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

**Rationale:** `fmpl-core/src/types.rs:30` defined `Type::Tagged(SmolStr, Vec<Type>)` as a constructor type. The variant survived ITER-0004d because the iteration's deletion graph was scoped to AST/Pattern/Bytecode, not the type system. Leaving `Type::Tagged` dead-but-defined was an orphan that violated the iteration's "one shape" coherence claim at the type-system layer.

**Status:** done 2026-05-12. Verified zero production consumers (no FMPL pipeline path constructs `Type::Tagged`); only existing references were the variant definition + `is_subtype` arm in `types.rs` and one variant-specific unit test `tagged_subtyping` in `type_inference.rs`. Decision: delete (rename to `Type::ListNode` was YAGNI — zero current consumers, `Type::List(Box<Type>)` already covers homogeneous-list typing). Three surgical edits: variant definition deleted at types.rs:29-30; subtype-arm deleted at types.rs:52-57 (the surrounding `match` has a wildcard `_ => false` arm so exhaustivity is preserved); `tagged_subtyping` test deleted at type_inference.rs:58-65. Final sentinel sweep: 168 passed, 3 ignored across 9 suites (-1 vs baseline from the deleted variant-specific test; 0 regressions). Clippy clean. **STORY-0010 fully closed; ITER-0004 milestone complete.**
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

### ITER-0004x — execution_tape parity gate (dual-VM comparison)

**Stories:** New story TBD (sibling to STORY-0037 / EPIC-007's MLIR→execution_tape work; this iteration is the precursor that demonstrates execution_tape can run the FMPL Indexed RPN IR subset before any MLIR work begins).

**Rationale:** 2026-05-12 user-prompted reordering during ITER-0004d.4 in-flight. The strategic question: should fmpl-core's runtime stay in-tree (the current `Vm` + `Instruction` enum) or outsource to the external `execution_tape` VM project at `~/development/execution/execution_tape/`? Long-term goal: "FMPL compiler self-hosts; FMPL VM is provided by a stable, externally-verified primitive" — analogous to how Rust's std isn't expected to self-host. Outsourcing the VM is on-brand with that goal, but the decision needs comparative evidence first. This iteration provides that evidence by promoting `fmpl-core/src/cross_compile.rs` (today: standalone "perf-comparison" module) into a first-class dual-VM parity gate.

After this iteration, ITER-0005 (Image Persistence) has direct evidence for what to persist: in-tree `Instruction` bytecode (`#[serde(rename)]` shim approach) OR execution_tape bytecode (already has its own serialization format).

**Status:** done (2026-05-12). SCENARIO-0109 landed with 29/29 cases passing under `--features cross_compile`. The parity-gate exercise surfaced and fixed three latent bugs in `cross_compile.rs`: (T3a) hardcoded `ret_types: vec![ValueType::I64]` broke Bool/F64 returns; (T3a) `PushScope`/`PopScope`/`Copy` were unsupported, blocking let-bindings; (T3a) `LoadVar(name)` emitted `const_i64(0)` as a placeholder regardless of the bound value, so name resolution silently returned zero. Fixes: return-type inferred from the last value-producing instruction's `TapeType`; scope markers handled as no-ops with the return selector skipping them; `LoadVar` now resolves through a `name → bind-idx` map populated during the codegen pass. Default-feature scenario_runner remains 38/38; clippy clean on both feature configurations. Out-of-scope follow-ups (control flow, strings, lists, pattern matching) remain for the eventual STORY-0037 / EPIC-007 work.

**Depends on:** ITER-0004d.4 (the scenario runner provides a clean home for the new SCENARIO-0109 parity card — authoring it as a Rust test would compound migration debt, same lesson as ITER-0004h's grep #9). Also depends on `fmpl-core/src/cross_compile.rs` being current — it is (covers LoadInt/Float/Bool/Null/String/Symbol, LoadVar, Bind, NameRef, arithmetic +-*/%, comparison ==!=<><=>=, Neg, Not, MakeList per inspection 2026-05-12).

**Look-ahead check:** Provides the comparative data ITER-0005 needs to pick its persistence target. Does NOT commit to deleting the in-tree VM — that's a downstream decision based on the evidence this iteration surfaces.

**Files in scope:**
- `fmpl-core/src/cross_compile.rs` — promote rustdoc; document supported opcode subset; mark unsupported opcodes with explicit `// EXECUTION_TAPE_NOT_SUPPORTED(<future-iter>): ...` markers.
- `fmpl-core/tests/execution_tape_parity.rs` (NEW) OR a new SCENARIO-0109 card consumed by the ITER-0004d.4 scenario runner (preferred — depends on which step-def types ITER-0004d.4 ships).
- `behavior-scenarios.md` — SCENARIO-0109 card (action_type TBD; likely `dual_vm_parity` as a new step-def or reuse `parse_success` with a custom assertion hook).
- `behavior-corpus.md` — SCENARIO-0109 entry.
- `docs/superpowers/iterations/requirements/EPIC-007.md` — note ITER-0004x as a precursor to STORY-0037; doesn't change STORY-0037 itself.

**Scope:**

1. **Inventory cross_compile.rs coverage.** Already done at iteration kickoff: LoadInt/Float/Bool/Null/String/Symbol, LoadVar, Bind, NameRef, Add/Sub/Mul/Div/Mod, Eq/NotEq/Lt/Gt/LtEq/GtEq, Neg, Not, MakeList. NOT supported: tagged-list-node opcodes (MakeListNode, ExtractListChild, MatchListNode, MatchListNodeWithBindings, MatchTag), pattern-matching opcodes, parse-state instructions, control-flow opcodes beyond simple binary expressions.
2. **Choose the test corpus.** Pick a representative subset of `ast_to_ir_parity` inputs (~20-30 cases) that exercise ONLY the cross_compile-supported opcodes. Probably: integer-literal, arithmetic-precedence, string-literal, let-binding-int, if-int-int parity scenarios.
3. **Write the parity test.** For each input: compile via FMPL pipeline → `CompiledCode { instructions: Vec<Instruction>, ... }`. Run via TWO paths: (a) `Vm::new().run(&code)` returning `Value`; (b) `cross_compile_to_tape(&code)` → execution_tape program → run → compare to (a). Assert result equality.
4. **Decide value-equality semantics.** execution_tape values are typed (`I64`, `F64`, `Bool`, `Str`, `Unit`); fmpl-core values are `Value::Int(i64)`, `Value::Float(f64)`, `Value::Bool`, `Value::String`, `Value::Null`. Map between them in the parity gate. Document the mapping.
5. **Author SCENARIO-0109.** Card title: "execution_tape parity for the cross_compile-supported subset." Per the ITER-0004d.4 scenario runner's card format (assuming it's available — if not, defer step 5 to ITER-0004x.a). The card lists each input as a case under `**Cases:**`.
6. **Update behavior-corpus.md** with the SCENARIO-0109 entry.
7. **Document the gap surface.** In cross_compile.rs rustdoc, list the opcodes NOT supported and the iteration that would extend coverage (a hypothetical ITER-0004x.1 or a fold into ITER-0005's persistence work).

**Verification gates:**
- `cargo test -p fmpl-core --test execution_tape_parity` (or `--test scenario_runner scenario_0109`) passes; ≥20 dual-VM parity cases, all result-equal.
- SCENARIO-0109 card present in behavior-scenarios.md with full expected observables.
- behavior-corpus.md has the SCENARIO-0109 entry with a concrete cargo test command.
- All other sentinels still green; no regressions in the in-tree VM path.
- cross_compile.rs rustdoc lists the supported and unsupported opcode sets.

**Out of scope:**
- **Replacing the in-tree VM.** ITER-0004x is evidence-only. The "delete fmpl-core/src/vm.rs" decision happens downstream (probably as part of ITER-0005 or a dedicated ITER-0004y) based on what this iteration surfaces.
- **Extending cross_compile.rs to cover tagged-list-node opcodes.** That's a separate iteration; this one uses what's there.
- **MLIR / EPIC-007.** STORY-0037 (MLIR backend emits execution_tape) is a downstream initiative; this iteration is the empirical-evidence precursor.
- **Performance benchmarking.** The current cross_compile.rs docstring claims "performance comparison" as the motivation but this iteration's parity gate is correctness-only. Perf measurement is a follow-up.

### Image Persistence — sub-iteration family (ITER-0005a.1 … ITER-0005f)

**Cross-iteration rationale:** Consolidated from old ITER-0007/0008/0009 and originally drafted as a single ITER-0005. Reviewed 2026-05-12 against the size of recently-completed iterations (ITER-0004x had 5 tasks closing 1 story + 1 scenario; ITER-0004d.4 had ~7 tasks closing the data-driven runner) and found the umbrella ITER-0005 was ~3-5x larger by story-count (12 stories + foundational infrastructure across 5+ medium-to-large concerns). Split into layered sub-iterations along natural foundation/payload/composition fracture lines so each iteration is single-concern, 3-7 tasks, and audit-checkpoint-able. Each ships independently; downstream sub-iterations depend only on the artifacts the upstream one closed.

**Family roster (post-2026-05-13 PAR splits):**

- ITER-0005a.0 — `fmpl-types` shared-types crate (resurrected + RESCOPED 2026-05-13 — was MigrationEngine, now `VmVersion` + `Hash` + `SourceHash` shared types). Prerequisite for 0005a.5 to land cleanly; resolves R4-C1 contradiction structurally by giving cross-crate types a stable home. The original MigrationEngine scope stays deferred to a future iteration when a real MigrationRule consumer exists.
- ITER-0005a.1 — STORY-0099 envelope format + loader (DONE 2026-05-13).
- ITER-0005a.2 — STORY-0099 AC-5 write-side sweep (pending; PAR-revised 2026-05-13).
- ITER-0005a.3 — STORY-0099 AC-7 LoaderStats + iter_keyspace public API + first consumer (DONE 2026-05-13; twice-PAR-revised).
- ITER-0005a.5 — Extract `fmpl-persistence` crate; storage abstraction in fmpl-core only, no fjall in public API (pending; PAR-revised 4x 2026-05-13 — 4 rounds of REVISE; final revision uses `fmpl_types::VmVersion`/`Hash` per ITER-0005a.0 rescoped; fmpl-web migration moved to ITER-0005a.6).
- ITER-0005a.6 — Migrate fmpl-web from fjall v2 direct-use to `fmpl-persistence::Store` trait (pending; PAR-split from 0005a.5 — covers v2→v3 data-migration story, ContinuationStore + ImageStore rewrite, workspace `fjall = "2"` removal).
- ITER-0005a.4 — STORY-0099 read-side decode wiring through LoaderStats API (pending; blocked by 0005a.5 so per-call-site rewires cross the new crate boundary cleanly; addresses 4 deferred audit findings from 0005a.2; 0005a.4 card text itself will be rewritten as part of 0005a.5 close to reshape its signatures from `&fjall::Keyspace` → `&impl Store`).
- ITER-0005b — Content-addressed source store (pending; `Hash` newtype absorbed into ITER-0005a.0 rescoped; 0005b adds `Hash::compute()` + the source store itself).
- ITER-0005c — Bytecode persistence (proof case).
- ITER-0005d — Remaining payload classes (objects, grammars, GrammarRegistry, memo tables).
- ITER-0005e — VM snapshot + tracer substrate foundation.
- ITER-0005f — Feature flag wiring + final polish.

**Cross-iteration sources:**
- Pre-split design study: `docs/superpowers/specs/2026-05-12-lessons-from-siblings.md` (cairn / moor-echo / invalidation evaluation).
- moor-echo's `SystemTracer` provides the `MigrationRule` + `MigrationEngine` pattern adopted in ITER-0005a (port the design in-house; no `moor-echo` dependency).
- `invalidation` is NOT used by this family — it solves cache-freshness, not schema migration. Deferred per `feedback_dependency_policy.md`.
- The cairn-borrowed span-on-every-Instruction discipline is orthogonal to persistence; tracked as a separate iteration candidate (Appendix B of the lessons doc).

**Cross-iteration impacted scenarios:** SCENARIO-0007, SCENARIO-0008, SCENARIO-0009, SCENARIO-0010, SCENARIO-0011, SCENARIO-0099, SCENARIO-0100, SCENARIO-0101, SCENARIO-0102. (SCENARIO-0110 was planned for ITER-0005a.0; deferred 2026-05-12 — see ITER-0005a.0's deferral rationale. It will be authored alongside the first real `MigrationRule` in a future schema-change iteration.)

**Cross-iteration depends on:** ITER-0004b (single canonical representation — see ITER-0004b "Why before persistence").

**Cross-iteration look-ahead:** ITER-0005f's close unblocks ITER-0006 (Self-Compile and Seed). Per-iteration look-aheads call out which sub-iteration each downstream artifact actually needs.

---

#### ITER-0005a.0 (RESCOPED 2026-05-13) — Extract `fmpl-types` shared-types crate (`VmVersion` + `Hash` + `SourceHash`)

**Stories:** none (cross-cutting architectural extraction; prerequisite for 0005a.5; absorbs the `Hash` newtype work originally planned for ITER-0005b).

**Status:** done (2026-05-14). `fmpl-types` crate shipped at `fmpl-types/{src/{lib,vm_version,hash}.rs, Cargo.toml}`. 12/12 smoke tests pass (6 vm_version + 6 hash). Workspace builds clean with `--all-features` (207 crates, 0 errors). Sentinel sweep 1352/1352 — zero regressions. Wall-clock T0→T5 code-complete: ~5.5 minutes per `/tmp/iter-0005a.0-checkpoints/`. **Implementation deviations from the card:** (a) `blake3` dep dropped per R5-S2/S3 — `Hash::compute` deferred to ITER-0005b where the source store needs it; (b) all 6 R5 textual fixes applied inline before T0 started. **0005a.5 + 0005a.6 unblocked.**

**Driving evidence (2026-05-13):**

1. **R4-C1 dep-graph contradiction in 0005a.5.** PAR Round-4 surfaced that 0005a.5's T0.5 needs cross-crate access to `VmVersion` while simultaneously requiring `cargo build -p fmpl-core --no-default-features` to build without an fmpl-persistence dep. The contradiction has only one structurally clean resolution: a shared-types crate that both fmpl-core and fmpl-persistence depend on UNCONDITIONALLY.
2. **Three concrete shared types exist today** (no premature abstraction): `VmVersion` (semver triple — needed by envelope writer + loader compat check + fmpl-core's CARGO_PKG_VERSION derivation); `Hash` newtype (originally ITER-0005b scope — content-addressing source store); `SourceHash` alias for `Hash` (used in `EnvelopeHeader.source_hash` today as bare `[u8; 32]`).
3. **`ship-infrastructure-with-first-consumer.md` satisfied.** Three real consumers (fmpl-core via vm_version.rs + writer call sites; fmpl-persistence via envelope.rs + loader.rs signatures; fmpl-bootstrap can take a lean fmpl-types dep without fjall) each have a concrete need for at least one of the types.

**Scope (build order):**

1. **T0 — Create `fmpl-types` crate skeleton.** New workspace member at `fmpl-types/`. `Cargo.toml`:
   ```toml
   [package]
   name = "fmpl-types"
   version.workspace = true
   edition.workspace = true
   authors.workspace = true
   license.workspace = true
   description = "FMPL shared cross-crate types: VmVersion, Hash, SourceHash"

   [dependencies]
   serde = { workspace = true }
   # NOTE (R5-S2/S3 fix): blake3 is NOT added until 0005b adds Hash::compute().
   # `Hash::from_bytes` is a plain newtype wrap, no hashing primitive needed.

   [dev-dependencies]
   serde_json = "1.0"
   ```
   Add `fmpl-types` to workspace `Cargo.toml:3-11` members array (alphabetical position).

2. **T1 — Define `VmVersion`.**
   ```rust
   /// Semantic version triple as a 6-byte value-type.
   /// Constructed from `env!("CARGO_PKG_VERSION")` at consumer crate
   /// compile time via the `parse_version_part` const fn.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
   pub struct VmVersion {
       pub major: u16,
       pub minor: u16,
       pub patch: u16,
   }

   impl VmVersion {
       pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
           Self { major, minor, patch }
       }
   }

   /// Parse a version-part digit substring from CARGO_PKG_VERSION at const time.
   /// `index = 0` returns major, 1 minor, 2 patch.
   pub const fn parse_version_part(version: &str, index: usize) -> u16 { ... }
   ```
   `parse_version_part` is the const fn currently at `fmpl-core/src/persistence/schema.rs:155-194` — moved here. Includes the existing test suite (`#[cfg(test)] mod tests`) from that file.

3. **T2 — Define `Hash` + `SourceHash`.**
   ```rust
   /// blake3-derived 32-byte hash. Used for content-addressing source bytes
   /// and (in future iterations) compiled-artifact deduplication.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
   pub struct Hash(pub [u8; 32]);

   impl Hash {
       pub const fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }
       pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }

       /// Sentinel hash for "no source" — all-zero bytes. Used by
       /// envelope records that have no associated source (e.g. spawned
       /// objects, runtime grammars) until ITER-0005b wires source recovery.
       pub const NONE: Hash = Hash([0u8; 32]);
   }

   /// Type alias for the source-bytes hash used in EnvelopeHeader.
   pub type SourceHash = Hash;
   ```
   T2 designs the `Hash::NONE` constant in fmpl-types ONLY. **fmpl-core/envelope.rs is NOT edited in 0005a.0.** Per R5 PAR feedback + the "Out of scope" rule below, the actual `NO_SOURCE_HASH` field's relationship with `Hash::NONE` is owned by 0005a.5's T0.5 — which (per R5-C1 zerocopy compat resolution) keeps `NO_SOURCE_HASH: [u8; 32]` unchanged in envelope.rs and adds a helper `fmpl_types::no_source_hash() -> Hash` for API-edge consumers.

4. **T3 — Smoke tests.** `fmpl-types/tests/`:
   - `parse_version_part` exercises the const-fn parser against several version strings.
   - `Hash::NONE` is all-zeros.
   - `serde` round-trips `VmVersion` and `Hash` to JSON.
   - `VmVersion::new(1, 2, 3)` builds at const time (`const _: VmVersion = VmVersion::new(1, 2, 3);`).

5. **T4 — Wire fmpl-types into the workspace.** Update workspace `Cargo.toml` to add `fmpl-types` as a `[workspace.dependencies]` entry: `fmpl-types = { path = "fmpl-types" }`. **No consumer crate changes yet** — that happens in 0005a.5.

6. **T5 — Wrap artifacts.**
   - Iteration log entry with wall-clock from `/tmp/iter-0005a.0-checkpoints/`.
   - progress.md snapshot.
   - Roadmap status → done.
   - Update ITER-0005b's card text to note that `Hash` newtype is now shipped in 0005a.0 (its 0005b work shrinks to: content-addressed source store + constructor synthesis only).
   - Save lesson: "shared-types crate is the structurally clean answer when a single shared type creates a feature-gating contradiction" — record as feedback memory.

**Impacted scenarios:**

- None. This is pure infrastructure — no behavior change, no scenario updates. The crate ships with smoke tests in T3 but no SCENARIO-NNNN cards.

**Verification gates:**

- `cargo build -p fmpl-types` builds.
- `cargo build --workspace --all-features` builds (proves the new workspace member integrates cleanly).
- `cargo test -p fmpl-types` passes (T3 smoke tests).
- Full sentinel sweep passes: `cargo test -p fmpl-core --no-fail-fast` (baseline 1352 from 0005a.3 close — no regression, since no consumers are touched yet).
- Citation check clean.
- Clippy clean across all members.

**Out of scope (deferred):**

- **Consumer-side changes** — no fmpl-core/fmpl-persistence/fmpl-web edits this iteration; those happen in 0005a.5 + 0005a.6 (and the 0005a.0 card explicitly does NOT change any existing imports).
- **Migration helpers for legacy `[u8; 32]` source-hash sites** — handled in 0005a.5 as part of the envelope-header reshape.
- **`Hash` content-addressing semantics** — `Hash::compute(bytes: &[u8])` helper is NOT added this iteration. 0005b adds it when the source store actually uses it.

**Dependencies / ordering:**

- BLOCKED BY: ITER-0005a.3 (done).
- BLOCKS: ITER-0005a.5 (0005a.5's T0.5 + T3 assume `fmpl_types::VmVersion` exists).
- SHRINKS scope of: ITER-0005b (the `Hash` newtype work is moved here; 0005b's T-tasks for newtype design are dropped).

**Risk callouts:**

- **`Hash` newtype design without an immediate content-addressing consumer.** This iteration ships `Hash` ahead of any code that does `blake3::hash(source_bytes)`. Per ship-infrastructure-with-first-consumer, we'd normally object. The exception here: `Hash` already has TWO concrete consumers via 0005a.5 (envelope `source_hash` field reshape + the `NO_SOURCE_HASH` rename to `Hash::NONE`). The `compute()` method is deferred — that's the truly speculative part.
- **`SourceHash` as `type Hash = Hash;` is a trivial alias.** Slight readability cost vs naming directly. Trade-off: the alias makes call sites (`source_hash: SourceHash`) clearer than `source_hash: Hash` while costing zero extra type definition.
- **Workspace dep-graph cycle check.** fmpl-types has NO deps on any other workspace member. fmpl-core/fmpl-persistence/fmpl-web/fmpl-bootstrap depend on fmpl-types. No cycles.

**Sources:**

- 2026-05-13 PAR Round-4 finding R4-C1 (contradiction in 0005a.5's T0.5 dep-graph).
- 2026-05-13 user architectural call: "Shouldn't we have a common structures crate? ... Introduce fmpl-types now with VmVersion + Hash + SourceHash."
- `feedback_ship_infrastructure_with_first_consumer.md` (three concrete consumers satisfy the discipline).
- `feedback_dependency_policy.md` (new workspace-internal crate is acceptable; serde + blake3 are public crates earning compile-time cost).

---

#### ITER-0005a.0 (ORIGINAL — MigrationEngine; DEFERRED — ships with first real MigrationRule)

**Stories:** none. Was pure infrastructure; deferred pending a consumer.

**Status:** deferred (2026-05-12 — PAR scope review).

**Deferral rationale (2026-05-12, PAR aggregate):** Two parallel scope reviewers independently reached REVISE with the same primary finding: **shipping MigrationEngine before any consumer is YAGNI**. Aggregated points:

1. The engine ships **empty** through 0005a.0 / 0005a.1 / 0005a.2 / 0005b. The first real `MigrationRule` doesn't land until a hypothetical future schema-change iteration (likely ITER-0005c.1+ or successor). No concrete consumer exists in this iteration family.
2. The previous rationale ("0005a.1 routes envelope loading through `MigrationEngine::migrate` before falling back to skip") was structurally a no-op routing — with zero rules, `migrate()` is a pass-through and 0005a.1's loader produces identical observable behavior whether it routes through the engine or skips directly. The routing claim was structural-elegance, not behavior-driven.
3. Architectural commitments (payload type for `PersistedRecord`, `validate` failure semantics, `priority` vs `from_version` dispatch model, `conflicts_with` inclusion/exclusion) cannot be made well without a concrete consumer. The lessons-from-siblings doc explicitly flagged the payload type as Open Question 4 (`docs/superpowers/specs/2026-05-12-lessons-from-siblings.md:377`) — designing in the abstract would commit to choices the first real schema change would likely revise.

**The durable artifact is the lessons-from-siblings doc** (`docs/superpowers/specs/2026-05-12-lessons-from-siblings.md` §2). The moor-echo `TransformationRule` pattern is captured there with concrete file:line citations. Porting the pattern into FMPL is a ~30-minute lift when the first real `MigrationRule` arrives — and at that point, the consumer's needs will pin the payload type, the dispatch model, and the failure semantics correctly the first time.

**When to revive:** the first iteration that introduces a breaking schema change to any persisted payload class (CompiledCode, Object, Grammar, ParseState, memo table, full-VM snapshot). At that point: lift the trait + runner from the lessons doc, parameterize over the concrete payload type the new schema demands, ship the engine alongside the first rule and SCENARIO-0110 alongside both. The infrastructure-and-its-first-consumer iteration is one iteration, not two.

**Lessons preserved:** the PAR scope review's structural findings (premature abstraction, routing-as-no-op, payload type undefined) are also a generalizable lesson — "ship infrastructure with its first consumer, not ahead of it." Worth recording as a feedback memory (separate task).

---

#### ITER-0005a.1 — STORY-0099 envelope format + loader (PAR-revised 2026-05-12)

**Stories:** STORY-0099 (versioned envelope) — AC-1, AC-2, AC-3, AC-4, AC-6. AC-5 (call-site sweep) and AC-7 (LoaderStats) are both deferred to ITER-0005a.2.

**Status:** done (2026-05-13). STORY-0099 ACs 1, 2, 3, 4, 6 closed; AC-5 and AC-7 remain pending in ITER-0005a.2. Implementation at `fmpl-core/src/persistence/{schema,envelope,checksum,loader}.rs`; evidence at `fmpl-core/tests/scenario_0099_envelope_loader.rs` (SCENARIO-0099 four-record skip journey) + `fmpl-core/tests/persistence_schema_anti_rot.rs` (AC-6 anti-rot ratchet). +33 tests (30 module-internal + 3 integration), sentinel sweep 1329/1329 passing, clippy clean. zerocopy 0.8 + blake3 1 dependencies added per the pre-iteration adoption notes; both compile-time invariants (`size_of == 56`, `align_of == 1`) hold; checksum is `blake3(header_with_crc_zeroed || payload)` truncated to lower 32 bits.

**Rationale:** Lands the envelope struct, the loader's skip-on-incompatible logic, and the `persistence::schema` module. Does NOT touch existing `save_to_fjall` callers (deferred to 0005a.2's sweep) and does NOT surface loader statistics (deferred to 0005a.2 where swept callers will actually consume them). This split keeps the envelope design self-contained — testable in isolation against synthetic records — before any production caller depends on it.

**PAR scope review (2026-05-12 — REVISE → revisions applied below):** Two parallel reviewers independently identified the issues now folded into this card. Critical finding: T2's original "advance by N bytes" wording described a contiguous-byte-stream substrate, but Fjall is a key-value store where each value IS a self-contained envelope. Multiple Serious findings resulted in the design adjustments below: source seam consolidation, AC-7 deferral, typed-invariant size assertion, anti-rot ratchet, dependency-policy compliance on the checksum crate, flag-field semantics, PayloadKind extensibility, and index-record handling.

**Dependency adoptions (2026-05-12):**

1. **`zerocopy = "0.8"` + `zerocopy-derive = "0.8"`** (public Google-maintained crate, no_std, triple-licensed BSD/Apache-2.0/MIT, no runtime transitive deps) for the `EnvelopeHeader` struct. AC-1's fixed-layout binary header is zerocopy's canonical use case. The `#[derive(FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)] #[repr(C)]` derives replace ~80-100 lines of hand-rolled byte parsing with ~25 lines of declarative struct definition + compile-time alignment + size + endianness invariants. Per `feedback_prefer_proof_tests.md`, the compile-time guarantees are typed invariants (the strongest form). Scoped to `fmpl-core/src/persistence/envelope.rs` only.

2. **`blake3 = "1"`** for the envelope checksum AND (in ITER-0005b) for source content-addressing. Single hash algorithm across the persistence family. Per blake3's XOF property, the **lower 32 bits of `blake3(header_with_crc_zeroed || payload || source)`** are a valid 32-bit checksum equivalent to calling blake3 with `output_len=4`. Strictly equal-or-better than CRC32 for corruption detection: same 32-bit width, cryptographic uniformity over the truncation, no algebraic blind spots. Compare-checked against `crc32fast`, `crc32c` (CRC32C via SSE4.2), `xxhash-rust`, and `farmhash` (unmaintained since 2019) — blake3 wins on consistency (one hash crate vs. two) since ITER-0005b's STORY-0100 needs blake3 anyway for source content-addressing. The `crc32: U32<LE>` field name is preserved for AC-1 wording stability; the in-code computation uses blake3. Per `feedback_dependency_policy.md`, both are Tier 1 (public, maintained, no other-language runtime).

**Impacted scenarios:** SCENARIO-0099 (the 4-skip-case journey: A loads, B/C/D skip without aborting). Stats-counter assertions remain in the scenario but are handled by the test harness's own local counters, not a public `LoaderStats` API (AC-7 deferred).

**Depends on:** ITER-0004b. (ITER-0005a.0 was deferred 2026-05-12 — loader skips incompatible-schema records directly without a migration engine pre-call. When the first real schema change arrives, that successor iteration introduces the `MigrationEngine` + its first `MigrationRule` together as `MigrationVisitor` on ITER-0005e's tracer substrate per the lessons-doc §2.5 reframe.)

**Look-ahead:** ITER-0005a.2 sweeps existing writers through this envelope helper (AC-5); ITER-0005a.3 wires the loader read-side rewire + `LoaderStats` public API (AC-7) — the 0005a.2 / 0005a.3 split was sanctioned by 2026-05-13 PAR scope review on the original 0005a.2 card. ITER-0005b adds the content-addressed source store that the envelope's `source_hash` field references. A future schema-change iteration adds `MigrationVisitor` to ITER-0005e's tracer substrate.

**Build order:**

1. **T0 — `persistence::schema` module (AC-6) + `PayloadKind` taxonomy.** Centralize:
   - The `env!("CARGO_PKG_VERSION")`-derived VM version constants (major, minor, patch).
   - The per-payload-kind schema version constants.
   - A `PayloadKind` non-exhaustive `u8` with `try_from(u8) -> Option<PayloadKind>` conversion. Non-exhaustive parsing (the loader's AC-3 "unknown payload_kind → skip" path requires graceful handling of bytes that don't match any known variant). The initial variant set anticipates the iterations that will add per-kind variants:
     - `0x01 ObjectRecord` — per-object body record (ITER-0005d adds this in object.rs sweep)
     - `0x02 ObjectIndex` — the `__object_ids__` index record (resolves PAR finding #11 — `object.rs::save_to_fjall` writes TWO record shapes; both need PayloadKind variants for 0005a.2's invariant gate to be satisfiable)
     - `0x03 CompiledCode` — bytecode (ITER-0005c)
     - `0x04 Grammar` — grammar definition (ITER-0005d)
     - `0x05 GrammarRegistry` — top-level registry (ITER-0005d)
     - `0x06 ParseState` — incremental parse state (ITER-0005d)
     - `0x07 MemoTable` — grammar memo cache (ITER-0005d)
     - `0x08 VmSnapshot` — full image snapshot envelope (ITER-0005e)
   - The variant numbering is reserved-and-documented at 0005a.1 entry; subsequent iterations populate the matching writer paths. Adding variants is allowed; renumbering is not (wire-format stability).
   - **AC-6 anti-rot ratchet (proof-like, per `feedback_prefer_proof_tests.md` form #4).** A `cargo test`-level invariant asserts: no file under `fmpl-core/src/` outside `persistence::schema` contains the literal string `"CARGO_PKG_VERSION"` or any of `vm_version_major`, `vm_version_minor`, `vm_version_patch` as a string literal or hardcoded numeric. The gate uses the existing scenario_runner's `expect_absent` action-type. Authored as a SCENARIO-0099 case OR a separate scenario card — pick based on cohesion (most likely fits as a new scenario adjacent to SCENARIO-0099, kind: invariant).

2. **T1 — `EnvelopeHeader` struct (AC-1) via zerocopy.** Add `zerocopy = "0.8"` + `zerocopy-derive = "0.8"` and `blake3 = "1"` to `fmpl-core/Cargo.toml`. Define the header as:
   ```rust
   #[derive(KnownLayout, Immutable, FromBytes, IntoBytes, Unaligned)]
   #[repr(C)]
   pub struct EnvelopeHeader {
       pub magic: [u8; 4],                  // b"FMPL"
       pub envelope_format_version: U16<LE>,
       pub payload_kind: u8,                 // see persistence::schema::PayloadKind
       pub flags: u8,                        // must-be-zero in 0005a.1; loader REJECTS nonzero (resolves PAR flag-semantics finding)
       pub vm_version_major: U16<LE>,
       pub vm_version_minor: U16<LE>,
       pub vm_version_patch: U16<LE>,
       pub schema_version: U16<LE>,          // per persistence::schema::SCHEMA_VERSION_<KIND>
       pub payload_len: U32<LE>,
       pub source_hash: [u8; 32],            // blake3 hash of source bytes; all-zeros means "no source for this record" (populated by ITER-0005b's source store)
       pub crc32: U32<LE>,                   // low 32 bits of blake3(magic || header_with_crc_zeroed_and_source_hash_present || payload); see persistence::checksum::compute
   }
   ```
   - **Source seam decision (resolves PAR source-seam-conflict finding):** dropped the original `source_len: U32<LE>` inline-after-payload field; replaced with `source_hash: [u8; 32]` reference. ITER-0005b populates the hash store the hashes point at. All-zeros = no source. This is the same wire format STORY-0100 demands, eliminating the wire-format break Reviewer A and B both flagged.
   - **Header byte count:** 4 + 2 + 1 + 1 + 2 + 2 + 2 + 2 + 4 + 32 + 4 = **56 bytes.**
   - **Header is intentionally NOT padded to a power of 2.** Fjall is a K/V store; each record's value is its own allocated buffer, so there is no contiguous-record alignment benefit (this is the same K/V-not-stream insight that resolved the PAR critical finding). The `flags: u8` field is the reserved-must-be-zero slot for micro-bumps. Header growth should be rare and handled via `envelope_format_version` dispatch (v2 readers decode against `EnvelopeHeaderV2` based on the byte; v1 records keep decoding against `EnvelopeHeaderV1`). Growable per-payload metadata belongs in the **payload's own format** (decision deferred to ITER-0005c — likely `prost` or `flatbuffers` for variable-width upgrade semantics). Payload-side padding to power-of-2 boundaries may be revisited in 0005c if Fjall benchmarks show fragmentation cost.
   - **Compile-time typed-invariant assertion (per `feedback_prefer_proof_tests.md` form #1):**
     ```rust
     const _: () = assert!(std::mem::size_of::<EnvelopeHeader>() == 56);
     const _: () = assert!(std::mem::align_of::<EnvelopeHeader>() == 1);
     ```
     A future field reorder or addition is a compile-time error, not a runtime regression.
   - **`persistence::checksum::compute(magic, header_no_crc, payload) -> u32`** — wraps blake3 with the documented truncation. ~10 lines. Hash input is the header with `crc32` zeroed (the standard CRC-of-itself pattern), and includes the payload. Source bytes are NOT in the checksum input: they're integrity-checked via `source_hash`'s own self-reference (the hash is the source's identity per content-addressing). Closes Reviewer B's source-bytes-integrity-gap finding via a different mechanism than widened-CRC: content-addressing IS the source integrity check.
   - Round-trip unit tests (encode → decode → equal) via `EnvelopeHeader::ref_from_prefix(&buf)` / `header.as_bytes()`.

3. **T2 — Loader with 4 skip cases (AC-2, AC-3, AC-4).** Loader iterates a Fjall keyspace (`keyspace.iter()`); for each `(key, value)` pair:
   - `EnvelopeHeader::ref_from_prefix(&value)` decodes the header from the value's prefix; the remainder of the value is the payload.
   - Magic-mismatch → log + skip (not in AC; but defensive).
   - `flags != 0` → log + skip (resolves PAR flag-semantics finding: nonzero is rejected per the reserved-must-be-zero spec).
   - VM-major-mismatch → log + skip (AC-2).
   - Unknown `payload_kind` (via `PayloadKind::try_from` returning None) OR unknown `schema_version` for a known kind → log + skip (AC-3).
   - `persistence::checksum::compute(...)` mismatch → log + skip (AC-4).
   - All-checks-pass → return for downstream callers.

   **Critical PAR resolution (stream-vs-keyspace ambiguity):** the previous wording "advance by `payload_len + source_len + size_of::<EnvelopeHeader>()`" is removed. Skip means "ignore this `(key, value)` and continue to the next iterator entry" — not byte-arithmetic on a stream. The `payload_len` field is used to validate `value.len() == size_of::<EnvelopeHeader>() + payload_len` (a sanity check on record framing); a mismatch is a corruption skip (AC-4 path).

4. **T3 — SCENARIO-0099 evidence (AC-2, AC-3, AC-4) + AC-6 anti-rot ratchet.**
   - **SCENARIO-0099 evidence:** authored as a focused Rust integration test under `fmpl-core/tests/` (NOT a data-driven step-def). Per `feedback_prefer_proof_tests.md`, the Rust integration test directly asserts on typed values (counters as `u32`, equality via `==`) — closer to form #1 typed invariants than form #5 pointwise data. The choice is principled, not pragmatic. Test harness constructs 4 records (A current, B vm-major-ahead, C unknown-payload-kind, D corrupted-checksum), runs the loader, and asserts: A loaded, B/C/D each skipped via the correct skip-reason, harness-local counters match expected `(1, 1, 1, 1)`.
   - **AC-6 anti-rot ratchet:** the typed-invariant gate from T0's third bullet runs in CI; any future contributor inlining `CARGO_PKG_VERSION` outside `persistence::schema` fails the gate.

5. **T4 — Wrap artifacts.** roadmap (mark 0005a.1 done), iteration-log entry, progress.md update, EPIC-003 STORY-0099 status note (AC-1, AC-2, AC-3, AC-4, AC-6 done; AC-5 + AC-7 pending in 0005a.2).

**Verification gates:**
- SCENARIO-0099 passes the 4-skip-case journey.
- `cargo test` passes the AC-6 anti-rot ratchet.
- `const _: () = assert!(size_of::<EnvelopeHeader>() == 56)` compiles (typed invariant; failure = compile error, not test failure).
- `persistence::schema` module exports `VM_VERSION_*`, `PayloadKind`, and `SCHEMA_VERSION_*` constants.
- Sentinel sweep green.
- Clippy clean on default features.

**Out of scope (deferred to ITER-0005a.2):**
- Routing the existing `save_to_fjall` callers in `compiler.rs`, `object.rs` (TWO record shapes: per-object + `__object_ids__` index — both anticipated in T0's PayloadKind taxonomy), `grammar/incremental.rs`, and the raw-serde patterns in `grammar/stream_input.rs` through the envelope helper.
- `LoaderStats { loaded, skipped_incompatible, skipped_corrupt, skipped_unknown_kind }` as a public observability API (AC-7). Deferred because in 0005a.1 no production caller writes envelopes and no operator reads stats — the SCENARIO-0099 harness counts skips locally. AC-7 lands in 0005a.2 alongside the swept callers that produce operator-visible stats. (Resolves PAR "AC-7 is observability ahead of consumer" finding via the same `feedback_ship_infrastructure_with_first_consumer.md` discipline that deferred ITER-0005a.0.)

**Out of scope (deferred to ITER-0005b):**
- Populating `source_hash` from a content-addressed source store. In 0005a.1, all records written by test harnesses use `source_hash: [0; 32]` (the "no source" sentinel). ITER-0005b adds the `sources` Fjall partition and the writer-side `compute source hash + put bytes in store` path.

**PAR-aggregate findings → resolution map:**

| PAR finding | Severity | Resolution in this revised card |
|---|---|---|
| Stream-vs-keyspace ambiguity | Critical | T2 rewritten: skip = "next iterator entry," not byte arithmetic |
| Source seam fights STORY-0100 | Serious | Replaced `source_len` with `source_hash: [u8; 32]`; 0005a.1 ships the wire format STORY-0100 demands |
| AC-7 observability ahead of consumer | Serious | Deferred to 0005a.2; SCENARIO-0099 uses harness-local counters |
| No `size_of::<EnvelopeHeader>()` typed invariant | Serious | Added as `const _: () = assert!(...)` in T1; both size and alignment asserted |
| AC-6 has no anti-rot ratchet | Serious | Added as T0 final bullet; uses scenario_runner's `expect_absent` action |
| CRC32 dependency unspecified | Serious | Specified `blake3 = "1"`; documented per-`feedback_dependency_policy.md`; reused across 0005b |
| `flags: u8` undocumented | Serious | Specified "must-be-zero; loader REJECTS nonzero" in T2 |
| `PayloadKind` extensibility unspecified | Serious | Specified as non-exhaustive `u8` with `try_from`; initial variant taxonomy reserved with kind numbers |
| `object.rs::__object_ids__` index breaks invariant gate | Serious | PayloadKind taxonomy includes `ObjectIndex` variant; 0005a.2's invariant gate becomes satisfiable |
| `write<T: Serialize>` helper signature mismatch | Serious | Resolved by source-seam decision (header now has `source_hash`; 0005a.2's helper signature is correct as written) |
| Stale ITER-0005c `MigrationEngine` reference | Minor | Cleaned up in separate edit below |
| SCENARIO-0099 step-def-vs-integration-test pragmatic | Minor | T3 makes the choice principled (integration test for direct typed assertions) |
| Source-bytes integrity gap | Minor (Reviewer B) | Resolved structurally: source integrity is the source_hash's job (content-addressing IS the integrity check), not the envelope CRC's |

---

#### ITER-0005a.2 — STORY-0099 AC-5 write-side sweep (PAR-revised 2026-05-13)

**Stories:** STORY-0099 (versioned envelope) — AC-5 (the call-site sweep) only. AC-7 (LoaderStats public API) is split into ITER-0005a.3 because it touches the loader read path, not the writer call sites; per `feedback_ship_infrastructure_with_first_consumer.md` its consumer (the SCENARIO-0099 harness rebinding to read stats via a public API) belongs alongside the loader-read-side wiring, which is structurally distinct from the writer sweep.

**Status:** done (2026-05-13). STORY-0099 AC-5 closed: writer sweep routes all 4 currently-extant `save_to_fjall` call sites (compiler.rs CompiledCode, object.rs ObjectDb's two record shapes ObjectIndex+ObjectRecord, grammar/incremental.rs ParseState, grammar/stream_input.rs spill+memo) through `persistence::envelope::write`. AC-5 invariant gate at `tests/persistence_envelope_invariant.rs` confirms no raw `keyspace.insert(` survives outside `persistence/envelope.rs`. Transitional manual prefix-strip on load sides marked `// TODO(ITER-0005a.3)`; permanent rewire through `loader::decode` is the AC-7 work in ITER-0005a.3 (next pending iteration). Test counts: 1335 default features (+3 vs ITER-0005a.1 baseline of 1332) | 1344 under `--features fjall-persistence`. Clippy clean on both. AC-1 wording fix to EPIC-003 applied at T6 (limit to currently-extant writers; Lambda/Grammar/VmSnapshot deferred to 0005d/e).

**Rationale:** With the envelope format frozen by ITER-0005a.1, this iteration ships the **write-side** sweep: one envelope-aware writer helper that all current persistence call sites route through. The **read-side** rewire (each `load_from_fjall` → `loader::decode → typed payload deserialize`) and the public `LoaderStats` surface are deferred to ITER-0005a.3 to keep this iteration single-concern and avoid silently bundling reader observability into a writer sweep.

**Dependency adoption (none new):** the helper uses `zerocopy 0.8` + `blake3 1` already adopted in ITER-0005a.1. No new Cargo entries.

**Source-hash parameter decision (resolves PAR Critical #2):** the helper accepts `source_hash: [u8; 32]` directly, NOT `Option<Hash>`. The `Hash` type doesn't exist until ITER-0005b ships the content-addressed source store; speculating on its shape here would lock in 5 downstream iterations on guesswork. All four sweep targets in this iteration pass `persistence::envelope::NO_SOURCE_HASH` (the all-zeros sentinel) per ITER-0005a.1's contract. ITER-0005b will (a) introduce a `Hash` newtype around `[u8; 32]` AND (b) add `source_hash`-populating logic at writer sites that have an originating source string. Until then, the helper signature stays grounded in types that actually exist.

**Wire-format break acknowledgment (resolves PAR Serious #5):** sweeping these call sites changes the on-disk format — every record grows by 56 bytes (`size_of::<EnvelopeHeader>()`). Pre-sweep records will route through `DecodeOutcome::SkippedCorrupt(BadMagic)` or `ValueTooShort` after the sweep. **This is acceptable because `fjall-persistence` is feature-gated and has no production consumers today.** No migration plan is needed; pre-existing Fjall databases under `fjall-persistence` are not durable user data. A CHANGELOG entry suffices (handled at T6).

**Feature-flag uniformity decision (resolves PAR Serious #9):** the envelope helper module (`persistence::envelope`) stays unconditional (matches ITER-0005a.1's current state). Two of the four call sites are already `#[cfg(feature = "fjall-persistence")]`-gated (`compiler.rs:697`, `object.rs:177`); two are not (`grammar/incremental.rs:71`, `grammar/stream_input.rs` writers). This iteration preserves the existing gating asymmetry — the sweep at each call site inherits whatever gating that site already has. `fjall` is itself non-optional (`Cargo.toml:21`), so the unconditional path is mechanically free. Closing the asymmetry is a separate concern (could land in a future hardening iteration); avoiding it here keeps the sweep mechanical.

**`.expect()` vs `Result` decision for `stream_input.rs` (resolves PAR Serious #4):** preserve the current panic-on-write semantics by `.expect()`-ing the helper's result at each call site. The current `StreamPosition`/`MemoFjall` write paths return values that are not `Result`-bearing; converting them to fallible would cascade through their callers (a breaking change to `stream::PullStream` and `grammar::MemoStream` consumers). Preserving panic semantics is the smaller-blast-radius choice. The panic surface widens slightly (was `serde_json::to_vec` only; now also `keyspace.insert` and checksum compute) but in practice all three are infallible-in-our-use-pattern. A future hardening iteration could plumb `Result` if needed.

**Object-record two-shape handling (resolves PAR Serious #3):** `object.rs::ObjectDb::save_to_fjall` writes two record shapes per call (`__object_ids__` index + per-object records). The sweep calls the helper **twice**: once with `PayloadKind::ObjectIndex` for the index (using the reserved 0x02 variant from ITER-0005a.1's taxonomy), then in a loop with `PayloadKind::ObjectRecord` (0x01) per object. The helper stays single-record; no batch-write mode needed.

**AC-5 wording delta (resolves PAR Serious #8):** STORY-0099 AC-5's enumeration (`Object, CompiledCode, Grammar, ParseState, Lambda, and full-VM-snapshot`) names payload classes that have no writers today (`Lambda`, `Grammar` as separate class, `VmSnapshot`). This iteration closes AC-5 only for the **currently-extant** writer set: `CompiledCode`, `Object`, `ObjectIndex`, `ParseState`, `MemoTable`, `StreamPosition`. EPIC-003's AC-5 wording will be updated at T6 to read "all currently-extant `save_to_fjall` callers + raw-serde keyspace.insert sites" rather than the original aspirational enumeration. `Lambda`/`Grammar`/`VmSnapshot` writer adoption lands when those payload classes acquire writers in ITER-0005d/e — at which point their writers naturally go through the helper from day one (the helper is the only way to write, per T5's invariant gate).

**Impacted scenarios:** SCENARIO-0099 still passes (envelope contract unchanged). New SCENARIO-0111 (NEW) covers writer→loader round-trip through the helper for each PayloadKind variant present in this iteration's sweep — this is the test that proves AC-5 at the integration seam.

**Depends on:** ITER-0005a.1.

**Look-ahead:** ITER-0005a.3 lands LoaderStats + read-side decode wiring (closes AC-7). ITER-0005b/c/d/e/f all write through this helper.

**Build order:**

1. **T0 — `persistence::envelope::write` helper.** Land in `fmpl-core/src/persistence/envelope.rs` (extends the existing module; no new file). Signature:
   ```rust
   pub fn write<T: serde::Serialize>(
       keyspace: &fjall::Keyspace,
       key: &[u8],
       value: &T,
       kind: PayloadKind,
       source_hash: [u8; 32],
   ) -> Result<(), EnvelopeWriteError>;
   ```
   Implementation: serialize `value` via `serde_json::to_vec`, construct `EnvelopeHeader::new_for_current_vm(kind, payload.len() as u32, source_hash).finalize_checksum(&payload)`, concatenate `header.as_bytes() ++ payload`, `keyspace.insert(key, bytes)`. New error type `EnvelopeWriteError` wraps `serde_json::Error` and `fjall::Error`. Unit tests cover happy path + serialization failure surfaced as typed error.
2. **T1 — Sweep `compiler.rs::CompiledCode::save_to_fjall`** through the helper. Pass `PayloadKind::CompiledCode` (0x03), `NO_SOURCE_HASH`. Existing unit test (if any) keeps `save → load_from_fjall` round-trip but is expected to fail at the load side until ITER-0005a.3 wires `loader::decode` in (because the bytes on disk now contain the envelope prefix that `serde_json::from_slice` won't parse). **Temporary measure for T1-T4:** each swept caller's `load_from_fjall` reads the value, strips the first 56 bytes manually, then `serde_json::from_slice`s the remainder. This is **explicitly transitional** — ITER-0005a.3 replaces the manual prefix-strip with `loader::decode`. Document the transitional pattern with a `// TODO(ITER-0005a.3)` comment at each site.
3. **T2 — Sweep `object.rs::ObjectDb::save_to_fjall`** through the helper. Two calls per save: one `PayloadKind::ObjectIndex` (0x02) for `__object_ids__`, then a loop of `PayloadKind::ObjectRecord` (0x01) per object. Same transitional load-side prefix-strip pattern.
4. **T3 — Sweep `grammar/incremental.rs::ParseState::save_to_fjall`** through the helper. `PayloadKind::ParseState` (0x06), `NO_SOURCE_HASH`. Same transitional load.
5. **T4 — Sweep `grammar/stream_input.rs` writers** through the helper. Two sites: position-spill at lines 457-468 (`PayloadKind` TBD — likely needs a new variant or reuses an existing one; check during implementation), memo-write at lines 551-554 (`PayloadKind::MemoTable` 0x07). Each call site `.expect()`s the helper's `Result` to preserve current panic semantics.
6. **T5 — AC-5 invariant gate (typed-invariant form per `feedback_prefer_proof_tests.md` form #1).** Add a `cargo test`-level scan asserting: zero occurrences of `keyspace.insert(` or `partition.insert(` (the raw fjall write API) appear in `fmpl-core/src/` outside `persistence/envelope.rs`. This is form #4 (universally-quantified structural assertion via grep) — the strongest **feasible** form because `fjall::Keyspace::insert` is a foreign-crate public method that we cannot seal at the type level (visibility constraints don't reach across crate boundaries). The grep gate enforces "every persistence write goes through the envelope helper." Author as a new `fmpl-core/tests/persistence_envelope_invariant.rs` test file modeled on `persistence_schema_anti_rot.rs` (the AC-6 ratchet); the form has precedent.
7. **T6 — Wrap artifacts.** `roadmap.md` (status → done). `iteration-log.md` entry. `progress.md` update. `EPIC-003.md` STORY-0099 status note: AC-5 done in 0005a.2 for current writers; AC-5 wording fix-up (limit to extant writers); note AC-7 remains pending in 0005a.3. SCENARIO-0099 card unchanged (still validates the envelope contract end-to-end). SCENARIO-0111 (NEW) authored — writer→loader round-trip per PayloadKind variant; cadence `sentinel` in `behavior-corpus.md`.

**Verification gates:**

- Each sweep target's round-trip test passes (compiler×1, object×2 [index + record], parse-state×1, position-spill×1, memo×1 — 6 round-trips total).
- T5's grep invariant gate is green (zero raw `keyspace.insert(` outside `persistence/envelope.rs`).
- SCENARIO-0099 passes (envelope contract unchanged from 0005a.1).
- SCENARIO-0111 (NEW) passes (writer→loader round-trip per PayloadKind variant).
- Sentinel sweep green (no regression).
- Clippy clean on default features AND `--features fjall-persistence`.

**Out of scope (deferred to ITER-0005a.3):**

- `LoaderStats { loaded, skipped_incompatible, skipped_corrupt, skipped_unknown_kind }` public API (AC-7).
- Each `load_from_fjall` site permanently rewired through `loader::decode` (instead of the T1-T4 transitional manual prefix-strip).
- SCENARIO-0099 harness rebinding from local `u32` counters to public `LoaderStats` reads.

**Out of scope (deferred to other iterations):**

- New payload classes (`Lambda`, `VmSnapshot`, `Grammar` as standalone — ITER-0005d/e).
- `Hash` newtype around `[u8; 32]` (ITER-0005b).
- Closing the `#[cfg(feature = "fjall-persistence")]` gating asymmetry between writer call sites (future hardening iteration).
- Converting `stream_input.rs` writers from `.expect()` panic-on-write to `Result` propagation (future hardening iteration).

**PAR-aggregate findings → resolution map:**

| Finding | Severity | Resolution in this revised card |
|---|---|---|
| AC-7 omitted from build order | Critical | Split AC-7 into new ITER-0005a.3 (loader read-side wiring + LoaderStats public API + harness rebinding). 0005a.2 stays writer-only. |
| Helper signature uses non-existent `Hash` type | Critical | Helper accepts `source_hash: [u8; 32]` directly; all callers pass `NO_SOURCE_HASH`. `Hash` newtype lands in ITER-0005b. |
| `object.rs` two-record-shape problem | Serious | Sweep calls helper twice per save: `PayloadKind::ObjectIndex` for `__object_ids__`, loop of `ObjectRecord` per object. No batch-mode in helper. |
| `stream_input.rs` `.expect()` vs `Result` | Serious | Preserve panic-on-write: `.expect()` the helper's result. Documented as transitional; future hardening can plumb `Result`. |
| Wire-format break not acknowledged | Serious | Explicitly acknowledged: fjall-persistence has no production consumers, format break is acceptable, CHANGELOG entry at T6. |
| T5 visibility-constraint infeasible as stated | Serious | T5 commits to form #4 (grep gate) as the strongest **feasible** form, with explicit rationale: `fjall::Keyspace::insert` is foreign-crate, can't be sealed. |
| T5 conflates 3 forms — pragmatic vs principled | Serious | T5 commits to form #4 with principled rationale; no hedging. |
| AC-5 wording names payload classes without writers | Serious | EPIC-003 AC-5 wording updated at T6 to "currently-extant writers"; `Lambda`/`Grammar`/`VmSnapshot` writer adoption naturally folds in when those classes acquire writers in 0005d/e. |
| Feature-flag asymmetry unresolved | Serious | Preserve existing asymmetry (mechanical sweep); envelope helper stays unconditional. Closing asymmetry is a future hardening iteration. |
| Read-side integration elided | Serious | T1-T4 use transitional manual prefix-strip (`// TODO(ITER-0005a.3)`); permanent `loader::decode` wiring lands in 0005a.3. |
| AC-7 has no consumer in scope | Serious | AC-7 moved to 0005a.3 where its consumer (SCENARIO-0099 harness rebinding) lives. |

---

#### ITER-0005a.3 — STORY-0099 AC-7 LoaderStats + iter_keyspace public API (PAR-revised + PAR-split 2026-05-13)

**Stories:** STORY-0099 (versioned envelope) — AC-7 (LoaderStats public API surface) only.

**Status:** done (2026-05-13). Public `LoaderStats` + `iter_keyspace` shipped in `fmpl-core/src/persistence/loader.rs`. T0-T3 evidence: 10 new LoaderStats unit tests (`loader::tests`), 4 new iter_keyspace integration tests (`tests/iter_keyspace.rs`), `tests/scenario_0099_envelope_loader.rs` extended with `scenario_0099_iter_keyspace_aggregates_stats` (existing decode-pathway test preserved), new `tests/scenario_0112_operator_detection.rs` with 2 tests proving histograms distinguish operator-actionable signals from isomorphic aggregates. Default-features sentinel: 1352 passing (baseline 1342, +10). fjall-persistence integration: 11 passing. AC-5 grep invariant gate green. The per-call-site rewire of `load_from_fjall` paths is deferred to ITER-0005a.4 as planned.

**Rationale (twice-PAR-revised):** Original 0005a.3 bundled the public API (T0-T1) + the per-call-site rewire (T2-T5) + scenario rebinding (T6-T7). The 2026-05-13 pre-iteration PAR review found this violated the same writer/reader split discipline that drove 0005a.0/0005a.1/0005a.2 — and additionally surfaced 2 Critical findings (fjall 3 iterator API mismatch in the proposed `iter_keyspace` signature; semantic regression from graceful `None` → panic in `stream_input.rs`'s read paths). Re-split per `feedback_split_iterations_on_reader_writer_asymmetry.md`: this iteration ships the public API + first consumer (SCENARIO-0099 rebinding); ITER-0005a.4 ships the per-call-site rewire + the 24+ caller-update fanout. The 0005a.4 split lets the API shape pressure-test against one real consumer before 4 downstream sites commit to it.

**Critical findings from 2026-05-13 PAR (both auditors) — resolved in this revision:**

1. **C1: fjall 3 `Keyspace::iter()` yields `Iter<Item=Guard>` where `Guard::into_inner() -> Result<(UserKey, Option<UserValue>)>` with `UserKey`/`UserValue` = owned `Slice` (reference-counted), NOT `&'k [u8]`.** The original card's `FnMut(&[u8], DecodedRecord<'_>)` callback with `'k`-tied lifetime cannot be implemented as written. Revised T1 signature below uses owned `(UserKey, UserValue)` per iteration step.
2. **C2: `iter_keyspace` must propagate `fjall::Error`.** Each `Guard::into_inner()` is fallible; the original card silently swallowed errors. Revised signature returns `Result<LoaderStats, fjall::Error>`.

**Impacted scenarios:** SCENARIO-0099 (rebinding to consume the public `LoaderStats` API + new SCENARIO-0099-iter sub-test for keyspace-iteration coverage — original decode-pathway test preserved per PAR S5/S8 finding); new SCENARIO-0112 (NEW; operator-detection scenario per PAR S7/S9 finding — writes a mixed-validity corpus, calls `iter_keyspace`, asserts an operator can pinpoint silent data loss from the returned stats).

**Depends on:** ITER-0005a.2 (writer sweep + transitional manual prefix-strip already in place at each load site — 0005a.4 will remove the markers).

**Look-ahead:** ITER-0005a.4 will rewire all `load_from_fjall` call sites through the API this iteration ships. ITER-0005b/c/d/e/f use `iter_keyspace` for keyspace-iterating reads + the public stats API for operator observability.

**LoaderStats granularity decision (resolves PAR Serious #4):**

`LoaderStats` carries both aggregate counters AND a per-skip-reason histogram so operators can distinguish "5 records all checksum-mismatch" (disk corruption — call sysadmin) from "5 records all UnknownSchemaVersion" (post-upgrade schema drift — re-extract or recompile). Concrete fields:

```rust
pub struct LoaderStats {
    pub loaded: u32,
    pub skipped_incompatible: u32,
    pub skipped_corrupt: u32,
    pub skipped_unknown_kind: u32,
    // Sub-reason histograms (added per PAR S4 to preserve DecodeOutcome's
    // 9 sub-reasons; aggregate fields above are the headline summary).
    pub incompatible_reasons: SubReasonCounts,
    pub corrupt_reasons: SubReasonCounts,
    pub unknown_kind_reasons: SubReasonCounts,
}
```

`SubReasonCounts` is a small struct with one `u32` field per sub-reason. The aggregate counters and histogram totals must agree (typed invariant — see T0).

**Return-shape decision (resolves PAR Serious #5):**

The `iter_keyspace` helper returns `Result<LoaderStats, fjall::Error>` (no value tuple — values are delivered via the `on_record` callback). For 0005a.4's per-call-site rewires (point-key `load_from_fjall` paths), the return shape decision is deferred to that iteration's PAR review; this iteration explicitly does NOT lock in `(Option<T>, LoaderStats)` for the point-key path.

**Feature-gate decision (resolves PAR Serious #6):**

`LoaderStats` itself is unconditional (lives in `persistence/loader.rs`, which is unconditional today). `iter_keyspace` takes `&fjall::Keyspace`; since `fjall = "3"` is unconditional in fmpl-core's Cargo.toml (NOT `optional = true`), `iter_keyspace` is unconditional. No new feature-gating decisions; the existing asymmetry in `load_from_fjall` call sites is inherited unchanged into 0005a.4.

**Build order:**

1. **T0 — `LoaderStats` + `SubReasonCounts` structs (AC-7).** Land in `fmpl-core/src/persistence/loader.rs`. Derives: `Debug, Clone, Copy, Default, PartialEq, Eq`. Public API for operators. Includes a typed invariant test: `loaded + skipped_incompatible + skipped_corrupt + skipped_unknown_kind == total processed records`, AND each skip-reason aggregate equals the sum of its sub-reason histogram. Compile-time invariant where feasible; runtime invariant via debug_assert otherwise.

2. **T1 — `loader::iter_keyspace` helper, corrected against fjall 3's actual API.** New public function:
   ```rust
   pub fn iter_keyspace<F>(
       keyspace: &fjall::Keyspace,
       mut on_record: F,
   ) -> Result<LoaderStats, fjall::Error>
   where
       F: FnMut(&[u8], DecodedRecord<'_>),
   ```
   Implementation: `for guard in keyspace.iter()` → `let (key, value) = guard.into_inner()?;` → `value.as_ref()` for the byte slice → `decode(value_bytes)` → if `Loaded`, invoke `on_record(key.as_ref(), record)`; otherwise increment the appropriate `LoaderStats` field (aggregate + sub-reason). Returns the accumulated stats at end-of-iteration or propagates `fjall::Error` from any per-guard `into_inner`. The callback's `&[u8]` and `DecodedRecord<'_>` borrows live within one iteration step (not `'k`-tied) — this is the corrected lifetime per PAR Critical #1.
   Unit tests in `loader.rs::tests`:
   - Empty keyspace → `LoaderStats::default()`.
   - One record, all-valid → `loaded=1`, all skip counters 0, callback fires once.
   - Mixed-validity records (1 valid + 1 vm-major-mismatch + 1 checksum-corrupt) → aggregate counters match, sub-reason histograms match.

3. **T2 — SCENARIO-0099 harness rebinding (AC-7 consumer — first real public API user) + new sub-test.** Per PAR S5/S8: do NOT replace the existing decode-pathway test. Instead:
   - Preserve the existing `scenario_0099_six_record_skip_journey` test as-is (proves the decoder dispatches correctly at the unit-of-decode seam — form-#1 typed invariant evidence per `feedback_prefer_proof_tests.md`).
   - Add a NEW test in the same file, `scenario_0099_iter_keyspace_aggregates_stats`, that constructs a real `fjall::Keyspace` (via the `tempfile` + `fjall::Database::builder` pattern used in `grammar/incremental.rs::tests`), writes 6 synthetic records (the same 6 as the existing test) via `keyspace.insert`, then calls `loader::iter_keyspace(&keyspace, |_key, _rec| {...})`. Asserts the returned `LoaderStats == (loaded=1, skipped_incompatible=1, skipped_unknown_kind=3, skipped_corrupt=1)` AND asserts the sub-reason histograms pinpoint the exact reason per skip.
   This is the AC-7 consumer rebinding per `feedback_ship_infrastructure_with_first_consumer.md` — the public surface is in active use by a test on the same day it ships.

4. **T3 — SCENARIO-0112 (NEW): operator-detection scenario (AC-7 narrative validation).** Per PAR S7/S9, AC-7's narrative is "operators can detect silent data loss after an upgrade." The proof-test:
   - Construct a keyspace simulating a real upgrade scenario: 3 valid CompiledCode records (would have loaded pre-upgrade), 2 vm-major-future records (data lost across upgrade), 2 unknown-schema-version records (post-upgrade schema drift), 1 checksum-corrupt record (disk corruption).
   - Call `iter_keyspace`, capture returned `LoaderStats`.
   - Assert: `loaded=3, skipped_incompatible=2, skipped_unknown_kind=2, skipped_corrupt=1`.
   - Assert sub-reason histograms: `incompatible_reasons.vm_major_mismatch=2`, `unknown_kind_reasons.unknown_schema_version=2`, `corrupt_reasons.checksum_mismatch=1`.
   - Author `scenario_0112_operator_detects_silent_data_loss` in a new file `fmpl-core/tests/scenario_0112_operator_detection.rs`. Sentinel cadence.

5. **T4 — Wrap artifacts.** `roadmap.md` ITER-0005a.3 status → done. `iteration-log.md` entry. `progress.md` update. `EPIC-003.md` STORY-0099 status note: AC-7 done in 0005a.3 (public API + first consumer + operator-detection scenario); the per-call-site load rewire deferred to 0005a.4. behavior-corpus.md: add SCENARIO-0099-iter (sentinel) + SCENARIO-0112 (sentinel) entries.

**Verification gates:**

- `LoaderStats` + `SubReasonCounts` compile with documented fields/derives; aggregate-vs-sub-reason invariant holds in unit tests.
- `iter_keyspace` iterates a real `fjall::Keyspace`, returns correct stats, propagates `fjall::Error`.
- SCENARIO-0099's existing test passes unchanged.
- New `scenario_0099_iter_keyspace_aggregates_stats` test passes.
- SCENARIO-0112 (operator-detection) passes.
- Sentinel sweep green on default features (1342 baseline +N new tests).
- Clippy clean on default features AND `--features fjall-persistence`.
- Citation check clean.

**Out of scope (deferred to ITER-0005a.4):**

- Rewiring `compiler.rs::CompiledCode::load_from_fjall`, `object.rs::ObjectDb::load_from_fjall`, `grammar/incremental.rs::ParseState::load_from_fjall`, `grammar/stream_input.rs::restore_from_fjall`/`get_memo` through `loader::decode`.
- Removing the 5 `// TODO(ITER-0005a.3)` markers in `src/` (they update to `// TODO(ITER-0005a.4)` here OR stay as-is — pick at iteration entry).
- Caller-update fanout (~24 test sites) for `load_from_fjall` signature changes.
- Decision on point-key load return shape: `(Option<T>, LoaderStats)` vs `LoadResult<T>` struct vs `&mut LoaderStats` parameter.
- SCENARIO-0111 rewrite (per PAR S6/S10 finding, SCENARIO-0111 stays at point-key roundtrip evidence; iter_keyspace evidence is the new SCENARIO-0099-iter sub-test).
- The 4 single-auditor Minor findings from 0005a.2's audit (inconsistent corruption-handling across load sites, saturation patterns, silent-None semantics) — all land in 0005a.4 when the call sites are actually touched.

**Out of scope (deferred to other iterations):**

- Closing `#[cfg(feature = "fjall-persistence")]` gating asymmetry (future hardening; preserved here).
- Converting `stream_input.rs` writers from panic-on-write to `Result` (future hardening).
- New payload classes (ITER-0005d/e).
- `Hash` newtype (ITER-0005b).

**PAR-aggregate findings → resolution map (2026-05-13):**

| PAR finding | Severity | Resolution in this revised card |
|---|---|---|
| C1 — fjall 3 `iter_keyspace` signature mismatch | Critical | T1 signature corrected: owned `(UserKey, UserValue)` per iteration step; `Result<LoaderStats, fjall::Error>` return; callback borrow lifetime per-step, not `'k`-tied |
| C2 — `iter_keyspace` swallowed `fjall::Error` | Critical | T1 returns `Result<LoaderStats, fjall::Error>` |
| C3 — 24+ hidden caller-update sites in T2/T3/T4 | Critical | Removed: T2-T4 of original card deferred wholesale to ITER-0005a.4 (split) |
| C4 — T5 panic-on-skip semantic regression | Critical | Removed: T5 of original card deferred to ITER-0005a.4 with explicit "graceful skip, no panic-on-skip" requirement in that card |
| S1 — `LoaderStats` 4-counter flattening | Serious | Added `SubReasonCounts` per category; aggregate + histogram both present |
| S2 — Tuple-return `(Option<T>, LoaderStats)` undeliberated | Serious | Decision deferred to 0005a.4 where the point-key callers actually live; this iteration explicitly does not lock in the tuple-return shape |
| S3 — Feature-gate × public API asymmetry | Serious | `LoaderStats` + `iter_keyspace` are unconditional (fjall itself is unconditional); 0005a.4 inherits existing asymmetry unchanged |
| S4 — Deferred 0005a.2 audit findings not enumerated | Serious | All 4 single-auditor findings explicitly moved to 0005a.4's scope (they're per-call-site, not API-side) |
| S5 — T6 rebinding loses SCENARIO-0099 decode-pathway coverage | Serious | T2 in revised card preserves existing test AND adds new iter sub-test; original decode-pathway evidence retained |
| S6 — T7 SCENARIO-0111 rewrite changes coverage same-day | Serious | Removed: SCENARIO-0111 stays at point-key roundtrip evidence; iter coverage comes from new sub-test |
| S7 — No operator-detection scenario for AC-7 narrative | Serious | New T3: SCENARIO-0112 explicit operator-detection scenario with mixed-validity corpus + sub-reason assertions |
| S8 — Iteration should split along API-vs-rewire axis | Serious | This split (0005a.3 = API + first consumer; 0005a.4 = per-call-site rewire) |
| S9 — `(Option<T>, LoaderStats)` tuple boxing-in | Serious | Tuple-return shape decision deferred to 0005a.4 |
| Minor — TODO marker count enumeration | Minor | Listed explicitly: 5 markers across 4 files (compiler.rs, object.rs, incremental.rs, stream_input.rs has 2) |
| Minor — `on_record` skip-context elision | Minor | Documented: `on_record` fires only on `Loaded`; per-skip context lives in `LoaderStats.SubReasonCounts` aggregated histograms. If per-record skip detail is needed later, add a separate `FnMut(&[u8], DecodeOutcome)` on-skip callback (deferred) |

---

#### ITER-0005a.4 — STORY-0099 read-side decode wiring through LoaderStats API (PAR-split 2026-05-13)

**Stories:** none new. Closes STORY-0099 fully by wiring every `load_from_fjall` call site through ITER-0005a.3's public API + addressing the 4 single-auditor Minor findings deferred from 0005a.2's audit (inconsistent corruption-handling across load sites).

> **⚠️ STATUS BANNER — partially superseded by ITER-0005a.5 (2026-05-13).**
>
> ITER-0005a.5 absorbed the per-call-site rewires this card scoped:
> - T1-T3 (rewire `CompiledCode`, `ObjectDb`, `ParseState` load paths) — **DONE** as part of 0005a.5 T4.1-T4.6. Methods renamed `load_from_fjall` → `load_from_store`; signatures take `&impl Store` not `&fjall::Keyspace`; `ParseStateError::Fjall` → `ParseStateError::Store(StoreError)`.
> - T4 (rewire `stream_input.rs::restore_from_store` + `get_memo`) — **DONE** as part of 0005a.5 PAR fix R-A-S-2 / R-B-S-1: these now route through `loader::decode` with full envelope integrity (magic + CRC + VM-major + payload-kind). Graceful-skip semantics preserved (the "Critical decision pinned" section below holds: corrupt records degrade to `None`, not panic).
>
> **What remains unique to 0005a.4 if/when resumed:**
> - The `&mut LoaderStats` parameter threading at the 24 call-site enumerated below. 0005a.5's rewires preserved the OLD `Result<Option<T>>` return shape without LoaderStats accumulator threading. If the LoaderStats-as-accumulator pattern is still wanted, this iteration would re-thread that one parameter through the call sites.
> - The 4 inconsistent-corruption-handling Minor findings from 0005a.2's audit.
> - T5 of this card (graceful-skip preservation gate) — covered by 0005a.5's `stream_input_store.rs::memo_with_bitflipped_record_is_cache_miss` integration test.
>
> Signatures shown below are the ORIGINAL pre-0005a.5 shape (`&fjall::Keyspace`, `load_from_fjall`). Read them as the iteration's historical scope, not as a target for new work. Any resumption of 0005a.4 must first reconcile against 0005a.5's actual implementation: methods are now `load_from_store<S: Store>(&S, key, ...)`, fjall is not named in fmpl-core, and the per-call-site rewires are already done.

**Status:** pending (partially superseded — see banner above).

**Rationale:** Split from the original ITER-0005a.3 per the 2026-05-13 pre-iteration PAR review. ITER-0005a.3 ships the public API (`LoaderStats`, `iter_keyspace`) + first consumer (SCENARIO-0099 iter sub-test); 0005a.4 wires every existing point-key `load_from_fjall` call site through the API. Splitting lets 0005a.3's API design pressure-test against one real consumer (the SCENARIO-0099 rebinding) before 4 downstream sites lock in the return-shape decision.

**Critical decision pinned in scope card (resolves PAR C4 from 0005a.3 review):**

`stream_input.rs::restore_from_fjall` + `get_memo` MUST preserve graceful-skip semantics. Today's behavior on envelope-short / corrupted bytes is `None` (graceful — caller falls back to pulling from upstream); changing to panic-on-skip would contradict AC-2/AC-3/AC-4's "skip and continue" promise specifically at the points where stream-position spills are read across an upgrade. T5 in this iteration explicitly preserves graceful skip — non-`Loaded` decode outcomes return `None` from these methods, AND increment a stats accumulator that the caller can read separately.

**Return-shape decision pinned in scope card (resolves PAR S2/S9 from 0005a.3 review):**

For point-key `load_from_fjall` methods (`CompiledCode`, `Object`, `ParseState`): use `&mut LoaderStats` parameter, returning `Result<Option<T>>` as today. This composes naturally for `ObjectDb::load_from_fjall` which does N+1 point reads and wants a single accumulated `LoaderStats`. Alternative (`(Option<T>, LoaderStats)` tuple) requires manual `.0 += .0; .1 += .1` aggregation across the N+1 calls. The `&mut LoaderStats` pattern is also the more Rust-idiomatic choice for "accumulator across multiple sub-operations."

Revised signature pattern:
```rust
pub fn load_from_fjall(
    keyspace: &fjall::Keyspace,
    key: &str,
    stats: &mut LoaderStats,
) -> Result<Option<Self>>;
```

`stats` is `&mut` so callers can accumulate across multiple loads; the existing `Result<Option<T>>` return shape is preserved (no breaking signature change for the "return value" side; only the new parameter is breaking).

**Caller-update enumeration (resolves PAR C3 from 0005a.3 review):**

The 24 caller sites that need updating to pass a `&mut LoaderStats` parameter (read each before scope-card finalization per `feedback_read_actual_code_before_scope_finalization.md`):

- `tests/bytecode_persistence.rs:38, 54, 70, 78, 86, 103, 123, 126` — 8 sites for `CompiledCode::load_from_fjall`.
- `tests/object_persistence.rs:33, 76, 114, 144` — 4 sites for `ObjectDb::load_from_fjall`.
- `tests/parse_state_persistence.rs:63, 84, 120, 179, 222, 255` — 6 sites for `ParseState::load_from_fjall`.
- `src/grammar/incremental.rs:228, 249` — 2 sites for `ParseState::load_from_fjall` inside its own tests.
- `fmpl-web` callers — TBD, check at iteration entry (likely 0 since fmpl-web persistence is the deferred ITER-0005-WEB-PERSISTENCE territory).

Total: ~20+ caller sites. Each receives `&mut LoaderStats::default()` in the simple case OR an accumulator threaded through a test.

**Impacted scenarios:** SCENARIO-0099 (no change — its existing test stays at decode-pathway seam; its iter sub-test stays at the API seam); SCENARIO-0111 (no change — stays at point-key roundtrip); SCENARIO-0112 (no change — operator-detection scenario stays as the AC-7 narrative proof).

**Depends on:** ITER-0005a.3 (`LoaderStats` + `iter_keyspace` public API + SCENARIO-0099 iter sub-test + SCENARIO-0112).

**Look-ahead:** ITER-0005b/c/d/e/f use the rewired `load_from_fjall` signatures with `&mut LoaderStats` accumulators.

**Build order:**

1. **T0 — Rewire `compiler.rs::CompiledCode::load_from_fjall`** through `loader::decode`. Signature becomes `load_from_fjall(ks, key, stats: &mut LoaderStats) -> Result<Option<Self>>`. Update 8 test caller sites. Remove the `// TODO(ITER-0005a.3)` marker.
2. **T1 — Rewire `object.rs::ObjectDb::load_from_fjall`** through `loader::decode`. Two-step: decode `__object_ids__` (PayloadKind::ObjectIndex), then loop decoding `obj:{id}` records (PayloadKind::ObjectRecord). Single `&mut LoaderStats` accumulator across the N+1 calls. Update 4 test caller sites. Remove the marker.
3. **T2 — Rewire `grammar/incremental.rs::ParseState::load_from_fjall`** through `loader::decode`. Replace the `.min(ENVELOPE_HEADER_SIZE)` saturation pattern (PAR deferred-finding #2 from 0005a.2 audit) with proper `DecodeOutcome` handling. Update 6 test caller sites + 2 src caller sites. Remove the marker.
4. **T3 — Rewire `grammar/stream_input.rs::restore_from_fjall`** through `loader::decode`. **Graceful-skip preservation:** non-`Loaded` outcomes return `None` (matches today's behavior). Stats accumulator threaded through. Remove the marker. (PAR deferred-finding #3 from 0005a.2 audit closed.)
5. **T4 — Rewire `grammar/stream_input.rs::get_memo`** through `loader::decode`. Same graceful-skip preservation as T3. Remove the marker. (PAR deferred-finding #4 from 0005a.2 audit closed.)
6. **T5 — Corruption-handling consistency gate** (PAR deferred-finding #1 from 0005a.2 audit closed). Add a behavior-corpus entry (sentinel cadence) that constructs a corrupted record (e.g., `ValueTooShort`) and verifies each load path surfaces the same shape of outcome (typed error OR stats-recorded skip; pick per-site appropriately). Per-site disposition documented inline:
   - `compiler.rs::load_from_fjall`: typed error → `BytecodePersistenceError`.
   - `object.rs::load_from_fjall`: typed error → `ObjectPersistenceError`.
   - `incremental.rs::load_from_fjall`: typed error → `ParseStateError`.
   - `stream_input.rs::restore_from_fjall` + `get_memo`: graceful `None` + stats increment.
7. **T6 — Wrap artifacts.** `roadmap.md` ITER-0005a.4 status → done. `iteration-log.md` entry. `progress.md` update. `EPIC-003.md` STORY-0099 status note: all 7 ACs done. behavior-corpus.md updates.

**Verification gates:**

- All 5 `// TODO(ITER-0005a.3)` markers removed from `src/`.
- All 4 `load_from_fjall` sites use `&mut LoaderStats` parameter.
- All 20+ caller sites updated.
- `stream_input.rs::restore_from_fjall` + `get_memo` graceful-skip preserved (verified by a new test that writes a corrupted value + asserts `restore_from_fjall` returns `None` AND stats records the skip).
- Corruption-handling consistency gate (T5) green.
- Sentinel sweep green.
- Clippy clean on default features AND `--features fjall-persistence`.
- SCENARIO-0099 (both existing decode test + new iter sub-test from 0005a.3) still passes.
- SCENARIO-0111 still passes.
- SCENARIO-0112 still passes.

**Out of scope:**

- Closing `#[cfg(feature = "fjall-persistence")]` gating asymmetry (future hardening).
- Converting `stream_input.rs` writers from panic-on-write to `Result` (future hardening).
- New payload classes (ITER-0005d/e).
- `Hash` newtype (ITER-0005b).
- fmpl-web persistence sweep (ITER-0005-WEB-PERSISTENCE — separately deferred).

---

#### ITER-0005a.5 — Extract `fmpl-persistence` crate; abstract storage in fmpl-core only (NOT fmpl-web)

**Stories:** none (cross-cutting architectural extraction; enables 0005a.4 + every downstream 0005x consumer; addresses dep-audit findings from 2026-05-13).

**Status:** **DONE** 2026-05-13 (UTC ~03:00 of 2026-05-14). See `iteration-log.md` ITER-0005a.5 entry for full close-out. Implementation PAR returned REVISE with 8 findings (2 Critical, 3 Serious, 3 Minor); all addressed in commit `3c80b3b8`. Verification: fmpl-core 1292/1292 + fmpl-persistence 69/69 passing; clippy clean; no-fjall-in-fmpl-core invariant verified (0 occurrences in fmpl-core/src/).

**Prior PAR history:** scope card was TWICE PAR-revised 2026-05-13 — original card returned REVISE from both reviewers with 7 Critical + 11 Serious findings; first split into 0005a.5 (fmpl-core scope) + 0005a.6 (fmpl-web scope); second PAR loop returned REVISE again with 4 new Critical + 14 new Serious findings; both revisions' resolution maps are below.

**PAR-aggregated findings from 2026-05-13 review (both reviewers, REVISE) — addressed in this revision:**

| Finding | Severity | Resolution in this revised card |
|---|---|---|
| C-AGG-1: Store trait missing `is_empty()` / `len()` | Critical | Trait sized to fmpl-core consumers only this iteration; `is_empty` deferred to 0005a.6 where fmpl-web is the first real consumer that needs it. |
| C-AGG-2: stream_input.rs has fjall struct fields + helper constructors not in T4 scope | Critical | T4 below explicitly enumerates ALL `fjall::*` reference sites in fmpl-core/src/, not just save/load methods. Includes `FjallOverflow.keyspace`, `MemoFjall(Keyspace)`, `from_async_with_fjall`, `from_async_memo_fjall`, `restore_from_fjall`, `spill_to_fjall`, `set_memo`, `get_memo`. Function/field renames are scheduled. |
| C-AGG-3: `EnvelopeWriteError::Keyspace(#[from] fjall::Error)` + `ParseStateError::Fjall(fjall::Error)` leak fjall | Critical | T3 + T4 explicitly reshape both error types. `StoreError` becomes the canonical wrapper; `fjall::Error` is wrapped inside `FjallStore`'s `From<fjall::Error> for StoreError` impl. No fjall::Error in any public type after this iteration. |
| C-AGG-4: AC-5/AC-6 invariant gates use `CARGO_MANIFEST_DIR/src` which silently passes after relocation | Critical | T5 below specifies the gate stays physically in `fmpl-core/tests/` (NOT relocated), so `CARGO_MANIFEST_DIR` still resolves to fmpl-core. Gate scope becomes "fmpl-core/src/ contains zero `fjall::*` substrings" — strengthened, not weakened. (Final implementation: kept the existing `fmpl-core/tests/persistence_envelope_invariant.rs` since the dep-graph already enforces no-fjall structurally; the typed gate upgrade was deferred. AC-6 stayed in fmpl-core/tests/persistence_schema_anti_rot.rs per R3-C3 below.) |
| C-AGG-5: fjall v2→v3 data migration unaddressed | Critical | **Deferred to 0005a.6.** 0005a.5 does NOT touch fmpl-web; fmpl-web continues using fjall v2 unchanged. Workspace `fjall = "2"` stays. fmpl-persistence pins fjall v3 directly (non-workspace). Both compile during the transition; the duplicate-compile cost stays until 0005a.6 closes — accepted as a temporary state. |
| C-AGG-6: T6 ≤40-deps target unachievable (default = [] already empty) | Critical | T6 removed. fmpl-bootstrap dep count is no longer a verification gate this iteration. The realistic floor (limited by unconditional curl, rkyv, tokio, blake3 in fmpl-core) is ~80-90 deps even after fjall becomes optional. The bootstrap-leanness story moves to a separate ITER-0005a-DEPS-CLEANUP (deferred). |
| C-AGG-7: parse_state_persistence.rs / bytecode_persistence.rs / object_persistence.rs not in relocation list | Critical | T0's file-relocation list expanded below. ALL fjall-touching tests in `fmpl-core/tests/` are enumerated and either relocated to `fmpl-persistence/tests/` OR updated to use the new `Store` API (keeping `FjallStore` construction local to test setup). |
| S-AGG-1: ITER-0005a.4 text references `&fjall::Keyspace`; 0005a.5 doesn't schedule its rewrite | Serious | T8 below (wrap) explicitly rewrites 0005a.4's card text as part of the wrap-up, reshaping every signature in that card from `&fjall::Keyspace` → `&impl Store`. (Final implementation: 0005a.4 is the per-call-site rewire; since 0005a.5 absorbed the call-site rewrites in T4.1-T4.8, the rewrite of 0005a.4's card text became moot — 0005a.4 itself is deferred/superseded.) |
| S-AGG-2: `Box<dyn Iterator>` in `Store::iter` forces dyn dispatch + boxing | Serious | T1 below uses GAT-based `type Iter<'a>: Iterator<Item = Result<(StoreKey, StoreValue), StoreError>>;`. Edition 2024 supports GATs natively (workspace pins edition 2024). Monomorphized. |
| S-AGG-3: Public re-export plan from fmpl-core unaddressed | Serious | T4 below explicitly adds `pub use fmpl_persistence::{EnvelopeHeader, PayloadKind, LoaderStats, DecodeOutcome, ...};` to fmpl-core's `lib.rs` to preserve the public API contract for downstream `fmpl_core::persistence::*` consumers. |
| S-AGG-4: tower-sessions risk callout misnamed | Serious | Risk callout removed (was wrong — `fmpl-web/src/main.rs:17` uses `tower_sessions::MemoryStore`, not fjall). |
| S-AGG-5: T2-T3 ordering: FjallStore can't be tested standalone | Serious | T2 ships a minimal smoke test of `FjallStore` using only the `Store` trait API; the relocated SCENARIO tests land in T3 after the envelope rewrite. |
| S-AGG-6: tempfile + dev-deps not enumerated for fmpl-persistence/Cargo.toml | Serious | T0 below enumerates the full dev-deps block: `tempfile = "3"`, `fmpl-scenario-runner` (path), `serde_json = "1.0"`, `fjall = "3"` (gated `fjall-backend`). |
| S-AGG-7: T8 conflated fjall=2 removal with dead-deps audit; async-trait/tokio-stream removal unverified | Serious | T8 reduced to just the workspace `fjall = "2"` removal — but **only** if 0005a.6 has closed. Otherwise T8 is skipped and stays for 0005a.6's wrap. Dead-deps audit deferred to a separate ITER-0005a-DEPS-CLEANUP. |
| S-AGG-8: `fjall-persistence` feature in fmpl-core becomes vestigial | Serious | T4 below specifies the disposition: the `fjall-persistence` feature in fmpl-core is renamed to `persistence` and activates the optional `fmpl-persistence` dep + its `fjall-backend` feature. `#[cfg(feature = "fjall-persistence")]` annotations on `save_to_fjall`/`load_from_fjall` methods drop; the methods now take `&impl Store` and are unconditional. |
| S-AGG-9: Function-name `save_to_fjall` still names fjall after fjall is supposedly an implementation detail | Serious | T4 below renames every `save_to_fjall` → `save_to_store`, `load_from_fjall` → `load_from_store`, `spill_to_fjall` → `spill_to_store`, `restore_from_fjall` → `restore_from_store`, `from_async_with_fjall` → `from_async_with_store`, `from_async_memo_fjall` → `from_async_memo_store`. **API-breaking rename** — flagged below. |
| S-AGG-10: tuplespace-bridge claim overstates | Serious | Rationale prose refined below: `PayloadKind::Tuple` bridges the write-side of a future durable tuplespace; the pattern-match query layer sits outside the Store trait and would need a separate abstraction at that future point. |
| S-AGG-11: Driving evidence #1 framing miscalibrated | Serious | Driving evidence rewritten below: duplicate fjall compile stems from fmpl-web pulling workspace `fjall = "2"` while fmpl-core pulls direct `fjall = "3"` — NOT from a workspace pin leak. Per `feedback_calibrate_claims_to_evidence.md`. |

**PAR round-2 findings → resolution (2026-05-13, second loop):**

| Finding | Severity | Resolution in this revision |
|---|---|---|
| R2-C1: `env!("CARGO_PKG_VERSION")` rebrands when schema.rs moves crates | Critical | `VM_VERSION_MAJOR/MINOR/PATCH` constants STAY in fmpl-core. fmpl-persistence's envelope writer accepts them as parameters: `pub fn write(store: &impl Store, key: &[u8], payload: &T, kind: PayloadKind, vm_version: VmVersion, source_hash: SourceHash)`. `VmVersion` is a small newtype struct in `fmpl-persistence`. fmpl-core defines `pub const VM_VERSION: VmVersion = VmVersion { major: env!("CARGO_PKG_VERSION", ...), ... }` (or a const fn that parses CARGO_PKG_VERSION) and passes it at every write site. The schema's compatibility-check constants (`ENVELOPE_FORMAT_VERSION`, `PayloadKind::current_schema_version`) stay in fmpl-persistence. **The version derivation never leaves fmpl-core.** |
| R2-C2: AC-6 ratchet exemption becomes vacuous post-relocation | Critical | T6 below specifies the new exemption rule. (Final implementation per R3-C3: exemption changed from `s.contains("/persistence/")` to `s.ends_with("/vm_version.rs") || s.ends_with("/lib.rs")` because the version constants moved to fmpl-core/src/vm_version.rs in T0.5 and lib.rs re-exports them.) |
| R2-C3: Re-export enumeration missing 10+ public items | Critical | T4.10 below now lists the COMPLETE enumeration from `grep -rn '^pub ' fmpl-core/src/persistence/*.rs`: `EnvelopeHeader`, `PayloadKind`, `LoaderStats`, `DecodeOutcome`, `IncompatibilityReason`, `UnknownKindReason`, `CorruptionReason`, `IncompatibilityReasonCounts`, `UnknownKindReasonCounts`, `CorruptionReasonCounts`, `DecodedRecord`, `EnvelopeWriteError`, `NO_SOURCE_HASH`, `MAGIC`, `VM_VERSION_MAJOR` (stays in fmpl-core), `VM_VERSION_MINOR` (stays in fmpl-core), `VM_VERSION_PATCH` (stays in fmpl-core), `ENVELOPE_FORMAT_VERSION`, `ENVELOPE_HEADER_SIZE`, and the free fns `write`, `decode`, `iter_store` (renamed from `iter_keyspace`), `checksum::compute`. |
| R2-C4: GAT-based `Store::iter` not stably implementable for fjall v3 | Critical | T1 below reverses S-AGG-2's resolution: trait uses `fn iter(&self) -> Box<dyn Iterator<Item = Result<(&[u8], &[u8]), StoreError>> + '_>;`. **Box<dyn Iterator> with BORROWED slice item type.** Rationale: (a) The loader is bootup-scale, not per-message — dyn dispatch cost is negligible; (b) borrowing key/value as `&[u8]` preserves the zero-copy property (resolving R2-S2 below). This is the principled trade: accept dyn dispatch in exchange for stable Rust + zero-copy. S-AGG-2's original "monomorphized GAT" framing was wrong; this revision corrects it. |
| R2-S1: Cyclic dev-dep risk; T0 missing `fmpl-core` in dev-deps | Serious | T0 dev-deps block updated to include `fmpl-core = { path = "../fmpl-core" }`. Cargo permits dev-dep cycles (dev-deps don't propagate). Explicit `cargo build --workspace` verification gate added (resolves R2-S9). |
| R2-S2: `StoreKey/StoreValue` as `Vec<u8>` regresses zero-copy | Serious | Resolved by R2-C4: trait's iter yields `(&[u8], &[u8])` borrowed from the iterator step. `StoreKey` / `StoreValue` newtypes are removed; the trait surfaces raw byte slices. Per `feedback_ship_infrastructure_with_first_consumer.md`, newtype wrappers added later when a real need surfaces. |
| R2-S3: Re-export shim shape unspecified | Serious | T4.10 below picks shape (b): `pub mod persistence` stays in fmpl-core/src/lib.rs. fmpl-core/src/persistence/mod.rs replaced with `pub use fmpl_persistence::{envelope, loader, schema, checksum};` (4 submodule re-exports). Preserves qualified paths `fmpl_core::persistence::envelope::write` etc. |
| R2-S4: T4.8 reshape text mismatched; `from_values_with_memo_fjall` missed | Serious | T4.8 rewritten below: `MemoFjall` struct field reshape is the real work (replacing `fjall::Keyspace` interior with `MemoStore<S: Store>` — or trait-object). `set_memo`/`get_memo` method signatures DON'T change (they already use `MemoEntry`, not fjall types). `from_values_with_memo_fjall` added to the rename list → `from_values_with_memo_store`. |
| R2-S5: In-source `#[cfg(test)]` modules using fjall directly | Serious | T4.11 (new sub-task) relocates the in-source `#[cfg(test)] mod tests` blocks in `grammar/incremental.rs:170-293` and `grammar/stream_input.rs:823-996` to integration tests under fmpl-persistence/tests/ (or fmpl-core/tests/ rewritten to use the Store API). After this, the strengthened gate's "any `fjall::*` substring" check passes. |
| R2-S6: `fjall-persistence` feature-rename cascade under-specified | Serious | T4.9 enumerates the 4 cfg-gate sites: compiler.rs:703, 726; object.rs:186, 223. All flip to `#[cfg(feature = "persistence")]` (the renamed feature) — but per T4.9 these annotations also DROP entirely (the methods become unconditional after taking `&impl Store`). The cfg-gates on the relocated test files (`bytecode_persistence.rs:3`, `object_persistence.rs:3`, `iter_keyspace.rs:14`) flip to `#![cfg(feature = "fjall-backend")]` (gating on fmpl-persistence's feature) since those tests construct `FjallStore`. |
| R2-S7: rustdoc cross-reference links break silently | Serious | T4.12 (new sub-task) rewrites the 7 rustdoc cross-reference links across `grammar/incremental.rs:88, 89, 91, 92, 100, 103` and `stream_input.rs:563`. Verified by `cargo doc -p fmpl-core --no-deps 2>&1 \| grep -c 'broken intra-doc link' = 0`. |
| R2-S8: 0005a.6 T0 `is_empty` "non-breaking" claim wrong without default impl | Serious | 0005a.6 T0 updated: `is_empty` ships with a default impl `fn is_empty(&self) -> Result<bool, StoreError> { Ok(self.iter().next().is_none()) }`. Now genuinely additive non-breaking. |
| R2-S9: No `cargo build --workspace` verification gate | Serious | New gate added below: `cargo build --workspace --all-features` must pass at every T-task boundary. Catches workspace-wide feature unification under resolver=2 even while per-crate builds pass. |
| R2-S10: 0005a.6 T1 data-migration discovery defers load-bearing decision | Serious | 0005a.6 T1 promoted to pre-iteration spike. The spike runs BEFORE 0005a.6 starts; its outcome is one of: (a) commit to clean-slate (no production data), or (b) pin a concrete v2→v3 migration design. 0005a.6 cannot enter with T1.3 unresolved. |
| R2-S11: 0005a.6 T6 cross-crate `CARGO_MANIFEST_DIR.parent()` fragile | Serious | 0005a.6 T6 updated: gate moves to a new workspace-level test crate `fmpl-workspace-tests` (no source code, just integration tests against the workspace's structural invariants). OR, simpler: a `build.rs`-emitted const in fmpl-core providing the workspace root path. Pick the simpler at iteration entry. |
| R2-S12: `iter_keyspace` call-site enumeration missed | Serious | T4.13 (new sub-task) explicitly lists the call-site updates: scenario_0099_envelope_loader.rs:221+325+335+339; scenario_0112_operator_detection.rs:26+143+211+212; iter_keyspace.rs (file rename target); plus the 5 doc-comment references at loader.rs:288, 356, 359, 367, 371. All `iter_keyspace` → `iter_store`. |
| R2-S13: `fjall-persistence` rename cascade across docs | Serious | T8 expanded: also rewrite CHANGELOG.md, specs/persistence.md, specs/grammar-system.md, docs/codebase/fjall-persistence-patterns.md to use the renamed feature `persistence`. |
| R2-S14: T5 dep-count delta empirical claim unsubstantiated | Serious | T5 dropped. The fmpl-bootstrap leanness story is entirely deferred to ITER-0005a-DEPS-CLEANUP. No claim made; no measurement gate. fmpl-bootstrap simply continues to work (it doesn't use the persistence feature, so the renamed feature flag is unset by default). |

**PAR round-3 findings → resolution (2026-05-13, third loop):**

| Finding | Severity | Resolution in this revision |
|---|---|---|
| R3-C1: Loader's `VM_VERSION_MAJOR` reference has no home (R2-C1 fixed writer, missed reader) | Critical | T3 below extended: `decode(value, expected_vm_major: u16) -> (DecodeOutcome, Option<DecodedRecord>)` and `iter_store<S: Store, F>(store: &S, expected_vm_major: u16, on_record: F)` both take the expected major as a parameter. fmpl-core's call sites pass `fmpl_core::VM_VERSION_MAJOR`. The constant never leaves fmpl-core. |
| R3-C2: `is_empty` default impl swallows iterator errors | Critical | 0005a.6 T0 default impl rewritten: `fn is_empty(&self) -> Result<bool, StoreError> { match self.iter().next() { None => Ok(true), Some(Ok(_)) => Ok(false), Some(Err(e)) => Err(e) } }`. Errors propagate correctly. |
| R3-C3: AC-6 ratchet relocated to wrong crate; constants stay in fmpl-core | Critical | T6 reversed: AC-6 ratchet STAYS in fmpl-core/tests/persistence_schema_anti_rot.rs, scans fmpl-core/src/ (where the constants now live per R2-C1). Exemption rule changes to `s.ends_with("/vm_version.rs")` (or wherever the constants are placed in fmpl-core post-T4.10). The relocated fmpl-persistence has its own schema-format anti-rot but for DIFFERENT constants (ENVELOPE_FORMAT_VERSION, PayloadKind variants) — that gate is a separate workspace member, NOT a relocation of AC-6. |
| R3-C4: `cargo doc` grep gate doesn't catch broken links (empirically verified) | Critical | Verification gate replaced: `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc -p fmpl-core --no-deps` — uses rustdoc lint with deny level. Fails the build on broken links. |
| R3-C5: schema.rs split between crates not decomposed | Critical | NEW T0.5 (between T0 and T1): split `fmpl-core/src/persistence/schema.rs` into TWO files. `fmpl-core/src/vm_version.rs` holds `VM_VERSION_MAJOR/MINOR/PATCH` constants + the `parse_version_part` const fn helper (lines 33-60 of current schema.rs). `fmpl-persistence/src/schema.rs` holds `PayloadKind`, `ENVELOPE_FORMAT_VERSION`, schema versions, the `let _ = VM_VERSION_*;` use-markers REMOVED (those were anti-rot reminders; that role moves to the new fmpl-core-side `vm_version.rs` exemption in T6). |
| R3-C6: T4.7's `OverflowStore<S: Store>` cascade un-bounded | Critical | T4.7 pinned to trait-object form (no generic parameter): `OverflowStore(Arc<dyn Store + Send + Sync>)`. Matches T4.8's MemoFjall→MemoStore decision; both fields hold `Arc<dyn Store + Send + Sync>` so no generic cascade through `StreamPosition`, `StreamSource`, or `Input::Position`. **T1 below now requires `Store: Send + Sync` supertrait bound** (resolving R3-S13 from B). |
| R3-S (multiple, both reviewers): stale task cross-references + renumbering | Serious | T-task numbering reflowed: T0, T0.5 (new schema split), T1, T2, T3, T4 (.1-.13), T5 (AC-5 gate / no-fjall-in-core; landed as the additional test in `persistence_envelope_invariant.rs`), T6 (AC-6 gate, STAYS in fmpl-core, + new schema-format gate in fmpl-persistence), T7 (cross-reference sweep), T8 (wrap). T7 sweep at iteration close updated 4 forward-references (line 16: C-AGG-4 "T7 below" → "T5 below"; line 20: S-AGG-1 "T9 below" → "T8 below" with supersession note for 0005a.4; line 37: R2-C2 "T7 below" → "T6 below"; line 45: R2-S6 "T4.0" → "T4.9") and rewrote line 1903's "Impacted scenarios" entry to record that AC-6 stayed in fmpl-core. Some intra-0005a.6-card "T1.3" references are intentionally preserved — they refer to 0005a.6's own pre-iteration spike numbering, not 0005a.5 tasks. The meta-claim "every reference rewritten" overstated what T7 needed to do; the accurate framing is "every 0005a.5-internal dangling reference rewritten." |
| R3-S: `fmpl-workspace-tests` crate never added to workspace members | Serious | 0005a.6 T5 explicitly adds `fmpl-workspace-tests` to `Cargo.toml:3-11` members array as part of the same atomic commit. |
| R3-S: Test-file cfg-gate flips miss scenario_0099 + scenario_0112 | Serious | T4.9 enumeration extended: `scenario_0099_envelope_loader.rs:218` (`#[cfg(feature = "fjall-persistence")]` on iter_keyspace_pathway submodule) → `fjall-backend`; `scenario_0112_operator_detection.rs:23` (`#![cfg(...)]`) → `fjall-backend`; `parse_state_persistence.rs` GAINS `#![cfg(feature = "fjall-backend")]` (didn't have one before; needs it after relocation). |
| R3-S: `EnvelopeHeader::new_for_current_vm` reads VM_VERSION_* internally | Serious | T3 extended: `new_for_current_vm` renamed to `new(vm_version: VmVersion, kind, payload_len, source_hash)` taking the version as a parameter, NOT reading from any const. The 13+ call sites at envelope.rs/loader.rs tests pass `fmpl_core::VM_VERSION` explicitly. |
| R3-S: Doc cascade misses specs/fmpl-core.md + 2 docs/plans/ files | Serious | T8 doc-cascade list extended: `specs/fmpl-core.md`, `docs/plans/2026-01-20-parse-state-serialization-implementation-plan.md`, `docs/plans/2026-01-20-streaming-grammar-push-model-implementation-plan.md`, `docs/superpowers/iterations/behavior-corpus.md` lines 87-88, `docs/superpowers/iterations/progress.md`, `docs/superpowers/iterations/requirements/EPIC-003.md` lines 201, 205-207, 235. |
| R3-S: T4.13 mis-cites doc-comment line numbers in loader.rs | Serious | T4.13 corrected: actual `iter_keyspace` doc-comment sites are loader.rs:288 and loader.rs:371 (verified by grep); loader.rs:356/359/367 do NOT contain iter_keyspace text — those were misidentified in the prior revision. Plus loader.rs:747 (inside `#[cfg(test)]` block — moved by T4.11). |
| R3-S: 0005a.6 Design B (migration writer) incompatible with T1-T3 build order | Serious | 0005a.6 reshaped: if pre-iteration spike picks Design B (migration writer), insert a new T0.5 BEFORE T1 that performs the one-time v2-read/v3-write migration as a standalone code path. T0.5 runs WHILE fjall v2 is still a workspace dep (T3 hasn't dropped it yet). After T0.5 completes successfully, T1+T2 (the rewrite) proceeds; T3 drops the workspace fjall v2 only after T0.5 has been run + deployed. Order is: spike → T0 (is_empty) → T0.5 (migration if Design B) → T1+T2 (rewrite to Store) → T3 (drop workspace fjall) → T4 (remove workspace fjall pin) → T5 (gate) → T6 (wrap). |
| R3-S: pre-iteration spike has no defined runner | Serious | Pre-iteration spike now invokes 2 paired investigators per the PAR-pair-pattern (parallel subagents, blind to each other, surface evidence, recommend Design A or Design B). Their convergent recommendation pins the spike's outcome; if disagreement, escalate to user. |
| R3-S: `Store: Send + Sync` supertrait not on T1's trait | Serious | T1 updated: `pub trait Store: Send + Sync { ... }`. Required for the `Arc<dyn Store + Send + Sync>` form used by T4.7 + T4.8. |
| R3-S: hard grep gate scope omissions | Serious | Gate scope extended to also include `fmpl-core/tests/` and `fmpl-web/src/` and `fmpl-bootstrap/src/`: `grep -rn 'iter_keyspace' fmpl-persistence/ fmpl-core/ fmpl-web/src/ fmpl-bootstrap/src/ | wc -l = 0`. |
| R3-S: workspace serde feature unification for MemoEntry | Serious | T4.8 extended: MemoEntry stays in fmpl-core/src/grammar/ (NOT relocated); the relocated MemoStore field holds `Arc<dyn Store + Send + Sync>` not MemoEntry. The serde derives on MemoEntry are unaffected by the move. The store reads/writes raw bytes; MemoEntry serialization happens at the call-site (compiler.rs / object.rs / grammar/incremental.rs / grammar/stream_input.rs), in fmpl-core, where serde + workspace serde-derive features are unchanged. |
| R3-S: `cargo build --workspace --all-features` at every task boundary unachievable | Serious | Verification gate softened: workspace-build runs at task-GROUP boundaries (after T0+T0.5 land; after T3 lands; after T4 lands; before T8). NOT at every micro-T-task. Intermediate states are allowed to break workspace-features as long as the gate passes at the named boundary. |
| R3-S: T4.7 `OverflowStore<S>` cascade — addressed by R3-C6 (trait-object pin). |  | (no new task) |
| R3-S: R2-S11 "simpler" path self-contradiction | Serious | 0005a.6 T5 reshaped: the `fmpl-workspace-tests` crate is the ONLY option (the build.rs-emitted const idea is dropped — it's brittle for the same reason CARGO_MANIFEST_DIR is brittle). Workspace-tests crate gets explicit member-array registration in 0005a.6 T5. |

**Driving evidence (2026-05-13, calibrated):**

1. **Duplicate fjall compile** — fmpl-web/Cargo.toml:20 pulls `fjall = { workspace = true }` resolving to workspace `Cargo.toml:40` `fjall = "2"`; fmpl-core/Cargo.toml:21 hard-pins `fjall = "3"` (not workspace). Both versions compile from scratch, each with its own `lsm-tree`, `byteview`, `dashmap`, `quick_cache`. This is the structural cause of a non-trivial fraction of the 36 GB `target/` reclaimed by `cargo clean` this session. **0005a.5 does NOT eliminate this — 0005a.6 does, by migrating fmpl-web off fjall v2.** 0005a.5's win is making fjall an implementation detail within `fmpl-persistence`, which is the prerequisite for 0005a.6.
2. **Storage leaks across the API seam** — current public APIs name `fjall::*` directly: `iter_keyspace(&fjall::Keyspace, F)`, `save_to_fjall(&fjall::Keyspace, ...)` and counterparts in compiler.rs/object.rs/grammar/{incremental,stream_input}.rs, public error variants `EnvelopeWriteError::Keyspace(#[from] fjall::Error)` at envelope.rs:222 and `ParseStateError::Fjall(fjall::Error)` at incremental.rs:70, and private struct fields `FjallOverflow.keyspace: fjall::Keyspace` + `MemoFjall(fjall::Keyspace)` in stream_input.rs. The storage backend should not appear in any consumer's public API.
3. **Tuplespace overlap is surface-level only** — tuplespaces are pure-RAM today (`fmpl-core/src/tuplespace/store.rs:23-30`); spec's `durable: true` (`specs/tuplespace.md:111-115`) is aspirational with no implementing field on `Tuple`. Per `feedback_ship_infrastructure_with_first_consumer.md`, do NOT design a shared abstraction now. A future `PayloadKind::Tuple = 0x0A` bridges the write-side of a durable tuplespace if/when it ships; the pattern-match query layer would need a separate abstraction at that future point (NOT part of the Store trait).

**The API-seam constraint (load-bearing, applies to fmpl-core only this iteration):**

`fmpl-persistence`'s public API must not name `fjall::*` in any signature, trait bound, error type, struct field, or re-export. `fjall` is an implementation detail behind a `fjall-backend` feature. After 0005a.5, fmpl-core also contains zero `fjall::*` substrings (verified by T7's typed gate). fmpl-web is explicitly OUT OF SCOPE this iteration and continues using fjall v2 directly until 0005a.6.

**Scope (build order):**

1. **T0 — Create `fmpl-persistence` crate skeleton.** New workspace member added to `Cargo.toml:3-11` members array (position after `fmpl-core`). `fmpl-persistence/Cargo.toml`:
   ```toml
   [package]
   name = "fmpl-persistence"
   version.workspace = true
   edition.workspace = true
   authors.workspace = true
   license.workspace = true
   description = "FMPL persistence layer: envelope writer, loader, Store trait"

   [features]
   default = []
   fjall-backend = ["dep:fjall"]

   [dependencies]
   fjall = { version = "3", optional = true }
   blake3 = "1"
   serde = { workspace = true }
   serde_json = "1.0"
   thiserror = { workspace = true }
   zerocopy = { version = "0.8", features = ["derive"] }

   [dev-dependencies]
   tempfile = "3"
   fmpl-scenario-runner = { path = "../fmpl-scenario-runner" }
   fmpl-core = { path = "../fmpl-core" }  # cyclic dev-dep — Cargo permits (dev-deps don't propagate); needed for relocated bytecode/object/parse_state tests
   serde_json = "1.0"
   fjall = "3"
   ```
   Move `fmpl-core/src/persistence/{envelope,loader,schema,checksum,mod}.rs` → `fmpl-persistence/src/{envelope,loader,schema,checksum,lib}.rs` (renaming `mod.rs` → `lib.rs`).
   Move ALL fjall-touching tests from `fmpl-core/tests/` to `fmpl-persistence/tests/`:
   - `scenario_0099_envelope_loader.rs`
   - `scenario_0111_envelope_writer_roundtrip.rs`
   - `scenario_0112_operator_detection.rs`
   - `iter_keyspace.rs`
   - `persistence_schema_anti_rot.rs` (AC-6 ratchet)
   - `bytecode_persistence.rs`
   - `object_persistence.rs`
   - `parse_state_persistence.rs`
   Keep `persistence_envelope_invariant.rs` IN PLACE at `fmpl-core/tests/` (rewritten in T7 as "no fjall in fmpl-core/src/" — the gate's correct location stays under fmpl-core so `CARGO_MANIFEST_DIR` resolves correctly).

**T0.5 (REVISED post-R4 + post-fmpl-types decision) — Split schema.rs; use `fmpl_types::VmVersion` everywhere.** Assumes ITER-0005a.0 (rescoped) shipped first. Before T1 lands the trait:
- Create `fmpl-core/src/vm_version.rs` containing:
  ```rust
  use fmpl_types::{VmVersion, parse_version_part};
  pub const VM_VERSION_MAJOR: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 0);
  pub const VM_VERSION_MINOR: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 1);
  pub const VM_VERSION_PATCH: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 2);
  pub const VM_VERSION: VmVersion = VmVersion::new(VM_VERSION_MAJOR, VM_VERSION_MINOR, VM_VERSION_PATCH);
  ```
  fmpl-core gains an unconditional `fmpl-types = { workspace = true }` dep (added in 0005a.0 T4). No feature-gating needed; fmpl-types has no fjall transitives.
- The relocated `fmpl-persistence/src/schema.rs` contains: `pub const ENVELOPE_FORMAT_VERSION: u16 = 1;`, `pub enum PayloadKind { ... }`, the `current_schema_version` methods, and the schema-version constants per PayloadKind. **Does NOT contain VM_VERSION_*.** fmpl-persistence ALSO depends on `fmpl-types` unconditionally — its `EnvelopeHeader` field types use `fmpl_types::Hash`.
- Split the existing schema.rs `#[cfg(test)] mod tests` (lines 196-288): tests for `parse_version_part` already moved to fmpl-types in 0005a.0 T3; tests for `PayloadKind`/`ENVELOPE_FORMAT_VERSION` move to fmpl-persistence/src/schema.rs.
- Remove the `let _ = VM_VERSION_*;` use-markers at schema.rs:284-286 (vm_version.rs's `pub const VM_VERSION` references the constants, eliminating the dead-code concern).
- Source-hash type strategy (R5-C1 RESOLVED — zerocopy compatibility): `EnvelopeHeader.source_hash` field STAYS as `[u8; 32]` because `EnvelopeHeader` derives zerocopy traits and `Hash` doesn't (it derives serde + Hash + PartialEq only). **`Hash` is used at the API edge, not as a struct field type.** Writer signature takes `source_hash: Hash`; internally `EnvelopeHeader::new` accepts `source_hash: Hash` and stores `source_hash.into_bytes()` (or `*source_hash.as_bytes()`) into the `[u8; 32]` field. Reader exposes `header.source_hash` as the raw `[u8; 32]` (callers wrap into `Hash` if needed). `pub const NO_SOURCE_HASH: [u8; 32]` STAYS as `[u8; 32]` in envelope.rs (preserves zerocopy compatibility and callers' direct-array comparisons). A new `pub const fn no_source_hash() -> Hash { Hash::NONE }` helper is added in fmpl-types for the API-edge form when needed.
- After T0.5: `cargo build -p fmpl-core --no-default-features` builds (fmpl-types is unconditional dep). `cargo build -p fmpl-persistence --features fjall-backend` builds. **Workspace `--all-features` build passes at T0.5 boundary** — fmpl-types is in the dep graph and resolves cleanly for both crates.

2. **T1 — Define the `Store` trait + `StoreError`.** Trait sized to actual current consumers (envelope writer, loader, the 4 `save_to_fjall`/`load_from_fjall` paths in fmpl-core/src/). **Box<dyn Iterator> with owned `Vec<u8>` items**, **`Send + Sync` supertrait per R3-C6 / R3-S**:
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum StoreError {
       #[error("backend error: {0}")]
       Backend(Box<dyn std::error::Error + Send + Sync>),
       // ... grows as real needs surface
   }

   pub trait Store: Send + Sync {
       fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;
       fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), StoreError>;
       fn iter(&self) -> Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StoreError>> + '_>;
   }
   ```
   **Design notes:**
   - `iter()` yields `(Vec<u8>, Vec<u8>)` — owned bytes, materializing from fjall's `Slice` into Vec on each step. This is a real per-record allocation cost. Documented trade-off: simplicity (stable Rust, no GATs, no lifetime acrobatics) at the cost of one Vec allocation per record at bootup-scan time. **The loader is bootup-scale, not per-message — the trade is acceptable.** A future `iter_borrowed()` variant returning `&[u8]` slices is non-breaking to add when a measured perf need surfaces.
   - `get()` returns `Vec<u8>` (owned) for the same reason — callers can drop the copy if they only need bytes-len or to deserialize.
   - `Box<dyn Iterator>` adds dyn dispatch on each `.next()`. Profiled trade-off: at bootup-scan rates (~10s-100s of records), dyn dispatch is negligible vs the deserialization cost downstream.
   - `is_empty()` is NOT in this trait — fmpl-core's consumers don't need it. 0005a.6 adds it (with a default impl) when fmpl-web is the first real consumer.
   - **No `StoreKey`/`StoreValue` newtypes** — bare `Vec<u8>` for now. Newtypes added later if a real semantic need surfaces (e.g., to enforce key prefixing).

3. **T2 — `FjallStore` implementation + smoke test.** `fmpl-persistence/src/fjall_backend.rs`, gated `#[cfg(feature = "fjall-backend")]`. Implements `Store` for a wrapper around `fjall::Keyspace`. `From<fjall::Error> for StoreError` impl (the ONLY place `fjall::Error` is named in fmpl-persistence). T2 ships a minimal smoke test (`fmpl-persistence/tests/fjall_store_smoke.rs`) that constructs a `FjallStore`, inserts one key, gets it back, iterates and finds it. Proves `Store` is implementable without depending on T3's envelope rewrite.

4. **T3 — Rewrite envelope writer + loader against `Store`. Uses `fmpl_types::VmVersion` and `fmpl_types::Hash` (R4-C1 RESOLVED via fmpl-types).**
   - **Writer:** `pub fn write<T: Serialize>(store: &impl Store, key: &[u8], payload: &T, kind: PayloadKind, vm_version: VmVersion, source_hash: Hash) -> Result<(), EnvelopeWriteError>` replaces `write(keyspace: &fjall::Keyspace, ...)`. Both `VmVersion` and `Hash` come from `fmpl_types` (shipped in ITER-0005a.0).
   - **Reader (R3-C1 fix):** `pub fn decode(value: &[u8], expected_vm_major: u16) -> (DecodeOutcome, Option<DecodedRecord<'_>>)` and `pub fn iter_store<S: Store, F>(store: &S, expected_vm_major: u16, on_record: F) -> Result<LoaderStats, StoreError>`. Both take `expected_vm_major: u16` (just the major — the only compat-check field). fmpl-core call sites pass `fmpl_core::VM_VERSION.major` or equivalently `fmpl_core::VM_VERSION_MAJOR`.
   - **`EnvelopeHeader::new_for_current_vm` renamed to `EnvelopeHeader::new(vm_version: VmVersion, kind, payload_len, source_hash: Hash)`** (per R3-S `new_for_current_vm` finding). Takes the version struct directly. The 13+ call sites in envelope.rs + loader.rs tests pass `fmpl_core::VM_VERSION` explicitly.
   - `EnvelopeWriteError::Keyspace(#[from] fjall::Error)` is reshaped to `EnvelopeWriteError::Store(#[from] StoreError)`. **API-breaking rename.**
   - The 8 relocated tests construct `FjallStore` locally and feed it to the new API with `fmpl_core::VM_VERSION` (or a synthetic `VmVersion::new(...)` for compat-failure tests).
   - **Ordering note (per R4-C2):** T3 lands AFTER T4.9 wires fmpl-persistence as fmpl-core's optional dep. **Implementation order:** T0 → T0.5 → T1 → T2 → T4.9 (Cargo.toml + persistence/mod.rs shim) → T3 → T4.1-T4.8 + T4.10-T4.13 → T5 → T6 → T7 → T8.
   - **Verification:** `cargo build --workspace --all-features` passes after T3 lands.

5. **T4 — Rewire fmpl-core call sites + ENUMERATE every fjall::* reference site.** Each subtask is one commit (atomic per jj split workflow).
   - T4.1: `compiler.rs::CompiledCode::save_to_fjall` → `save_to_store(&impl Store, ...)`. Drop `#[cfg(feature = "fjall-persistence")]`.
   - T4.2: `compiler.rs::CompiledCode::load_from_fjall` → `load_from_store(&impl Store, ...)`.
   - T4.3: `object.rs::ObjectDb::save_to_fjall` → `save_to_store(&impl Store, ...)`.
   - T4.4: `object.rs::ObjectDb::load_from_fjall` → `load_from_store(&impl Store, ...)`.
   - T4.5: `grammar/incremental.rs::ParseState::save_to_fjall` → `save_to_store(&impl Store, ...)`. Reshape `ParseStateError::Fjall(fjall::Error)` → `ParseStateError::Store(#[from] StoreError)`.
   - T4.6: `grammar/incremental.rs::ParseState::load_from_fjall` → `load_from_store(&impl Store, ...)`.
   - T4.7: `grammar/stream_input.rs::FjallOverflow` struct rename → **`OverflowStore(Arc<dyn Store + Send + Sync>)`** (trait-object form pinned per R3-C6 — no generic parameter; no cascade through StreamSource/StreamPosition/Input). `from_async_with_fjall` → `from_async_with_store(store: Arc<dyn Store + Send + Sync>, ...)`. `spill_to_fjall` → `spill_to_store`. `restore_from_fjall` → `restore_from_store`. `from_values_with_memo_fjall` → `from_values_with_memo_store` (per R2-S4 — was missed in prior revision).
   - T4.8: `grammar/stream_input.rs::MemoFjall(fjall::Keyspace)` struct → **`MemoStore(Arc<dyn Store + Send + Sync>)`** (trait-object form pinned per R3-C6, matching T4.7's choice). **`set_memo`/`get_memo` signatures DON'T change** — they already operate on `MemoEntry`, not fjall types. The real work is the interior-field reshape. MemoEntry itself stays in fmpl-core/src/grammar/ (NOT relocated); the store reads/writes raw bytes; MemoEntry's serde derives are unaffected.
   - T4.9: fmpl-core's `Cargo.toml` — drop direct `fjall = "3"`, gain `fmpl-persistence = { path = "../fmpl-persistence", optional = true, features = ["fjall-backend"] }` under a renamed feature `persistence = ["dep:fmpl-persistence"]` (was `fjall-persistence`). The 4 sites with `#[cfg(feature = "fjall-persistence")]` in fmpl-core/src/ (compiler.rs:703, 726; object.rs:186, 223) **drop the cfg-annotation entirely** — the methods take `&impl Store` and are unconditional. Cfg gates on RELOCATED tests flip to `#![cfg(feature = "fjall-backend")]` (gating on fmpl-persistence's feature since they construct `FjallStore`): `bytecode_persistence.rs:3`, `object_persistence.rs:3`, `iter_keyspace.rs:14`, **`scenario_0099_envelope_loader.rs:218`** (per R3-S), **`scenario_0112_operator_detection.rs:23`** (per R3-S), and **`parse_state_persistence.rs` GAINS `#![cfg(feature = "fjall-backend")]`** (didn't have one before; needs it after relocation — per R3-S).
   - T4.10: fmpl-core's `persistence/mod.rs` becomes a re-export shim. `pub mod persistence` STAYS in lib.rs. New `persistence/mod.rs` body:
     ```rust
     pub use fmpl_persistence::envelope;
     pub use fmpl_persistence::loader;
     pub use fmpl_persistence::schema;
     pub use fmpl_persistence::checksum;
     ```
     This preserves qualified paths like `fmpl_core::persistence::envelope::write` for downstream consumers (the relocated tests still use these qualified paths, plus any external code). Per R2-S3 this is shape (b) from the analysis.

     Additionally, fmpl-core defines (NOT re-exports) its own VM version constants:
     ```rust
     // in fmpl-core/src/lib.rs or a new src/vm_version.rs
     pub const VM_VERSION: fmpl_persistence::VmVersion = /* const-parse from env!("CARGO_PKG_VERSION") */;
     pub const VM_VERSION_MAJOR: u16 = VM_VERSION.major;
     pub const VM_VERSION_MINOR: u16 = VM_VERSION.minor;
     pub const VM_VERSION_PATCH: u16 = VM_VERSION.patch;
     ```
     **The version constants stay in fmpl-core** (per R2-C1); only the carrier struct `VmVersion` lives in fmpl-persistence.

   - T4.11 (NEW per R2-S5): Relocate in-source `#[cfg(test)] mod tests` blocks containing fjall direct-use. Source sites: `grammar/incremental.rs:170-293` (4 fjall::Database::builder calls), `grammar/stream_input.rs:823-996` (multiple fjall calls). Two options per block (pick at task entry):
     - (a) Move to integration tests under fmpl-persistence/tests/ — clean separation but loses access to private/internal types.
     - (b) Rewrite to use the Store trait directly with FjallStore constructed in test setup — keeps the test in-source but removes fjall::* references.
     Option (b) is preferred where the test exercises private items.

   - T4.12 (NEW per R2-S7): Rewrite rustdoc cross-reference links broken by the rename. Sites: `grammar/incremental.rs:88, 89, 91, 92, 100, 103`; `grammar/stream_input.rs:563`. Verification: `cargo doc -p fmpl-core --no-deps 2>&1 | grep -c 'broken intra-doc link' = 0`.

   - T4.13 (NEW per R2-S12; line refs CORRECTED per R3-S): Rewrite `iter_keyspace` → `iter_store` at call sites: scenario_0099_envelope_loader.rs (relocated, verify by grep), scenario_0112_operator_detection.rs (relocated, verify by grep), iter_keyspace.rs (file body + the rename of the file itself to iter_store.rs). Doc-comment refs at loader.rs:288 and loader.rs:371 (verified — the prior revision's claim of 5 sites was wrong; only 2 are actual iter_keyspace refs). Plus loader.rs:747 if it survives the in-source `#[cfg(test)]` relocation in T4.11. Verification: `grep -rn 'iter_keyspace' fmpl-persistence/ fmpl-core/ fmpl-web/src/ fmpl-bootstrap/src/ | wc -l = 0` (scope extended per R3-S).

6. **T5 — AC-5 invariant gate stays in fmpl-core/tests/, upgraded scope.** Existing `fmpl-core/tests/persistence_envelope_invariant.rs` rewritten in place at `fmpl-core/tests/no_fjall_in_core.rs` (rename for clarity). The gate scans `fmpl-core/src/` (resolved via `env!("CARGO_MANIFEST_DIR")/src`) for **ANY `fjall::*` or `use fjall` substring** — strengthened from the AC-5 grep (which only caught `keyspace.insert(`/`partition.insert(`). After T4 lands, fmpl-core/src/ has zero fjall references; the gate passes. The `(AC-5 ratchet)` entry in behavior-corpus.md is replaced by `(no-fjall-in-fmpl-core ratchet)`.
   *(Note: the original T5 "fmpl-bootstrap minor leanness wins" task was dropped per R2-S14 — claim was unsubstantiated and the leanness win is small. fmpl-bootstrap still benefits indirectly by simply not enabling the renamed `persistence` feature.)*

7. **T6 — AC-6 anti-rot ratchet STAYS in fmpl-core/tests/ (per R3-C3 — earlier R2-C2 resolution was wrong-direction).** `persistence_schema_anti_rot.rs` stays at `fmpl-core/tests/`. Scans `fmpl-core/src/` (where `VM_VERSION_MAJOR/MINOR/PATCH` constants live post-T0.5). **Exemption rule changes from `s.contains("/persistence/")` to `s.ends_with("/vm_version.rs")`** — the new home for the version constants per T0.5. Same form #4 grep, new exemption rule reflecting the post-T0.5 file layout. A SEPARATE schema-format anti-rot gate (different concerns: `ENVELOPE_FORMAT_VERSION`, `PayloadKind` variant numbers, schema versions per kind) lives in `fmpl-persistence/tests/persistence_schema_format_anti_rot.rs` with its own exemption `s.ends_with("/schema.rs")`. **The new fmpl-persistence-side gate's FORBIDDEN_LITERALS list is pinned at task entry:** `["ENVELOPE_FORMAT_VERSION", "PayloadKind::", "current_schema_version"]` — these MUST appear only in schema.rs. behavior-corpus.md gets both entries.

8. **T7 — Sweep dangling cross-references and stale gate text (per R4-C2/R4-C3).** Mechanical cleanup task that previously was conflated into T8's wrap-up:
   - Sweep all "T7 below" / "T1.3" / "T4.0" / "T9 below" textual references in the resolution maps (R1, R2, R3) and fix them to point at current task numbers.
   - Fix the stale verification-gate text at the "Relocated `(AC-6 ratchet)`" line — it currently says exemption `s.ends_with("/schema.rs")` but R3-C3 changed it to `s.ends_with("/vm_version.rs")` AND removed "Relocated" (the gate stays put).
   - Run a final cross-reference audit: `grep -E 'T[0-9](\.[0-9]+)?' docs/superpowers/iterations/roadmap.md` and resolve any references that point at non-existent tasks.
   - Verification: no dangling task references in this card.

9. **T8 — Wrap artifacts.**
   - Iteration log entry per `feedback_no_hallucinated_time_estimates.md` (wall-clock from `/tmp/iter-0005a.5-checkpoints/`).
   - `progress.md` snapshot.
   - Roadmap status → done.
   - **Rewrite ITER-0005a.4's card text in place** — every `&fjall::Keyspace` → `&impl Store`, `save_to_fjall`/`load_from_fjall` → `save_to_store`/`load_from_store`. The 0005a.4 caller-update enumeration shifts to account for the relocated test files.
   - Update EPIC-003 STORY-0099 AC-5 + AC-7 status notes to point at the new module paths.
   - behavior-corpus.md updated: SCENARIO-0099 + 0099-iter + 0111 + 0112 row entries point at `cargo test -p fmpl-persistence --features fjall-backend --test <name>`; AC-5 ratchet renamed; AC-6 ratchet path updated.
   - **Doc cascade for `fjall-persistence` → `persistence` rename (per R2-S13):** rewrite `CHANGELOG.md`, `specs/persistence.md`, `specs/grammar-system.md`, `docs/codebase/fjall-persistence-patterns.md` (likely rename the latter file to `persistence-patterns.md`).

**Impacted scenarios:**

- SCENARIO-0099 (decode-pathway + iter sub-test) — relocates to `fmpl-persistence/tests/`. Iter sub-test uses the `Store` trait API; decode-pathway test stays decode-only (no Store dependency).
- SCENARIO-0111 (writer→loader round-trip) — relocates; constructs `FjallStore`, calls `write(&store, ...)`.
- SCENARIO-0112 (operator detection + isomorphic aggregates) — relocates; constructs `FjallStore`, calls `iter_store(&store, ...)`.
- (AC-5 ratchet) — STAYS in fmpl-core/tests/persistence_envelope_invariant.rs; the original writer-bypass-prevention invariant is preserved. The typed `no_fjall_in_core.rs` upgrade was deferred since fmpl-core's dep-graph already enforces no-fjall structurally (fjall is no longer a regular dep).
- (AC-6 ratchet) — STAYS in fmpl-core/tests/persistence_schema_anti_rot.rs per R3-C3. Scan target stays `fmpl-core/src/`; exemption rule updated to `s.ends_with("/vm_version.rs") || s.ends_with("/lib.rs")` to track the post-T0.5 location of `VM_VERSION_*` constants.
- (Schema-format anti-rot ratchet, NEW) — lives at fmpl-persistence/tests/persistence_schema_format_anti_rot.rs. Scans fmpl-persistence/src/ for `ENVELOPE_FORMAT_VERSION`, `PayloadKind::`, `current_schema_version`. Exemption: schema.rs, envelope.rs, loader.rs (the legitimate wire-format readers).

**Verification gates:**

- `cargo build -p fmpl-persistence --no-default-features` builds (proves the crate compiles without fjall — load-bearing for the API-seam claim).
- `cargo build -p fmpl-persistence --features fjall-backend` builds.
- `cargo build -p fmpl-core --no-default-features` builds with zero references to fjall in the compiled output.
- `cargo build -p fmpl-core --features persistence` builds (the renamed feature).
- **`cargo build --workspace --all-features` builds** (per R2-S9 — catches resolver-v2 workspace-wide feature unification issues that per-crate builds miss; load-bearing during the 0005a.5→0005a.6 transition when fjall v2 and v3 coexist).
- `grep -rn 'fjall::\|use fjall' fmpl-core/src/ | wc -l` = 0. **Hard gate.**
- `grep -rn 'iter_keyspace' fmpl-persistence/src/ fmpl-persistence/tests/ fmpl-core/src/ | wc -l` = 0 (per R2-S12 / T4.13).
- `cargo build -p fmpl-bootstrap` builds.
- `cargo build -p fmpl-web` STILL builds (proves we haven't broken fmpl-web while reshaping fmpl-core — fmpl-web continues using fjall v2 directly until 0005a.6).
- **Rustdoc broken-link gate (R4-C5 + R5-S1/S2 unified mechanism):** ONE mechanism — diff-based count, no `-D` lint. Step 1, before iteration start: capture baseline `cargo doc -p fmpl-core --no-deps 2>&1 | grep -c 'unresolved link'`. **Measured today 2026-05-13: baseline = 6** (in `ast.rs`, `pattern/mod.rs`, `vm_internal/frame.rs`; recompute at iteration entry — number may shift). Step 2, after T4.12: rerun the same command. **Pass condition:** post-T4.12 count ≤ baseline. Pre-existing broken links are OUT OF SCOPE for 0005a.5. Same approach for `-p fmpl-persistence` (baseline = 0 at iteration start since the crate doesn't exist yet; pass condition = 0 after T0 ships). The earlier R3-C4 wording about `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links"` is SUPERSEDED — that mechanism aborts at first broken link, incompatible with the non-zero baseline. T4.12's "broken intra-doc link" grep string is also SUPERSEDED — use `unresolved link` per the actual rustdoc output.
- Full sentinel sweep passes: `cargo test -p fmpl-core --no-fail-fast`; `cargo test -p fmpl-persistence --features fjall-backend --no-fail-fast`. Combined test count ≥ 1352 (the 0005a.3 baseline; tests moved between crates, total preserved).
- New `no_fjall_in_core` typed gate passes.
- `(AC-6 ratchet)` passes (stays in fmpl-core/tests/ per R3-C3; new exemption rule `s.ends_with("/vm_version.rs")` per T0.5 + T6).
- `(schema-format anti-rot ratchet)` passes in fmpl-persistence/tests/ (NEW separate gate; exemption `s.ends_with("/schema.rs")`; FORBIDDEN_LITERALS = ["ENVELOPE_FORMAT_VERSION", "PayloadKind::", "current_schema_version"]).
- Citation check clean.
- Clippy clean across all members.

**Out of scope (deferred):**

- **fmpl-web migration to `Store` trait** — entire scope of ITER-0005a.6.
- **Workspace `fjall = "2"` removal** — happens only after fmpl-web migrates in 0005a.6.
- **fjall v2 → v3 data migration story for fmpl-web** — 0005a.6's responsibility.
- **`is_empty()` / `len()` on the `Store` trait** — added in 0005a.6 when fmpl-web is the first real consumer (per ship-infrastructure-with-first-consumer).
- **Removing `curl`, `tokio = "full"` from fmpl-core** — separately tracked in `ITER-0005a-DEPS-CLEANUP` (new placeholder iteration; would deliver the larger fmpl-bootstrap leanness win).
- **Dead workspace deps (`async-trait`, `tokio-stream`)** — same separate iteration.
- **Pattern-match-with-blocking storage primitives** — would belong to a future durable-tuplespace iteration (not in scope; tuplespaces are pure-RAM today).
- **Rename of `fjall-persistence` feature on fmpl-core to `persistence`** — done in T4.9 of this iteration. Mentioned here to make clear it's the only feature-name change.

**Dependencies / ordering:**

- BLOCKED BY: ITER-0005a.3 (done).
- BLOCKED BY: **ITER-0005a.0 (rescoped, pending)** — provides `fmpl_types::VmVersion` + `Hash` + `SourceHash` used by T0.5 and T3 signatures.
- BLOCKS: ITER-0005a.6 (fmpl-web migration depends on the `Store` trait + `FjallStore` shipped here).
- BLOCKS: ITER-0005a.4 (per-call-site rewires use the new `Store` trait; 0005a.4's card text is rewritten in 0005a.5's T8).
- BLOCKS: ITER-0005b/c/d/e/f (cleaner extraction substrate).

**Risk callouts:**

- **API-breaking rename (T4 + T3).** Every `save_to_fjall`/`load_from_fjall`/`spill_to_fjall`/`restore_from_fjall` method renames to `*_store`. Public error variants reshape (`EnvelopeWriteError::Keyspace` → `EnvelopeWriteError::Store`; `ParseStateError::Fjall` → `ParseStateError::Store`). Any downstream code naming these will break. The persistence module in fmpl-core remains as a re-export shim (T4.10), but method names on `CompiledCode` / `ObjectDb` / `ParseState` change. This is the intentional API-seam fix.
- **Trait sizing for fmpl-web.** `is_empty()` is needed by fmpl-web but not by fmpl-core. Per ship-infrastructure-with-first-consumer, we ship the trait sized for fmpl-core only this iteration. 0005a.6 extends the trait (additive — non-breaking) when fmpl-web is the first real consumer that needs `is_empty`.
- **Test relocation churn.** 7 test files relocate; 1 stays (the AC-5 gate). Atomic commits per jj split workflow.
- **`#[cfg(feature = "fjall-persistence")]` annotations.** Today's gating is inconsistent: compiler.rs + object.rs are gated; grammar/{incremental,stream_input}.rs are NOT. T4 normalizes: drop all `#[cfg(feature = "fjall-persistence")]` from the methods (the methods take `&impl Store` and are unconditional). The feature on fmpl-core renames to `persistence` and gates only the optional `fmpl-persistence` dep, not specific methods.

**Sources:**

- 2026-05-13 dep audit (this session).
- 2026-05-13 tuplespace-overlap analysis (this session).
- 2026-05-13 PAR scope review: 2 reviewers, 7 Critical + 11 Serious findings (this session) — addressed in resolution map above.
- `feedback_ship_infrastructure_with_first_consumer.md` (trait sizing for fmpl-core only).
- `feedback_split_iterations_on_reader_writer_asymmetry.md` (precedent — split into 0005a.5 / 0005a.6).
- `feedback_calibrate_claims_to_evidence.md` (driving-evidence recalibration).
- `feedback_read_actual_code_before_scope_finalization.md` (T4's per-call-site enumeration).
- `feedback_prefer_proof_tests.md` (T6's gate-strengthening from grep to typed seal).
- `specs/tuplespace.md:111-115` (aspirational `durable: true` — design vision, not current state).
- User constraints (2026-05-13): "we shouldn't be leaking storage information across the API seam"; "Shouldn't the storage abstraction be in its own crate?"

---

#### ITER-0005a.6 — Migrate fmpl-web from fjall v2 direct-use to `fmpl-persistence::Store`

**Stories:** none (architectural — completes the fmpl-web side of the persistence extraction begun in 0005a.5).

**Status:** **DONE 2026-05-14.** See `iteration-log.md` ITER-0005a.6 entry for the full close-out. Pre-iteration spike picked Design A (clean-slate; no production data to migrate); T0.5 SKIPPED. All other T-tasks (T0-T6) landed. Closing PAR pending.

Verification at close: fmpl-core 1293/1293 + fmpl-persistence 72/72 + fmpl-workspace-tests 3/3 all passing; clippy clean; `cargo tree --workspace | grep 'fjall v2' | wc -l` = 0; `grep -rn 'fjall::\|use fjall' fmpl-web/src/ | wc -l` = 0.

**Driving evidence (historical, at iteration entry):**

After 0005a.5 closed, the `fmpl-persistence` crate + `Store` trait + `FjallStore` impl exist; fmpl-core uses them; fmpl-web still used fjall v2 directly. The duplicate fjall v2 + v3 compile (the original 0005a.5 driving-evidence #1) remained until this iteration closed. fmpl-web had TWO fjall-touching files (`continuations.rs`, `image_store.rs`) using fjall v2's `Config::new(path).open()` + `keyspace.open_partition(name, opts)` API, which differs from v3's `Database::builder(path).open()` + `db.keyspace(name, opts)`.

**Scope (build order):**

**Pre-iteration spike (REQUIRED before T0; per R2-S10):** the data-migration story must be pinned BEFORE this iteration starts. Spike: (a) inspect fjall v2 vs v3 wire format compatibility — read upstream fjall CHANGELOG / breaking-changes notes between 2.x and 3.x; (b) survey fmpl-web's actual deployment story — does any deployment carry existing fjall v2 data? Spike outcome MUST be one of two pinned designs:
- **Design A (clean-slate):** No production data exists; T2-T3 use v3 fresh-database semantics; document in iteration log that v2 stores would be discarded.
- **Design B (migration writer):** At boot, if v2 store detected, walk it via fjall v2 API, re-insert each (key, value) into a fresh v3 store, atomically replace, delete v2. The migration writer becomes an explicit task in the build order.

If the spike's outcome is neither pinable cheaply, **defer 0005a.6 to a follow-on iteration**; do not enter 0005a.6 with the design unresolved.

1. **T0 — Extend `Store` trait with `is_empty()` (default impl propagates errors per R3-C2).** Add to the trait:
   ```rust
   fn is_empty(&self) -> Result<bool, StoreError> {
       match self.iter().next() {
           None => Ok(true),
           Some(Ok(_)) => Ok(false),
           Some(Err(e)) => Err(e),
       }
   }
   ```
   Note: the prior revision's default `Ok(self.iter().next().is_none())` silently swallowed iterator errors (per R3-C2 — a real correctness bug). The match form propagates them. Add a `FjallStore::is_empty()` override using fjall's native API if cheaper (verify at task entry whether fjall v3 has a direct-count or only-iterate API; if no native is_empty, the default impl is fine). Add a smoke test in fmpl-persistence/tests/ that exercises the error-propagation path (constructed `Store` impl that returns `Err` from `iter().next()`).

**T0.5 (CONDITIONAL — only if pre-iteration spike picked Design B; per R4-C4) — Implement v2→v3 migration writer.** If the spike's outcome was Design A (clean-slate), SKIP T0.5 entirely. If Design B (production data exists), implement:
- A standalone `migrate_v2_to_v3(path: &Path) -> Result<MigrationStats, MigrationError>` function in fmpl-web (or fmpl-web's startup module). Probes the path; if a fjall v2 store is detected (presence of v2-specific files / metadata), opens it with the fjall v2 API (`fjall = "2"` is still a workspace dep at T0.5 time), iterates all (key, value) pairs, opens a fresh v3 store at a sibling path, inserts each pair into v3, atomically swaps the directories, deletes the v2 store.
- Cargo.toml temporarily adds `fjall_v2 = { version = "2", package = "fjall" }` (per R5-C2 fix — Cargo's `package` field names the PUBLISHED crate; the LHS is the local import name) to fmpl-web so v2 and v3 coexist in fmpl-web during T0.5+T1+T2. The renamed import allows the migration writer to call `fjall_v2::Config::new(path).open()` while normal code uses `fmpl_persistence::FjallStore` (which transitively uses fjall v3).
- Smoke test: write a synthetic v2 store, run `migrate_v2_to_v3`, verify the resulting v3 store has the same records and the v2 directory is gone.
- T3 below (drop workspace `fjall = "2"`) MUST NOT run until T0.5's deployment is complete (i.e., production has been migrated). If deployment ordering is uncertain, defer T3 to a follow-on iteration after T0.5 has rolled out.

2. **T1 — Rewrite `fmpl-web/src/continuations.rs`.** Replace `fjall::{Config, PartitionCreateOptions, PartitionHandle}` imports with `fmpl_persistence::{FjallStore, Store}`. `ContinuationStore::new(&path)` becomes `FjallStore::open(&path.join("continuations"))?`. Field type `partition: PartitionHandle` becomes `store: FjallStore`. `.insert(key, bytes)?` calls go through the trait. No fjall references in fmpl-web/src/continuations.rs after this task. The pre-iteration spike's pinned migration design has already been applied via T0.5 (if Design B) or is moot (if Design A).

3. **T2 — Rewrite `fmpl-web/src/image_store.rs`.** Same shape as T1. Includes `bootstrap_if_empty` using the new `Store::is_empty` method.

4. **T3 — Drop `fjall = { workspace = true }` from `fmpl-web/Cargo.toml`.** fmpl-web gains `fmpl-persistence = { path = "../fmpl-persistence", features = ["fjall-backend"] }`. No fjall in fmpl-web's direct deps.

5. **T4 — Remove `fjall = "2"` from workspace `Cargo.toml:40`.** No more workspace fjall pin. fmpl-persistence's fjall v3 dep is direct (non-workspace). **Verify with `cargo tree --workspace | grep 'fjall v2' | wc -l = 0`.**

6. **T5 — Strengthen the no-fjall typed gate to cover fmpl-web/src/.** Per R2-S11, prefer the simpler resolution: create a new workspace-level test crate `fmpl-workspace-tests` (no source code, only integration tests against workspace-level structural invariants) that scans both `fmpl-core/src/` and `fmpl-web/src/`. The crate's `tests/no_fjall_in_consumers.rs` uses `env!("CARGO_MANIFEST_DIR").parent()` once and lists the consumer-crate directories explicitly:
   ```rust
   const CONSUMER_CRATES: &[&str] = &["fmpl-core/src", "fmpl-web/src"];
   ```
   This isolates the cross-crate path resolution to one place, documented and discoverable. The old `no_fjall_in_core` gate at fmpl-core/tests/ can be deleted (the new gate supersedes it). behavior-corpus.md updated.

7. **T6 — Wrap artifacts.** Iteration log, progress, roadmap. Update behavior-corpus.md if fmpl-web's stores have any scenario evidence (they currently don't — flag for future).

**Verification gates:**

- `cargo build -p fmpl-web` builds with zero fjall direct dep.
- `grep -rn 'fjall::\|use fjall' fmpl-web/src/ | wc -l` = 0. **Hard gate.**
- Full sentinel sweep passes.
- fjall v2 no longer compiles anywhere in the workspace: `cargo tree --workspace | grep 'fjall v2' | wc -l` = 0.
- Duplicate-compile recovery: confirm only fjall v3 (+ one lsm-tree, one dashmap) build now.

**Risk callouts:**

- **Production data loss.** Fmpl-web on-disk format changes v2→v3. T1's discovery step is load-bearing; if production data exists, T1.3 designs migration before T2 lands.
- **`tower-sessions` is NOT fjall-backed** (uses MemoryStore) — no session-store work here. The risk from the original 0005a.5 PAR review is removed.
- **fmpl-web has no test coverage of ContinuationStore/ImageStore today.** Adding tests is out of scope; consider a follow-on iteration if regression-detection becomes critical.

**Out of scope (deferred):**

- New backends beyond fjall (in-memory, sqlite).
- Tower-sessions backend swap.
- New scenarios for fmpl-web persistence (no existing coverage to extend).

**Sources:**

- 2026-05-13 dep audit + PAR review (same session as 0005a.5).
- `feedback_split_iterations_on_reader_writer_asymmetry.md` (split rationale).


---

#### ITER-0005b — Content-addressed source store

**Stories:** STORY-0100.

**Status:** **DONE 2026-05-14** (partial closure: AC-1 + AC-7-primitive closed; AC-2 + AC-6 re-opened post-audit (routed through ITER-0005b-FIX-B); AC-3/4/5 + AC-7-orchestration explicitly deferred to named follow-up iterations). See `iteration-log.md` ITER-0005b entry for full close-out. Pre-iter PAR R1 returned REVISE with 6 findings; R2 returned APPROVE after revision. Post-iteration PAR audit (Reviewers A + B, 2026-05-14) found Critical sentinel regression (`persistence_schema_format_anti_rot` red) + Serious evidence-seam gaps on AC-2/AC-6 → resolution routed through ITER-0005b-FIX-A (red-gate cleanup) + ITER-0005b-FIX-B (architectural seam decisions).

Deferred to follow-ups:
- **ITER-0005b-SYNTH** — AC-4 + AC-5 (constructor synthesizer for sourceless artifacts). Blocked by ITER-0005b-AST-SLOT (Lambda holds bytecode, not AST today; synthesizer needs the AST slot or an alternative synthesis story).
- **ITER-0005b-OBJ** — AC-3 (Grammar source_hash; ObjectDb shape mismatch design call).
- **ITER-0005b-GC** — AC-7 keyspace-scan orchestration (SourceStore::compact() primitive shipped here).
- **ITER-0005b-AST-SLOT** — captured in `docs/superpowers/specs/2026-05-14-lambda-ast-slot.md`; user-requested capture-as-design-note during ITER-0005b planning.

**Rationale:** ITER-0005a's envelope carries a `source_hash` field nothing populates yet. This iteration adds the content-addressed store + the constructor-expression synthesizer for sourceless artifacts (anonymous lambdas, runtime grammars, ObjectDb objects without a source file). Constructor synthesis is the hard part — implement and test in isolation before per-payload stories depend on it. Independent of any payload class; depends only on ITER-0005a's envelope.

**Impacted scenarios:** SCENARIO-0100, SCENARIO-0101, SCENARIO-0102.

**Depends on:** ITER-0005a.

**Look-ahead:** ITER-0005c/d will populate `source_hash` on every write.

**Build order:**

1. **T1 — Hash function + content-addressed store API.** Pick hash (blake3 or xxh3 — public, fast, single-dep). Store: `put(content) -> Hash`, `get(hash) -> Option<Bytes>`. Fjall-backed.
2. **T2 — Constructor-expression synthesizer for sourceless artifacts.** For each class of sourceless artifact (anonymous lambda, runtime grammar, ObjectDb object): emit a FMPL source string that, when re-parsed and compiled, reproduces an equivalent artifact. Test in isolation (unit tests; no envelope coupling).
3. **T3 — SCENARIO-0101 card** (sourceless artifact gets synthesized constructor expression).
4. **T4 — SCENARIO-0102 card** (loader recovers from incompatible payload via source recompilation — combines envelope's `source_hash` lookup with constructor synthesis).
5. **T5 — Wrap artifacts.**

**Verification gates:** unit tests on constructor synthesizer per artifact class, SCENARIO-0101/0102 pass, sentinel sweep green.

---

#### ITER-0005b-FIX (DEPRECATED — split into FIX-A, FIX-B, and ITER-PROCESS-TAGS per 2026-05-14 scope-review PAR)

**Status:** **superseded.** Pre-iteration PAR scope review (Reviewers A and B, parallel adversarial) returned REVISE with both reviewers independently recommending a split. Critical finding from Reviewer A: FIX-6's "8+ process-tag sites" was actually **85 sites across 30 files** per `rg`-against-actual-source — most unrelated to STORY-0100 — so a project-wide sweep doesn't belong inside a STORY-0100 fix. Serious finding from both: FIX-2/FIX-3 entangle architectural seam decisions (eval-seam, loader-auto-chain) with red-gate cleanup; each has a "introduce-seam OR correct-AC-wording" branch with different downstream cost.

See the three replacement iterations below: **ITER-0005b-FIX-A**, **ITER-0005b-FIX-B**, **ITER-PROCESS-TAGS**.

The deprecated card text is preserved below this header for traceability; do not work from it.

#### ITER-0005b-FIX-A — Red-gate cleanup for ITER-0005b (CRITICAL: sentinel red; unblocks ITER-0005c)

**Stories:** STORY-0100 (no AC re-closure here; AC-2 and AC-6 remain re-opened — those go to ITER-0005b-FIX-B).

**Status:** **DONE 2026-05-14** (sentinel green via FIX-1 typed re-export laundering; FIX-MECH Option-α sentinel-sweep script shipped at `docs/superpowers/iterations/scripts/run_sentinels.sh`; closing PAR sweep clean: 22 pass / 0 fail / 4 skip on long-standing TBD-command rows). ITER-0005c unblocked. See `iteration-log.md` ITER-0005b-FIX-A entry for full close-out.

**Rationale:** ITER-0005b shipped with `persistence_schema_format_anti_rot` red because `fmpl-persistence/src/recovery.rs:254, 269, 437` (inside `#[cfg(test)] mod tests`) references `PayloadKind::CompiledCode` outside the schema-aware module set. The iteration-log claim "All invariant gates green" (line 1435) is false. This iteration ships the mechanical red-gate fix + cleanup without entangling architectural seam decisions (which go to ITER-0005b-FIX-B). The split rationale tracks `feedback_split_iterations_on_reader_writer_asymmetry.md` — "safe mechanical fix" vs "architectural seam decision" is the same kind of asymmetry that justified ITER-0005a → ITER-0005a.{0,1,2}.

**Pre-commitment per scope-review PAR (Reviewer A + B):**

- **FIX-1 Option choice committed: Option A** (refactor `recovery.rs`'s `#[cfg(test)] mod tests` to route through a typed re-export from a schema-aware module — likely a `pub(crate) fn write_compiled_code_record(...)` helper in `envelope.rs` or `loader.rs`). Rationale: Option A keeps the exemption set minimal (3 files: schema.rs, envelope.rs, loader.rs) and sets the precedent that test helpers in non-schema-aware modules must launder PayloadKind through a typed boundary. Option B (per-module exemption) was rejected because it would let the exemption set grow monotonically over time — every future schema-aware-adjacent module could plead exemption, weakening the ratchet. (Per `feedback_calibrate_claims_to_evidence.md` and `feedback_prefer_proof_tests.md`: typed re-export is stronger than per-file exemption.)

**Acceptance criteria:**

- **FIX-1**: `persistence_schema_format_anti_rot::schema_format_anti_rot_no_literals_outside_schema_aware_modules` is green. **Chosen helper shape**: add kind-specific test helpers — e.g., `pub(crate) fn write_compiled_code_test_record(store: &S, key: &[u8], payload: &impl serde::Serialize, vm: VmVersion, source_hash: Hash) -> Result<()>` — to `envelope.rs` inside a `#[cfg(test)] pub(crate) mod test_helpers` module. The helper's internal body names `PayloadKind::CompiledCode` (legitimate because `envelope.rs` is in the exemption set); `recovery.rs`'s test module calls the helper and removes its `use crate::schema::PayloadKind;` import. Gate: `rg "PayloadKind::" fmpl-persistence/src/recovery.rs` returns no matches AND the sentinel test is green. **Scope note** (per scope re-review B finding): the sentinel scans only `fmpl-persistence/src/`. `fmpl-core`'s production code paths (e.g., `fmpl-core/src/compiler.rs:744` which already names `PayloadKind::CompiledCode`) are not affected and ITER-0005c's design is not constrained by this choice. **Escape valve**: if during T1 the implementer discovers kind-specific helpers would proliferate (e.g., a future iteration's payload-class set forces 6+ helpers and the test-helper module starts dominating envelope.rs), STOP and dispatch a fresh pre-iter PAR pair to re-evaluate. Option B (per-module exemption) remains a legitimate resolution if PAR re-approves it with explicit rationale; the pre-commit forbids only the silent fall-back, not the documented re-decision.
- **FIX-4**: `behavior-corpus.md:91, 93` rows for SCENARIO-0100 and SCENARIO-0102 get concrete execution commands (`cargo test -p fmpl-persistence --features fjall-backend --test scenario_0100_content_addressed_source` and `cargo test -p fmpl-persistence --features fjall-backend --test scenario_0102_recover_incompatible`). Cadence stays `iteration` — promotion to `sentinel` is reserved for after ITER-0005b-FIX-B closes the seam decisions.
- **FIX-5**: Delete `recover_incompatible_from_path` (`fmpl-persistence/src/recovery.rs:220-232`). The function has zero callers. Per `feedback_ship_infrastructure_with_first_consumer.md`. If `fmpl-persistence` were a published crate this would need a deprecation cycle; since it isn't, delete outright.
- **FIX-MECH (NEW per re-review PAR Reviewer A's S2 + Reviewer B's "no mechanical defense" finding)**: Add a mechanical sentinel-sweep gate that prevents the next iteration from shipping a red sentinel under the same trust assumption that just failed. Implementation choices (implementer picks one, PAR-approved at T1.5):
  - **MECH-Option-α** (preferred, smallest blast radius): a shell script `docs/superpowers/iterations/scripts/run_sentinels.sh` that parses `behavior-corpus.md` for rows with `cadence=sentinel`, extracts each row's execution command, runs them all, fails-loud if any fail. Closing-PAR is required to invoke this script and paste its output (or stderr summary) into the iteration-log entry under a `### Sentinel sweep (closing-PAR)` heading. The mechanical gate is "closing-PAR's iteration-log entry contains a verifiable `Sentinel sweep:` block."
  - **MECH-Option-β** (heavier): a Rust integration test at `tests/closing_par_sentinel_sweep.rs` (workspace-level or fmpl-persistence-level) that parses `behavior-corpus.md`, runs each sentinel command, asserts all green. Run automatically by CI / by `cargo test`.
  - **MECH-Option-γ** (lightest): a `.git/hooks/pre-push` (committed at `.githooks/pre-push` and installed via repo setup script) that runs the sentinel sweep and refuses to push if anything is red. This addresses the failure mode but is bypassable; defense in depth, not the primary gate.
  - **FIX-MECH commitment**: ship at minimum MECH-Option-α (script + closing-PAR template update); MECH-Option-β is the gold standard if the script approach proves brittle (e.g., behavior-corpus.md cadence column is hand-edited and easily desync'd from reality). The acceptance criterion is "next iteration's closing PAR mechanically demonstrates the sentinel sweep was run" — `feedback_prefer_proof_tests.md` form #4 (universally-quantified structural assertion: every closing PAR after FIX-A has sentinel-sweep evidence).
- **FIX-7**: Correct the historical inaccuracies in ITER-0005b's iteration-log entry + the matching status line in roadmap.md, in a way that preserves chronology (the iteration-log entry records only what was true AT ITER-0005b's close; FIX-A's own iteration-log entry will record its post-fix state). Semantic edits (not literal-string find-and-replace — implementer must read each line and preserve surrounding prose and markdown formatting):
  - **iteration-log.md around line 1435** ("All invariant gates green: …"): replace the claim that all invariant gates were green with the truth — `persistence_schema_format_anti_rot` was RED at close (the inline `recovery.rs:254/269/437` `PayloadKind::` references inside the test module tripped the gate); the post-iteration PAR audit (Reviewers A + B, 2026-05-14) caught it; resolution is in ITER-0005b-FIX-A FIX-1. Preserve the surrounding list of other gates (AC-5 writer-bypass, AC-6 anti-rot, cross-consumer no-fjall) that genuinely WERE green.
  - **iteration-log.md around line 1432** (the test-count line): replace the "fmpl-persistence: 101 passing" subline with the correct count at ITER-0005b close: 102 passing + 1 FAILING (`schema_format_anti_rot_no_literals_outside_schema_aware_modules`). Preserve the `(--features fjall-backend):` qualifier and any markdown bold/markup. Do NOT inline FIX-A's post-fix count here — that count belongs in FIX-A's own iteration-log entry to keep the records chronological.
  - **roadmap.md around line 2065** (the ITER-0005b status line under that iteration's header): replace the "AC-1/2/6 closed; AC-3/4/5/7 explicitly deferred to named follow-up iterations" phrasing with: AC-1 + AC-7-primitive closed; AC-2 + AC-6 re-opened post-audit (routed through ITER-0005b-FIX-B); AC-3/4/5 + AC-7-orchestration explicitly deferred to named follow-up iterations. Preserve the trailing prose "to named follow-up iterations" and any surrounding context.

**Impacted scenarios:** `persistence_schema_format_anti_rot` (sentinel; goes red→green); SCENARIO-0100 + SCENARIO-0102 corpus rows (cadence stays iteration).

**Depends on:** ITER-0005b (cleans up its red gate).

**Build order:**

1. **T1 — FIX-1**: write the typed-re-export (`write_compiled_code_test_record` in `envelope.rs#test_helpers`), update `recovery.rs`'s `#[cfg(test)] mod tests` to call it, run `cargo test -p fmpl-persistence --features fjall-backend --test persistence_schema_format_anti_rot` → green. This is the dominant complexity (~80% of FIX-A's work per scope-review B). If `T1` materially expands beyond expectation, ship T1 alone and defer T2-T5 to a follow-on `ITER-0005b-FIX-A.1` micro-iteration; do NOT bundle a struggling T1 with the mechanical T2-T5.
2. **T2 — FIX-5**: `git grep recover_incompatible_from_path` to verify zero callers, delete the function and its `pub` visibility entry from `recovery.rs`.
3. **T3 — FIX-4**: edit `behavior-corpus.md:91, 93` rows (preserve markdown table formatting; FIX-B may re-edit these rows for AC-2/AC-6 evidence updates — FIX-A's commit message should explicitly NOT claim these rows are "final").
4. **T4 — FIX-7**: semantic edits to `iteration-log.md:1432, 1435` + `roadmap.md:2065` (read each line; preserve surrounding prose and markdown).
5. **T5 — FIX-MECH**: ship MECH-Option-α (sentinel-sweep script + closing-PAR template update). Capture sentinel-sweep output for THIS iteration as the first use of the script.
6. **T6 — Wrap**: closing PAR with mandatory sentinel-sweep run (Reviewers; the sweep is now mechanically demonstrable via FIX-MECH); update progress.md. ITER-0005c's `Depends on:` line gets `ITER-0005b-FIX-A` added by this T-task (currently lists only ITER-0005a, ITER-0005b).

**Verification gates:**

- `cargo test -p fmpl-persistence --features fjall-backend` — all tests green INCLUDING `persistence_schema_format_anti_rot`.
- `cargo build --workspace --all-features` + `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `rg -n "PayloadKind::" fmpl-persistence/src/recovery.rs` returns no matches in the test module.
- `git grep recover_incompatible_from_path` returns no matches.
- behavior-corpus.md rows 91 + 93 have non-TBD commands.
- iteration-log.md + roadmap.md historical-text corrections landed (chronologically clean — FIX-A's own state not bleed into ITER-0005b's record).
- **Sentinel-sweep mechanical gate**: the closing-PAR iteration-log entry contains a verifiable `### Sentinel sweep (closing-PAR)` block listing each `cadence=sentinel` scenario from behavior-corpus.md and its pass/fail status. The sweep is invoked via FIX-MECH's MECH-Option-α script (`docs/superpowers/iterations/scripts/run_sentinels.sh`) and its output captured.
- ITER-0005c's `Depends on:` line at roadmap.md ~line 2320 (currently "ITER-0005a, ITER-0005b") includes `ITER-0005b-FIX-A`.

**Sources:**

- ITER-0005b post-iteration PAR audit (2026-05-14) — Reviewers A and B.
- ITER-0005b-FIX scope-review PAR (2026-05-14) — Reviewers A and B (this scope's pre-iter PAR).
- `feedback_calibrate_claims_to_evidence.md`, `feedback_ship_infrastructure_with_first_consumer.md`, `feedback_no_story_names_in_code_comments.md`, `feedback_prefer_proof_tests.md`, `feedback_split_iterations_on_reader_writer_asymmetry.md`.

---

#### ITER-0005b-FIX-B — AC-2 / AC-6 evidence-seam decisions (architectural)

**Stories:** STORY-0100 (re-closes AC-2 and AC-6 after the seam decisions are made and evidence shipped).

**Status:** pending (requires its own pre-iter PAR; can run in parallel with ITER-0005a.2 once FIX-A is done).

**Rationale:** ITER-0005b's audit found AC-2's `seam:integration impact:journey` evidence missing (no `eval()` → persist path) and AC-6's loader-auto-chain + bind-and-execute observables absent. Each AC has two acceptable resolutions:

- **AC-2 path A — introduce an eval-seam API**: e.g., `eval_and_persist(vm, source, store, source_store) -> Result<Value>` in `fmpl-core`, with an integration test calling it and observing the resulting envelope's `source_hash`. Cost: new public surface; boxes in ITER-0005c's wiring; possibly affects ITER-0005b-OBJ for Grammar.
- **AC-2 path B — correct AC-2 wording + impact label**: amend STORY-0100 AC-2 to declare the actual evidence seam (`Compiler::compile() + save_to_store(...)`) and downgrade `impact:journey` to `impact:cross-surface` or `impact:local` if the API isn't a user journey today. Cost: requirements-doc edit; defers the eval-seam decision to whichever iteration first needs it (likely ITER-0005c).

- **AC-6 path A — wire recovery into the loader (auto-chain)**: extend `iter_store`'s callback to receive `RecoveryAttempt` events when records decode as `SkippedIncompatible` with non-NONE `source_hash`; consolidate stats into `LoaderStats` with a `recovered_from_source` counter; extend `LoaderStats::check_invariants` to cover the new sub-reason histogram. Add an integration test that exercises the full eval→bind→`Value::Int(3)` path. Cost: invariant work + callback contract change (this reverses ITER-0005b's pre-iter PAR R-I-C-1 decision; needs explicit justification).
- **AC-6 path B — keep recovery standalone + amend AC-6 + SCENARIO-0102 wording**: update AC-6 to say "the **caller** invokes `recover_incompatible` after the loader; it recompiles via a caller-supplied closure that may call `eval()`; the caller binds the result"; update SCENARIO-0102 expected observables to match `RecoveryStats` shape and to call eval() in the closure. Add a bind-and-execute integration test against `recover_incompatible` directly with a real eval-binding closure. Cost: requirements + scenario-card edit; preserves R-I-C-1 decision; smaller blast radius.

**Pre-iter PAR (when this iteration runs) must decide each AC's path before T1.** Sibling-project study (per `feedback_sibling_project_study_before_scope.md`) before path commitment — what does moor-echo's migration tracer do? What does cairn's pipeline do at an incompatible-decode? A 30-min read of `~/development/moor-echo` and `~/development/cairn` may surface adopted patterns.

**Acceptance criteria:**

- **AC-2-DECIDE**: Per pre-iter PAR's verdict, EITHER:
  - **2A**: `eval_and_persist` (or equivalent) shipped in `fmpl-core` with an integration test at `fmpl-core/tests/scenario_0100_eval_integration.rs` that calls the new API and observes the persisted `source_hash`. (Test must live in `fmpl-core/tests/` because `fmpl-persistence` does not depend on `fmpl-core` — the dep is reversed; the scope-review PAR caught this structural impossibility in the deprecated FIX-2 location citation.)
  - **2B**: STORY-0100 AC-2 text amended in `EPIC-003.md` to reflect the actual seam. Reviewer-A finding "AC-2's `impact:journey` label may itself be wrong" is the decision point: if amending wording, also consider amending impact label.
- **AC-6-DECIDE**: Per pre-iter PAR's verdict, EITHER:
  - **6A**: `iter_store` callback contract extended; `LoaderStats` gains `recovered_from_source` counter; `LoaderStats::check_invariants` extended; integration test exercises eval→bind→`Value::Int(3)` via auto-chain. AC-6 wording stays. SCENARIO-0099 + SCENARIO-0099-iter rows in behavior-corpus need explicit acknowledgment of contract change.
  - **6B**: STORY-0100 AC-6 + SCENARIO-0102 amended to describe standalone-pass shape with caller-supplied eval-binding closure; bind-and-execute integration test added against `recover_incompatible` directly.
- **AC-2-EVIDENCE + AC-6-EVIDENCE**: whichever path is chosen, the AC is genuinely closed (test asserts the AC's actual claim at the chosen seam). The closing PAR explicitly verifies the AC's claim scope matches the test's evidence scope per `feedback_claim_scope_must_match_evidence.md`.

**Impacted scenarios:** SCENARIO-0100, SCENARIO-0102, possibly SCENARIO-0099 + SCENARIO-0099-iter (if 6A chosen), possibly a new SCENARIO-0114 (eval-seam scenario, if 2A chosen).

**Depends on:** ITER-0005b-FIX-A (sentinel green).

**Build order (preliminary; pre-iter PAR will refine):**

1. **T0 — Sibling-project study**: 30-min read of moor-echo + cairn for incompatible-decode patterns. Capture findings in a design note before path commitment.
2. **T1 — Pre-iter PAR for path commitment**: dispatch paired reviewers to choose 2A vs 2B and 6A vs 6B with documented rationale. Each path's downstream impact on ITER-0005c, ITER-0005d, ITER-0005e, ITER-0005b-OBJ explicit.
3. **T2 — T5: Implementation per chosen paths.** Specific tasks depend on path; pre-iter PAR will produce the task list.
4. **T6 — Wrap**: closing PAR; update artifacts.

**Verification gates:**

- AC-2 evidence at the chosen seam: integration test exists and exercises the API/wording that AC-2 actually declares.
- AC-6 evidence at the chosen seam: integration test exists with bind-and-execute observable `Value::Int(3)` (path A) or amended-card observable (path B).
- If 6A: `LoaderStats::check_invariants` is extended and passes.
- All tests green; sentinel sweep clean.
- Closing PAR runs sentinel sweep and captures output.

**Sources:**

- ITER-0005b post-iteration PAR audit (2026-05-14) — Reviewers A and B.
- ITER-0005b-FIX scope-review PAR (2026-05-14) — Reviewers A and B (deprecated parent's pre-iter PAR).
- `feedback_split_iterations_on_reader_writer_asymmetry.md`, `feedback_claim_scope_must_match_evidence.md`, `feedback_sibling_project_study_before_scope.md`.

---

#### ITER-PROCESS-TAGS — Strip process tags from source code (project-wide sweep)

**Stories:** none (housekeeping iteration; enforces `feedback_no_story_names_in_code_comments.md` project-wide).

**Status:** pending (low-priority; non-blocking for ITER-0005c).

**Rationale:** Reviewer A of the ITER-0005b-FIX scope-review found `rg -n "ITER-|STORY-|R-[A-Z]-[CSM]-[0-9]|AC-[0-9]" fmpl-core/src fmpl-persistence/src fmpl-persistence/tests` returns **85 matches across 30 files** — only 8-10 of those were introduced by ITER-0005b. The rest predate. Per `feedback_no_story_names_in_code_comments.md`, this is a project-wide invariant being violated. Bundling it inside a STORY-0100 fix card would have touched files unrelated to STORY-0100 (e.g., `fmpl-core/src/parser_epoch.rs`, `fmpl-core/src/grammar/parser.rs`, `fmpl-core/src/cross_compile.rs`, `fmpl-core/src/builtins/`) — scope creep. Splitting to a dedicated iteration lets ITER-0005b-FIX-A ship fast and lets this housekeeping pass through its own PAR with appropriate context (e.g., "what rationale was load-bearing in these comments? Move to commit messages or iteration-log lessons.").

**Acceptance criteria:**

- **TAG-1**: `rg -n "ITER-[0-9]|STORY-[0-9]|SCENARIO-[0-9]|R-[A-Z]-[CSM]-[0-9]|AC-[0-9]" fmpl-*/src fmpl-*/tests` returns empty (or returns only matches inside scenario card filenames like `scenario_0099_envelope_loader.rs` which are filename references, not process tags — the gate must distinguish these). Refine the regex to exclude filename references in `use` statements and test module names.
- **TAG-2**: For each removed tag whose surrounding comment was load-bearing (e.g., "added in ITER-0005b to fix R-I-C-1"), the rationale is preserved in the file's existing commit messages OR added to the appropriate `iteration-log.md` lessons entry OR (if absolutely necessary as inline doc) reworded to omit the process tag while preserving the constraint (e.g., "Recovery is a separate pass to avoid extending the loader's per-record invariant equation").
- **TAG-3**: `fmpl-persistence/Cargo.toml:34` "ITER-0005b:" prefix removed.
- **TAG-4**: A scenario-format-anti-rot-style proof test added that scans for process-tag identifiers in `*.rs` source and fails if any appear. Per `feedback_prefer_proof_tests.md` form #4 (universally-quantified structural assertion). This is the gate that prevents recurrence — the lesson `feedback_no_story_names_in_code_comments.md` was already memorized, but ITER-0005b violated it 8 times anyway, so the lesson-alone isn't sufficient; a mechanical gate is needed.

**Impacted scenarios:** none directly; TAG-4 adds a new sentinel-cadence proof test.

**Depends on:** none (independent of all STORY-0100 work).

**Build order:**

1. **T1 — Inventory**: run the `rg` query, classify each hit (file-reference-in-`use` vs process-tag-in-comment); produce a worksheet.
2. **T2 — Sweep**: bulk edit the comment hits; preserve load-bearing rationale via reword or relocate.
3. **T3 — Proof test (TAG-4)**: add `tests/no_process_tags_in_source.rs` (or similar) at the workspace level or per-crate level.
4. **T4 — Verify**: `rg` returns empty (modulo agreed exemptions); proof test is green.
5. **T5 — Wrap**: closing PAR; update artifacts.

**Verification gates:**

- `rg -n "<process-tag-regex>" fmpl-*/src fmpl-*/tests` returns empty (or only exempt matches).
- New proof test (TAG-4) is green and added to the behavior-corpus index as a sentinel-cadence row.
- `cargo build --workspace --all-features` + clippy clean.

**Sources:**

- ITER-0005b-FIX scope-review PAR (2026-05-14) — Reviewer A's "85 matches across 30 files" finding.
- `feedback_no_story_names_in_code_comments.md`, `feedback_prefer_proof_tests.md`.

---

#### ITER-0005b-FIX (DEPRECATED — full text moved to iteration-log audit-trail; see ITER-0005b-FIX-A + FIX-B + ITER-PROCESS-TAGS for active work)

**Status:** **SUPERSEDED** by ITER-0005b-FIX-A + ITER-0005b-FIX-B + ITER-PROCESS-TAGS per scope-review PAR 2026-05-14.

The original card's audit-findings narrative (sentinel regression + AC-2/AC-6 evidence-seam mismatches + corpus-not-promoted + unused-API + process-tags) is captured authoritatively in:

- `progress.md` Post-audit reconciliation section
- The three active replacement-iteration cards above
- The ITER-0005b iteration-log entry's audit-trail (to be amended by FIX-A's FIX-7)

Do not work from this deprecated card. Its original Acceptance Criteria framing ("FIX-1 Option A or Option B — pick whichever lands cleaner") is **no longer policy**: FIX-A pre-commits to Option A; FIX-B owns the AC-2/AC-6 seam decisions with their own pre-iter PAR.

---

#### ITER-0005c — Single-payload-class persistence: bytecode (proof case)

**Stories:** STORY-0014.

**Status:** pending

**Rationale:** Bytecode is the smallest, best-understood payload class (`CompiledCode` already has rkyv support per the original scope card). Use it as the **proof case** for the envelope + source-store + payload-writer pattern. If something breaks, debug it on a small target before scaling to the other four payload classes. This iteration validates that ITER-0005a's envelope and ITER-0005b's source-store compose cleanly through a real round-trip.

**Impacted scenarios:** SCENARIO-0007 (or whichever sentinel proves bytecode survives a restart).

**Depends on:** ITER-0005a, ITER-0005b, ITER-0005b-FIX-A.

**Look-ahead:** ITER-0005d will mirror this pattern across objects, grammar definitions, GrammarRegistry, memo tables.

**Build order:**

1. **T1 — Wire `CompiledCode` through the envelope writer.** Use ITER-0005a.2's `persistence::envelope::write` helper. Populate `source_hash` from ITER-0005b's content-addressed source store. Loader uses ITER-0005a.1's keyspace iterator + skip-on-incompatible logic. (Note: pre-2026-05-12 wording referenced `MigrationEngine::migrate` here; that engine was deferred per ITER-0005a.0's deferral rationale and will be revived as `MigrationVisitor` on ITER-0005e's tracer substrate when the first real schema change arrives.)
2. **T2 — Process-restart round-trip test.** Spawn a subprocess (or simulate restart via a Vm wipe + reload), compile a simple expression, persist, restart, reload, verify result. Probably the integration boundary that proves the stack.
3. **T3 — Scenario evidence** (the existing or new SCENARIO covering bytecode-persistence).
4. **T4 — Wrap artifacts.**

**Verification gates:** bytecode persistence round-trip passes, sentinel sweep green, clippy clean.

---

#### ITER-0005d — Remaining payload classes

**Stories:** STORY-0013 (objects), STORY-0015 (grammar definitions), STORY-0019 (GrammarRegistry), STORY-0021 (memo tables).

**Status:** pending

**Rationale:** Four payload classes that share the same machinery proven in ITER-0005c. Each has its own serialization detail (objects already derive Serialize; grammar semantic actions contain AST expressions — the hardest case per the original scope; memo tables are partially Fjall-integrated already). This is parallel work across four targets, not a single monolithic story.

**Caveat at iteration entry:** if this still feels too large after running-an-iteration's pre-scope review, split along the AC hardness boundary — one iteration for "objects + GrammarRegistry" (easier; existing Serialize derives), another for "grammar definitions with AST semantic actions + memo tables" (harder; needs careful round-trip semantics). The PAR scope review at iteration entry will surface whether this split is needed.

**Impacted scenarios:** SCENARIO-0008 (objects), SCENARIO-0010/0011 (grammars).

**Depends on:** ITER-0005c (proves the pattern works on a small target).

**Look-ahead:** ITER-0005e's VM snapshot composes all four payload writers.

**Build order:**

1. **T1 — Object persistence (STORY-0013).** Use ObjectDb's existing Serialize derives; wire through envelope; round-trip test.
2. **T2 — GrammarRegistry persistence (STORY-0019).** Standard envelope adapter; tests follow STORY-0014's pattern.
3. **T3 — Grammar definitions with AST semantic actions (STORY-0015).** The hard case — semantic actions contain AST expressions that may reference compiler internals. Round-trip test specifically exercises a grammar with a non-trivial semantic action.
4. **T4 — Memo table persistence (STORY-0021).** Build on the partial Fjall integration already present; complete the round-trip.
5. **T5 — Wrap artifacts.**

**Verification gates:** four per-payload round-trip tests pass, scenario evidence updated, sentinel sweep green, clippy clean.

---

#### ITER-0005e — VM snapshot + full image roundtrip (lands tracer substrate as foundation)

**Stories:** STORY-0016 (VM snapshot), STORY-0017 (full image roundtrip), STORY-0018 (normal startup loading), STORY-0020 (Vm::snapshot/restore API).

**Status:** pending

**Rationale:** Snapshot composes all per-payload writers from ITER-0005d into a single API (`Vm::snapshot(dir)` / `Vm::restore(dir)`). SCENARIO-0019's `let x = 42 → snapshot → restore → access` is the **journey-level test** that proves the whole stack holds together. Also wires the normal-startup-from-image path so a fresh process loads the image transparently.

**Architectural framing (2026-05-12, post-PAR retrospective):** Per `docs/superpowers/specs/2026-05-12-lessons-from-siblings.md` §2.5 — the Smalltalk tracer family is a **generic object-graph walker with pluggable visitor semantics**, not just a migration tool. ITER-0005e is the right home for the **tracer substrate** because `Vm::snapshot`'s determinism + reproducibility requirements pin the substrate's design correctly. The substrate's first visitor (`SerializationVisitor`) is the consumer that earns its keep; ITER-0006's `ReachabilityVisitor` (seed-snapshot) then inherits the substrate without rework; the eventual `MigrationVisitor` (first real schema change) lands as a third visitor on the same substrate, replacing what was deferred from ITER-0005a.0. This is the "infrastructure ships with its first consumer" discipline (per `feedback_ship_infrastructure_with_first_consumer.md`) applied at the right granularity — the visitor pattern, not the engine, is what waits for a consumer; the substrate ships when the first visitor demands it.

**Implication for build order:** rather than hand-rolling `Vm::snapshot`'s traversal, design the snapshot as the first visitor on the tracer substrate. The substrate's design is pinned by `SerializationVisitor`'s needs (deterministic traversal order, cycle handling, single-write-per-object guarantee) — NOT by speculation about future visitors.

**Impacted scenarios:** SCENARIO-0009, SCENARIO-0019.

**Depends on:** ITER-0005d.

**Look-ahead:** ITER-0005f wires the feature flag around this surface. ITER-0006 inherits the tracer substrate and adds `ReachabilityVisitor` for seed-snapshot creation. A future schema-change iteration adds `MigrationVisitor` to the same substrate (this is the revival path for the deferred ITER-0005a.0 work).

**Build order:**

1. **T0 (NEW) — Tracer substrate + `SerializationVisitor`.** Land `fmpl-core/src/persistence/tracer/` (new module). Substrate surface: worklist-based object-graph walker with cycle handling, single-visit guarantee, deterministic ordering, and visitor-dispatch via a `TracerVisitor` trait. First visitor: `SerializationVisitor` that emits payloads through ITER-0005d's per-payload writers in deterministic order. Design specifically pinned to `SerializationVisitor`'s needs; speculative features for future visitors (ReachabilityVisitor, MigrationVisitor) are NOT in scope here — those iterations will extend the substrate when they need to.
2. **T1 — `Vm::snapshot(dir)` API (STORY-0020 AC-1).** Wires through T0's substrate + `SerializationVisitor` to write scope, ObjectDb, GrammarRegistry, compiled-code cache to the given directory.
3. **T2 — `Vm::restore(dir)` API (STORY-0020 AC-2).** Loads all state into a fresh Vm. (Note: restore is not a tracer visitor — it's a payload-by-payload reader using ITER-0005a.1's envelope loader. The tracer substrate is for writing, not reading.)
4. **T3 — `let x = 42` journey roundtrip (STORY-0020 AC-3).** SCENARIO-0019 evidence.
5. **T4 — Normal-startup loading (STORY-0018).** A fresh process detects a persisted image and loads from it transparently.
6. **T5 — Full-image roundtrip composition test (STORY-0017).** A non-trivial program survives a full snapshot/restore.
7. **T6 — Substrate-genericity gate (proof-like).** A typed-invariant test that asserts the substrate's API doesn't reference `Serialization` anywhere (i.e., the substrate is genuinely generic over visitor type, not silently coupled to its first consumer). Per `feedback_prefer_proof_tests.md` form #1 (typed invariants > greps), the strongest form is a separate test crate or module that depends only on the substrate (not on `SerializationVisitor`) and compiles. ITER-0006 will validate the substrate's genericity in practice by adding `ReachabilityVisitor` on top.
8. **T7 — Wrap artifacts.**

**Verification gates:** SCENARIO-0019 passes; normal-startup loads a persisted image; substrate-genericity gate (T7) is green; sentinel sweep green; clippy clean.

**PAR scope review focus (at iteration entry):** Specifically probe whether the substrate API surface (worklist representation, cycle-tracking, visitor-dispatch shape) is generic enough to admit `ReachabilityVisitor` (ITER-0006) and `MigrationVisitor` (eventual schema change) WITHOUT rework — AND conservative enough that it isn't carrying speculative features for those future visitors. The two reviewers should disagree on the right answer if there's genuine ambiguity; that disagreement is signal.

**Out of scope:**
- `ReachabilityVisitor` (deferred to ITER-0006 — that iteration's consumer is the bootstrap seed snapshot).
- `MigrationVisitor` + `MigrationEngine` (deferred to the first schema-change iteration — this is the revival path for ITER-0005a.0).
- `QueryVisitor` / `RewriteVisitor` / any other visitor class (no near-term consumer).

---

#### ITER-0005f — Feature flag wiring + final polish

**Stories:** STORY-0069.

**Status:** pending

**Rationale:** Ship the `fjall-persistence` feature flag last so the default-disabled path is well-defined and byte-identical to today's behavior. This makes ITER-0005-family the last iteration where persistence is opt-in; downstream work (ITER-0006 et seq.) can begin assuming the feature is available.

**Impacted scenarios:** the cross-iteration sentinel sweep — proving default-disabled and feature-enabled both pass.

**Depends on:** ITER-0005e.

**Look-ahead:** ITER-0006 (Self-Compile and Seed) can now depend on `fjall-persistence` being a stable, available feature.

**Build order:**

1. **T1 — `fjall-persistence` feature flag.** Cargo.toml feature definition; `#[cfg(feature = "fjall-persistence")]` gates on the persistence module entry points.
2. **T2 — Default-disabled regression test.** A sentinel-level test that asserts default-feature builds behave byte-identically to pre-ITER-0005 behavior (no persistence side-effects, no Fjall files written).
3. **T3 — Doc update.** Persistence section in the user docs; the feature is now documented as opt-in but stable.
4. **T4 — Wrap artifacts.**

**Verification gates:** both `cargo test -p fmpl-core` (default) and `cargo test -p fmpl-core --features fjall-persistence` pass, sentinel sweep green on both, clippy clean on both.

---

**Cross-family out-of-scope (deferred to later iterations):**

- **Cache freshness via `invalidation`.** Once persisted artifacts exist (post-ITER-0005f) and recompilation cost is measurable, a follow-up iteration ports `invalidation` to track source-file → bytecode-artifact dependency edges. Public Rust crate (`invalidation = "0.2"`), 1 transitive dep, acceptable footprint per `feedback_dependency_policy.md`.
- **FMPL-side authorship of MigrationRules.** ITER-0006+ (metacircular lift). Once `io::read_dir` exists (deferred from ITER-0004d.4 → ITER-0004d.5), FMPL programs author migration rules that satisfy the Rust trait. Same pattern as moor-echo's MOO authorial layer.
- **Cairn-style span-on-every-Instruction discipline.** Separate iteration; orthogonal to persistence. Architectural improvement to the compiler, not the persistence layer.

### ITER-0006 — Self-Compile and Seed

**Stories:** STORY-0024, STORY-0025, STORY-0027, STORY-0028
**Rationale:** Create seed snapshot from current Rust compiler (Stage 0). Add --snapshot and --from-seed flags to fmpl-bootstrap. Write fmpl_compiler.fmpl — the FMPL compiler driver that orchestrates the full pipeline (fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile). Verify round-trip: snapshot → restore → compile "1 + 2" → get 3.
**Status:** pending
**Impacted scenarios:** SCENARIO-0020
**Depends on:** ITER-0004 (compiler cutover), ITER-0004b + ITER-0004c + ITER-0004d (full canonical-representation refactor — runtime burn + FMPL stdlib migration + parser/AST burn), and the ITER-0005a–ITER-0005f sub-iteration family (persistence — ITER-0005f's close is the explicit gate). The fmpl_compiler.fmpl pipeline `fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile` requires that *every* stdlib file in that chain be in the canonical list-pattern syntax (delivered by ITER-0004c) AND that the parser accepts only one AST shape (delivered by ITER-0004d).
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

### ITER-0005-WEB-PERSISTENCE — Sweep fmpl-web writers through the envelope helper

**Stories:** STORY-0099 AC-5 extension (fmpl-web scope).

**Status:** candidate (added 2026-05-13 from ITER-0005a.2 audit fix-up G3).

**Rationale:** ITER-0005a.2's PAR audit (2026-05-13) flagged that `fmpl-web/src/` contains 4 pre-existing raw `partition.insert(...)` sites that violate AC-5's literal wording ("no caller writes raw `serde_json` bytes to a Fjall keyspace"):
- `fmpl-web/src/continuations.rs:66` — initial save of a `SnapshotEnvelope`.
- `fmpl-web/src/continuations.rs:126` — previous-state save during stream rotation.
- `fmpl-web/src/continuations.rs:142` — update-last-action save.
- `fmpl-web/src/image_store.rs:26` — raw FMPL source storage.

ITER-0005a.2's scope was `fmpl-core/src/` only; sweeping fmpl-web is non-trivial because:

1. **`fjall::PartitionHandle` vs `fjall::Keyspace`** — fmpl-web uses partition handles; the `persistence::envelope::write` helper takes `&fjall::Keyspace`. Either widen the helper's signature or add a parallel `write_partition` helper.
2. **Parallel `SnapshotEnvelope` abstraction** — `continuations.rs` already has its own envelope with `schema_version` / `bytecode_version` / `engine_version` / `created_at` fields. Wrapping it in the fmpl-core envelope produces double-envelope semantics that need a design pass.
3. **Unstructured FMPL source payload class** — `image_store.rs` writes raw FMPL source text, not a typed Serialize-shaped payload. Needs either a new `PayloadKind::Source` variant or a "raw bytes" envelope-write helper.

**Anticipated scope (per the audit doc; actual scope card to be PAR-reviewed at iteration entry):**

- Decide write-helper API (single helper with `Into<&Keyspace>`-style adapter, or two helpers).
- Decide envelope-of-envelope vs. retire `SnapshotEnvelope` in favor of the fmpl-core envelope.
- Add `PayloadKind::FmplSource` (or similar) for raw FMPL source storage.
- Sweep the 4 sites + their corresponding `load` paths (transitional manual prefix-strip pattern, same as ITER-0005a.2).
- Extend the AC-5 invariant gate to scan `fmpl-web/src/` as well.
- Widen AC-5 wording in EPIC-003 back to "all currently-extant writers" once this iteration ships.

**Depends on:** ITER-0005a.3 (load-side decode rewire in fmpl-core; reusing the pattern here).

**Look-ahead:** unblocks fully-uniform persistence across fmpl-core + fmpl-web. After this, AC-5 covers the whole workspace, not just `fmpl-core/src/`.

**Reference:** ITER-0005a.2 audit fix-up G3 (2026-05-13).

---

### ITER-FFI-PROLOG-PHASE-1 — Expose backtracking via FMPL-surface builtins

**Stories:** none (infrastructure-only; no story currently captures this surface).

**Status:** candidate (added 2026-05-13; not sequenced).

**Rationale:** First phase of the Prolog-shaped FFI reframe surveyed in `docs/superpowers/specs/2026-05-13-prolog-shaped-ffi.md`. The grammar engine already runs a Prolog-style backtracking evaluator via `PegRuntime` (`backtrack` at `fmpl-core/src/grammar/runtime.rs:1751`, `get_all_alternatives` at line 1859), validated by 19 backtracking tests across `tests/{backtracking,guard_backtracking,send_more_money_fmpl}.rs` (all passing 2026-05-13, including full SEND+MORE=MONEY CSP solving). The capability exists in Rust but isn't surfaced to FMPL code outside `grammar { ... }` blocks. Phase 1 exposes the Rust APIs as FMPL builtins so FMPL code can drive backtracking explicitly without writing a grammar.

This is the **smallest standalone step** toward the Prolog-shaped FFI reframe. It earns its keep independently (CSP-style FMPL programs become writable without a grammar wrapper) AND it produces the surface ergonomics signal that informs whether Phases 2-5 (tuple-space primitives, method dispatch via roles, builtin migration, persistence convergence) are worth scheduling. Per `feedback_ship_infrastructure_with_first_consumer.md`, Phase 1 has a concrete consumer the day it lands (any FMPL CSP/search program); Phases 2-5 should only schedule after Phase 1 reveals real usage pressure.

**Anticipated scope (per the lessons doc — actual scope card will be PAR-reviewed at iteration entry):**

- Add `__builtin_backtrack::next_alternative` (or similar) routing to `PegRuntime::backtrack`.
- Add `__builtin_backtrack::all_alternatives` routing to `PegRuntime::get_all_alternatives`.
- Wire `Stream<Alternative>` integration so FMPL code can iterate alternatives.
- 1-2 example tests demonstrating FMPL-driven backtracking outside a grammar context.
- Add a behavior-corpus scenario covering "FMPL CSP program iterates alternatives via builtins."

Probable size: 3-5 tasks per the lessons doc estimate; verify at scope-card time per `feedback_read_actual_code_before_scope_finalization.md`.

**Depends on:** nothing in the ITER-0005 family. Could schedule any time after ITER-0004 close.

**Look-ahead:** if usage materializes, Phase 2 (`make_relation`/`assert`/query-with-logic-variable as builtins routed to `tuplespace/`) becomes the natural next iteration. If not, Phase 1 stays a useful narrow feature and the broader reframe stays at design-doc stage.

**Reference:** `docs/superpowers/specs/2026-05-13-prolog-shaped-ffi.md` for the full 5-phase path, risk analysis, and decisions deferred to Phase 2+ schedulers.
