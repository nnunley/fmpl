# EPIC-029 — Grammar Integration

**Summary:** Grammar Integration
**Stories:** STORY-0087
**Primary sources:** `specs/vm.md`
**Status:** 0/1 done

## STORY-0087

**Epic:** EPIC-029 — Grammar Integration
**Title:** Apply grammars to inputs via GrammarApply instruction

**As a** bootstrap pipeline
**I want** the GrammarApply instruction to apply a grammar to an input value and return the parsed result
**So that** grammar-based parsing can be invoked from compiled bytecode

**Acceptance criteria:**
- AC-1: GrammarApply reads input from values[input] and grammar from values[grammar], applies the named rule, and stores the result at values[ip] · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0063`
- AC-2: GrammarApply supports string, list, and AsyncStream input types · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0063`

**Sources:**
- `specs/vm.md:432-444`

**Status:** pending
