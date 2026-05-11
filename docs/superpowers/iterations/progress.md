# Progress

**Phase:** ITER-0004d.1 — DONE 2026-05-12 with T18 deferred to ITER-0004d.3. Two new follow-on iterations scheduled: ITER-0004d.3 (bootstrap-parse + T18) and ITER-0004d.4 (data-driven scenario runner).
**Iterations:** 7/13 done (ITER-0004d.1 closed; ITER-0004d.2, 0004d.3, 0004d.4, 0004e, 0004f, 0004g, 0004h, 0005+ pending).
**Sentinel corpus (final, 2026-05-12):** ast_to_ir_parity 57/57 (2 #[ignore]); scenario_0103 32/32 (1 ignored); tavern_demo 6/6; no_legacy_fmpl_syntax 1/1 (baseline mode, gate flip deferred to ITER-0004d.3); structural_invariants 17/17 (NEW — SCENARIO-0104/0105/0106 evidence). 113 passed, 3 ignored across 5 suites. Full fmpl-core suite: 1200 passed, 182 ignored across 71 suites (fallback parser — metacircular path pending ITER-0004d.3).
**Last event:** 2026-05-12 — ITER-0004d.1 wrap-up: roadmap updated (ITER-0004d.1 marked done; ITER-0004d.3 + ITER-0004d.4 added with binding preconditions); iteration-log.md entry written; progress.md (this file) finalized. User design decisions noted: thin Rust driver + parsed scenario cards (→ ITER-0004d.4); investigate bootstrap-parse first then T18 (→ ITER-0004d.3).

## Session 2 of 2026-05-12

Resumed from the same-day pause. The consolidated T7-T14 + parser-epoch commit was already in jj `@` (zrzuorru 7053fc13) — described and committed; `git status` showing "modified files" was a jj/git view mismatch, not uncommitted work. User feedback recorded: use `jj` for status, not `git`.

### T15 — Repaired STORY-0095/AC-4 in EPIC-032.md

Rewrote the AC text at `docs/superpowers/iterations/requirements/EPIC-032.md:21` to drop the `Value::Tagged` reference (deleted in ITER-0004b). New phrasing: "Structured 'tagged' data uses the canonical list-shape form `Value::List([Value::Symbol(tag), child1, child2, ...])` per DESIGN-002; introspection via `Value::as_node()` returns `(SmolStr tag, &[Value] children)` for any list whose first element is a Symbol".

### T16 — Updated EPIC-002 STORY-0010 AC tags

Added `· scenario:SCENARIO-0104 · scenario:SCENARIO-0106` to AC-9; `· scenario:SCENARIO-0105 · scenario:SCENARIO-0106` to AC-10 and AC-12.

### T17 — Reconciled/added scenarios

- SCENARIO-0039 rewritten to list-pattern form (drops `:int(n)` value-pattern in example grammar).
- SCENARIO-0066 rewritten to assert list-node shape via `Value::as_node()` (drops `Value::Tagged` references).
- SCENARIO-0104 added — parser rejects `:Tag(args)` value-constructor syntax. Includes preconditions (Rust parser + grammar DSL parser), action (3 distinct input cases), and expected observables (structured error, location, message phrase).
- SCENARIO-0105 added — parser rejects `:Tag(p1, p2)` pattern-position syntax. Distinct from 0104 because it exercises a different parser path (`parse_pattern` vs `parse_expr`).
- SCENARIO-0106 added — Rust-side greppable invariant: seven structural greps over `fmpl-core/src/` ensuring the deleted variants (`Value::Tagged`, `Expr::Tagged`, `Pattern::Constructor`, `Pattern::Tagged`, `Pattern::TagMatch`) stay deleted and the canonical replacement (`ExtractTaggedChild`) stays present. Grep #6 was scoped to `compiler.rs` only after discovering live `Instruction::MakeTagged` references survive in `vm.rs` (runtime dispatch) and `builtins/ir.rs` (IR-node handler) — these are explicitly out of scope until ITER-0004d.2's opcode rename.

Behavior-corpus.md index updated with the three new SCENARIO entries.

### T19 — Implemented evidence tests as `fmpl-core/tests/structural_invariants.rs`

17 tests, all passing:
- 5 tests for SCENARIO-0104 (3 rejection cases + 2 control parses + 1 error-message-quality check)
- 4 tests for SCENARIO-0105 (2 rejection cases + 1 control + 1 error-message-quality)
- 7 tests for SCENARIO-0106 (6 absent-name greps + 1 present-name positive grep)
- 1 helper-internal test for the comment-aware grep substrate

Key implementation choices recorded:
- `assert_rejected` is unparameterized for the specific message — the contract is "parse returns Err", not "Err with specific text". Reason: multiple parser-grammar paths can reject the same surface (e.g., `let (:Pair(...) = ...)` is rejected at the let-binding ident-expectation step before reaching the F2 arm).
- Greppable invariant test uses an inline src-tree walker rather than the `diagnostics_fmpl_source_scan` helper because the targets are Rust type names, not FMPL `:Tag(args)` syntax — the existing scanner doesn't apply.
- `find_word_in_code` strips `//`-line-comments before matching so historical narratives in `parser_epoch.rs` doc comments don't trip the gate. Block comments are NOT stripped (rare and harder to handle robustly).

The new test file (`structural_invariants.rs`) was added to `TESTS_RS_EXCLUDES` in `no_legacy_fmpl_syntax.rs` because it intentionally contains `":Foo(1)"` strings as parser-input fixtures.

### T18 — paused

Two distinct deferrals:

1. **Bootstrap parse-error follow-up** (carried from session 1) — the `fmpl-bootstrap lib/core/parser_generator.fmpl` parse error reproduces with parent-commit source, isolating it to a pre-existing bug in the bootstrap parser's handling of three consecutive `io::load(...)` calls. T18 should not flip the gate to `== 0` while the metacircular pipeline silently relies on the fallback parser; the gate-passes would be misleading.

2. **Data-driven-scenario-runner design discussion** (new, session 2) — user observed that the per-scenario Rust tests in `structural_invariants.rs` are stylish but make the test file harder to read at a glance ("hard to figure out what's being tested"). User raised cucumber / FitNesse SLIM-style as a possible architecture: scenario cards in behavior-scenarios.md become the source of truth, Rust tests collapse to a thin driver. T18's tests/rs surface is downstream — if scenario tests become data-driven, multiple existing test files might collapse into a single driver, which changes what the legacy-syntax gate should exclude. Pending user decision before resuming T18.

## Session of 2026-05-12 (first half — preserved verbatim below)

## Session of 2026-05-12

Resumed from the paused state captured 2026-05-11. Two commits landed cleanly atop T6 before resuming the T-task sequence:

- `cb225806` docs(iter-0004d.1): audit-trail repair (F3, F12, F13, F18) + design-principles infra
- `(latest)` feat(parser): reject :Tag(args) in pattern position + parse_inline_pattern_block separator fix (F2+F1+F9+MF1)

Then proceeded through the T-task sequence:

### T7 — Delete orphan fmpl test files

Deleted `fmpl-core/tests/fmpl/ast_to_ir.fmpl` and `fmpl-core/tests/fmpl/fmpl_parser.fmpl` — 2026-01-29 spike solutions in legacy `:Tag(args)` syntax with zero remaining references. Baseline `tests/fmpl` dropped 72 → 0 (all 72 hits lived in these two files — a windfall: the `tests/fmpl` surface is now `== 0` and ready for T18's CI-gate flip).

### T8 — Update `lib/core/fmpl_parser.fmpl`

Deleted FMPL stdlib parser rules implementing the now-removed `:Tag(args)` syntax: `tag_name`, `tagged_arg_rest`, `tagged_args`, `tagged_with_args`, `tagged_empty`, `tagged`, `pat_constructor`. Updated `primary` and `pat_primary` alternation rules to drop the deleted alternatives. Preserved `pat_arg_rest` and `pat_args` (shared with the canonical list-pattern `pat_list`).

Grammar-parser scope-expansion question (open at session pause) resolved: F1+F2 covered the Rust-side rejection in this session; T8 was purely the FMPL stdlib parser counterpart.

### T9 — Delete `Expr::Tagged` AST variant

Producer (`parser.rs:619`) already replaced with rejection in T6. Consumer-side cleanup landed atomically with the variant deletion (all 6 sites + the now-zero-caller `ir_builder::tagged` helper). Variant definition at `ast.rs:157` deleted.

Consumer sites deleted:
- `compiler.rs:869-878` — compile_expr arm emitting `Instruction::MakeTagged`
- `repr.rs:225-237` — Display impl arm
- `builtins/ast.rs:27-33` — `expr_to_value` encoder arm
- `builtins/grammar_to_ir.rs:311-320` — encoder arm
- `value_to_ast.rs:353-368` — `"Tagged"` decoder arm
- `builtins/ir_to_rust.rs:1435-1450` — decoder arm
- `ir_builder.rs:238-244` — `fn tagged` helper (verified zero callers via cargo check)

### T10 — Delete `[:Tagged, ...]` rule from `lib/core/ast_to_ir.fmpl`

Single-line deletion at `ast_to_ir.fmpl:21`. After T9 removed `Expr::Tagged`, no AST value of `[:Tagged, ...]` shape can be produced through the legacy or generated parser, so the rule had no live input. Removed.

### T11 — Delete `ast::Pattern::Constructor` variant

Producer (`parser.rs:1839`) already replaced with rejection in F2. Consumer sites deleted:
- `compiler.rs:2530-2538, 2643-2693, 2990-3000` — three arms (outer constructor match, nested-constructor handling in `compile_match_bindings`, and the let-binding arm in `compile_pattern_binding`)
- `repr.rs:101-110` — Display impl
- `builtins/ast.rs:390-396` — `pattern_to_value` arm
- `value_to_ast.rs:1212-1226` — `"PatternTagged"` decoder
- `builtins/ir_to_rust.rs:1854-1866` — same decoder in the postlude raw-string
- `parser.rs:1646, 1999-2003` — `is_symbol_with_paren` helper + its `parse_let` caller (the latter was the destructuring detection for `let (:Tag(args) = ...)`; with F2's rejection, this code path is unreachable)
- Variant definition: `ast.rs:115-116`
- Test fixture: `tests/diagnostics_fmpl_source_scan.rs:138-141` — rewrote `Pattern::Constructor` to `MyPattern::Constructor` (synthetic enum) so the fixture doesn't reference a deleted type

### T12 — Delete `pattern::Pattern::Tagged` variant

Test-side rewrites for `pattern::Pattern::Tagged { tag, patterns }` constructions:
- `tests/pattern_unification.rs:39, 43, 176, 279, 284` — 4 sites rewritten to `Pattern::ListMatch([SymbolLiteral(tag), ...children], None)`
- `tests/context_aware_compilation.rs:95, 340, 549` — 3 sites rewritten same way
- `tests/context_aware_compilation.rs::test_full_mode_tagged_pattern_uses_match_tagged` — deleted (asserted on the `MatchTagged` opcode name, an implementation detail being renamed in ITER-0004d.2; behavior covered by sentinels)

Added a special-case in `Pattern::requires_full_mode`: `ListMatch([SymbolLiteral(...), ...], None)` reports `Fast` mode (matches the deleted `Pattern::Tagged`'s fast-mode-compatible classification). Added a new `UP::ListMatch` arm in `compile_pattern_fast` that handles the tagged-shape via `ExtractTaggedChild` (replaces the deleted `UP::Tagged` arm).

Consumer sites deleted:
- `compiler.rs:3122-3131, 3333-3350, 3784-3794` — three `UP::Tagged`/`GP::Tagged` arms
- `grammar/runtime.rs:1145-1153` — let-binding pattern handler (now collapses to 4-variant or-pattern)
- `grammar/trampoline.rs:1178-1188` — same in trampoline
- `grammar/optimizer.rs:213-224` — first-set arm
- `builtins/grammar_to_ir.rs:245-252` — let-binding error path
- `repr.rs:663-676` — Display impl
- Variant definition: `pattern/mod.rs:57-61`

### T14 — Delete `pattern::Pattern::TagMatch` variant

Producers (`grammar/parser.rs:899, 1136, 1333`) already replaced with rejection in F1. Consumer sites deleted:
- `grammar/runtime.rs:784-857` — tagged-value matcher (74 lines)
- `grammar/trampoline.rs:148-161, 411-425, 999-1047, 1506-1552` — `WorkItem::TagMatchContinue` variant + dispatch + handler (~150 lines total across the trampoline state machine)
- `grammar/optimizer.rs:220` — first-set arm
- `builtins/grammar_to_ir.rs:234` — error-arm entry
- `compiler.rs:3634, 4343-4377` — `UP::TagMatch` fallback + full `GP::TagMatch` compilation logic (the larger of these emitted `MatchTaggedWithBindings` / `MatchTagged` opcodes — scheduled for renaming in ITER-0004d.2, now zero remaining emit sites; the opcode definitions remain until 0004d.2)
- `repr.rs:592-605` — Display impl
- Variant definition: `pattern/mod.rs:143`
- Now-orphan `Pattern::contains_repeat` helper at `pattern/mod.rs:162-170` — deleted (only called from inside the deleted runtime TagMatch arm)

### Parser-epoch system (new infrastructure)

Per user request to address build.rs freshness gap:

- `fmpl-core/src/parser_epoch.rs` — new `pub const PARSER_EPOCH: u32 = 3;` with full bump-policy documentation. Bumped twice this session (1 → 2 at T9+T11, 2 → 3 at T12+T14).
- `fmpl-core/src/parser.rs` — compile-time `const _: () = assert!(PARSER_EPOCH == GENERATED_PARSER_EPOCH, ...);` gated by `#[cfg(has_generated_parser_epoch)]` (active only when the real generator succeeds; dormant under fallback).
- `fmpl-core/src/builtins/ir_to_rust.rs` — generator emits `pub const GENERATED_PARSER_EPOCH: u32 = N;` into every generated parser's preamble.
- `fmpl-core/build.rs` — added freshness checks: rerun-if-changed for `src/parser_epoch.rs` and `src/builtins/ir_to_rust.rs` (the postlude raw-string), rerun-if-env-changed for the FMPL_* env vars (which were missing — previously cargo would cache builds even when env-var-driven flags flipped), epoch-mismatch detection that forces regen, `has_generated_parser_epoch` cfg gate.

The system replaces timestamp-only freshness with content-addressed version checking. A future bump+rebuild now produces a clear "parser epoch mismatch" error rather than a cryptic `E0599` deep in `out/generated_parser.rs`.

## Audit-trail corrections (carried from earlier in session)

F12 + F13 + F18 fixes from the morning are already in commit `cb225806`. Cited briefly here:

- F13: the `163` baseline value never existed; tests/rs transitioned 625 → 108 in a single commit (f4b91cef), not in two steps.
- F12: commit 6abab103 modified only `lib/core/grammar_optimizer.fmpl` and `.agent/memory/episodic/AGENT_LEARNINGS.jsonl`; the legacy-syntax-validation test file deletions its message claims happened in f4b91cef.
- F18: the line range `grammar/parser.rs:1309-1136` was reversed; actual sites were 899, 1136, 1333.

## Build/freshness issue discovered (deferred follow-up)

Running `fmpl-bootstrap lib/core/parser_generator.fmpl` fails with `Parser error at token 20: expected Comma (at position 20, token 20: Symbol("name"))`. This reproduces with both the May-10-cached bootstrap binary AND a fresh rebuild from current source — so it's not a stale-binary issue. Bisection: a single `io::load(...)` works; two consecutive loads work; three consecutive loads fail. The failure is independent of my T-task changes (reproduces when fmpl_parser.fmpl is restored to the parent-commit version).

Test sentinel keeps passing because `FMPL_SKIP_PARSER_GEN=1` falls back to the legacy parser, which works end-to-end. But the metacircular pipeline (fmpl-bootstrap → generated_parser.rs → fmpl-core parsing) is broken until this is fixed. Worth investigating before T18 flips the `no_legacy_fmpl_syntax` CI gate to `== 0` mode (which assumes the canonical pipeline works).

Likely culprits to investigate:
1. Cross-file scope issue: loading three grammars exposes some name-resolution problem.
2. Bootstrap parser bug: the legacy parser embedded in fmpl-bootstrap has an edge case in handling the third load.
3. Side-effect ordering: the third `io::load` may trigger evaluation that depends on something the previous two haven't fully set up.

Recommended next-session entry point: run the bootstrap with detailed tracing, or break parser_generator.fmpl into per-load test cases.

## Current baseline

`no_legacy_fmpl_syntax.baseline.json`:
```json
{
  "lib/core": 0,
  "src/rs": 26,
  "tests/fmpl": 0,
  "tests/rs": 4
}
```

Tests/rs went from 108 → 4 (only 4 remaining are non-syntax `module:function(args)` calls the gate's `Symbol+LParen` heuristic confuses with the legacy form; T18 will address). Src/rs went from 38 → 26 (chunks deleted with each variant + helper deletion). Tests/fmpl went 72 → 0 (entirely via T7's orphan-file deletion). Lib/core stays 0.

## Remaining work (T15-T19)

**T15:** Repair STORY-0095/AC-4 text in `docs/superpowers/iterations/requirements/EPIC-032.md:21`. Current text references the deleted `Value::Tagged` type; rewrite to describe the list-node `[Symbol(tag), ...children]` form.

**T16:** Update EPIC-002.md STORY-0010 AC tags. Add `· scenario:` tags for AC-9, AC-10, AC-12 once their new scenarios (T17) are landed.

**T17:** Reconcile/add scenarios in behavior-scenarios.md:
- Rewrite SCENARIO-0039 (currently uses `:int(n)` value-pattern syntax in grammar definitions) to list-pattern form
- Rewrite SCENARIO-0066 per scope item 13 (drop `Value::Tagged` refs, assert list-node shape via `Value::as_node()`)
- Add SCENARIO-0104 — parser rejects `:Tag(args)` value-constructor syntax (already implemented behaviorally; this writes the contract card)
- Add SCENARIO-0105 — parser rejects `:Tag(p1, p2)` pattern syntax
- Add SCENARIO-0106 — Rust-side greppable invariant (seven structural greps per the F19/round-6 corrections)

**T18:** Flip `no_legacy_fmpl_syntax.rs` CI gate from baseline mode to `== 0` mode. Requires:
- Eliminate the remaining 4 tests/rs hits and 26 src/rs hits (most are false-positive `module:function(args)` matches; may need allowlist additions or scanner refinement)
- Edit `no_legacy_fmpl_syntax.rs` to drop baseline-loading + assert `total == 0`
- Delete `no_legacy_fmpl_syntax.baseline.json`
- **Dependency:** investigate the bootstrap parse error first — if T18 lands while the metacircular pipeline is broken, the gate-pass would silently rely on the fallback parser

**T19:** Implement SCENARIO-0104, 0105, 0106 evidence tests as Rust integration tests (the SCENARIO cards from T17 each have a specific observable that becomes a test).

## Resume notes for next session

1. Read `docs/design-principles.md` first.
2. Read this file.
3. Review the bootstrap parse-error follow-up (the "Build/freshness issue discovered" section above). Consider whether to investigate before T18 or accept that T18 lands with a broken metacircular path documented as known-issue.
4. If continuing T15-T19: start with T17 (the scenarios), then T15+T16 (the EPIC.md text + AC tags reference these scenarios), then T19 (the evidence tests), then T18 (the gate flip — safest last).
5. FOLLOWUP #30 (ir::compile arity check + nested pattern alignment) remains outside this iteration.

## Sentinel green status (2026-05-12, paused)

- ast_to_ir_parity: 57 passed, 2 ignored (FOLLOWUP #30)
- scenario_0103_optimizer_pipeline: 32 passed, 1 ignored
- tavern_demo: 6 passed (no `;` workarounds; MF1 root-cause fix landed)
- no_legacy_fmpl_syntax: 1 passed (baseline regenerated)
- Full fmpl-core suite: 1200 passed, 182 ignored across 71 suites (one fewer than the 1201 in commit `cb225806` because `test_full_mode_tagged_pattern_uses_match_tagged` was deleted as part of T12 — see above)

Sentinels are GREEN. No regressions to recover from on next-session resume.

## Pending in working tree (uncommitted)

A coherent change spanning T7-T14 + the parser-epoch infrastructure. Recommend one commit at resume:

  - 8 files in `lib/core/` and `fmpl-core/src/` for the T-task variant deletions
  - 5 test files for the T11-T12 fixture rewrites
  - 2 new files: `fmpl-core/src/parser_epoch.rs`, this progress.md update
  - `fmpl-core/build.rs` and `fmpl-core/src/lib.rs` for the epoch wiring
  - `fmpl-core/tests/no_legacy_fmpl_syntax.baseline.json` regenerated
