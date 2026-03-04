# EPIC-006 — FMPL MLIR Backend

**Summary:** FMPL MLIR Backend
**Stories:** STORY-0033, STORY-0034, STORY-0035, STORY-0036
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`
**Status:** 0/4 done

## STORY-0033

**Epic:** EPIC-006 — FMPL MLIR Backend
**Title:** Compile simple FMPL expressions to native code via MLIR

**As a** FMPL developer
**I want** FMPL expressions like 1 + 2 to compile to native code through the MLIR pipeline
**So that** FMPL programs can run as native executables for production performance

**Acceptance criteria:**
- AC-1: The expression '1 + 2' compiles to native code via MLIR and returns 3 when executed · impact:`journey` · seam:`e2e` · scenario:`SCENARIO-0012`
- AC-2: Lambda/closure compilation through MLIR produces working native code · impact:`journey` · seam:`e2e` · scenario:`SCENARIO-0012`
- AC-3: fmpl-opt is an FMPL program (not a C++ tool) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0012`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:268-283`

**Status:** pending

## STORY-0034

**Epic:** EPIC-006 — FMPL MLIR Backend
**Title:** Write ir_to_mlir.fmpl tree grammar for MLIR emission

**As a** FMPL developer
**I want** an ir_to_mlir.fmpl tree grammar that emits fmpl.high MLIR from IR tagged values
**So that** the FMPL compiler can target MLIR for optimization and native code generation

**Acceptance criteria:**
- AC-1: ir_to_mlir.fmpl accepts IR tagged values and emits valid fmpl.high MLIR text · impact:`local` · seam:`integration` · scenario:`SCENARIO-0012`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:273`

**Status:** pending

## STORY-0035

**Epic:** EPIC-006 — FMPL MLIR Backend
**Title:** Define fmpl.high and fmpl.low dialects via IRDL

**As a** FMPL developer
**I want** IRDL definitions for fmpl.high and fmpl.low MLIR dialects
**So that** FMPL-specific operations have proper MLIR dialect definitions without C++ TableGen

**Acceptance criteria:**
- AC-1: fmpl_dialects.fmpl emits valid IRDL text defining fmpl.high operations (object_spawn, grammar_apply, facet_check, async_send, bcom_commit) · impact:`local` · seam:`integration`
- AC-2: fmpl_dialects.fmpl emits valid IRDL text defining fmpl.low operations · impact:`local` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:122-137`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:148-166`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:274`

**Status:** pending

## STORY-0036

**Epic:** EPIC-006 — FMPL MLIR Backend
**Title:** Write FMPL lowering passes from fmpl.high to standard MLIR dialects

**As a** FMPL developer
**I want** lowering passes written in FMPL that transform fmpl.high to fmpl.low to standard MLIR dialects
**So that** FMPL programs can be lowered to LLVM IR for native code generation

**Acceptance criteria:**
- AC-1: Lowering passes transform fmpl.high operations (closures, objects, grammars, pattern match) to fmpl.low equivalents (struct+funcptr, vtable, state machines, decision trees) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0012`
- AC-2: Lowering passes transform fmpl.low operations to standard MLIR dialects (arith, func, scf, memref, llvm) · impact:`local` · seam:`integration` · scenario:`SCENARIO-0012`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:148-166`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:276`

**Status:** pending
