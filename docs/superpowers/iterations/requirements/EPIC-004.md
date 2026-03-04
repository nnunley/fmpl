# EPIC-004 — Self-Compile and Seed

**Summary:** Self-Compile and Seed
**Stories:** STORY-0022, STORY-0023, STORY-0024, STORY-0025, STORY-0026, STORY-0027, STORY-0028
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`, `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md`
**Status:** 0/7 done

## STORY-0022

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** FMPL compiler self-compiles to identical output

**As a** FMPL developer
**I want** the FMPL compiler to compile its own source and produce identical bytecode output
**So that** the system reaches a fixpoint proving correctness of self-hosting

**Acceptance criteria:**
- AC-1: fmpl_compiler.fmpl compiles itself (stage 1) and the output compiles itself again (stage 2), producing identical bytecode (fixpoint) · impact:`journey` · seam:`process-level` · scenario:`SCENARIO-0007`
- AC-2: Seed bytecode is serialized to disk as a build artifact · impact:`local` · seam:`integration` · scenario:`SCENARIO-0007`
- AC-3: Seed bytecode is checked into the repository · impact:`local` · seam:`process-level` · scenario:`SCENARIO-0007`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:237-252`

**Status:** pending

## STORY-0023

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Cold bootstrap from seed bytecode

**As a** FMPL developer
**I want** a cold bootstrap path that loads seed bytecode and produces a working compiler
**So that** the system can be bootstrapped from scratch without a running image

**Acceptance criteria:**
- AC-1: When no Fjall image exists, VM loads seed bytecode from disk and runs the compiler · impact:`journey` · seam:`process-level` · scenario:`SCENARIO-0008`
- AC-2: After cold bootstrap, the compiler compiles itself and populates the Fjall image · impact:`journey` · seam:`process-level` · scenario:`SCENARIO-0008`
- AC-3: After cold bootstrap, a new seed is snapshot to disk · impact:`local` · seam:`process-level` · scenario:`SCENARIO-0008`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:170-186`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:248-252`

**Status:** pending

## STORY-0024

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Seed creation from Rust compiler (Stage 0)

**As a** FMPL developer
**I want** the Rust compiler to compile the FMPL compiler pipeline into serialized seed bytecode
**So that** there is an initial seed for the bootstrap chain

**Acceptance criteria:**
- AC-1: The Rust compiler compiles fmpl_parser.fmpl + ast_to_ir.fmpl + compiler driver into bytecode and serializes it to disk · impact:`local` · seam:`integration` · scenario:`SCENARIO-0014`
- AC-2: The serialized seed bytecode can be loaded by a fresh VM and executed · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0014`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:170-172`

**Status:** pending

## STORY-0025

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Create seed snapshot from current compiler

**As a** FMPL bootstrap developer
**I want** fmpl-bootstrap to support --snapshot and --from-seed flags
**So that** I can create a seed image of the compiler and boot from it without recompiling

**Acceptance criteria:**
- AC-1: fmpl-bootstrap --snapshot <output> creates a VM, loads the FMPL compiler pipeline (prelude.fmpl, fmpl_parser.fmpl, ast_to_ir.fmpl), and snapshots VM state to the output file · impact:`local` · seam:`app-level` · scenario:`SCENARIO-0020`
- AC-2: fmpl-bootstrap --from-seed <seed> creates a VM, restores from the seed file, and makes the FMPL compiler available as loaded grammars/functions · impact:`local` · seam:`app-level` · scenario:`SCENARIO-0020`
- AC-3: Round-trip test: --snapshot then --from-seed -e '1 + 2' produces output 3 · impact:`journey` · seam:`e2e` · scenario:`SCENARIO-0020`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:364-398`

**Status:** pending

## STORY-0026

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Verify self-compilation fixpoint

**As a** FMPL bootstrap developer
**I want** to verify that the FMPL compiler compiles itself to produce identical output as the Rust compiler
**So that** I can confirm the language is truly self-hosting with a stable fixpoint

**Acceptance criteria:**
- AC-1: Stage 0 (Rust compiler compiling parser_generator.fmpl to Rust source) produces identical output to Stage 1 (FMPL pipeline from seed compiling parser_generator.fmpl) · impact:`journey` · seam:`e2e` · scenario:`SCENARIO-0021`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:400-431`

**Status:** pending

## STORY-0027

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Write fmpl_compiler.fmpl compiler driver

**As a** FMPL developer
**I want** a compiler driver written in FMPL that orchestrates the full compilation pipeline
**So that** the self-hosting compiler has a single entry point for self-compilation

**Acceptance criteria:**
- AC-1: fmpl_compiler.fmpl exists and orchestrates: fmpl_parser.fmpl -> ast_to_ir.fmpl -> ir::compile() · impact:`journey` · seam:`integration`
- AC-2: The driver can compile arbitrary FMPL source files passed as arguments · impact:`journey` · seam:`app-level`

**Sources:**

**Status:** pending

## STORY-0028

**Epic:** EPIC-004 — Self-Compile and Seed
**Title:** Write FMPL compiler driver that orchestrates the bootstrap pipeline

**As a** FMPL developer
**I want** a fmpl_compiler.fmpl that orchestrates the full compilation pipeline (parser + ast_to_ir + optimizer + ir::compile)
**So that** the compiler can be invoked as a single FMPL program for self-compilation

**Acceptance criteria:**
- AC-1: fmpl_compiler.fmpl exists and orchestrates fmpl_parser.fmpl, ast_to_ir.fmpl, and ir::compile() into a single callable pipeline · impact:`local` · seam:`integration`
- AC-2: The compiler driver can compile arbitrary FMPL source to bytecode · impact:`journey` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:243`

**Status:** pending
