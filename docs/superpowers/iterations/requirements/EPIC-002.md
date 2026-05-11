# EPIC-002 — Compiler Cutover

**Summary:** Compiler Cutover
**Stories:** STORY-0005, STORY-0006, STORY-0007, STORY-0008, STORY-0009, STORY-0010, STORY-0011, STORY-0012
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`, `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md`
**Status:** 3/8 done; STORY-0010 Phase A (AC-3..AC-7 + AC-13) shipped in ITER-0004c, Phase B pending in ITER-0004d

## STORY-0005

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Replace Rust compiler with FMPL ast_to_ir and ir::compile

**As a** FMPL developer
**I want** ast_to_ir.fmpl combined with the ir::compile() Rust builtin to replace the Rust compiler
**So that** the compiler is self-hosted in FMPL with only the assembler remaining in Rust

**Acceptance criteria:**
- AC-1: The FMPL compiler (ast_to_ir.fmpl + ir::compile()) produces identical bytecode for all existing test cases compared to the Rust compiler · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0003`
- AC-2: ir::compile() builtin handles all IR tagged values emitted by ast_to_ir.fmpl · impact:`local` · seam:`integration` · scenario:`SCENARIO-0003`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:206-220`

**Status:** pending

## STORY-0006

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Route AST through ast_to_ir.fmpl tree grammar

**As a** FMPL developer
**I want** AST tagged values to be routed through ast_to_ir.fmpl tree grammar producing IR tagged values
**So that** the FMPL compiler pipeline replaces the Rust compiler's AST-to-bytecode path

**Acceptance criteria:**
- AC-1: AST tagged values from the parser are passed through ast_to_ir.fmpl tree grammar, producing IR tagged values (:LoadInt, :Add, :Call, etc.) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0003`
- AC-2: IR tagged values from ast_to_ir.fmpl feed into ir::compile() Rust builtin producing execution_tape bytecode · impact:`local` · seam:`integration` · scenario:`SCENARIO-0003`
- AC-3: Compiled bytecode is diffable against Rust compiler output for parity verification · impact:`none` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:210-215`

**Status:** done:ITER-0002

## STORY-0007

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Test ast_to_ir.fmpl parity with Rust compiler

**As a** FMPL language developer
**I want** parity tests that compare Rust compiler output with the FMPL compiler pipeline
**So that** I can verify ast_to_ir.fmpl produces equivalent IR to the Rust compiler for all supported constructs

**Acceptance criteria:**
- AC-1: Parity test for integer literals: run('42') via Rust compiler equals run_fmpl_pipeline('42') via ast::parse -> ast_to_ir.expr -> ir::compile -> code::eval · impact:`none` · seam:`integration`
- AC-2: Parity test for arithmetic: run('1 + 2 * 3') equals run_fmpl_pipeline('1 + 2 * 3') · impact:`none` · seam:`integration`
- AC-3: Parity test for strings: run('"hello"') equals run_fmpl_pipeline('"hello"') · impact:`none` · seam:`integration`
- AC-4: Parity test for let bindings: run('let (x = 42) x + 1') equals run_fmpl_pipeline('let (x = 42) x + 1') · impact:`none` · seam:`integration`
- AC-5: Parity test for if expressions: run('if true then 1 else 2') equals run_fmpl_pipeline('if true then 1 else 2') · impact:`none` · seam:`integration`
- AC-6: Parity test for lambdas: run('let (f = \x x + 1) f(41)') equals run_fmpl_pipeline equivalent · impact:`none` · seam:`integration`
- AC-7: Parity test for lists: run('[1, 2, 3]') equals run_fmpl_pipeline('[1, 2, 3]') · impact:`none` · seam:`integration`
- AC-8: Parity test for maps: run('%{a: 1, b: 2}') equals run_fmpl_pipeline('%{a: 1, b: 2}') · impact:`none` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:63-163`

**Status:** done:ITER-0000

## STORY-0008

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Fix ast_to_ir.fmpl gaps for failing parity tests

**As a** FMPL language developer
**I want** ast_to_ir.fmpl to handle all AST node types that the Rust compiler handles
**So that** the FMPL compiler pipeline produces identical results to the Rust compiler for all core constructs

**Acceptance criteria:**
- AC-1: Each failing parity test has a corresponding rule added to lib/core/ast_to_ir.fmpl following the pattern :NodeType(args...) => :IrOp(transformed_args...) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0016`
- AC-2: All parity tests from Task 4 (integers, arithmetic, strings, let bindings, if expressions, lambdas, lists, maps) pass after fixes · impact:`local` · seam:`integration` · scenario:`SCENARIO-0016`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:165-192`

**Status:** done:ITER-0002 (partial: arithmetic, string, if, let, sequence pass; lambda/list/map/block blocked by grammar engine Star-in-TagMatch limitation)

## STORY-0009

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Expand parity test coverage to full language

**As a** FMPL language developer
**I want** parity tests covering all remaining language features
**So that** ast_to_ir.fmpl is verified against the full FMPL language surface area

**Acceptance criteria:**
- AC-1: Parity tests exist for: while loops, do-while, for loops, try/catch, pattern matching with @, objects, grammars, async <-, spawn, pipe |>, method calls, property access, indexing, slicing, symbols, tagged values · impact:`none` · seam:`integration`
- AC-2: ast_to_ir.fmpl is updated to pass all new parity tests · impact:`local` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:193-208`

**Status:** pending

## STORY-0010

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Single canonical representation — lists everywhere, optimizer integrated, dual-shape eliminated

**As a** FMPL maintainer
**I want** structured data (AST, IR, user constructors) represented by exactly one shape — `Value::List([Symbol(tag), ...children])` — and matched by exactly one pattern syntax — `[:Tag, p1, p2]`
**So that** the self-hosted compiler has a single canonical representation, the optimizer runs against the same shape it emits, and there is no runtime/parser ambiguity between `Value::Tagged` and `Value::List`

**Background:** Today FMPL has two interchangeable shapes for tagged/structured data: `Value::Tagged(tag, children)` and `Value::List([Symbol(tag), ...children])`, plus two parser surfaces: `:Tag(args)` and `[:Tag, args]`. This story collapses both axes to the list-shaped value with list-pattern syntax. The cutover (make the AST pipeline emit/consume lists; integrate the optimizer) and the cleanup (delete the dual representation and the syntax that produces it) are one refactor; splitting them produces a worse interim state than either before or after the full work. They land together.

**Acceptance criteria:**

*Phase A — cutover and optimizer integration:*
- AC-1: `ast::parse` emits list-shaped AST values exclusively. Every `Value::Tagged("X", [...])` previously produced by `expr_to_value` is replaced by `Value::list_node("X", [...])` · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-2: `ir::compile` consumes list-shaped IR values exclusively (no Tagged dispatch path) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-3: `lib/core/ast_to_ir.fmpl` and `lib/core/ast_optimizer.fmpl` are rewritten to list patterns (`[:Binary, :+, expr:l, expr:r] => [:Add, l, r]`). The optimizer keeps INT_MIN-overflow and division/modulo-by-zero guards · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-4: `ast_optimizer.fmpl` runs between `ast::parse` and `ast_to_ir.expr` in `eval_via_fmpl_pipeline`. Pipeline order: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval` · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-5: An end-to-end test verifies an actual fold fires when real `ast::parse` output is fed through the optimizer · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-6: All 55 ast_to_ir parity tests pass with the optimizer enabled · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-7: A TODO comment in `lib/core/ast_optimizer.fmpl` lists the AST node kinds that fall through unchanged (Lambda bodies, Let, Match, Call, List, Map, Block) · impact:`none` · seam:`unit`

*Phase B — burn the bridge:*
- AC-8: `Value::Tagged` enum variant is removed from `fmpl-core/src/value.rs`. All ~349 source-and-test sites that referenced it use `Value::list_node(tag, children)` (producer) or `Value::as_node()` (consumer) · impact:`cross-surface` · seam:`integration`
- AC-9: `Expr::Tagged` AST variant is removed. The parser production for `:Tag(args)` value-constructor syntax is deleted; bare `:foo` symbol literals (`Expr::Symbol`) remain · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0104` · scenario:`SCENARIO-0106`
- AC-10: `Pattern::Constructor(tag, [pats])` is removed. The parser productions for `:Tag(p1, p2)` pattern syntax are deleted; `[:Tag, p1, p2]` list-pattern syntax is the only way to match structured data · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0105` · scenario:`SCENARIO-0106`
- AC-11: Tagged-specific bytecode instructions (`MakeTagged`, `MatchTag`, `ExtractTaggedChild`, `MatchTagged`, `MatchTaggedWithBindings`) are removed (or the surviving ones renamed to reflect list-node semantics) · impact:`local` · seam:`integration`
- AC-12: `Pattern::TagMatch` and its grammar runtime/trampoline handlers are removed; `Pattern::ListMatch` is the only constructor-shape matcher · impact:`local` · seam:`integration` · scenario:`SCENARIO-0105` · scenario:`SCENARIO-0106`
- AC-13: All FMPL stdlib files (`lib/core/*.fmpl`) use list-pattern syntax exclusively — no `:Tag(args)` patterns or constructions remain · impact:`cross-surface` · seam:`integration`
- AC-14: All Rust tests use `Value::list_node` for construction and `value.as_node()` for shape assertions — no `Value::Tagged(...)` literals remain in test code · impact:`local` · seam:`unit`
- AC-15: Full test suite passes with zero `Value::Tagged` references remaining in the repo (`grep -r "Value::Tagged" .` returns no source matches; only documentation references in `docs/` remain) · impact:`cross-surface` · seam:`integration`

**Implementation strategy (transformer-driven, from ITER-0004b 2026-05-08 attempt):**

The bulk rewrite (~349 sites) is mechanical and gets done by two transformers, not by hand. This converts what was a "multi-hour atomic refactor" into "build a tool, run it, verify, then delete the dead code." The roadmap entry for ITER-0004b describes the three phases (build transformers → apply + integrate optimizer → delete dead code) in detail.

1. **ast-grep handles the Rust side.** Already installed at `~/.cargo/bin/ast-grep`. Pattern files at `tools/list-transform/rust-rules/*.yml`:
   - `Value::Tagged(SmolStr::new($TAG), Arc::new(vec![$$$ARGS]))` → `Value::list_node($TAG, vec![$$$ARGS])` — verified working 2026-05-08
   - `if let Value::Tagged($T, $C) = $V { ... }` → `if let Some(($T, $C)) = $V.as_node() { ... }`
   - `match v { Value::Tagged(t, c) if t.as_str() == "X" => ... }` → if-let-chain on `as_node()`
   - Run `ast-grep scan --rule tools/list-transform/rust-rules/*.yml --update-all` repeatedly until idempotent.

2. **A small FMPL transformer handles the FMPL stdlib.** Tree grammar at `tools/list-transform/list_transform.fmpl` rewrites `:Tag(args)` → `[:Tag, args]` for both expressions and patterns. Driver in Rust (~50 lines) walks `lib/**/*.fmpl`. Special-case rules in the transformer:
   - **Trailing comma** for single-element list patterns (`exprs = [expr*:xs,] => xs`) to disambiguate from char classes.
   - **Pair sentinel wrap** (`pair => [:Pair, k_ir, v_ir]`) to prevent the runtime "list-of-lists ⇒ spread" flatten.
   - **List-pattern binding repair** — `[:Tag, name]` → `[:Tag, any:name]` where `name` was a binding (list-pattern bare identifiers are rule references, not bindings).

3. **Add helpers first.** `Value::list_node(tag, children)` and `Value::as_node() -> Option<(&str, &[Value])>` on `Value`. Both transformer outputs depend on these.

4. **Phase B is a natural pause point.** After applying the transformers and integrating the optimizer, the tree is coherent (lists everywhere, but `Value::Tagged` variant still defined and unused). If a session ends, Phase C is a clean follow-on.

5. **Don't try to keep tests green during Phase C deletions.** Get the build green first (drive cargo error count to zero), then run tests.

**FMPL-specific gotchas:**
- **List-pattern bare identifiers are rule references**, not bindings (unlike tag-child patterns). Use `any:n` or `_:n` to bind to any single element, or `expr:l` to recursively transform.
- **Single-element list patterns require trailing comma** to disambiguate from char classes (`exprs = [expr*:xs,] => xs`). The grammar parser's lookahead requires comma or pipe to commit to list-pattern interpretation.
- **Map pair sentinel:** runtime "list-of-lists ⇒ spread" collapse means pair-emitting rules must wrap with a sentinel symbol (`pair = [_:k, expr:v] => [:Pair, [:LoadString, k], v]`) and the consumer must unwrap.

**Sources:**
- ITER-0004b PAR scope review (2026-05-08)
- `lib/core/ast_optimizer.fmpl` (existing optimizer)
- `fmpl-core/src/lib.rs:112` (`eval_via_fmpl_pipeline` integration point)
- `fmpl-core/src/value.rs` (`Value::Tagged` variant; needs deletion)
- `fmpl-core/src/builtins/ast.rs:14` (`expr_to_value`; needs to emit lists)
- `fmpl-core/src/builtins/ir.rs` (`compile_ir`; collapse to list-only dispatch)
- `fmpl-core/src/grammar/runtime.rs:794` (`Pattern::TagMatch` handler; delete)
- `fmpl-core/src/vm.rs:1176, 1195` (`ExtractTaggedChild`/`MatchTag`; delete or rename)

**Status:** Phase A done:ITER-0004c (AC-3 through AC-7 + AC-13). AC-1, AC-2, AC-8, AC-15 already satisfied by ITER-0004b's runtime burn. Phase B (AC-9, AC-10, AC-11, AC-12, AC-14) pending in ITER-0004d.

## STORY-0011

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Retire Rust compiler from main compilation path

**As a** FMPL developer
**I want** the Rust compiler to be removed from the main compilation path and retained only in fmpl-bootstrap for seed generation
**So that** the FMPL compiler pipeline (ast_to_ir.fmpl + ir::compile) is the sole compilation path

**Acceptance criteria:**
- AC-1: Rust compiler.rs is only compiled in fmpl-bootstrap crate · impact:`local` · seam:`integration`

**Sources:**

**Status:** pending

## STORY-0012

**Epic:** EPIC-002 — Compiler Cutover
**Title:** Integrate ast_optimizer.fmpl into bootstrap compiler pipeline

**As a** FMPL developer
**I want** ast_optimizer.fmpl (constant folding, algebraic simplification) to be wired into the bootstrap compilation path
**So that** the self-hosted compiler includes optimization passes that produce correct optimized IR

**Acceptance criteria:**
- AC-1: IR tagged values pass through ast_optimizer.fmpl before reaching ir::compile() · impact:`local` · seam:`integration`
- AC-2: Optimized IR produces identical execution results to unoptimized IR for all parity tests · impact:`cross-surface` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:79-80`

**Status:** consolidated:STORY-0010 (functionally identical — both wire ast_optimizer.fmpl into the pipeline; STORY-0010 is the canonical entry going forward)
