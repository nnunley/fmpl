# FMPL Roadmap

**Status**: Living document
**Updated**: 2026-07-22

The authoritative, lean view of where FMPL is and what's next. For the detailed
per-feature gap breakdown (which `#[ignore]`d tests pin which unfinished
behavior), see [`docs/known-gaps.md`](../docs/known-gaps.md). For durable design
invariants, see [`docs/design-principles.md`](../docs/design-principles.md).

> This replaces the retired iterative-development roadmap
> (`docs/superpowers/iterations/`), which the project rehab removed. All planning
> lives here now.

## Foundation (done)

- **Metacircular parser — closed.** The canonical parser is generated from
  `lib/core/fmpl_parser.fmpl` (stage-0 Rust parser → stage-1 generated parser
  cutover per `DESIGN-001`). The generated parser parses the stdlib
  (`prelude.fmpl`, `ast_to_ir.fmpl`) end to end, and the interpreted grammar
  runtime runs `fmpl_parser.fmpl` on itself. `bootstrap_determinism`,
  `generated_parser_correctness`, `parser_equivalence`, and `core_prelude` have
  no ignored tests; `canonical_pipeline_must_be_active` is green.
- **Image persistence.** Extracted `fmpl-persistence` crate behind a `Store`
  trait (fjall backend, native). Content-addressed source store, `recover_and_rebind`
  recovery path, durable `TupleSpace`, and same-process bytecode-persistence
  proof (drop + reopen). `fmpl-core` names no fjall in its regular deps
  (`no_fjall_in_fmpl_core` ratchet).
- **Buildable, shippable project.** Builds without the external `execution_tape`
  crate; wasm REPL (`fmpl-wasm`); GitHub Actions CI; README/LICENSE; doctest
  harness executing the fmpl blocks in TUTORIAL/DEMO/README.

## In progress / next

Sequenced sub-projects (see the design docs under `docs/design/`):

- **A₀ — Computable-value model** *(foundation)*. `Value::Deferred(Arc<dyn Computation>)`;
  a value *is* a computation. Trampolined `Computation` (`step → Done | Yield | Pending`);
  the `Pending`/`Yield` continuation is the serializable snapshot. Subsumes
  `Partial`/`Stream`/`AsyncStream`/`SuspendedStream`/`ParseStream` incrementally.
  Enables async-without-tokio, no_std/wasm cooperative forcing, and unified persistence.
  → [`docs/design/computable-value-model.md`](../docs/design/computable-value-model.md)
- **A₁ — Streams as `Computation`** (on A₀). Folds the stream variants into
  `Value::Deferred`; op pipeline becomes step-combinators; retires the tokio
  busy-poll bridge.
  → [`docs/design/stream-computation.md`](../docs/design/stream-computation.md)
- **A₂ — Builtin & stream registry** (on A₀). A `HostRegistry` the VM holds (injected
  like `Store`) replacing hardcoded builtin dispatch and hand-rolled tokio streams;
  `HostCtx` is A₀'s `StepCtx`. *(Supersedes the earlier combined
  [`host-extensibility-seam.md`](../docs/design/host-extensibility-seam.md).)*
  → [`docs/design/builtin-registry.md`](../docs/design/builtin-registry.md)
- **B — `#![no_std]` fmpl-core.** A default-on `std` feature gates
  async/persistence/io/http/threads/time; sync-thunk stream provider keeps
  streams alive in no_std; `hashbrown` HashMap + no_std lock replace the std
  couplings. Unlocks true wasm/embedded targets. Wasm persistence picks a
  browser `Store` backend (IndexedDB / OPFS sync handles) — the sync↔async
  impedance is B's key design decision. *(spec pending)*
- **C — execution_tape as an opt-in VM backend.** Wire the existing
  `cross_compile.rs` optional dependency into a clean feature flag (default-off,
  no external crate needed for the default build). Independent, small.
  *(spec pending)*

## Open frontier

- **Self-compile milestone.** Parsing is self-hosted; the full compiler
  (AST → IR → bytecode) expressed in FMPL is the next stage of `DESIGN-001`.
- **Optimizer on the bootstrap path (ITER-0004c).** `lib/core/ast_optimizer.fmpl`
  is still in legacy `:Tag(args)` syntax and not wired into the bootstrap compile
  path; migrate it to `[:Tag, …]` and onto the canonical pipeline.
- **Pattern-matching completeness (~50 ignored tests).** `@` matching on
  expressions (not just grammars), list-as-stream tree matching, pattern
  unification. Currently the largest lever — see `known-gaps.md` §2 and
  [`pattern-matching.md`](./pattern-matching.md).
- **Language features (pending design).** For-loop body mutation of outer
  bindings, mutable closure capture / recursive `let`, `yield`.
- **Web storylet.** The `/play` storylet-rendering route (`fmpl-web`) is WIP.

## References

- [`docs/known-gaps.md`](../docs/known-gaps.md) — detailed gap breakdown by root cause
- [`docs/design-principles.md`](../docs/design-principles.md) — durable invariants (`DESIGN-001` metacircular bootstrap)
- [`docs/STANDARDS.md`](../docs/STANDARDS.md) — documentation structure
- [`README.md`](./README.md) — specs index
