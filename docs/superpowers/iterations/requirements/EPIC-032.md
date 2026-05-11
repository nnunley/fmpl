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
- AC-4: Structured "tagged" data uses the canonical list-shape form `Value::List([Value::Symbol(tag), child1, child2, ...])` per DESIGN-002; introspection via `Value::as_node()` returns `(SmolStr tag, &[Value] children)` for any list whose first element is a Symbol · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0066` · note:`rewritten 2026-05-12 (ITER-0004d.1 T15) — the original AC referenced a Value::Tagged variant deleted in ITER-0004b; the canonical form is now a list whose head is a symbol`
- AC-5: Grammar values are first-class (Grammar(Arc<Grammar>)) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0066`
- AC-6: Code values are opaque compiled bytecode (Code(Arc<CompiledCode>)) · impact:`local` · seam:`unit` · scenario:`SCENARIO-0066`

**Sources:**
- `specs/fmpl-core.md:90-104`

**Status:** pending
