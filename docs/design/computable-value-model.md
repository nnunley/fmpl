# Computable Value Model (A₀)

**Status**: Draft
**Date**: 2026-07-22
**Related**: [host-extensibility-seam.md](./host-extensibility-seam.md), [async-streams.md](../../specs/async-streams.md), [persistence.md](../../specs/persistence.md), [design-principles.md](../design-principles.md)

## Overview

A `Value` is a computable item. This design adds one arm — `Value::Deferred(Arc<dyn Computation>)` —
representing a value that is a not-yet-forced computation, and a **trampolined**
`Computation` interface that the evaluator drives one bounded step at a time. Over
time it **subsumes** today's special-purpose deferred variants (`Partial`, `Stream`,
`AsyncStream`, `SuspendedStream`, `ParseStream`) into that single seam.

This is the foundation (**A₀**) beneath the host-extensibility work: streams (**A₁**)
become one shape of `Computation`, and the builtin registry (**A₂**) passes and
returns `Value`s that may be `Deferred`. It is also what makes async work without a
runtime, no_std/wasm forcing work with a bounded stack, and stream/promise/parse
persistence collapse into one mechanism.

## Motivation

The value model already *is* a computation graph — it just expresses it as a handful
of special cases instead of one idea. At HEAD, `Value` contains:

- `Lambda`, `Partial` — a partial application is a **computation awaiting more
  arguments** (a thunk in all but name).
- `Code(Arc<CompiledCode>)`, `Grammar` — compiled bytecode and a parser:
  **computations as values**.
- `Stream` (lazy source + deferred ops), `AsyncStream` (a future of a sequence),
  `SuspendedStream`, `ParseStream` — **suspended/deferred sequences**.

And the grammar runtime already has the machinery this design generalizes: a
trampoline (`grammar/trampoline.rs`, the `trampolined-grammar` feature described as
*"the bounded-stack alternative on wasm"*) that does *"Suspend: serialize state to
Fjall,"* memoized continuations, and explicit continuation points. That is
cooperative forcing with serializable continuations — implemented once, for grammars.

Three things fall out of making this the nature of `Value` rather than five special
cases plus a grammar-only trampoline:

- **Async without a runtime.** A deferred value is driven by the evaluator/event
  loop pumping `step`, not by tokio. On native the driver can loop to completion; on
  wasm the browser event loop pumps across yields. Same code, both targets — and it
  retires the `recv_blocking` busy-poll (`std::thread::sleep`) that only works with
  native threads.
- **no_std / wasm.** Bounded-stack, step-at-a-time forcing is exactly what a
  single-threaded, no-runtime target needs — the same reason `trampolined-grammar`
  exists.
- **One persistence mechanism.** A suspended computation's continuation *is* its
  serializable snapshot. Streams, promises, and parse state persist by the same
  path the grammar trampoline already uses.

## Design

### `Value::Deferred`

```rust
pub enum Value {
    // … ready data: Null, Bool, Int, Float, String, Symbol, List, Map, Object, …
    Deferred(Arc<StreamCell<dyn Computation>>),
}
```

- Only `Value::Deferred` carries forcing cost. Ready values (`Int`, `List`, …) pay
  nothing — the driver forces a value **only when a concrete value is demanded**
  (arithmetic, comparison, pattern match, field access, etc.), i.e. to weak-head
  normal form on demand. This keeps the hot path free.
- `StreamCell<T>` is the interior-mutability alias (`Arc<Mutex<T>>` today) shared with
  A₁, so B can swap the lock for a no_std-compatible one in one place.

### `Computation` — trampolined

```rust
pub trait Computation: Send + Sync {
    /// Advance one bounded step. Either finishes with a value, or hands back a
    /// continuation to resume later.
    fn step(&mut self, ctx: &mut StepCtx) -> Step;

    /// Serializable snapshot of the current continuation (durable suspension).
    /// This is the same snapshot the A₁ StreamProvider exposes.
    fn snapshot(&self) -> ComputationSnapshot;
}

pub enum Step {
    /// Computation is complete.
    Done(Value),
    /// Emitted a sequence element; resume for more. (streams — see A₁)
    Yield(Value, Continuation),
    /// Suspended, awaiting IO or a sub-value; resume when ready.
    Pending(Continuation),
}
```

- `step` does a **bounded** amount of work and returns — never blocks a runtime,
  never recurses unboundedly. This is what bounds the stack and lets the driver
  interleave/suspend.
- The `Yield` arm is what [A₁](./stream-computation.md) needs: a stream `Computation`
  `Yield`s elements and finally `Done`s; a non-stream computation never yields. An
  async stream `Pending`s (awaiting IO) *between* `Yield`s — that is how async
  elements arrive without a runtime.
- A promise is a `Computation` that `Pending`s until its IO resolves, then `Done`s; a partial
  application `Done`s once fully applied. One interface, several shapes.

### The driver

The VM's evaluation loop becomes a **driver** that, when a concrete value is
demanded from a `Value::Deferred`, pumps `step` until `Done`:

- **Native, synchronous context:** loop `step` to completion (optionally polling an
  IO source between steps). No threads required for pure/lazy computations.
- **Cooperative / wasm context:** pump `step`; on `Pending(awaiting-IO)`, yield to
  the host event loop and resume when the IO is ready. This is the async-without-tokio
  path.

The driver replaces the current split between the synchronous VM loop and the
grammar trampoline — the grammar trampoline becomes one consumer of the driver.

### `StepCtx`

`StepCtx` is what a computation receives while stepping — and it is the same context
the A₂ builtin seam calls its `HostCtx`. It carries exactly:

- **evaluator re-entrancy** — `eval(source/code) -> Value` (this is where
  `io::load`'s re-enter-the-evaluator need from the A₂ pushback lives);
- **the injected `Store`** (persistence);
- **id generation** and any capability-scoped effects (clock, entropy) that B gates.

Keeping `StepCtx` narrow is what lets a `Computation` (or a builtin) be understood
and tested without reaching into VM internals.

### Serialization

`Value::Deferred` serializes via `snapshot()` → `ComputationSnapshot`, a serializable
continuation. This unifies:

- the A₁ `StreamProvider::snapshot()` (Issue [2] resolution) — a stream's snapshot is
  its continuation;
- the existing grammar trampoline's *"serialize state to Fjall"*;
- `SuspendedStream`/`ParseStream` — which become `ComputationSnapshot`s.

Resume rebuilds a `Computation` from its snapshot (through the A₂ registry for
host-backed sources).

## Phased subsume plan

A₀ is **not** a big-bang rewrite. It lands additively, then absorbs the existing
variants one at a time, each behind its current tests:

1. **Phase 1 — additive.** Introduce `Value::Deferred`, `Computation`, `Step`, and the
   driver *alongside* the existing variants. No existing behavior changes; the new
   arm is unused by the stdlib yet. All tests stay green.
2. **Phase 2 — subsume, one variant per step:**
   - `Partial` → a `Computation` awaiting arguments (simplest; no async).
   - `Stream` → a `Computation` yielding a sequence (this is A₁'s landing).
   - `AsyncStream` / `SuspendedStream` → `Computation`s whose `Pending` awaits IO
     (retires `recv_blocking`).
   - `ParseStream` → fold into the driver, unifying with the grammar trampoline.
   Each step deletes a special-case arm and its bespoke handling once its tests pass
   on the unified path.

The end state — one deferred seam — is reached incrementally, never in a single diff.

## Non-goals

- **Not** subsuming all variants at once (see the phased plan).
- **Not** the streams refactor itself (A₁) or the builtin registry (A₂) — those are
  separate docs that build on A₀.
- **Not** the no_std/`std`-feature work (B) or execution_tape (C).
- **No** change to FMPL surface semantics — forcing is transparent to programs.

## Testing

- Phase 1 is behavior-preserving: the full suite stays green with `Value::Deferred`
  present but unused by the stdlib.
- **New proofs:** a `Value::Deferred` forces cooperatively (observable step-by-step,
  bounded stack), and its `Pending` continuation round-trips through
  `snapshot()`/serialization and resumes to the same result. These are the seeds of
  the wasm/no_std and durable-suspension guarantees.
- Each Phase-2 subsume is gated on the migrated variant's existing tests passing on
  the unified path before its old arm is removed.

## Open items (settle during implementation)

- Exact shapes of `Continuation` / `ComputationSnapshot` and how much state a
  `Pending` carries.
- The precise `StepCtx` contract (esp. the `eval` re-entrancy surface).
- Driver integration order with the current synchronous VM loop and the grammar
  trampoline.
- Forcing-point audit: enumerate every site that demands a concrete value so none
  silently skips forcing a `Deferred`.

## Relationship to the rest of the arc

**A₀ (this)** → **A₁** streams-as-`Computation** → **A₂** builtin registry
(`StepCtx` = `HostCtx`) → **B** no_std/wasm (cooperative forcing; IndexedDB/OPFS
`Store`) → **C** execution_tape opt-in. See [ROADMAP.md](../../specs/ROADMAP.md).

## References

- [host-extensibility-seam.md](./host-extensibility-seam.md) — A₁/A₂, to be reframed onto A₀
- `fmpl-core/src/grammar/trampoline.rs` + the `trampolined-grammar` feature — existing cooperative-forcing precedent
- [async-streams.md](../../specs/async-streams.md), [persistence.md](../../specs/persistence.md) — the surfaces A₀ unifies
- [design-principles.md](../design-principles.md) — durable invariants
