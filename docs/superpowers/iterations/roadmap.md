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

**Already-satisfied AC verification (no re-work needed):**
- AC-1 (`ast::parse` emits list-shaped exclusively): verified at `fmpl-core/src/builtins/ast.rs` — every `expr_to_value` arm returns `Value::list_node(...)`.
- AC-2 (`ir::compile` consumes list-shaped exclusively): verified at `fmpl-core/src/builtins/ir.rs` — `compile_node` dispatches on `Value::as_node()` only.
- AC-8 (`Value::Tagged` enum variant removed): verified — the variant is deleted; `grep -n 'Value::Tagged' fmpl-core/src/value.rs` returns nothing.
- AC-15 (full test suite passes; no `Value::Tagged` source matches): verified at workspace baseline — 1170 passing, no `Value::Tagged` source matches.
**Rationale:** ITER-0004b shipped only the Rust-runtime half of the canonical-representation refactor. The 7 FMPL stdlib files (six listed in the original ITER-0004b plan + `ast_to_ir_indexed.fmpl`, missed in the original list) still use legacy `:Tag(args)` syntax. This iteration: (1) builds the FMPL transformer ITER-0004b's plan called for, (2) applies it to all 7 stdlib files, (3) wires `ast_optimizer.fmpl` into `eval_via_fmpl_pipeline` so the parity corpus actually exercises the optimizer. Acceptance gate is SCENARIO-0103 passing — every parity input matches Rust-compiler output AND at least one demonstrably folds AND no INT_MIN/div-zero panics. The dual-syntax parser surface (`Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch`, tagged bytecode) survives this iteration unchanged — it's permitted but no longer used by the stdlib. ITER-0004d removes it.
**Status:** pending
**Impacted scenarios:** SCENARIO-0103 (sentinel — completes here), SCENARIO-0016 (sentinel — must continue passing with optimizer wired into pipeline). SCENARIO-0003 and SCENARIO-0039 are ITER-0004d concerns (scenario rewrite + reconfirm).
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

### ITER-0004d — Parser/AST/Bytecode Burn (Phase B of STORY-0010)

**Stories:** STORY-0010 Phase B (AC-9, AC-10, AC-11, AC-12, AC-14 — the AST/parser/bytecode deletions plus the Rust-test source-string sweep). AC-1, AC-2, AC-8, and AC-15 already satisfied by ITER-0004b. AC-3 through AC-7 and AC-13 (stdlib greppable invariant) are Phase A (ITER-0004c). ITER-0004d's primary observables are the new parse-rejection scenarios SCENARIO-0104/0105 plus a `fmpl-core/src/`-greppable-invariant scenario SCENARIO-0106 (Rust-side `Expr::Tagged`/`Pattern::Constructor`/`Pattern::TagMatch` absence).
**Rationale:** With the stdlib in canonical list-pattern syntax (ITER-0004c), the parser can stop accepting `:Tag(args)` value-constructor and pattern syntax. This iteration deletes the surviving AST/parser/bytecode surfaces and sweeps the Rust test corpus that still feeds `:Tag(args)` strings into `eval()`. After this iteration there is genuinely one shape and one syntax — no silent fallback path.
**Status:** pending
**Impacted scenarios:** SCENARIO-0104 NEW (parse-rejection of `:Tag(args)` value-construction), SCENARIO-0105 NEW (parse-rejection of `:Tag(p)` pattern syntax), SCENARIO-0106 NEW (greppable-invariant: stdlib + `fmpl-core/src/` clean). SCENARIO-0039 must be rewritten to list-pattern syntax (or owning stories deferred). SCENARIO-0066 hygiene update — references `Value::Tagged` which no longer exists. SCENARIO-0003 reconfirms with the post-burn parser.
**Depends on:** ITER-0004c.
**Look-ahead check:** Unblocks ITER-0006 (self-compile seed) — the seed compiles `fmpl_parser.fmpl + ast_to_ir.fmpl + ast_optimizer.fmpl` through a pipeline that has exactly one AST shape with no parser ambiguity, so the seed is reproducible.

**Scope:**

1. **Sweep FMPL source strings inside Rust test files** to list-pattern syntax. Targets identified by PAR review (~50 files); enumerate exhaustively with `grep -rn ':[A-Z][a-zA-Z_]*(' fmpl-core/tests/`. Hot files: `tests/parser_equivalence.rs:82-85`, `tests/tagged_values.rs:8,32,45,58,70,82,96`, `tests/tagged_pattern_match.rs:20-100`, `tests/fmpl_interpreter.rs:36-69`, `tests/ast_to_ir_parity.rs:88-122`. Use sed/ast-grep where mechanical; hand-edit otherwise.
2. **Update internal grammar production at `fmpl-core/src/grammar/parser.rs:2072`** — current `expr = :Tagged(tag, expr*:args) => :MakeTagged(tag, args)`. Either rewrite to list-pattern syntax with `:MakeListNode` opcode, or delete entirely if the production is dead post-migration. Decide and document.
3. **Make the rename-vs-delete decision for tagged bytecode.** Two paths for AC-11: (a) DELETE — remove `Instruction::MakeTagged`, `MatchTag`, `ExtractTaggedChild`, `MatchTagged`, `MatchTaggedWithBindings` entirely; or (b) RENAME — rename surviving instructions to `MakeListNode`, `MatchListNode`, etc. to reflect list-shape semantics. Recommendation: DELETE if no remaining IR pattern emits these (which after ITER-0004c should be the case); RENAME only if the IR pattern landscape requires preserving an opcode. Make the decision at iteration start and document it as a binding precondition.
4. **Delete `Expr::Tagged`** AST variant. Update `fmpl-core/src/value_to_ast.rs:358` (constructor arm), `fmpl-core/src/builtins/ir_to_rust.rs:1440,1873`, `fmpl-core/src/repr.rs:101`, and any other producer/consumer site enumerated by `grep -rn 'Expr::Tagged' fmpl-core/src/` at iteration start. PAR review estimate: ~25 deletion sites total across `parser.rs`, `grammar/parser.rs`, `compiler.rs`, `value_to_ast.rs`, `builtins/ir_to_rust.rs`, `repr.rs`, `pattern/mod.rs`, `grammar/runtime.rs`, `grammar/trampoline.rs`, `grammar/optimizer.rs`, `builtins/grammar_to_ir.rs`, `builtins/ast.rs`. Re-grep at iteration start to enumerate authoritatively.
5. **Delete `Pattern::Constructor`** and `Pattern::TagMatch` runtime/trampoline handlers. Update `fmpl-core/src/value_to_ast.rs:1241`, `fmpl-core/src/repr.rs:225`, `fmpl-core/src/pattern/mod.rs`, `fmpl-core/src/grammar/runtime.rs:794`, `fmpl-core/src/grammar/trampoline.rs`, etc. Re-grep at iteration start.
6. **Delete the parser productions** for `:Tag(args)` value-construction expressions and `:Tag(p1, p2)` patterns at `fmpl-core/src/grammar/parser.rs::parse_value_pattern` and the corresponding expression production. Bare `:foo` symbol literals (`Expr::Symbol`) remain — only the parenthesized-arguments form is deleted.
7. **Update `lib/core/ast_to_ir.fmpl:21`** rule `[:Tagged, any:tag, exprs:xs] => [:MakeTagged, tag, xs]` becomes dead after AC-9 (no `Expr::Tagged` produced) and AC-11 (no `MakeTagged` instruction). Either delete the rule or rewrite to whatever the rename decision in scope item 3 demands.
8. **Update `lib/core/fmpl_parser.fmpl`** lines 82-83 and 287-292 (`=> :Tagged(tag, items)`, `=> :PatternTagged(tag, pats)`) — these RHS expressions emit AST node shapes that downstream `value_to_ast.rs` decodes. After ITER-0004c the syntax is `[:Tagged, tag, items]` / `[:PatternTagged, tag, pats]`, but the underlying AST shape is the same `Tagged`/`PatternTagged` node. After the AC-9 deletion of `Expr::Tagged`, either the FMPL parser stops emitting `Tagged`/`PatternTagged` AST nodes, OR the decoder keeps handling them. Decide which and update both ends together to avoid an asymmetric coherence gap.
9. **Reconcile scenarios:** rewrite SCENARIO-0039 to list-pattern syntax (or defer the owning stories STORY-0057/0054/0053 if SCENARIO-0039 is no longer authoritative). Update SCENARIO-0066 to reflect post-burn `Value` shape. Add SCENARIO-0104, SCENARIO-0105, SCENARIO-0106.
10. **Verification:**
    - `grep -rnE 'Expr::Tagged|Pattern::Constructor|Pattern::TagMatch' fmpl-core/src/` returns no matches (AC-9, AC-10, AC-12).
    - `grep -rnE ':[A-Z][a-zA-Z_]*\(' fmpl-core/tests/` returns no matches in FMPL source string positions (AC-14 — Rust-test source-string sweep).
    - AC-13 invariant established by ITER-0004c remains satisfied: `grep -cE ':[A-Z][a-zA-Z_]*\(' lib/core/*.fmpl` returns 0 (sanity check; not new work for ITER-0004d).
    - SCENARIO-0104, SCENARIO-0105 fail-fast on `:Tag(args)` input.
    - SCENARIO-0106 (new): `grep -rnE 'Expr::Tagged|Pattern::Constructor|Pattern::TagMatch' fmpl-core/src/` returns no matches.
    - Full workspace test suite passes.
    - SCENARIO-0103 still passes.

**Out of scope:** Removing `FMPL_USE_FMPL_COMPILER` opt-in. Cleanup of any dead-tagged residue inside the bootstrap parser that is gated only by tooling around it.

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
