# EPIC-030 — Async Support

**Summary:** Async Support
**Stories:** STORY-0088
**Primary sources:** `specs/vm.md`
**Status:** 0/1 done

## STORY-0088

**Epic:** EPIC-030 — Async Support
**Title:** Support async operations with runtime handle

**As a** bootstrap pipeline
**I want** AsyncCall to wrap values in AsyncStream when a tokio runtime handle is available
**So that** async expressions using <- operator execute correctly

**Acceptance criteria:**
- AC-1: Vm::with_runtime(handle) creates a VM capable of async operations · impact:`local` · seam:`integration`
- AC-2: AsyncCall wraps the value at values[target] in an AsyncStream · impact:`cross-surface` · seam:`integration`
- AC-3: MakeStream creates a Stream from the source value at values[source] · impact:`local` · seam:`integration`

**Sources:**
- `specs/vm.md:253-258`
- `specs/vm.md:386-408`

**Status:** pending
