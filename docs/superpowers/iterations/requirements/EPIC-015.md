# EPIC-015 — Tree Transformation

**Summary:** Tree Transformation
**Stories:** STORY-0057, STORY-0058
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0057

**Epic:** EPIC-015 — Tree Transformation
**Title:** Tree matching with input stack for nested structure descent

**As a** bootstrap pipeline
**I want** grammar rules that descend into nested lists and maps using an input stack
**So that** AST transformation grammars can match and restructure tree-shaped data

**Acceptance criteria:**
- AC-1: ParsePush pushes a value as a new input stream for tree descent · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-2: ParsePop returns to the previous input stream after tree descent · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-3: ListMatch pattern descends into a list value and matches its elements sequentially · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-4: MapMatch pattern matches specific keys in a map value · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-5: SymbolMatch matches a specific symbol value (used for AST node tags) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-6: Apply pattern descends into a value and applies a sub-pattern to it · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`

**Sources:**
- `specs/grammar-system.md:369-403`
- `specs/grammar-system.md:313-322`

**Status:** pending

## STORY-0058

**Epic:** EPIC-015 — Tree Transformation
**Title:** Object and tree pattern matching primitives

**As a** bootstrap pipeline
**I want** patterns to match specific values, types, symbols, lists, and maps in tree input
**So that** AST nodes can be recognized and transformed by tree grammars

**Acceptance criteria:**
- AC-1: MatchValue(v) matches only when the current input equals v · impact:`local` · seam:`unit`
- AC-2: MatchType(t) matches when the current input has type t (null, bool, int, float, string, symbol, list, map) · impact:`local` · seam:`unit`
- AC-3: SymbolMatch(s) matches only when the current input is the specific symbol s · impact:`cross-surface` · seam:`integration`
- AC-4: ListMatch descends into a list and matches its elements against sub-patterns in sequence · impact:`cross-surface` · seam:`integration`
- AC-5: MapMatch checks for the presence of specific keys and matches their values against sub-patterns · impact:`cross-surface` · seam:`integration`

**Sources:**
- `specs/grammar-system.md:313-322`

**Status:** pending
