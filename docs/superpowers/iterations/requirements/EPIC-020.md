# EPIC-020 — Grammar Application

**Summary:** Grammar Application
**Stories:** STORY-0066, STORY-0067
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0066

**Epic:** EPIC-020 — Grammar Application
**Title:** Apply grammar to input via @ operator

**As a** FMPL programmer
**I want** to apply a grammar rule to input using the `@` operator syntax
**So that** grammars integrate naturally with the language for parsing and transformation

**Acceptance criteria:**
- AC-1: `"input" @ grammar.rule` parses the input string using the named grammar rule and returns the result · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0048`

**Sources:**
- `specs/grammar-system.md:206-208`

**Status:** pending

## STORY-0067

**Epic:** EPIC-020 — Grammar Application
**Title:** Convenience parse functions for common use cases

**As a** grammar user
**I want** convenience functions for parsing text, parsing fully, and applying grammars to values
**So that** common grammar application patterns are easy to use without manual runtime setup

**Acceptance criteria:**
- AC-1: parse(text, registry, grammar_name, rule) parses text with a named grammar rule · impact:`local` · seam:`unit` · scenario:`SCENARIO-0052`
- AC-2: parse_full(text, registry, grammar_name, rule) parses and fails if the entire input is not consumed · impact:`local` · seam:`unit` · scenario:`SCENARIO-0052`
- AC-3: apply_grammar_to_value(value, grammar, registry, rule) polymorphically applies a grammar rule to any value type · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0052`

**Sources:**
- `specs/grammar-system.md:569-579`

**Status:** pending
