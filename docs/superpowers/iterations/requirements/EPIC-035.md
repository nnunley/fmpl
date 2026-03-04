# EPIC-035 — Architecture Constraint

**Summary:** Architecture Constraint
**Stories:** STORY-0098
**Status:** 0/1 done

## STORY-0098

**Epic:** EPIC-035 — Architecture Constraint
**Title:** Define and enforce irreducible Rust kernel boundary

**As a** FMPL developer
**I want** a clear boundary defining which components must remain in Rust permanently
**So that** the self-hosting effort does not attempt to rewrite performance-critical and system-interface components

**Acceptance criteria:**
- AC-1: VM bytecode dispatch (vm.rs), value types (value.rs), async runtime (tokio), I/O builtins, grammar PEG engine, Fjall persistence engine, and MLIR FFI bridge are documented as permanently-Rust components · impact:`none` · seam:`process-level`
- AC-2: No FMPL replacement is attempted for any irreducible kernel component during self-hosting phases · impact:`none` · seam:`process-level`

**Sources:**

**Status:** pending
