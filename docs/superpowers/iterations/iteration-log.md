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

## ITER-0004d.0 — FMPL-Source-Grep Tooling Precursor

**Completed:** 2026-05-10

**Stories delivered:** none (this iteration commits no STORY-0010 ACs; it is a bottom-up tooling carve-out from ITER-0004d after 3 PAR review rounds discovered the original monolithic plan was too speculative). The tool ships baseline ground-truth numbers that replace 3 rounds of inferred-from-roadmap-text counting for the AC-13/AC-14 sweep ITER-0004d.1 will run.

**Tasks executed (in commit order):**

1. **Pre-iteration scope review (PAR ×1 → REVISE, user opted to build as-spec'd)** — both reviewers flagged 3 agreed serious issues: (a) `syn` library-vs-dev-dep contradiction (a function in `fmpl-core/src/diagnostics/mod.rs` cannot `use syn` if `syn` is `[dev-dependencies]`), (b) baseline JSON granularity unspecified, (c) no scenario card for the tool. Reviewer B additionally caught a structural correctness issue: the spec's "Symbol(s) immediately followed by LParen" detector produces false positives on grammar-DSL binding patterns (`unary:first (mult_op unary)*:rest` — 5 sites in `lib/core/fmpl_parser.fmpl`, 1 in `tests/fmpl/fmpl_grammar.fmpl`). User direction: "build as-spec'd; surface real data" — proceed with the implementation, applying minimum-viable resolutions to the contradictions, and let the tool's actual first-run baseline replace speculation.
2. **Binding resolutions made by orchestrator before implementer dispatch:**
   - **`syn` placement:** `scan_fmpl_source` stays in `fmpl-core/src/diagnostics/mod.rs` (no `syn`). `scan_rust_strings` moves to test-only `fmpl-core/tests/common/rust_string_scanner.rs` (where `[dev-dependencies]` `syn` is accessible). Runtime crate remains `syn`-free.
   - **Grammar-DSL false positives:** use the spec's own allowlist mechanism (roadmap line 360) to cover the 6 known sites. Allowlist key: `(path-suffix, tag)`. Pre-populated with two entries: `("lib/core/fmpl_parser.fmpl", "first")` covers all 5 lib/core sites; `("fmpl-core/tests/fmpl/fmpl_grammar.fmpl", "first")` covers the 1 fixture site.
   - **Self-scanning gate fixtures:** exclude `diagnostics_fmpl_source_scan.rs` and `no_legacy_fmpl_syntax.rs` from the `fmpl-core/tests/*.rs` scan surface.
3. **Implementer dispatch + TDD discipline (4 atomic commits):**
   - `vkupoxzt` — `fmpl-core/src/diagnostics/mod.rs` (NEW: `SourceKind`, `TaggedSyntaxHit`, `DiagnosticsError`, `scan_fmpl_source`) + `lib.rs` registers `pub mod diagnostics;`. Runtime crate verified `syn`-free.
   - `lrpkkwzl` — `fmpl-core/Cargo.toml` (`syn = { version = "2", features = ["full", "extra-traits", "visit"] }` to `[dev-dependencies]`) + `fmpl-core/tests/common/{mod.rs, rust_string_scanner.rs}` (`syn::visit::Visit` walker) + `fmpl-core/tests/diagnostics_fmpl_source_scan.rs` (11 unit tests, all 8 spec cases + 3 extras).
   - `psqpzpus` — `fmpl-core/tests/no_legacy_fmpl_syntax.rs` CI gate (4-surface walker, allowlist filter, baseline JSON compare with `FMPL_REGEN_BASELINE=1` regen) + `fmpl-core/tests/no_legacy_fmpl_syntax.baseline.json` (committed baseline).
   - `nyxkknnu` — deleted `fmpl-core/tests/stdlib_no_legacy_syntax.rs` (the new gate's `lib/core/` surface subsumes it).
4. **Post-implementation PAR (spec + quality) → REVISE, applied 3 targeted fixes in `a4752461`:**
   - `rust_byte_offset: usize` → `Option<usize>`; helper emits `None` rather than `0` placeholder so future consumers can distinguish "exact location unavailable" from "offset 0". (PAR A serious.)
   - `scan_fmpl_source` filters to identifier-style tags only; `:+`, `:==`, etc. excluded (they were never legacy tagged-constructor syntax). (PAR B serious.)
   - Added 2 unit tests: documented-invariant ("FMPL string literals don't produce false hits" — load-bearing for `scan_rust_strings`) and operator-symbol-exclusion. (PAR B finding.) Test count: 11 → 13.

**Scenarios:** none added (the iteration's spec intentionally defers scenario coverage to ITER-0004d.1's SCENARIO-0106). The CI gate `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` is not a behavior scenario but an iteration-internal regression sentinel; ITER-0004d.1 promotes the gate's `== 0` form to a permanent CI sentinel pinning AC-13 + AC-14.

**Baseline ground truth (`fmpl-core/tests/no_legacy_fmpl_syntax.baseline.json` committed):**
```json
{ "lib/core": 0, "src/rs": 43, "tests/fmpl": 72, "tests/rs": 625 }
```
- `lib/core = 0` post-allowlist: AC-13 invariant from ITER-0004c is preserved at the lexer-token-level (stricter than the old gate's hand-rolled uppercase-only regex).
- `tests/fmpl = 72`: dominated by the two orphan fixtures `ast_to_ir.fmpl` + `fmpl_parser.fmpl` (ITER-0004d.1 deletes these per its scope item 5).
- `tests/rs = 625` and `src/rs = 43`: target volume for ITER-0004d.1's sweep. The roadmap's round-2 "14 files" estimate referred to file count not hit count; the 625 hits are concentrated in fewer files.

**Known limitations carried forward to ITER-0004d.1 planning** (PAR findings not addressed in this iteration; documented here so d.1's planning can address them with real baseline data):
- **Macro-body coverage gap (B-S1):** `syn::visit::Visit` does not descend into macro `TokenStream` bodies. Common Rust idioms like `eval!(":Foo(1, 2)")` or `assert_eq!(eval(":Foo(1)"), ...)` are invisible to the scanner. Today's codebase doesn't use this pattern with legacy syntax (verified via grep), but d.1's `== 0` flip should add a `visit_macro` override that re-parses tokens for embedded `LitStr` OR document the gap as acceptable risk.
- **Tests-walk vs src-walk asymmetry (A-S2):** `fmpl-core/src/` is walked recursively; `fmpl-core/tests/` is walked flat. A future test helper under a subdirectory of `tests/` would be silently unwatched. Cheap to fix during d.1.
- **Strict-equality baseline (B-S3):** developer workflow trap — any incidental hit-count reduction during normal work forces a `FMPL_REGEN_BASELINE=1` ritual. d.1 should consider switching to `>= baseline` (asserts only against growth) or document the regen workflow before ITER-0004d.1's `== 0` flip removes the baseline entirely.
- **Coarse allowlist (B-S6):** the `(path-suffix, tag)` key suppresses every hit matching the tag in the file. A future legitimate `:first(args)` hit (under d.1's explicit-rejection regime) would be silently swallowed. d.1 should narrow to `(path, byte_offset_range)` if the false-suppression window proves real.

**Tests:** post-iteration test counts:
- Pre-iteration sentinel: 55 (ast_to_ir_parity) + 32 + 1 ignored (scenario_0103) + 8 (ac7_optimizer_pass_through) + 1 (stdlib_no_legacy_syntax) = 96 baseline.
- Post-iteration sentinel + new tests: 55 + 32 + 1 ignored + 8 + 13 (diagnostics_fmpl_source_scan, NEW) + 1 (no_legacy_fmpl_syntax, NEW) = 109 passed, 1 ignored across 5 suites. Net change: -1 (stdlib_no_legacy_syntax deleted), +13 + 1 = +13 net. No regressions.
- All 4 sentinels still clean post-iteration. Workspace clippy: zero warnings.

**Lessons:**

1. **Spec contradictions surface only when an implementer reads them end-to-end.** The roadmap simultaneously required (a) `scan_rust_strings` in `fmpl-core/src/diagnostics/mod.rs` and (b) `syn` as `[dev-dependencies]`. Both PAR reviewers caught this independently. Rust's dev-dep visibility rules make these incompatible; no amount of spec re-review fixes it because the contradiction is logical, not citation-level. The orchestrator-side resolution (split the function across `src/` and `tests/`) is the only valid Rust partition.
2. **Lexer-level "Symbol+LParen" is too coarse for grammar-DSL contexts.** The grammar-DSL binding syntax `name:bindName (group)` tokenizes identically to a tagged-constructor `:bindName(args)`. The naive scanner cannot distinguish them without contextual knowledge of grammar blocks. Allowlist works as an escape valve for the small known set of grammar-binding sites; a future iteration could investigate context-aware tokenization if the false-positive set grows.
3. **"Build the tool; let it produce ground truth" beats one more revision round when speculation has already failed twice.** Three prior PAR rounds on the monolithic ITER-0004d produced revisions that never escaped speculation about deletion-site counts. ITER-0004d.0's first run produced concrete numbers (43, 72, 625) that ITER-0004d.1 can plan against directly. The active hypothesis in the WORKSPACE checkpoint ("the tool's first run replaces speculation with measurement") was validated.
4. **PAR fix-loop discipline pays off even on small iterations.** Two of the post-implementation PAR findings (lying `rust_byte_offset = 0`; operator-style symbol false positives) were latent bugs that would have surfaced only when a future consumer relied on them. Applying the fixes inline (rather than deferring to d.1) closed the holes while context was fresh, at the cost of one extra commit. The deeper architectural findings (macro bodies, strict equality, allowlist granularity) were deferred to d.1 with explicit documentation rather than churned in this iteration.

**Summary:**
ITER-0004d.0 shipped a Rust library `fmpl_core::diagnostics` exposing `scan_fmpl_source` (production-crate-safe, `syn`-free) plus a test-only `scan_rust_strings` helper. The new CI gate `no_legacy_fmpl_syntax` records a 4-surface baseline (`lib/core=0`, `src/rs=43`, `tests/fmpl=72`, `tests/rs=625`) that ITER-0004d.1 will sweep to zero. The old `stdlib_no_legacy_syntax` gate was deleted (subsumed by the new gate's stricter token-level scan). PAR scope review returned REVISE; user opted to build as-spec'd, which proved correct — the implementation surfaced both the spec contradictions and the real baseline numbers. PAR post-implementation review caught 2 inline-fixable bugs (lying offset field, operator-symbol false positives) plus 4 architectural concerns documented as ITER-0004d.1 inputs. Workspace test count: +13 net, 0 regressions.

---

## ITER-0004d.1 — Parser/AST/Pattern Burn (AC-9, AC-10, AC-12) — done 2026-05-12 (T18 deferred)

**Stories committed:** STORY-0010 Phase B AC-9, AC-10, AC-12 (parser-rejection contract + AST/pattern variant deletions). Also: SCENARIO-0039 rewrite, SCENARIO-0066 rewrite, EPIC-032 STORY-0095/AC-4 text repair.

**Result:** Five distinct surfaces deleted and replaced with the canonical list-shape form per DESIGN-002:

| Surface | Before | After |
|---|---|---|
| FMPL expression `:Tag(args)` | `parser.rs:619` produced `Expr::Tagged(SmolStr, Vec<Expr>)` | `parser.rs:678` returns `Error::Parser` with phrase `use [:Tag] or [:Tag, ...] instead` |
| FMPL pattern `:Tag(p1, p2)` | `parser.rs:1849` produced `ast::Pattern::Constructor(SmolStr, Vec<Pattern>)` | `parser.rs:1900` returns `Error::Parser` (same phrase) |
| Grammar DSL pattern (3 sites) | `grammar/parser.rs:899/1136/1333` produced `pattern::Pattern::TagMatch` and friends | `grammar/parser.rs:884, 1161` reject; third site collapsed during cleanup |
| `Expr::Tagged` enum variant | defined at `ast.rs:157` | deleted; all 6 consumer arms deleted |
| `ast::Pattern::Constructor` variant | defined at `ast.rs:115-116` | deleted; all 8 consumer sites deleted (including `is_symbol_with_paren` helper) |
| `pattern::Pattern::Tagged` variant | defined at `pattern/mod.rs:57-61` | deleted; replaced in compile path by `UP::ListMatch` arm using `ExtractTaggedChild` |
| `pattern::Pattern::TagMatch` variant | defined at `pattern/mod.rs:143` | deleted; ~150 lines of grammar runtime/trampoline state-machine code removed |

**Plus a new freshness signal:**

- `fmpl-core/src/parser_epoch.rs` defines `PARSER_EPOCH: u32` (bumped to 3 in this iteration).
- The generated parser embeds `GENERATED_PARSER_EPOCH`; a gated `const _ = assert!(PARSER_EPOCH == GENERATED_PARSER_EPOCH)` triggers a clear compile-time mismatch error on stale generators.
- `build.rs` adds `rerun-if-changed` for `parser_epoch.rs` and `builtins/ir_to_rust.rs` (the postlude raw-string), plus `rerun-if-env-changed` for the `FMPL_*` env vars (which were missing — cargo had been caching builds across env-flag flips).

**Plus the evidence corpus:**

- `fmpl-core/tests/structural_invariants.rs` (NEW) — 17 tests covering SCENARIO-0104, SCENARIO-0105, SCENARIO-0106. Tests are scenario-tagged at the function name level; rejection-checking helper is unparameterized for specific message text because multiple parser paths can reject the same surface form. Greppable-invariant tests strip `//`-line-comments before matching so historical narratives in `parser_epoch.rs` don't trip the gate.
- `behavior-scenarios.md` — SCENARIO-0039 and SCENARIO-0066 rewritten in place to drop references to deleted types; SCENARIO-0104/0105/0106 added (full Preconditions/Action/Expected observables/Execution command).
- `behavior-corpus.md` — 3 new entries with concrete `cargo test` execution commands.
- `requirements/EPIC-032.md` STORY-0095/AC-4 — rewrote to describe the canonical list-node form via `Value::as_node()`.
- `requirements/EPIC-002.md` STORY-0010 — added `· scenario:` tags to AC-9, AC-10, AC-12.

**Plus three CI gate moves:**

- `no_legacy_fmpl_syntax.baseline.json` ratcheted: lib/core=0 (unchanged), src/rs=38→26 (T-task deletions), tests/fmpl=72→0 (T7 orphan-file deletion), tests/rs=108→4 (test-file sweeps). The `== 0` flip is deferred to ITER-0004d.3 — see "Deferred to ITER-0004d.3" below.
- `structural_invariants.rs` added to `TESTS_RS_EXCLUDES` because it intentionally contains `":Foo(1)"` parser-input fixtures.
- The `scan_fmpl_source` allowlist was unchanged — no new false-positive sites surfaced during the sweep.

**Sentinels (final, 2026-05-12):**
- ast_to_ir_parity: 57 passed, 2 ignored (FOLLOWUP #30)
- scenario_0103_optimizer_pipeline: 32 passed, 1 ignored
- tavern_demo: 6 passed (no `;` separator workarounds — MF1 fixed the root cause)
- no_legacy_fmpl_syntax: 1 passed (baseline mode)
- structural_invariants: 17 passed (NEW)
- Full fmpl-core suite: 1200 passed, 182 ignored across 71 suites (fallback parser; the metacircular generator path has a pre-existing parse error pending ITER-0004d.3)
- Net test count change: -1 (deleted `test_full_mode_tagged_pattern_uses_match_tagged` — asserted on a soon-to-be-renamed opcode name), +17 (`structural_invariants.rs`), +0 net for the gate suite. Roughly +16 net new tests with 0 regressions.

**Deferred to ITER-0004d.3 (T18 — the gate flip):**

The `no_legacy_fmpl_syntax.rs` gate flip from baseline mode to `== 0` mode was deferred because:
1. The metacircular pipeline is currently broken — `fmpl-bootstrap lib/core/parser_generator.fmpl` fails with `expected Comma` on the third `io::load(...)` call. The failure pre-dates the T-task work; sentinels stay green because `FMPL_SKIP_PARSER_GEN=1` falls back to the legacy parser. A green `== 0` gate would silently rely on this fallback, contradicting DESIGN-001 (the metacircular pipeline must be the proof surface).
2. The 4 + 26 residual hits in tests/rs + src/rs are false positives — `module:function(args)` patterns the scanner's coarse `Symbol+LParen` heuristic confuses with the legacy `:Tag(args)` form. Eliminating them needs either an allowlist extension or a scanner refinement (a single-token lookback distinguishing `Ident COLON Symbol LParen` from `COLON Symbol LParen`).

Both are sized for a focused iteration. ITER-0004d.3 has been added to the roadmap with these as binding preconditions.

**Deferred to ITER-0004d.4 (new — data-driven scenario runner):**

User feedback during the T19 review: per-scenario Rust tests work but the scenario name + asserts don't fully tell the story; a reader has to flip back to the scenario card to know what's being verified. A cucumber/FitNesse-SLIM-style data-driven runner where the scenario card IS the source of truth, and Rust step-definitions handle dispatch, would (a) make the cards directly executable, (b) collapse the per-test boilerplate, and (c) let future scenarios land as card-authoring tasks rather than test-writing tasks. Sized for a focused iteration; sequencing-independent of ITER-0004d.3.

**Lessons (graduated to LESSONS.md once cooled):**

1. **`jj` vs `git` status disagree in a meaningful way.** In jj, the working-copy `@` is always a committed change (described or not). `git status` showing "modified files" against `HEAD` does NOT mean "uncommitted work"; it means "the diff between `HEAD` and `@` content" — `@` itself is already a commit. Treating jj's `@` as if it were git's "uncommitted state" leads to false alarms ("I need to commit this") when the change is already in `@`. Saved as feedback memory and added to the project-local MEMORY.md index.
2. **Doc-comment narratives age into false signals.** Comments like `// Old: Value::Tagged(tag, children)` survive deletion of the type. A greppable-invariant test that doesn't strip comments will trip on these. Comment-strip is a one-line fix in the test helper but the principle generalizes: any structural invariant that grep-walks source code should distinguish live references from historical narratives, and the helper should make that distinction explicit (rather than relying on every test caller to filter).
3. **"Behavior contract" vs "test message text" is a meaningful distinction.** My first cut at the rejection tests pinned each test to a specific error message phrase ("constructor syntax is not supported"). The `let (:Pair(...) = ...)` case rejected through a different parser path with a different message ("expected identifier"). The fix was to make the contract "the parse returns Err" and add a separate message-quality test that asserts the canonical-form hint appears. The lesson: when the contract is "X is rejected", don't over-specify HOW it's rejected unless the HOW is itself observable.
4. **An iteration's `Out of scope` list is load-bearing.** Grep #6 for `Instruction::MakeTagged` initially found two live emit sites the iteration explicitly considers out of scope (`vm.rs` runtime dispatch and `builtins/ir.rs` IR-node handler — both wait on ITER-0004d.2's opcode rename). Rather than re-scoping the iteration or weakening the grep, I scoped grep #6 to `compiler.rs` only and documented the surviving references in the scenario card's note. The iteration's `Out of scope` list is what a future reader compares against when wondering whether a finding is a regression or expected; making it precise is worth the words.

**Summary:**
ITER-0004d.1 closed the parser surface for `:Tag(args)` syntax (both expression and pattern positions) by deleting four producer/consumer AST/pattern variants and ~300+ lines of dead consumer arms across compiler.rs, vm.rs, grammar runtime/trampoline, repr, builtins, and value_to_ast. Three new behavior-corpus scenarios (SCENARIO-0104/0105/0106) document the contracts; 17 passing Rust evidence tests in `structural_invariants.rs` provide proof at the unit seam. The parser-epoch system was added as a freshness signal for future generator-regeneration cycles. T18 (CI gate flip to `== 0`) was deferred to ITER-0004d.3 pending the bootstrap-parse follow-up; ITER-0004d.4 was scheduled for the data-driven scenario runner the user raised during T19 review. Sentinel corpus is green (113 passed, 3 ignored across 5 suites covering the impacted scenarios + sentinels).

---

## ITER-0004d.3 — Bootstrap-Parse Fix + `no_legacy_fmpl_syntax` Gate Flip — done 2026-05-12

**Stories committed:** STORY-0010 Phase B AC-9 / AC-10 / AC-12 final CI-gate ratchet (the deferred T18 from ITER-0004d.1). Adds SCENARIO-0108 (canonical-pipeline parity).

**Result:** The metacircular pipeline now works end-to-end. The `no_legacy_fmpl_syntax` gate runs in `== 0` mode against the canonical generated parser. SCENARIO-0108 provides positive behavioral evidence that the canonical pipeline produces the same parse results as the source-tree Rust parser.

| Surface | Before | After |
|---|---|---|
| `is_inline_pattern_block` heuristic | misclassified `g @ { [:Tag, any:name] => ... }` as AST inline pattern, broke metacircular bootstrap on 3rd load | distinguishes via `Ident Symbol` adjacency inside `[ ... ]`; routes grammar-DSL blocks to `parse_anonymous_grammar_block` |
| `no_legacy_fmpl_syntax` gate mode | baseline-mode (`lib/core=0, src/rs=26, tests/fmpl=0, tests/rs=4`) | `== 0` mode (baseline JSON deleted) |
| `SourceKind::RustString` | no doc-attr discrimination — `syn` visited doc comments as `LitStr`, producing 30 false positives | `from_doc_attr: bool` flag; gate suppresses doc-attr origin hits |
| `lib/core/fmpl_parser.fmpl` | accepted `:Foo(1)` as `Call(Symbol("Foo"), [...])` — silently weaker than source-tree parser | rejects `:Foo(1)` with same error as source-tree (via `legacy_tagged_ctor` rule + `"LegacyTagCtor"` postlude arm) |

**Plus new evidence corpus:**

- `fmpl-core/tests/canonical_pipeline_parity.rs` (NEW) — 7 evidence tests covering SCENARIO-0108: 3 rejection-parity tests (`:Foo(1)`, `:Bar(1, 2, 3)`, `match x { :Pair(a, b) => 1 }`), 4 AST-parity tests (`42`, `1 + 2 * 3`, `:Foo`, `[:Foo, 1, 2]`).
- `behavior-scenarios.md` — SCENARIO-0108 added with full Preconditions / Action / Expected observables (two equivalence classes: rejection equivalence + AST equivalence).
- `behavior-corpus.md` — SCENARIO-0108 entry with concrete `cargo test -p fmpl-core --test canonical_pipeline_parity` command, marked `sentinel` cadence.
- `requirements/EPIC-002.md` STORY-0010 status updated: AC-9/AC-10/AC-12 done:ITER-0004d.{1,3}; AC-11 pending in ITER-0004d.2.

**Sentinels (final, 2026-05-12):**
- ast_to_ir_parity: 57 passed, 2 ignored (FOLLOWUP #30)
- scenario_0103_optimizer_pipeline: 32 passed, 1 ignored
- tavern_demo: 6 passed
- no_legacy_fmpl_syntax: 1 passed (`== 0` mode, baseline JSON deleted)
- structural_invariants: 17 passed
- diagnostics_fmpl_source_scan: 21 passed (was 17; added 4 doc-attr discrimination tests in T4)
- canonical_pipeline_parity: 7 passed (NEW — SCENARIO-0108 evidence)
- **Total: 137 passed, 3 ignored across 7 suites** (vs prior 113/3 across 5 suites — net +24 tests, 0 regressions)
- Confirmed canonical pipeline active: `unset FMPL_BOOTSTRAP_PHASE FMPL_SKIP_PARSER_GEN; cargo build` produces no "Parser generation skipped" or "using legacy parser" warning; `target/debug/build/fmpl-core-*/out/generated_parser.rs` has `GENERATED_PARSER_EPOCH: u32 = 3` matching source.

**PAR scope review impact:**

Two adversarial reviewers returned REVISE in parallel with six aggregated findings:

1. **Sentinel gap — no test exercises `parser::generated_parse`.** Both reviewers flagged independently. Resolution: SCENARIO-0108 + canonical_pipeline_parity.rs added. **Caught a real divergence on first run** — the FMPL parser silently accepted `:Foo(1)` while the source-tree parser rejected it. T7b fixed the FMPL grammar. Without this finding, ITER-0004d.3 would have shipped with the metacircular pipeline silently weaker than the source-tree parser.
2. **ACs conflated syntactic vs behavioral gates.** Resolution: Acceptance section split into "Syntactic-cleanliness gate" and "Behavioral-correctness gate" as distinct AC families.
3. **Scanner discriminator direction may be wrong** (the `module:function(args)` characterization). T1a confirmed the description was wrong — all 30 hits were doc-attr origin, not `module:function(args)` token sequences. The single-token lookback approach was the wrong design; doc-attr origin discrimination was the right one.
4. **Scope-creep allowlist hedge.** Resolution: "no new allowlist entries" binding precondition added.
5. **Grammar-runtime fix → PARSER_EPOCH bump unmodeled.** Resolution: T3a contingency added. T1's investigation showed the fix lives in `parser.rs` (not grammar/runtime.rs), so T3a became a no-op.
6. **Redundant allowlist cleanup.** Resolution: T5a planned but turned out to be a no-op — the allowlist entries cover non-doc-attr grammar-DSL sites in actual `.fmpl` files (still load-bearing).

The PAR loop did not need a second round; all six findings were addressed with surgical scope edits. The fact that finding #1 caught a real bug (FMPL parser weaker than source-tree) is concrete evidence of PAR's value on this iteration.

**Lessons:**

1. **"All sentinels pass" is not the same as "the canonical pipeline is correct".** Both PAR reviewers flagged this independently. Every sentinel routed through either `Parser::with_source` (source-tree) or `eval_via_legacy_parser`; none used `generated_parse`. The claim "we test the canonical pipeline" was empirically false — and the absence was invisible without explicitly checking which parser entry point each sentinel called. Lesson: when a proof obligation says "X works through pipeline Y", at least one sentinel must mechanically execute through pipeline Y.
2. **The label can mislead.** The bootstrap failure was called "the three-load failure" for two days because it surfaced consistently on the third `io::load(...)` call. T1 showed the real mechanism was content-dependent (only files with `g @ { [:Tag, any:name] => ... }` blocks failed); the three-load coincidence was just the order in which files of different content were loaded. Lesson: when an investigation produces a heuristic label, treat it as a hypothesis to verify, not a name to keep.
3. **PAR review can find bugs by predicting them.** Finding #1 (no sentinel exercises generated_parse) was not a hypothetical concern — adding the missing sentinel immediately surfaced a real divergence (FMPL parser silently weaker). The reviewer didn't know the bug existed; they inferred from the test architecture that a divergence COULD hide there. Adding the coverage forced the bug into view. Lesson: when a reviewer says "you're missing coverage at boundary X", evaluate whether boundary X is one where bugs could hide silently, regardless of whether you have direct evidence of a bug.
4. **FMPL grammar runtime lacks parse-failure primitive.** T7b discovered that the grammar runtime / IR-to-Rust transpiler has no native `fail(msg)` mechanism: no `"Throw"` arm in `ir_to_rust::transpile_tagged`, and `ParseChoice` wraps alternatives in closures that catch early returns. Workaround: emit a poison AST node (`[:LegacyTagCtor, tag]`) from the grammar and reject it in the postlude `value_to_expr`. This works but is unconventional. Documented follow-up: a real grammar-side `fail()` primitive would require both a new IR arm and a rework of `ParseChoice` codegen to allow controlled hard-failure across alternatives. Out of scope for ITER-0004d.3 but tracked for a future iteration.

**Cross-iteration TODO resolution:**

- TODO(ITER-0004d.3) markers in source: none found (`grep -rn 'TODO(ITER-0004d.3)' fmpl-core/ lib/`). All forward-references resolved.

**Summary:**

ITER-0004d.3 closed the metacircular-pipeline correctness gap that ITER-0004d.1 deferred. The bootstrap-parse failure was root-caused (not three-load; content-dependent `is_inline_pattern_block` misclassification) and fixed. The legacy-syntax gate is now in `== 0` mode with doc-attr origin discrimination. PAR scope review caught a real bug class (FMPL stdlib parser silently weaker than source-tree) that the new SCENARIO-0108 surface evidenced and T7b fixed. Sentinel corpus: 137 passed, 3 ignored across 7 suites (+24 net tests, 0 regressions). Canonical pipeline confirmed active. ITER-0004's correctness ratchet has tightened by one more turn.

---

## ITER-0004d.3a — SCENARIO-0108 audit fix-up (G1+G2+G3) — done 2026-05-12

**Stories committed:** STORY-0010 Phase B AC-9/AC-10/AC-12 evidence strengthening. Direct response to ITER-0004d.3's three-tier audit findings.

**Result:** All three audit-flagged gaps closed. SCENARIO-0108 evidence is now genuinely falsifiable; the magic-string coupling between FMPL grammar and Rust postlude is statically asserted; the postlude arms have an isolated regression test.

**Audit findings closed:**

| Gap | Severity | Fix |
|---|---|---|
| G1: SCENARIO-0108 tests unfalsifiable under fallback parser (both auditors) | CRITICAL | Added `pub const IS_GENERATED_PARSER: bool` to both real (true) and fallback (false) parser binaries. `canonical_pipeline_must_be_active` test asserts the constant; tests now FAIL loudly under fallback rather than passing trivially. PARSER_EPOCH bumped 3→4. |
| G2: SCENARIO-0108 hint assertions missing on 2/3 cases + pattern-position couldn't produce hint (both auditors) | CRITICAL | Strengthened `parity_rejects_value_constructor_multi_arg` and `parity_rejects_pattern_constructor_in_match_arm` with `use [:` assertions on both parsers' errors. Added `pat_legacy_tagged_ctor` rule to `lib/core/fmpl_parser.fmpl` placed first in `pat_primary` alternation; added `"PatternLegacyTagCtor"` arm in `value_to_pattern` postlude. PARSER_EPOCH bumped 4→5. |
| G3: magic-string coupling between fmpl_parser.fmpl and ir_to_rust.rs not tested (both auditors) | SERIOUS | Added `scenario_0106_grep_8_legacy_tag_ctor_coupling` structural grep asserting both `LegacyTagCtor` and `PatternLegacyTagCtor` appear in both files. Added `g3_postlude_arms_fire_on_poison_nodes` isolated test exercising both postlude arms via crafted `generated_parse` inputs (route A2 since `value_to_expr` / `value_to_pattern` are private to the generated `__generated` module). |

**Plus residual-gap-fix found in the re-audit:**

The first-cut G3 implementation forgot the `IS_GENERATED_PARSER` guard on `g3_postlude_arms_fire_on_poison_nodes` — exactly the failure mode G1 was designed to prevent (test would pass trivially under fallback because the source-tree parser also rejects `:Foo(1)` and `match x { :Pair(a, b) => 1 }` with messages containing `use [:`). Caught by the focused re-audit (which the iteration's acceptance criteria specifically required). Fixed by adding the same `assert!(IS_GENERATED_PARSER)` guard at the top of the test.

**Sentinels (final, 2026-05-12):**
- ast_to_ir_parity: 57 passed, 2 ignored
- scenario_0103_optimizer_pipeline: 32 passed, 1 ignored
- tavern_demo: 6 passed
- no_legacy_fmpl_syntax: 1 passed (== 0 mode)
- structural_invariants: 19 passed (was 17 — +2 from G3)
- diagnostics_fmpl_source_scan: 21 passed
- canonical_pipeline_parity: 8 passed (was 7 — +1 from G1)
- **Total: 140 passed, 3 ignored across 7 suites** (vs end-of-ITER-0004d.3 baseline 137/3; net +3 tests, 0 regressions)

**PARSER_EPOCH lineage:**

ITER-0004d.3a touched the postlude raw-string twice (G1 adds `IS_GENERATED_PARSER`; G2 adds `PatternLegacyTagCtor` arm). Each change independently bumped per the `parser_epoch.rs:27-29` policy:
- 3 → 4 (G1, 2026-05-12 — postlude const addition)
- 4 → 5 (G2, 2026-05-12 — postlude match arm addition for pattern-position rejection)

Both bumps are recorded in `parser_epoch.rs:66-82` bump history.

**Lessons:**

1. **Audit-fix-up iterations can themselves require audits.** The re-audit step caught a residual G3 gap (missing `IS_GENERATED_PARSER` guard) that would have shipped silently. Without re-auditing, the iteration would have closed with an evidence test that was itself unfalsifiable — defeating the iteration's own purpose. The lesson: a fix-up iteration's acceptance criterion MUST include "the re-audit confirms the gaps are closed," not just "the new tests pass."
2. **Pattern: PAR finds bugs by predicting them.** Same pattern as ITER-0004d.3's PAR finding #1: auditors saw a structural weakness and predicted a class of regression. The class wasn't hypothetical — adding the guards immediately surfaced the trivially-passing condition. Demonstrates concrete PAR value on TWO consecutive iterations.
3. **Magic strings across files need a structural test.** The T7b workaround (poison AST node) created a hardcoded coupling between the FMPL grammar's emitted tag name and the Rust postlude's match-arm string. Without the structural grep in G3, a rename in either file would silently break the rejection. Future work: consider extracting the magic strings into a single Rust constant referenced from both sides (cleaner than a structural grep, but requires changes to the FMPL grammar to reference Rust-side identifiers — out of scope for this iteration).
4. **Parser-epoch bump policy works.** Two independent bumps in one iteration handled cleanly by `build.rs`'s `read_generated_parser_epoch` invalidation — no manual `cargo clean` needed. The mechanism is robust to legitimate concurrent edits.

**Summary:**
ITER-0004d.3a closed three audit-flagged gaps in SCENARIO-0108 evidence: fallback-detection guard (G1), hint assertions on all three rejection inputs + pattern-position canonical rejection (G2), structural-grep-plus-isolated-test for the T7b magic-string coupling (G3). Sentinel corpus expanded to 140/3 across 7 suites. The re-audit caught a residual G3 gap (missing falsifiability guard on the postlude test) that was fixed inline. ITER-0004 progress: AC-9, AC-10, AC-12 now have genuinely-strong canonical-pipeline evidence. ITER-0004d.2 (opcode rename) and ITER-0004h (Type::Tagged cleanup) remain.
