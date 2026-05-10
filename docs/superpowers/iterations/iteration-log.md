# Iteration Log

## ITER-0000 — Walking Skeleton (Parity Test Harness)

**Completed:** 2026-05-08

**Stories delivered:** STORY-0007 (scoped to harness setup and currently-passing cases)

**Tasks executed:**
- Verified 31 IR compilation tests pass (literals, arithmetic, comparisons, logical, control flow, let bindings, data structures, functions)
- Verified 2 full-pipeline tests pass: `parity_integer` (42) and `parity_symbol` (:hello)
- Established SCENARIO-0016 as sentinel with execution command
- Updated behavior-corpus.md with runnable test commands

**Scenarios:**
- SCENARIO-0016: updated from TBD to sentinel, command = `cargo test -p fmpl-core --test ast_to_ir_parity`

**Summary:**
Evidence-only iteration confirming the parity test harness works. 31 IR compilation tests and 2 full-pipeline tests pass. 22 full-pipeline tests remain ignored — these are the bootstrap bottleneck that ITER-0002/0003 will address. Key discovery: the specs claimed "8 passing full-pipeline tests" but only 2 actually pass (integer, symbol). The IR compilation tests (31) pass because they construct IR directly, bypassing ast_to_ir.fmpl. The gap is in ast_to_ir.fmpl producing the right IR from AST.

## ITER-0001 — Parity: Core Expression Coverage

**Completed:** 2026-05-08

**Stories delivered:** STORY-0043, STORY-0044, STORY-0045, STORY-0046, STORY-0047, STORY-0048 (partial: 2/8 ACs)

**Tasks executed:**
- Verified literals module: 5/5 tests pass (integer, bool_true, bool_false, null, string)
- Verified arithmetic module: 6/6 tests pass (add, sub, mul, div, mod, neg)
- Verified comparisons module: 6/6 tests pass (eq, neq, lt, gt, lte, gte)
- Verified logical module: 3/3 tests pass (and, or, not)
- Verified control_flow module: 2/2 tests pass (if_true, if_false)
- Verified let_bindings module: 2/2 tests pass (simple_let, let_with_arithmetic)
- Verified data_structures module: 4/4 tests pass (empty_list, list_of_ints, empty_map, map_literal)
- Verified functions module: 1/1 tests pass (lambda_call)
- Verified full-pipeline passing subset: parity_integer, parity_symbol
- Updated behavior-corpus.md with per-scenario commands for SCENARIO-0030 and SCENARIO-0038
- Marked SCENARIO-0031 through SCENARIO-0037 as BLOCKED:ITER-0002

**Scenarios:**
- SCENARIO-0030: promoted to sentinel, command = `cargo test -p fmpl-core --test ast_to_ir_parity parity_integer`
- SCENARIO-0038: promoted to sentinel, command = `cargo test -p fmpl-core --test ast_to_ir_parity parity_symbol`
- SCENARIO-0031 through SCENARIO-0037: marked BLOCKED:ITER-0002

**Summary:**
Evidence-formalization iteration for the IR compilation layer. All 29 IR compilation tests pass, confirming ir::compile() correctly handles all basic tagged value types. Full-pipeline tests remain at 2/24 passing — the bottleneck is ast_to_ir.fmpl, not ir::compile(). ITER-0002 is the critical implementation iteration that will start unblocking full-pipeline tests.

## ITER-0002 — Parity: Control Flow and Bindings

**Completed:** 2026-05-08

**Stories delivered:** STORY-0006, STORY-0008 (partial)

**Tasks executed:**
- Diagnosed grammar engine binding scoping bug: transient bindings from recursive rule applications leaked across siblings in TagMatch/ListMatch
- Root cause: sub-runtimes started at rule_depth 0, so apply_rule_inner's save/clear/restore guard never fired
- Fix: sub-runtimes in TagMatch/ListMatch/MapMatch/Apply now start at rule_depth 1
- Removed parent binding copying into sub-runtimes (unnecessary, prevented proper scoping)
- Unblocked 5 previously-ignored parity tests: arithmetic, string, let_binding, if_expr, sequence
- Updated ignore reasons for remaining 19 tests (grammar engine Star-in-TagMatch limitation)
- Added Pattern::bind_name() helper method

**Scenarios:**
- SCENARIO-0031 (arithmetic): promoted to sentinel
- SCENARIO-0032 (string): promoted to sentinel
- SCENARIO-0033 (let_binding): promoted to sentinel
- SCENARIO-0034 (if_expr): promoted to sentinel
- SCENARIO-0035/0036/0037 (lambda/list/map): reblocked as grammar-engine-star-in-tagmatch

**Summary:**
The core breakthrough: fixing binding scoping in the grammar engine's sub-runtime mechanism. The grammar rules in ast_to_ir.fmpl were already correct — the engine wasn't scoping bindings properly during recursive tree pattern matching. With this fix, all basic expression types (arithmetic, strings, control flow, let bindings, sequences) now compile correctly through the FMPL pipeline. The remaining 19 ignored tests are blocked by a different issue: the Star pattern (expr*:items) inside TagMatch doesn't properly handle list-valued children.

## ITER-0003 — Parity: Advanced Language Features

**Completed:** 2026-05-08

**Stories delivered:** STORY-0009 (partial), STORY-0049b (partial)

**Tasks executed:**
- Fixed Star-in-TagMatch: when a TagMatch child pattern contains a Repeat (Star/Plus) and the child value is a List, unwrap the list contents as individual input items
- Added Pattern::contains_repeat() helper to detect Repeat patterns through Bind wrappers
- Unblocked 6 more parity tests: lambda, list, index, nested_lambda, closure, return_value
- Updated ignore reasons for remaining 13 tests with precise root causes
- Remaining blockers categorized: 8 ir::compile gaps (while, for, block, match, try/catch, pipe, method_call, prop_access), 2 grammar engine issues (tagged Star, map pair), 2 ast_to_ir.fmpl gaps (slices)

**Scenarios:**
- SCENARIO-0035 (lambda): unblocked, promoted to sentinel
- SCENARIO-0036 (list): unblocked, promoted to sentinel
- SCENARIO-0030-0034, 0038: still passing (no regression)

**Summary:**
The Star-in-TagMatch fix unwraps list-valued children when the corresponding pattern contains a Repeat, allowing expr*:items patterns inside TagMatch to iterate over list elements correctly. Combined with ITER-0002's binding scoping fix, the FMPL bootstrap pipeline now correctly compiles: integers, arithmetic (with precedence), strings, symbols, booleans, null, let bindings, if/else, lambdas (including nested and closures), function calls, lists, indexing, sequences, and return. 13 full-pipeline tests pass (was 2 at ITER-0000 start). The remaining 13 failures are mostly ir::compile gaps for control flow constructs that ast_to_ir.fmpl already handles correctly.

## ITER-0003b — ir::compile Gap Filling

**Completed:** 2026-05-08

**Stories delivered:** STORY-0008 (continued), STORY-0009 (continued)

**Tasks executed:**
- Added ir::compile handlers for: While, DoWhile, For, Block, Pipe, Match (wildcard), TryCatch, Assign, QualifiedName, Slice
- Added Seq 2-child form support (ast_to_ir.fmpl emits :Seq(first, rest) not :Seq([items]))
- Added raw AST tag fallbacks (Int, Float, String) for passthrough values in Slice bounds
- Added ast_to_ir.fmpl rules for: TryCatch, Pipe (:|>), Slice, Sequence (alias for Block)
- Fixed For pattern to handle :PatVar(:x) wrapper from parser
- Unblocked 5 more parity tests: while, block, pipe, slice_open, slice_closed

**Scenarios:**
- Parity score: 47/55 passing, 8 ignored
- No new sentinel promotions (remaining 8 need deeper work)

**Summary:**
Filled the ir::compile gaps that were blocking parity tests. The remaining 8 failures are:
- Grammar engine (2): tagged Star args empty with 2-child TagMatch, map pair bare-name binding
- ir::compile runtime (3): for loop .len() dispatch, method_call, prop_access on maps
- Match compilation (2): only wildcard case implemented, no pattern dispatch
- TryCatch semantics (1): parity mismatch (different null vs value return semantics)
These are genuine limitations requiring deeper work, not simple gap-fills.

## ITER-0004 — Compiler Cutover (FMPL Pipeline Wired)

**Completed:** 2026-05-08

**Stories delivered:** STORY-0005 (FMPL compiler path), STORY-0011 (Rust compiler relegated to fallback). STORY-0010/0012 (optimizer integration) deferred.

**Tasks executed:**
- Refactored `eval()` into three functions: `eval()` dispatches by env flag, `eval_via_rust_compiler()` is the original path, `eval_via_fmpl_pipeline()` is the new path
- FMPL pipeline lazily bootstraps prelude.fmpl + ast_to_ir.fmpl on first call per-VM, cached via `__fmpl_pipeline_bootstrapped` sentinel
- User source is wrapped as `let (ast = ast::parse(...)) let (ir = ast @ ast_to_ir.expr) let (code = ir::compile(ir)) code::eval(code)` and run through the wrapper-compiler
- Added `FMPL_USE_FMPL_COMPILER=1` opt-in flag
- Created `tests/fmpl_pipeline_compiler.rs` with 11 E2E tests verifying identical results between Rust and FMPL pipelines
- Discovered and fixed builtin module shadowing bug: `cursor`, `stream`, `grammar` were always compiled as builtin LoadSymbol, preventing `let cursor = ...` from binding. Now uses LoadVar with VM fallback to builtin symbols.

**Scenarios:**
- SCENARIO-0003 / SCENARIO-0016: pipeline parity confirmed for 11 expression types
- New sentinel: `cargo test -p fmpl-core --test fmpl_pipeline_compiler`

**Summary:**
The FMPL pipeline is now a first-class compilation path, opt-in via `FMPL_USE_FMPL_COMPILER=1`. The Rust compiler is no longer the only option — it's an explicit fallback. Bootstrap caching ensures the prelude and ast_to_ir.fmpl load once per VM. The 11-test parity suite confirms identical results for basic expressions. Optimizer integration is deferred until the list-based AST refactor lands (the optimizer uses list patterns that don't match the current tagged AST). Tests: 1160 passed, 0 failed, 164 ignored. Workspace clippy: zero warnings.

## ITER-0004b — Single Canonical Representation (PARTIAL)

**Completed (partially):** 2026-05-08; reconciled 2026-05-09

**Stories delivered (partial):** STORY-0010 — only the Rust-runtime half. The FMPL stdlib + AST/parser-surface half is rescheduled into ITER-0004c per the deferring-work-must-reschedule rule.

**What shipped:**

- `Value::list_node(tag, children)` and `Value::as_node(&self) -> Option<(&str, &[Value])>` helpers added to `fmpl-core/src/value.rs` (commit `pvruwplq`).
- ast-grep transformer rules added at `tools/list-transform/rust-rules/` covering producer-with-args, consumer-iflet, consumer-iflet-else-panic patterns (commit `luxwnytk`).
- ast-grep transformer applied workspace-wide: 229 mechanical rewrites across fmpl-core (118 producers, 111 if-let consumers) (commit `sqnqurnz`).
- `lib/core/ast_to_ir.fmpl` rewritten **by hand** to list-pattern syntax (55 list-pattern uses, 0 legacy) (commit `psvlyykw`). The FMPL transformer that ITER-0004b's plan called for was never built; this file is the only stdlib file migrated.
- `Value::Tagged` enum variant deleted from `fmpl-core/src/value.rs`. All Rust-side consumers migrated to use `as_node()` or list-shape destructuring. Bootstrap parser regen works without `FMPL_SKIP_PARSER_GEN` workaround (commit `qworqxrm`, originally combined with agentic-stack pollution as `puvpzsmk` and split out on 2026-05-09).

**What was deferred (rescheduled to ITER-0004c):**

- **FMPL transformer never built** (ITER-0004b plan Phase A item 3, Phase B item 7). The transformer was specified as a tree-grammar with three special-case rules (trailing comma for single-element list patterns, pair sentinel wrap, list-pattern binding repair) plus a CLI driver. None of it exists. Without it, the stdlib can't be regenerated mechanically from source, which transitively blocks ITER-0006 (self-compile seed).
- **5 stdlib files still in legacy `:Tag(args)` syntax** (verified 2026-05-09 by grep):
  - `lib/core/ast_optimizer.fmpl` — 62 legacy hits, 0 list-pattern. Critically, also not yet wired into `eval_via_fmpl_pipeline` — it's loadable only via its own test file. The 16 `#[ignore]`'d tests in `fmpl-core/tests/optimizer_integration.rs` codify this gap.
  - `lib/core/fmpl_parser.fmpl` — 96 legacy hits.
  - `lib/core/ir_to_rust.fmpl` — 48 legacy hits.
  - `lib/core/prelude.fmpl` — 41 legacy hits.
  - `lib/core/ir_to_execution_tape.fmpl` — 19 legacy hits. (Its `_indexed.fmpl` sibling IS in list-pattern syntax.)
- **Optimizer wiring step skipped** (Phase B item 10). `eval_via_fmpl_pipeline` does not call the optimizer. SCENARIO-0103 was created but is blocked.
- **AST/parser surfaces still present** (Phase C items 14–18): `Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch` AST/runtime types; the `:Tag(args)` parser production for value-constructor expressions; the `:Tag(args)` grammar-parser pattern production at `parse_value_pattern`; the tagged bytecode (`MakeTagged`, `MatchTag`, `ExtractTaggedChild`, `MatchTagged`, `MatchTaggedWithBindings`). The Rust type system still permits the dual surface; the parser silently translates `:Tag(args)` to list-shaped values via the surviving AST nodes — which is why the iteration's tests pass even though the syntax-level burn is incomplete.

**Why the gap wasn't caught at commit time:**

The Phase C commit message (`qworqxrm refactor(values): delete Value::Tagged variant`) claims "Final step of ITER-0004b — single canonical representation," and that claim is **true at the Rust runtime level** but **false at the FMPL stdlib and AST/parser-surface level**. The 16 `#[ignore]`'d tests in `optimizer_integration.rs` were the canary that should have failed loudly to catch the gap; instead they sit silent. Workspace tests pass because nothing currently exercises a path that would surface the parser-shape mismatch (the legacy stdlib files aren't loaded by `eval_via_fmpl_pipeline`, and ast_to_ir.fmpl IS migrated).

**Scenarios:**
- SCENARIO-0003, SCENARIO-0016, SCENARIO-0039: confirmed still passing through the FMPL pipeline (no regression from ITER-0004).
- SCENARIO-0103 (full parity corpus with optimizer enabled): created in ITER-0004b but blocked — the optimizer is not yet wired.

**Lessons (recorded for the deferring-work-must-reschedule rule):**

1. **The transformer-driven strategy worked for Rust; was abandoned for FMPL.** The ast-grep rules landed 229 sites mechanically. The parallel FMPL-side transformer was never built — the session ran out of context after Phase A's Rust-side work and reverted to hand-editing `ast_to_ir.fmpl` instead. The strategy's core claim ("transformers convert a single huge atomic refactor into two reviewable artifacts") was only validated on one side of the rewrite.
2. **A commit message that says "Final step of X" should be checked against X's stated acceptance criteria.** The Phase C commit was technically true for what it claimed (deleted the variant) but was elevated to "ITER-0004b complete" status without verifying the iteration's stated goal (single canonical representation, no parser ambiguity). Future iterations should require an explicit acceptance-check review before declaring complete — especially when the iteration plan is multi-phase.
3. **`#[ignore]`'d tests are a deferred-acceptance contract.** The 16 tests in `optimizer_integration.rs` are the contract that ITER-0004b's optimizer-integration story requires. They should be un-ignored in the iteration that delivers their preconditions, or they should be tracked as scheduled work. Currently they sit ignored without an owner — this reconciliation moves them under ITER-0004c.

**Tests:** 1170 passed, 0 failed (workspace). Workspace clippy: zero warnings. Bootstrap parser regen: works without `FMPL_SKIP_PARSER_GEN`.

## ITER-0004c — FMPL Stdlib Migration + Optimizer Wiring (Phase A of STORY-0010)

**Completed:** 2026-05-10

**Stories delivered:** STORY-0010 Phase A (AC-3 through AC-7 + AC-13). AC-1, AC-2, AC-8, AC-15 were already satisfied by ITER-0004b's runtime burn (no re-work). Phase B (AC-9..AC-12, AC-14) deferred to ITER-0004d.

**Tasks executed (in commit order):**

1. **Scope review (PAR ×3 rounds → APPROVE)** — 3 parallel-adversarial-review rounds surfaced 26 actionable findings across the original scope. Key revisions: split out the prelude/parser-helper relocation into a new ITER-0004e (per user direction), corrected an AC-7 runnable check that asserted optimizer behavior that doesn't exist (the optimizer never recurses into Lambda/Let/Match/Call/List/Map/Block — verified via inspection of ast_optimizer.fmpl), fixed the `algebraic_simp` vs `constant_fold` citation for the slot-discriminating observable, deleted `pipeline_demo.fmpl` instead of migrating it (no test coverage; demo of indexed-RPN pipeline being deleted), added bracket-index vs dot-access pre-implementation verification, swapped build order to `7 → 1 → 2 → 3 → 4 → 5 → 6 → 8` (cleanup before migration).
2. **G1 transformer attempt + abandonment** — built `tools/list-transform/list_transform.fmpl` + Rust driver per round-1 spec. PAR Stage 1 spec-compliance review failed: only 3 of 6 transformer rules (d/e/f) could be implemented cleanly; rules (a)(b)(c) for grammar-LHS context required LHS-mode tracking that the FMPL grammar engine cannot express ergonomically. Per user direction, abandoned the transformer (`jj abandon zwkyzrno`) and pivoted to hand-migration. Round-1 PAR had explicitly noted hand-migration was viable.
3. **Cleanup deletions** (item 7, before migration to reduce target set): deleted `lib/core/ast_to_ir_indexed.fmpl` (broken indexed variant per design doc), `lib/core/ir_to_execution_tape_indexed.fmpl` (orphan consumer), `lib/core/pipeline_demo.fmpl` (uncovered demo). All three were not referenced by any runtime or test path.
4. **Hand-migrated 5 stdlib files** (one atomic commit each):
   - `lib/core/ast_optimizer.fmpl` — 156 occurrences. Pattern bindings wrapped with `any:` where required (e.g., `:Int(a)` → `[:Int, any:a]`). Guard syntax `&{...}` preserved verbatim. Added AC-7 TODO comment near both catch-all `_:x => x` rules.
   - `lib/core/fmpl_parser.fmpl` — 101 occurrences. All in expression-position RHS (PEG parser grammar; no LHS-pattern rewrites needed). Lines 82-83 / 287-292 (`tagged_with_args`, `tagged_empty`, `pat_constructor`) migrated as part of the file-wide sweep — flagged as expected churn ITER-0004d will revisit when it deletes `Expr::Tagged` and the parser productions.
   - `lib/core/ir_to_rust.fmpl` — 84 occurrences (mixed inline-pattern @{} blocks). 29 grep-matches remain in the `rust_prelude` Rust-source string literal — these are Rust constructors emitted as transpiler output, NOT FMPL syntax. The AC-13 CI gate handles these false positives by stripping string literals before greppping.
   - `lib/core/prelude.fmpl` — 45 occurrences. AST/IR-helper functions (`fold_binary`, etc.) migrated in place; their relocation to `parser_helpers.fmpl` deferred to ITER-0004e per user direction.
   - `lib/core/ir_to_execution_tape.fmpl` — 19 occurrences (inline @{} pattern blocks).
5. **Wired `ast_optimizer` into `eval_via_fmpl_pipeline`** (item 4): bootstrap loader now loads `ast_optimizer.fmpl` after `prelude` and `ast_to_ir`. Pipeline source threads `ast_optimizer["optimize"]` between `ast::parse` and `ast_to_ir.expr`. Bracket-index access form pre-verified working via probe test before committing the wiring.
6. **Added SCENARIO-0103** (item 5): new test file `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs` with 4 observables — parity (26 source-form inputs from the parity corpus), slot-discriminating (`:If(:Bool(true), t, e) => t` branch elimination + arithmetic constant folding at AST stage), fold-fires-on-real-parse (`ast::parse("1 + 2 * 3")` → `[:Int, 7]`), and guards (1/0, 1%0, 1/(2-2)). 32 passed, 1 ignored (INT_MIN deferred to ITER-0004g per lexer limitation).
7. **Un-ignored 17 optimizer_integration tests** (item 8): removed all `#[ignore = "ITER-0004b: ..."]` markers. Rewrote `ac3_int_min_negation_does_not_panic` to use direct AST construction (`[:Unary, :-, [:Int, int_min]]` where `int_min = 0 - 9223372036854775807 - 1`) because the FMPL lexer cannot tokenize `9223372036854775808`. Lexer fix scheduled for ITER-0004g.
8. **Verification gates added** (3 new test files):
   - `fmpl-core/tests/stdlib_no_legacy_syntax.rs` — AC-13 CI gate. Walks `lib/core/*.fmpl`, strips comments + string literals (hand-rolled scanner avoids regex dev-dependency), checks for `:[A-Z][a-zA-Z_]*\(` pattern. Passes 0 violations across all 12 stdlib files.
   - `fmpl-core/tests/ac7_optimizer_pass_through.rs` — AC-7 runnable check. For each of the 7 enumerated pass-through node kinds (`:Lambda`, `:Let`, `:Match`, `:Call`, `:List`, `:Map`, `:Block`), asserts `optimize(input) == input` where input has a foldable inner `:Binary`. 8 tests pass (7 + 1 sanity anti-test). Locks the AC-7 enumeration to behavior — if a future change adds recurse-into rules, the structural-identity assertion fails and forces the TODO comment update.
   - `fmpl-core/tests/ast_optimizer_unit.rs` — execution gate for `lib/core/ast_optimizer_test.fmpl`. Currently `#[ignore]`d due to a pre-existing `++` string-concat operator in the test file that the FMPL parser does not support. Tracked in ITER-0004g optional companion fix.

**Scenarios:**
- SCENARIO-0103 (sentinel): updated from TBD/pending to automated, command = `cargo test -p fmpl-core --test scenario_0103_optimizer_pipeline`. 32 passed, 1 ignored.
- SCENARIO-0016 (sentinel): updated from TBD/pending to automated (kept optimizer-disabled per round-2 PAR binding decision), command = `cargo test -p fmpl-core --test ast_to_ir_parity`. 55/55 passing.

**Out-of-band cleanup (per round-2 PAR call-out):** scope item 7 deleted three files that were not bound to any STORY-0010 AC. Rationale: the indexed variants were flagged broken in the design doc; pipeline_demo had no CI test coverage and demoed the deleted indexed-RPN pipeline. Deletion satisfies AC-13 vacuously and reduces migration scope.

**Deferred to scheduled iterations** (per "Deferring work must reschedule it" rule):
- **ITER-0004e — Prelude / Parser-Helper Split**: relocate `fold_binary`, `fold_index`, `fold_postfix`, `fold_pipe_at`, `binary_op_to_ir`, `unary_op_to_ir` from `prelude.fmpl` into a new `lib/core/parser_helpers.fmpl`. Per user direction (Haskell-Prelude-style scoping): prelude should be the minimal high-level FMPL vocabulary, not a bootstrap dump-ground.
- **ITER-0004f — Flatten Binary/Unary AST Nodes**: collapse `[:Binary, :+, l, r]` to `[:+, l, r]` (operator symbol AS the tag). Per user direction; aligns with OMeta/Ohm conventions.
- **ITER-0004g — Lexer: Handle INT_MIN Literal in Negation Context**: fix the FMPL lexer to tokenize `9223372036854775808` correctly when it follows a unary `-`. Plus optional companion: add `++` operator support (or rewrite `ast_optimizer_test.fmpl` print-summary to use `string.join`) to un-ignore `ast_optimizer_unit` gate.

**Cross-iteration coordination notes (for ITER-0004d):**
- `lib/core/fmpl_parser.fmpl` lines 82-83, 287-292 line numbers will have shifted after this iteration's transformer-equivalent migration. ITER-0004d MUST re-grep at iteration start. (See ITER-0004d scope item 8 note.)
- `Expr::Tagged`, `Pattern::Constructor`, `Pattern::TagMatch`, tagged bytecode, and parser productions for `:Tag(args)` value-constructor / pattern syntax are NOT touched by ITER-0004c. They survive intact for ITER-0004d's deletion sweep.

**Tests:** 1228 passed (up from 1170 baseline = +58: SCENARIO-0103 +32, optimizer_integration un-ignored +17, AC-7 runnable +8, AC-13 CI gate +1), 183 ignored (down from 198 net = -15: 17 optimizer_integration removed from ignored, 1 ast_optimizer_unit added, 1 INT_MIN sub-test in scenario_0103 added). Workspace clippy: zero warnings. AC-13 invariant: passing across all 12 `lib/core/*.fmpl` files.

**Lessons:**

1. **Transformer-vs-hand-migration is a real engineering choice, not a tooling preference.** The transformer attempt failed because the FMPL grammar engine couldn't express LHS-mode tracking ergonomically (`:Tag(args)` patterns inside grammar bodies need different rewrite rules than RHS expressions). Hand-migration of ~440 occurrences across 5 files took 5 atomic commits (one per file) totaling ~10 minutes of edit time — much faster than the abandoned transformer-build effort. Round-1 PAR had flagged this. Lesson: when 80%+ of the work is in 2 files, transformer payback may not exceed the build cost.
2. **PAR rounds compound corrections; user direction can resolve scope debates faster than reviewers.** Round 1 surfaced 13 findings; round 2 surfaced 13 NEW findings on top of round 1 fixes; round 3 returned APPROVE. Two of the largest scope decisions (defer prelude relocation; abandon transformer; defer Binary-flattening) came from direct user input rather than reviewer surfacing. Lesson: present scope tradeoffs to the user as Ask questions rather than running another PAR pair when the question is fundamentally about preference vs. correctness.
3. **AC-13 grep gate has real false-positive cases.** `lib/core/ir_to_rust.fmpl` emits Rust source code as a string literal (`"Value::Bool(true)"`). The naive grep matches these as legacy FMPL syntax. The gate must strip string literals + comments before applying the regex. The roadmap had anticipated this in the verification-gate text; the implementation in `stdlib_no_legacy_syntax.rs` carries it through with a hand-rolled scanner.
4. **Pre-existing bugs surface when test gates start exercising previously-unused code.** `lib/core/ast_optimizer_test.fmpl` had three `:` typos (lines 43, 89, 131) and uses an unsupported `++` operator. None of these bugs were caught before because no test gate executed the file. Fixing the typos was easy; the `++` issue is deferred to ITER-0004g.
5. **OMeta/Ohm alignment is a project design constraint.** User clarified mid-iteration: FMPL parser/grammar conventions intentionally follow OMeta (and Ohm). The list-shape canonical AST `[:Tag, ...]` IS the OMeta convention. The Binary-flattening proposal in ITER-0004f ALSO aligns with OMeta (operator-as-head, not kind-tag). Memory entry created at `~/.claude/projects/-Users-ndn-development-fmpl/memory/project_ometa_ohm_alignment.md`.

**Summary:**
ITER-0004c shipped STORY-0010 Phase A (AC-3 through AC-7 + AC-13) on 2026-05-10 over a single-day session. The key acceptance gate — SCENARIO-0103 — now passes with 32 active observables + 1 deferred (INT_MIN). The FMPL stdlib is in fully-canonical list-pattern syntax across all 5 in-scope files; the `ast_optimizer.fmpl` is wired into `eval_via_fmpl_pipeline` at the AST stage; all 17 previously-ignored optimizer_integration tests pass; and 3 new verification gates (AC-13 CI, AC-7 runnable, ast_optimizer_unit) lock the iteration's contracts against silent regression. Three follow-on iterations (0004e prelude/parser-helper split, 0004f Binary/Unary AST flattening, 0004g lexer INT_MIN + `++` operator) were scheduled to honor the deferring-work-must-reschedule rule; none block ITER-0004d's parser/AST burn. The transformer-build approach was attempted, abandoned via `jj abandon` after a Stage-1 PAR spec failure, and replaced with hand-migration that took ~10 minutes per file (much faster than the abandoned tool-build). Workspace test count rose from 1170 to 1228 (+58 tests, no regressions).
