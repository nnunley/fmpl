# EPIC-002 — Compiler Cutover

**Summary:** Compiler Cutover
**Stories:** STORY-0005, STORY-0006, STORY-0007, STORY-0008, STORY-0009, STORY-0010, STORY-0011, STORY-0012
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`, `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md`
**Status:** 3/8 done

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
**Title:** Integrate ast_optimizer.fmpl into bootstrap compilation pipeline

**As a** FMPL developer
**I want** the ast_optimizer.fmpl constant folding pass to be part of the standard compilation pipeline
**So that** the self-hosted compiler includes optimization and optimized code produces identical results

**Acceptance criteria:**
- AC-1: ast_optimizer.fmpl runs between `ast::parse` and `ast_to_ir.expr` in the FMPL pipeline (NOT between ast_to_ir and ir::compile — the optimizer matches AST shapes like `[:Binary, :+, ...]`, not IR shapes like `:LoadInt`/`:Add`). Pipeline order: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval` · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-2: An end-to-end test verifies an actual fold fires when real `ast::parse` output is fed through the optimizer (not just the synthetic list-literal tests in `lib/core/ast_optimizer_test.fmpl`) — proves the Tagged↔List shape mismatch has been resolved · impact:`local` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-3: The optimizer guards against `INT_MIN` overflow in `:Unary(:-, [:Int, a])` and against division/modulo by zero in `:Binary(:/, ...)` and `:Binary(:%, ...)` — these patterns must fall through to the recursive identity case rather than producing incorrect or panicking folds · impact:`cross-surface` · seam:`integration`
- AC-4: All 55 ast_to_ir parity tests pass when the FMPL pipeline runs with the optimizer enabled — optimization preserves execution semantics across the full parity corpus · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0103`
- AC-5: A TODO comment in `lib/core/ast_optimizer.fmpl` lists the AST node kinds that currently fall through `x => x` without folding (Lambda bodies, Let, Match, Call, List, Map, Block) so the coverage gap is visible for a future iteration · impact:`none` · seam:`unit`

**Sources:**
- ITER-0004b PAR scope review (this conversation, 2026-05-08)
- `lib/core/ast_optimizer.fmpl` (existing optimizer)
- `fmpl-core/src/lib.rs:112` (`eval_via_fmpl_pipeline` integration point)

**Status:** pending

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
