# EPIC-027 — Data Structures

**Summary:** Data Structures
**Stories:** STORY-0085
**Primary sources:** `specs/vm.md`
**Status:** 0/1 done

## STORY-0085

**Epic:** EPIC-027 — Data Structures
**Title:** Construct data structures via MakeList and MakeMap

**As a** bootstrap pipeline
**I want** MakeList and MakeMap instructions to construct lists and maps from indexed element references
**So that** compound data structures can be built from compiled code

**Acceptance criteria:**
- AC-1: MakeList collects values from each InstrIndex in elements into a new List value at values[ip] · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0061`
- AC-2: MakeMap collects key-value pairs from InstrIndex pairs and stores a new Map value at values[ip] · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0061`
- AC-3: Index retrieves a value from a collection using a key, both referenced by InstrIndex · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0061`
- AC-4: Slice extracts a sub-range from a collection with optional start and end InstrIndex references · impact:`local` · seam:`integration` · scenario:`SCENARIO-0061`

**Sources:**
- `specs/vm.md:116-120`

**Status:** pending
