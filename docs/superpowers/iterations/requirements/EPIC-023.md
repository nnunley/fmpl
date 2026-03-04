# EPIC-023 — Name Resolution

**Summary:** Name Resolution
**Stories:** STORY-0074, STORY-0075
**Primary sources:** `specs/vm.md`
**Status:** 0/2 done

## STORY-0074

**Epic:** EPIC-023 — Name Resolution
**Title:** Resolve variable names at compile time via resolve_names pass

**As a** bootstrap pipeline
**I want** LoadVar instructions to be converted to NameRef instructions pointing directly at Bind instructions during compilation
**So that** runtime scope lookup is eliminated and variable access is O(1)

**Acceptance criteria:**
- AC-1: After resolve_names, LoadVar(name) instructions are replaced with NameRef(bind: InstrIndex) pointing to the corresponding Bind instruction · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0054`
- AC-2: NameRef reads the value from the Bind instruction's stored result at values[bind.0] · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0054`
- AC-3: The resolve_names pass completes in a single O(n) traversal · impact:`none` · seam:`unit`

**Sources:**
- `specs/vm.md:192-208`
- `specs/vm.md:326-333`

**Status:** pending

## STORY-0075

**Epic:** EPIC-023 — Name Resolution
**Title:** Manage lexical scoping with BlockStart, BlockEnd, and Bind

**As a** bootstrap pipeline
**I want** BlockStart/BlockEnd to delimit scope boundaries and Bind to introduce named bindings within scopes
**So that** let bindings are properly scoped and do not leak across block boundaries

**Acceptance criteria:**
- AC-1: BlockStart opens a new scope boundary; variables bound after BlockStart are visible until the matching BlockEnd · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0062`
- AC-2: BlockEnd closes the current scope; bindings introduced since the matching BlockStart are no longer visible · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0062`
- AC-3: Bind stores value at values[ip] and associates name with that instruction index for subsequent NameRef resolution · impact:`cross-surface` · seam:`unit` · scenario:`SCENARIO-0062`

**Sources:**
- `specs/vm.md:122-127`

**Status:** pending
