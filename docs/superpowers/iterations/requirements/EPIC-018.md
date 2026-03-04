# EPIC-018 — Grammar Infrastructure

**Summary:** Grammar Infrastructure
**Stories:** STORY-0062, STORY-0063
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0062

**Epic:** EPIC-018 — Grammar Infrastructure
**Title:** GrammarRegistry manages named grammars with built-in base grammars

**As a** grammar user
**I want** a registry that stores named grammars and auto-registers built-in base grammars
**So that** grammars can reference each other by name and inherit from base grammars

**Acceptance criteria:**
- AC-1: GrammarRegistry automatically registers base::parser, base::binary, and base::tree on construction · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0046`
- AC-2: register() adds a grammar to the registry, retrievable by get() using its name · impact:`local` · seam:`unit` · scenario:`SCENARIO-0046`
- AC-3: Grammar inheritance resolution uses the registry to look up parent grammars by name · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0046`

**Sources:**
- `specs/grammar-system.md:509-517`

**Status:** pending

## STORY-0063

**Epic:** EPIC-018 — Grammar Infrastructure
**Title:** GrammarParser parses grammar definition syntax

**As a** grammar system
**I want** a parser that turns grammar definition source text into Grammar structs
**So that** grammars defined in FMPL syntax are usable by the runtime

**Acceptance criteria:**
- AC-1: GrammarParser::parse() parses named grammar definitions with optional parent, producing a Grammar struct · impact:`local` · seam:`integration`
- AC-2: GrammarParser::parse_anonymous() parses anonymous grammar blocks (braces only, no name) · impact:`local` · seam:`integration`
- AC-3: Rules within a grammar can be separated by semicolons or commas · impact:`local` · seam:`unit`
- AC-4: Alternatives within a rule use the `|` separator · impact:`local` · seam:`unit`

**Sources:**
- `specs/grammar-system.md:519-528`
- `specs/grammar-system.md:210-229`

**Status:** pending
