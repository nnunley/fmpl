# Streams as Computation (A₁)

**Status**: Draft
**Date**: 2026-07-23
**Related**: [computable-value-model.md](./computable-value-model.md) (A₀), [builtin-registry.md](./builtin-registry.md) (A₂), [async-streams.md](../../specs/async-streams.md)

## Overview

A stream is a `Computation` (from [A₀](./computable-value-model.md)) that yields a
sequence of elements. This folds today's three stream flavours — `Value::Stream`
(lazy), `Value::AsyncStream` (tokio), `Value::SuspendedStream` — into the single
`Value::Deferred` seam, so a stream is "transparently async" because the driver
pumps it the same way whether its elements come from a synchronous thunk or an
awaited IO source. The lazy op pipeline (`Map`/`Filter`/`FlatMap`/`Take`/…) becomes
computation combinators; the async busy-poll bridge (`recv_blocking` +
`std::thread::sleep`) is retired.

A₁ depends on A₀ and is the first real consumer of it (the phased-subsume plan's
`Stream` step).

## The one new design point — `Step::Yield`

A₀'s trampolined `Step` is `Done(Value) | Pending(continuation)`. That models a
computation producing **one** value. A stream produces a **sequence**, so stepping a
stream needs a third outcome — emit an element and continue:

```rust
pub enum Step {
    /// Computation finished with its final value.
    Done(Value),
    /// Emitted a sequence element; resume for more. (streams)
    Yield(Value, Continuation),
    /// Suspended (awaiting IO / a sub-value); resume when ready.
    Pending(Continuation),
}
```

**This is a small refinement to A₀** — `Yield` is added to `Step`. Non-stream
computations never yield; stream computations `Yield` repeatedly and finally `Done`
(with, e.g., `Null` or a summary). A consumer that wants the whole sequence pumps
`step` collecting `Yield`s until `Done`; a consumer that wants one element (a cursor
advance) pumps until the next `Yield`. `Pending` is orthogonal: an async stream
`Pending`s (awaiting IO) *between* `Yield`s — that is exactly how async elements
arrive without a runtime.

## Design

### A stream is a `Computation`

```rust
Value::Deferred(Arc<StreamCell<dyn Computation>>)   // a stream is just this
```

Element production is the computation's `step`:
- **Synchronous source** (a collection `Value`, a generator): each `step` computes
  and `Yield`s the next element; `Done` at end. No runtime, no_std-friendly.
- **Async source** (http/human/sse, registered via [A₂](./builtin-registry.md)):
  `step` `Pending`s while its IO is outstanding and `Yield`s when an element
  arrives. The driver/event loop pumps it; on wasm the browser loop drives the
  `Pending → Yield` transitions. No tokio, no `recv_blocking`.

### Op pipeline as combinators

The existing `StreamOp` pipeline (`Map(f)`, `Filter(f)`, `FlatMap(f)`, `Take{n}`,
`Drop{n}`, `Reduce`, `Collect`, `Parse`, `AsyncParse`) becomes **wrapping
computations** over an inner stream computation:

- `Map(f)` steps the inner stream; on `Yield(v)` it `Yield`s `f(v)` (forcing `f(v)`
  if it is itself `Deferred`); passes `Pending`/`Done` through.
- `Filter(f)` steps the inner stream, dropping non-matching `Yield`s (re-stepping),
  passing `Pending`/`Done` through.
- `Take{n}`/`Drop{n}` count `Yield`s. `Collect`/`Reduce` pump to `Done` and return an
  aggregate. `Parse`/`AsyncParse` drive the grammar computation over the element
  stream.

Because each combinator only forces what it emits, laziness and pipelining are
preserved, and the whole pipeline runs in bounded steps.

### Serialization (`snapshot`)

A stream's `snapshot()` (the A₀ `Computation::snapshot`) is its continuation:
source metadata (`StreamSource`) + position + the residual op pipeline. This
subsumes `SuspendedStream` — a suspended stream *is* a serialized stream
computation — and resume rebuilds it through the [A₂](./builtin-registry.md) registry
(the factory for the source kind reconstructs the source computation; the op
combinators wrap back around it). This is the same mechanism the grammar trampoline
already uses.

### Cursors

`Value::Cursor` (CoW observation into a stream) is preserved: a cursor holds a
position and pumps its stream computation to the requested `Yield`, memoizing
emitted elements so multiple cursors observe independently. Cursors are the
read side; they do not change under A₁ beyond pointing at a `Value::Deferred`
stream instead of a `Value::Stream`.

## Phased migration (within A₀'s subsume plan)

A₁ *is* the `Stream` step of A₀'s Phase 2, sequenced as:

1. Add `Step::Yield` (the A₀ refinement) and a generic stream-driving helper on the
   driver. No behavior change yet.
2. Migrate `Value::Stream` (lazy) → a synchronous stream `Computation` + op
   combinators. All existing lazy-stream tests green on the new path, then remove
   the `Value::Stream` arm.
3. Migrate `Value::AsyncStream` / `Value::SuspendedStream` → async stream
   `Computation`s that `Pending` between `Yield`s. Retire `recv_blocking`. Async
   stream + suspension tests green, then remove the arms.

Each step deletes a special-case arm only after its tests pass on the unified path.

## Non-goals

- Not A₀ itself (the value model / driver) or A₂ (the registry) — separate docs.
- Not new stream *sources* (file/websocket as first-class) — they remain
  `StreamSource` + a registered factory.
- No change to FMPL stream syntax or `StreamOp` semantics.

## Testing

- Behavior preservation: the full stream suite (map/filter/pipe/take, http/human/sse,
  tuplespace streams, cursors) stays green across each migration step.
- New: a synchronous stream driven purely through the `Yield` path with **zero
  tokio** on it (the no_std seed), and an async stream whose `Pending → Yield`
  transitions are pumped by a test driver (no `recv_blocking`).
- `snapshot()`/resume round-trip for both a lazy and an async stream.

## Open items

- Exact `Continuation` payload for a stream (source handle vs serialized source +
  op residual).
- Whether `Reduce`/`Collect` (which fully consume) return via `Done` or a distinct
  terminal — likely `Done(aggregate)`.
- Back-pressure semantics for async `Yield` (how far ahead the driver may pump).

## References

- [computable-value-model.md](./computable-value-model.md) — A₀ (`Value::Deferred`, `Step`, driver)
- [builtin-registry.md](./builtin-registry.md) — A₂ (stream factories, `StepCtx`)
- [async-streams.md](../../specs/async-streams.md) — current stream semantics
- `fmpl-core/src/stream.rs`, `fmpl-core/src/value.rs` (`Stream`, `StreamOp`, `Cursor`)
