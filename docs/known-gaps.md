# Known gaps

> **Ground truth for work remaining.** The actionable checkbox form of this
> analysis lives at [`specs/gaps.md`](../specs/gaps.md) — pick and check off work
> there. This document is the prose root-cause narrative behind that tracker,
> paired with [`specs/ROADMAP.md`](../specs/ROADMAP.md) (the arc), and registered
> in [`specs/README.md`](../specs/README.md).

FMPL is an experimental prototype, and the test suite encodes where the language
is *going* as well as where it *is*. Roughly **75 tests are `#[ignore]`d** — not
because they're broken, but because they pin behavior for features that aren't
finished yet. Every one carries a machine-readable reason:

```sh
cargo test --workspace                       # ignored tests print their reason
cargo test --workspace -- --ignored          # actually run them (most will fail — that's the point)
FMPL_SCENARIO_LIST_SKIPPED=1 cargo test -p fmpl-core scenario   # list skipped behavior scenarios
```

This file groups those gaps by root cause so the count reads as intent, not
neglect. Counts are approximate and drift as work lands.

## 1. Metacircular parser (CLOSED)

Formerly the largest bucket (~120). FMPL is self-hosting by design (see
`docs/design-principles.md` DESIGN-001): the canonical parser is generated from
`lib/core/fmpl_parser.fmpl`.

Issue #4 (2026-07-22) closed it: the generated parser parses
`lib/core/prelude.fmpl` and `lib/core/ast_to_ir.fmpl` end to end (tree/value
patterns, multi-rule grammar bodies, newline-separated top-level statements,
non-ASCII source, `AtInlineBlock` conversion, and full-input enforcement in
`generated_parse`), and the interpreted grammar runtime executes
fmpl_parser.fmpl itself (`"src" @ fmpl_parser.code`).
`bootstrap_determinism.rs`, `core_prelude.rs`, `parser_equivalence.rs`, and
`generated_parser_correctness.rs` have no ignored tests left. Remaining
self-hosting work is tracked by the roadmap (self-compile milestone), not by
ignored tests.

## 2. Pattern-matching completeness (~50)

Pieces of the pattern/grammar system that aren't wired end-to-end: pattern
unification, list-as-stream tree matching, and compilation of specific pattern
shapes (symbol / literal / list / map / nested).

- `fmpl-core/tests/integration_pattern_unification.rs` (22)
- `fmpl-core/tests/integration_polymorphic_streams.rs` (16) — "list-as-stream tree matching not working correctly"
- `fmpl-core/tests/anonymous_patterns.rs` (6)
- `fmpl-core/tests/tool_execution.rs` (10) — `@` pattern-matching on *expressions* (not just grammars); see `specs/pattern-matching.md`

## 3. Optimizer pipeline (2)

The FMPL AST optimizer (`lib/core/ast_optimizer.fmpl`) isn't wired into the
bootstrap compile path yet (still in legacy syntax; roadmap ITER-0004c).

- `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs`, `ast_optimizer_unit.rs`

## 4. Language features with a pending design decision

- **For-loop scope / mutation** (4) — `fmpl-core/tests/for_loop.rs`: a `for` body
  can't mutate an outer binding yet ("mutations not persisting"). Design decision
  pending; workaround is `map`/`fold`.
- **Closures** (2) — `fmpl-core/tests/lambda_closures.rs`: mutable closure capture
  and recursive `let`; see `specs/parser-limitations.md`.
- **`ast_to_ir` parity FOLLOWUP #30** (2) — `fmpl-core/tests/ast_to_ir_parity.rs`:
  `ir::compile` arity check + nested pattern alignment.
- **yield** (1) — `fmpl-core/tests/yield.rs`.

## 5. Web storylet (WIP) (3)

- `fmpl-web/tests/storylet_http.rs` — the `/play` storylet-rendering route is
  in progress; these assert rendered content not yet emitted.

## How to help

Pick a bucket, run its file with `-- --ignored`, and land the feature the tests
describe. With the metacircular-parser bucket (#1) closed, the
pattern-matching bucket (#2) is the largest lever.
