# EPIC-033 — Streaming Pipeline

**Summary:** Streaming Pipeline
**Stories:** STORY-0096
**Primary sources:** `specs/fmpl-core.md`
**Status:** 0/1 done

## STORY-0096

**Epic:** EPIC-033 — Streaming Pipeline
**Title:** Support stream operations on async streams

**As a** FMPL program using streams
**I want** to apply Map, Filter, FlatMap, Reduce, Parse, and AsyncParse operations on streams
**So that** streams can be transformed and parsed incrementally

**Acceptance criteria:**
- AC-1: StreamOp::Map applies a function value to each stream element · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-2: StreamOp::Filter applies a predicate function value to filter stream elements · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-3: StreamOp::FlatMap applies a function returning a stream and flattens results · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-4: StreamOp::Reduce applies an accumulator function across stream elements · impact:`local` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-5: StreamOp::Parse performs blocking grammar parse on a stream with a grammar value and rule name · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-6: StreamOp::AsyncParse performs incremental grammar parse on a stream · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0067`
- AC-7: Collect, Take, and Drop are NOT implemented as StreamOp variants · impact:`none` · seam:`unit`

**Sources:**
- `specs/fmpl-core.md:106-119`

**Status:** pending
