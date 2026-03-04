# EPIC-028 — Exception Handling

**Summary:** Exception Handling
**Stories:** STORY-0086
**Primary sources:** `specs/vm.md`
**Status:** 0/1 done

## STORY-0086

**Epic:** EPIC-028 — Exception Handling
**Title:** Handle exceptions with try/catch unwinding

**As a** bootstrap pipeline
**I want** PushHandler, PopHandler, and Throw to implement cross-frame exception handling
**So that** runtime errors can be caught and handled gracefully

**Acceptance criteria:**
- AC-1: PushHandler registers a catch target instruction index with the current stack/frame depth · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0057`
- AC-2: When Throw executes, the VM unwinds frames to the handler's depth, pushes the error value, and jumps to the catch target · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0057`
- AC-3: PopHandler removes the most recently registered handler on normal (non-exception) exit from a try block · impact:`local` · seam:`integration` · scenario:`SCENARIO-0057`
- AC-4: try { 1 / 0 } catch (e) { 99 } evaluates to 99 · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0057`

**Sources:**
- `specs/vm.md:412-428`

**Status:** pending
