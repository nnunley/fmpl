# EPIC-012 — Grammar Inheritance

**Summary:** Grammar Inheritance
**Stories:** STORY-0051, STORY-0052
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0051

**Epic:** EPIC-012 — Grammar Inheritance
**Title:** Support grammar inheritance with rule override and super calls

**As a** grammar author
**I want** child grammars to inherit parent rules, override them, and call parent rules via super
**So that** grammars can be incrementally extended without duplicating rules

**Acceptance criteria:**
- AC-1: A child grammar declared with `<:` inherits all parent rules · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0040`
- AC-2: A child rule with the same name as a parent rule overrides the parent rule · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0040`
- AC-3: `<super.rule>` in a child grammar calls the parent's version of the rule · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0040`

**Sources:**
- `specs/grammar-system.md:241-248`
- `specs/grammar-system.md:200-208`

**Status:** pending

## STORY-0052

**Epic:** EPIC-012 — Grammar Inheritance
**Title:** Support anonymous grammars and inline grammar extension

**As a** grammar author
**I want** anonymous grammar literals and inline extension of existing grammars
**So that** grammars can be created ad-hoc without naming and existing grammars extended without mutation

**Acceptance criteria:**
- AC-1: `grammar { rule = pattern }` creates an anonymous grammar with the given rules · impact:`local` · seam:`integration` · scenario:`SCENARIO-0049`
- AC-2: `base <: { rule = pattern }` extends base with new rules without mutating base · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0049`

**Sources:**
- `specs/grammar-system.md:250-256`

**Status:** pending
