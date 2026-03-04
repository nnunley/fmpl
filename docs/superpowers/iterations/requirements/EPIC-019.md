# EPIC-019 — Streaming

**Summary:** Streaming
**Stories:** STORY-0064, STORY-0065
**Primary sources:** `specs/grammar-system.md`
**Status:** 0/2 done

## STORY-0064

**Epic:** EPIC-019 — Streaming
**Title:** Incremental parsing with suspend and resume

**As a** streaming pipeline
**I want** to suspend parsing when input is exhausted and resume when more arrives
**So that** grammars can parse async streams incrementally without blocking

**Acceptance criteria:**
- AC-1: start(rule_name) initiates parsing and returns a ParseState · impact:`local` · seam:`integration` · scenario:`SCENARIO-0047`
- AC-2: resume(state) returns Match(value) when the rule successfully matches · impact:`local` · seam:`integration` · scenario:`SCENARIO-0047`
- AC-3: resume(state) returns NeedInput(state) when more input is required, with serializable state · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0047`
- AC-4: resume(state) returns End when the input stream is complete · impact:`local` · seam:`integration` · scenario:`SCENARIO-0047`
- AC-5: ParseState is serializable via rkyv for durable persistence · impact:`local` · seam:`unit` · scenario:`SCENARIO-0047`

**Sources:**
- `specs/grammar-system.md:54-87`
- `specs/grammar-system.md:121-141`

**Status:** pending

## STORY-0065

**Epic:** EPIC-019 — Streaming
**Title:** Async stream parsing with ParseDriver

**As a** async pipeline
**I want** a driver that connects async input streams to grammar rules and emits matched values
**So that** LLM output and HTTP chunks can be parsed incrementally as they arrive

**Acceptance criteria:**
- AC-1: ParseDriver collects values from an async stream, runs a grammar rule, and sends matches to an output channel · impact:`cross-surface` · seam:`integration`
- AC-2: Pipeline syntax `stream |> parser.rule |> handler` connects stream source to grammar to handler · impact:`cross-surface` · seam:`integration`
- AC-3: AsyncParse StreamOp variant enables incremental parsing within stream pipelines · impact:`local` · seam:`integration`

**Sources:**
- `specs/grammar-system.md:111-119`
- `specs/grammar-system.md:166-193`

**Status:** pending
