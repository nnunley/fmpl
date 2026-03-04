# EPIC-011 — Grammar Polymorphism

**Summary:** Grammar Polymorphism
**Stories:** STORY-0050
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/1 done

## STORY-0050

**Epic:** EPIC-011 — Grammar Polymorphism
**Title:** Parse any stream type with PEG grammars

**As a** grammar author
**I want** grammars that can parse text, binary, and object streams uniformly
**So that** the same grammar infrastructure supports character parsing, protocol decoding, and AST transformation

**Acceptance criteria:**
- AC-1: A grammar extending base::parser can parse character streams and return matched text · impact:`local` · seam:`integration` · scenario:`SCENARIO-0050`
- AC-2: A grammar extending base::binary can parse byte streams using uint8/uint16/uint32 patterns · impact:`local` · seam:`integration` · scenario:`SCENARIO-0050`
- AC-3: A grammar extending base::tree can match and transform lists, maps, symbols, and typed values · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0050`

**Sources:**
- `specs/grammar-system.md:9-16`
- `specs/grammar-system.md:407-455`

**Status:** pending

## STORY-0097

**Epic:** EPIC-034 — Grammar System
**Title:** Provide OMeta-style PEG grammar engine

**As a** FMPL program
**I want** access to an OMeta-style extensible PEG grammar engine with memoization, backtracking, trampolining, streaming input, and incremental parsing
**So that** grammars can parse any stream type with bounded stack and suspension support

**Acceptance criteria:**
- AC-1: Grammar, GrammarRegistry, Pattern, and Rule types are publicly exported · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-2: Grammar runtime supports memoization (packrat) and backtracking · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-3: Trampoline module provides stack-safe recursion for grammar evaluation · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-4: Grammar engine accepts string and list input sources · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-5: Streaming input supports Fjall overflow for large streams · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-6: Incremental parsing supports ParseState/ParseNext for suspension and resumption · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0067`

**Sources:**
- `specs/fmpl-core.md:19`
- `specs/fmpl-core.md:58-66`

**Status:** pending
