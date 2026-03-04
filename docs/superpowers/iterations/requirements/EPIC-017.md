# EPIC-017 — Compilation

**Summary:** Compilation
**Stories:** STORY-0061
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/1 done

## STORY-0061

**Epic:** EPIC-017 — Compilation
**Title:** Grammar compilation lowers patterns to base IR bytecode

**As a** grammar compiler
**I want** grammar patterns lowered to Indexed RPN bytecode using loops and conditional jumps
**So that** no specialized VM instructions are needed for pattern matching logic

**Acceptance criteria:**
- AC-1: Star(pattern) compiles to a loop with checkpoint, pattern attempt, null check, and zero-length guard · impact:`local` · seam:`integration` · scenario:`SCENARIO-0045`
- AC-2: Choice([p1, p2, ...]) compiles to sequential attempts with checkpoint save/restore between alternatives · impact:`local` · seam:`integration` · scenario:`SCENARIO-0045`
- AC-3: Specialized instructions (MatchStarCharClass, MatchPlusCharClass, etc.) are emitted for common character-level patterns as optimizations · impact:`none` · seam:`unit`

**Sources:**
- `specs/grammar-system.md:325-403`

**Status:** pending
