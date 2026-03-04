# EPIC-032 — Value System

**Summary:** Value System
**Stories:** STORY-0095
**Primary sources:** `specs/fmpl-core.md`
**Status:** 0/1 done

## STORY-0095

**Epic:** EPIC-032 — Value System
**Title:** Support all runtime value types

**As a** FMPL program
**I want** the runtime to represent primitives, collections, objects, functions, grammars, streams, and code as Value variants
**So that** all language constructs have a uniform runtime representation

**Acceptance criteria:**
- AC-1: Value enum includes Null, Bool, Int, Float, String, Symbol primitives · impact:`none` · seam:`unit`
- AC-2: List values use Arc<Vec<Value>> for shared ownership · impact:`none` · seam:`unit`
- AC-3: Map values use Arc<HashMap<SmolStr, Value>> for shared ownership · impact:`none` · seam:`unit`
- AC-4: Tagged values carry a SmolStr tag and Arc<Vec<Value>> children for constructor values · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0066`
- AC-5: Grammar values are first-class (Grammar(Arc<Grammar>)) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0066`
- AC-6: Code values are opaque compiled bytecode (Code(Arc<CompiledCode>)) · impact:`local` · seam:`unit` · scenario:`SCENARIO-0066`

**Sources:**
- `specs/fmpl-core.md:90-104`

**Status:** pending
