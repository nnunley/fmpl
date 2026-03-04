# EPIC-014 — Pattern Matching

**Summary:** Pattern Matching
**Stories:** STORY-0054, STORY-0055, STORY-0056
**Primary sources:** `specs/fmpl-core.md`, `specs/grammar-system.md`
**Status:** 0/3 done

## STORY-0054

**Epic:** EPIC-014 — Pattern Matching
**Title:** Support PEG combinators for pattern composition

**As a** grammar author
**I want** sequence, ordered choice, repetition, optional, and lookahead combinators
**So that** complex patterns can be composed from simpler ones

**Acceptance criteria:**
- AC-1: Sequence `a b` matches a then b, failing if either fails · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-2: Ordered choice `a / b` tries a first, then b if a fails, with backtracking · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-3: `a*` matches zero or more repetitions of a, always succeeding · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-4: `a+` matches one or more repetitions of a, failing on zero matches · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-5: `a?` matches zero or one occurrence of a, always succeeding · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-6: `&a` positive lookahead succeeds if a matches but does not consume input · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`
- AC-7: `!a` negative lookahead succeeds if a fails and does not consume input · impact:`local` · seam:`unit` · scenario:`SCENARIO-0039`

**Sources:**
- `specs/grammar-system.md:273-282`

**Status:** pending

## STORY-0055

**Epic:** EPIC-014 — Pattern Matching
**Title:** Backtracking with checkpoint save and restore

**As a** grammar runtime
**I want** checkpoint-based backtracking that saves and restores parse position and input stack depth
**So that** ordered choice and repetition patterns can try alternatives and rewind on failure

**Acceptance criteria:**
- AC-1: ParseCheckpoint saves the current (stack_depth, position) tuple · impact:`local` · seam:`unit` · scenario:`SCENARIO-0044`
- AC-2: ParseRestore restores to a previously saved checkpoint, rewinding position and input stack · impact:`local` · seam:`unit` · scenario:`SCENARIO-0044`
- AC-3: Zero-length guard in Star prevents infinite loops when a sub-pattern matches but consumes no input · impact:`local` · seam:`integration` · scenario:`SCENARIO-0044`

**Sources:**
- `specs/grammar-system.md:386-392`
- `specs/grammar-system.md:334-361`

**Status:** pending

## STORY-0056

**Epic:** EPIC-014 — Pattern Matching
**Title:** Provide unified pattern type for let bindings and grammars

**As a** FMPL compiler
**I want** a unified Pattern enum with compilation modes for both let bindings and grammar rules
**So that** pattern matching logic is shared across language features

**Acceptance criteria:**
- AC-1: Pattern type is exported from grammar module and usable in both let bindings and grammar rules · impact:`cross-surface` · seam:`integration`

**Sources:**
- `specs/fmpl-core.md:40-41`
- `specs/fmpl-core.md:83`

**Status:** pending
