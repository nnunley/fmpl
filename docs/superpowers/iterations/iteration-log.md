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
