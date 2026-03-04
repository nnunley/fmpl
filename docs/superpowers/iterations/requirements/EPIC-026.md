# EPIC-026 — Control Flow

**Summary:** Control Flow
**Stories:** STORY-0082, STORY-0083, STORY-0084
**Primary sources:** `specs/vm.md`
**Status:** 0/3 done

## STORY-0082

**Epic:** EPIC-026 — Control Flow
**Title:** Execute control flow with conditional jumps

**As a** bootstrap pipeline
**I want** Jump, JumpIfFalse, and JumpIfTrue instructions to alter the instruction pointer based on indexed condition values
**So that** if/else, loops, and other control flow constructs execute correctly

**Acceptance criteria:**
- AC-1: Jump sets ip to target unconditionally · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0058`
- AC-2: JumpIfFalse sets ip to target when values[cond] is falsy, otherwise advances to next instruction · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0058`
- AC-3: JumpIfTrue sets ip to target when values[cond] is truthy, otherwise advances to next instruction · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0058`

**Sources:**
- `specs/vm.md:96-99`

**Status:** pending

## STORY-0083

**Epic:** EPIC-026 — Control Flow
**Title:** Provide backpatching for forward jump targets

**As a** bootstrap pipeline
**I want** the compiler to emit placeholder jump targets and patch them once the target instruction is known
**So that** forward jumps in if/else and loops compile correctly

**Acceptance criteria:**
- AC-1: emit() returns the InstrIndex of the emitted instruction for later reference · impact:`local` · seam:`unit`
- AC-2: next_index() returns the InstrIndex that will be assigned to the next emitted instruction · impact:`local` · seam:`unit`
- AC-3: patch_jump_target(idx, target) updates the jump target at instruction idx to point to target · impact:`cross-surface` · seam:`unit`

**Sources:**
- `specs/vm.md:337-345`

**Status:** pending

## STORY-0084

**Epic:** EPIC-026 — Control Flow
**Title:** Copy instruction for control flow convergence

**As a** bootstrap pipeline
**I want** Copy instruction to duplicate a value from one instruction index to the current position
**So that** if/else branches can converge their results to a single instruction index

**Acceptance criteria:**
- AC-1: Copy { source } reads values[source] and stores it at values[ip] · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0064`

**Sources:**
- `specs/vm.md:168`

**Status:** pending
