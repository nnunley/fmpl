# FMPL Gap Tracker

**Updated**: 2026-07-25

The actionable checkbox form of the gap analysis in
[`docs/known-gaps.md`](../docs/known-gaps.md) — one item per closeable gap,
grouped by root cause. This is **ground truth for work remaining** (registered in
[`README.md`](./README.md), paired with [`ROADMAP.md`](./ROADMAP.md)): the
autonomous loop and contributors pick work off it. Check an item when its
`#[ignore]`d tests pass.

Keep it in sync with reality:

```sh
cargo test --workspace -- --ignored                          # run the ignored tests
FMPL_SCENARIO_LIST_SKIPPED=1 cargo test -p fmpl-core scenario # list skipped scenarios
grep -rn '#\[ignore' --include='*.rs' .                       # the raw list
```

**71 ignored tests** across the buckets below.

## Pattern matching — ~54 tests (the largest lever)

The pattern/grammar system isn't wired end-to-end. See
[`pattern-matching.md`](./pattern-matching.md).

- [ ] **Pattern unification** — `fmpl-core/tests/integration_pattern_unification.rs` (22)
- [ ] **List-as-stream tree matching** — `fmpl-core/tests/integration_polymorphic_streams.rs` (16)
- [ ] **`@` pattern-matching on *expressions*** (not just grammars) — `fmpl-core/tests/tool_execution.rs` (10)
- [ ] **Anonymous patterns** — `fmpl-core/tests/anonymous_patterns.rs` (6)

## Language features — pending design (9 tests)

- [ ] **For-loop body mutation of an outer binding** ("mutations not persisting"; workaround: `map`/`fold`) — `fmpl-core/tests/for_loop.rs` (4)
- [ ] **Mutable closure capture / recursive `let`** — `fmpl-core/tests/lambda_closures.rs` (2); see [`parser-limitations.md`](./parser-limitations.md)
- [ ] **`ast_to_ir` parity FOLLOWUP #30** — `ir::compile` arity check + nested pattern alignment — `fmpl-core/tests/ast_to_ir_parity.rs` (2)
- [ ] **`yield`** — `fmpl-core/tests/yield.rs` (1)

## Optimizer on the bootstrap path — ITER-0004c (3 tests)

`lib/core/ast_optimizer.fmpl` is still in legacy `:Tag(args)` syntax and not wired
into the bootstrap compile path; migrate it to `[:Tag, …]` and onto the canonical
pipeline.

- [ ] **Optimizer pipeline scenario** — `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs` (1)
- [ ] **Optimizer unit** — `fmpl-core/tests/ast_optimizer_unit.rs` (1)
- [ ] **Optimizer integration** — `fmpl-core/tests/optimizer_integration.rs` (1)

## Web storylet — WIP (3 tests)

- [ ] **`/play` storylet rendering** (asserts rendered content not yet emitted) — `fmpl-web/tests/storylet_http.rs` (3)

## Misc (2 tests)

- [ ] **Interpreter path** — `fmpl-core/tests/fmpl_interpreter.rs` (1)
- [ ] **repr** — `fmpl-core/src/repr.rs` (1)

## Closed

- [x] **Metacircular parser** (~120, formerly the largest bucket) — closed by Issue #4 (2026-07-22). The generated parser parses the stdlib end to end and the grammar runtime runs `fmpl_parser.fmpl` on itself. See [`docs/known-gaps.md`](../docs/known-gaps.md) §1 and [`ROADMAP.md`](./ROADMAP.md).
