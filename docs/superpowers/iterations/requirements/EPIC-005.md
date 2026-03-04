# EPIC-005 — MLIR FFI

**Summary:** MLIR FFI
**Stories:** STORY-0029, STORY-0030, STORY-0031, STORY-0032
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`
**Status:** 0/4 done

## STORY-0029

**Epic:** EPIC-005 — MLIR FFI
**Title:** Implement MLIR FFI builtins for context and module operations

**As a** FMPL developer
**I want** FFI builtins that wrap MLIR C API for creating contexts, parsing modules, and emitting text
**So that** FMPL code can create and manipulate MLIR modules

**Acceptance criteria:**
- AC-1: mlir::context.create() creates an MLIR context accessible from FMPL · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`
- AC-2: mlir::module.parse(ctx, text) parses MLIR text assembly into a module · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`
- AC-3: mlir::module.to_string(module) emits MLIR text from a module · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`
- AC-4: Round-trip: MLIR text -> parse -> emit produces text matching the expected output · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:107-121`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:253-267`

**Status:** pending

## STORY-0030

**Epic:** EPIC-005 — MLIR FFI
**Title:** Implement MLIR FFI builtins for pass manager

**As a** FMPL developer
**I want** FFI builtins for creating pass managers, adding passes, and running pass pipelines
**So that** FMPL code can run MLIR optimization passes

**Acceptance criteria:**
- AC-1: mlir::pass_manager.create(ctx) creates a pass manager · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`
- AC-2: mlir::pass_manager.add_pass(pm, pass) adds a pass to the pipeline · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`
- AC-3: mlir::pass_manager.run(pm, module) executes the pass pipeline on a module · impact:`local` · seam:`integration` · scenario:`SCENARIO-0010`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:114-116`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:253-267`

**Status:** pending

## STORY-0031

**Epic:** EPIC-005 — MLIR FFI
**Title:** Register FMPL lambdas as MLIR passes

**As a** FMPL developer
**I want** mlir::pass.from_lambda() to register a FMPL lambda function as an MLIR pass
**So that** optimization and lowering passes can be written in FMPL rather than C++

**Acceptance criteria:**
- AC-1: mlir::pass.from_lambda(fn) accepts a FMPL lambda and registers it as an MLIR pass · impact:`local` · seam:`integration` · scenario:`SCENARIO-0011`
- AC-2: A registered FMPL lambda pass can transform MLIR operations when run through the pass manager · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0011`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:117`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:139-146`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:265`

**Status:** pending

## STORY-0032

**Epic:** EPIC-005 — MLIR FFI
**Title:** Implement MLIR operation, type, and attribute builtins

**As a** FMPL developer
**I want** FFI builtins for querying, creating, and modifying MLIR operations, types, and attributes
**So that** FMPL passes can inspect and transform MLIR IR programmatically

**Acceptance criteria:**
- AC-1: mlir::operation.* builtins allow querying, creating, and modifying MLIR operations from FMPL · impact:`local` · seam:`integration`
- AC-2: mlir::type.* builtins allow manipulating MLIR types from FMPL · impact:`local` · seam:`integration`
- AC-3: mlir::attribute.* builtins allow manipulating MLIR attributes from FMPL · impact:`local` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:118-120`

**Status:** pending

## STORY-0039

**Epic:** EPIC-009 — MLIR Backend
**Title:** Add mlir-sys dependency behind feature flag

**As a** FMPL compiler developer
**I want** an optional mlir-sys dependency behind an 'mlir' feature flag
**So that** MLIR integration can be developed without affecting users who don't need it

**Acceptance criteria:**
- AC-1: fmpl-core/Cargo.toml has an 'mlir' feature flag with optional mlir-sys dependency · impact:`none` · seam:`unit`
- AC-2: cargo build -p fmpl-core --features mlir compiles successfully with a stub mlir module · impact:`none` · seam:`integration`
- AC-3: cargo build -p fmpl-core (without mlir feature) compiles without mlir-sys · impact:`none` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:436-466`

**Status:** pending


## STORY-0040

**Epic:** EPIC-009 — MLIR Backend
**Title:** Implement mlir::context and mlir::module builtins

**As a** FMPL compiler developer
**I want** FMPL builtins for creating MLIR contexts, parsing modules, and emitting MLIR text
**So that** FMPL programs can construct and manipulate MLIR modules

**Acceptance criteria:**
- AC-1: mlir::context.create() returns an MLIR context value usable from FMPL · impact:`local` · seam:`integration` · scenario:`SCENARIO-0022`
- AC-2: mlir::module.parse(ctx, 'module {}') parses MLIR text into a module value · impact:`local` · seam:`integration` · scenario:`SCENARIO-0022`
- AC-3: mlir::module.to_string(m) converts an MLIR module back to text, returning a Value::String · impact:`local` · seam:`integration` · scenario:`SCENARIO-0022`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:468-499`

**Status:** pending


## STORY-0041

**Epic:** EPIC-009 — MLIR Backend
**Title:** Implement mlir::pass_manager and lambda-based passes

**As a** FMPL compiler developer
**I want** to register FMPL lambdas as MLIR passes via pass_manager
**So that** FMPL programs can define custom MLIR transformation passes

**Acceptance criteria:**
- AC-1: An FMPL lambda can be registered as an MLIR pass via mlir::pass.from_lambda and invoked through mlirPassManagerAddOwnedPass · impact:`local` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:502-521`

**Status:** pending


## STORY-0042

**Epic:** EPIC-009 — MLIR Backend
**Title:** Write ir_to_mlir.fmpl tree grammar

**As a** FMPL compiler developer
**I want** a tree grammar that transforms IR tagged values into MLIR text
**So that** FMPL's IR can be lowered to MLIR for optimized code generation

**Acceptance criteria:**
- AC-1: ir_to_mlir.fmpl exists at lib/core/ir_to_mlir.fmpl as a tree grammar · impact:`local` · seam:`integration` · scenario:`SCENARIO-0023`
- AC-2: Compiling '1 + 2' through ast::parse -> ast_to_ir.expr -> ir_to_mlir.expr produces MLIR text containing arith.constant and arith.addi operations · impact:`local` · seam:`integration` · scenario:`SCENARIO-0023`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:523-566`

**Status:** pending
