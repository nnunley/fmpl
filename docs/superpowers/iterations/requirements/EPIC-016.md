# EPIC-016 — Performance

**Summary:** Performance
**Stories:** STORY-0059, STORY-0060
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0059

**Epic:** EPIC-016 — Performance
**Title:** Packrat memoization for grammar rule results

**As a** grammar runtime
**I want** per-position memoization of rule match results
**So that** repeated rule applications at the same position are O(1) and backtracking does not cause exponential behavior

**Acceptance criteria:**
- AC-1: Each StreamPosition maintains a per-position memo table keyed by rule name · impact:`none` · seam:`unit`
- AC-2: A rule applied at a previously-memoized position returns the cached result without re-executing · impact:`local` · seam:`unit` · scenario:`SCENARIO-0051`
- AC-3: Memoization correctly handles both match success and failure entries · impact:`local` · seam:`unit` · scenario:`SCENARIO-0051`

**Sources:**
- `specs/grammar-system.md:23`
- `specs/grammar-system.md:89-108`

**Status:** pending

## STORY-0060

**Epic:** EPIC-016 — Performance
**Title:** Stack-safe grammar execution via trampolining

**As a** grammar runtime
**I want** deeply recursive grammar rules to execute without stack overflow
**So that** complex nested grammars can parse deeply nested input safely

**Acceptance criteria:**
- AC-1: Trampoline execution converts recursive rule calls to continuation-passing style that loops without growing the Rust call stack · impact:`local` · seam:`integration`
- AC-2: BacktrackEntry provides Prolog-style choice points for depth-first search through alternatives · impact:`local` · seam:`integration`

**Sources:**
- `specs/grammar-system.md:609-617`

**Status:** pending
