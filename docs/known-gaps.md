# Known gaps

FMPL is an experimental prototype, and the test suite encodes where the language
is *going* as well as where it *is*. Roughly **180 tests are `#[ignore]`d** — not
because they're broken, but because they pin behavior for features that aren't
finished yet. Every one now carries a machine-readable reason:

```sh
cargo test --workspace                       # ignored tests print their reason
cargo test --workspace -- --ignored          # actually run them (most will fail — that's the point)
FMPL_SCENARIO_LIST_SKIPPED=1 cargo test -p fmpl-core scenario   # list skipped behavior scenarios
```

This file groups those gaps by root cause so the count reads as intent, not
neglect. Counts are approximate and drift as work lands.

## 1. Metacircular parser not yet complete (~120)

The largest bucket. FMPL is self-hosting by design (see
`docs/design-principles.md` DESIGN-001): the canonical parser is generated from
`lib/core/fmpl_parser.fmpl`, but that FMPL grammar doesn't yet cover the whole
language, and the generated pipeline still produces incorrect ASTs for some
constructs. These tests are gated on finishing that work (roadmap ITER-0004c and
the self-compile milestone).

- `fmpl-core/tests/core_prelude.rs` (98) — "fmpl_parser.fmpl grammar not yet ready"
- `fmpl-core/tests/generated_parser_correctness.rs` (2) — `AtInlineBlock`
  conversion missing from the generated-parser postlude
- `fmpl-core/tests/parser_equivalence.rs`, `fmpl_interpreter.rs` — related parser-parity gaps

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

## 6. Intentionally not run by default (2)

Not gaps — excluded from the default run for other reasons.

- `fmpl-core/tests/bootstrap_determinism.rs` — slow / mutates process-global build
  state; run explicitly with `-- --ignored`.

## How to help

Pick a bucket, run its file with `-- --ignored`, and land the feature the tests
describe. The metacircular-parser bucket (#1) is the critical path — most of the
other gaps ease once the FMPL-generated parser is complete.
