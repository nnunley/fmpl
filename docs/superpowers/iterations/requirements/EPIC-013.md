# EPIC-013 — Semantic Actions

**Summary:** Semantic Actions
**Stories:** STORY-0053
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/1 done

## STORY-0053

**Epic:** EPIC-013 — Semantic Actions
**Title:** Bind match results and execute semantic actions

**As a** grammar author
**I want** to bind matched subpatterns to variables and transform them with semantic actions
**So that** grammars can produce structured output values from parsed input

**Acceptance criteria:**
- AC-1: `pattern:name` binds the match result of pattern to variable name, accessible in actions · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-2: `p => expr` evaluates expr with bound variables after p matches, returning the expr result · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`
- AC-3: `&{ expr }` succeeds if expr evaluates to a truthy value, fails otherwise, without consuming input · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0039`

**Sources:**
- `specs/grammar-system.md:290-297`
- `specs/grammar-system.md:230-238`

**Status:** pending
