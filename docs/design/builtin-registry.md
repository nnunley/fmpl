# Builtin & Stream Registry (A₂)

**Status**: Draft
**Date**: 2026-07-23
**Related**: [computable-value-model.md](./computable-value-model.md) (A₀), [stream-computation.md](./stream-computation.md) (A₁), [persistence.md](../../specs/persistence.md)

## Overview

A `HostRegistry` the VM holds — injected the same way it holds a persistence
`Store` — that resolves **effectful builtins** and **stream sources** through
registered implementations instead of hard-coded `match` arms and hand-rolled tokio
channels. Builtins and stream factories are the plugin seam; the registry you
*compose* is the capability gate (this is what [B](./computable-value-model.md#relationship-to-the-rest-of-the-arc)
leans on for no_std/wasm).

A₂ sits on [A₀](./computable-value-model.md): a builtin's context is A₀'s `StepCtx`,
its args are `Value`s that may be `Deferred`, and it may *return* a `Deferred`
(a promise or an [A₁](./stream-computation.md) stream).

## Motivation

At HEAD the VM's host surface is closed:

- **Builtins are hard-coded** — `vm.rs` dispatches `CurlBuiltin::get(...)`,
  `RandBuiltin::float()`, etc. through literal `(namespace, name)` `match` arms. No
  downstream crate or restricted build can add, replace, or omit one.
- **Streams are hand-rolled and tokio-bound** — each async builtin builds its own
  `mpsc` + `StreamHandle` (`builtins/curl.rs`). There is no factory seam and no way
  to resume a `SuspendedStream` generically.
- **No capability story** — the tokio/IO couplings are woven into dispatch, so
  there's no clean place to say "this target has no async host; register only the
  synchronous surface."

The `Store` trait already solved the analogous problem for persistence. A₂ applies
the same pattern to the host surface.

## Design

### `HostRegistry`

```rust
pub struct HostRegistry {
    builtins: HashMap<(SmolStr, SmolStr), Arc<dyn BuiltinFn>>,          // (namespace, name)
    stream_factories: HashMap<StreamSourceKind, Arc<dyn StreamFactory>>, // by source variant
}
```

Populated by composition, read-only for the VM's lifetime, injected at construction
(`Vm::with_registry(store, registry)`).

### Builtin seam

```rust
pub trait BuiltinFn: Send + Sync {
    fn call(&self, ctx: &mut StepCtx, args: &[Value]) -> Result<Value, VmError>;
}
```

`vm.rs` replaces the hard-coded arms with `registry.call(ns, name, ctx, args)`;
an unregistered `(ns, name)` yields the same "unknown builtin" error as today.

**Scope:** the effectful/host builtins only — `curl`, `human`, `io`, `env`, `sse`,
`time`, `rand`. Pure-computational operations stay as VM opcodes.

`ctx` is A₀'s `StepCtx`, which is what resolves the pushback's re-entrancy gap:
`io::load` evaluates loaded FMPL source via `ctx.eval(...)`; stream builtins build
their result via `ctx.create_stream(...)`; `rand`/`time` use capability-scoped
effects that a restricted registry can withhold.

```rust
// StepCtx (defined by A0, shared with the driver and A1):
//   ctx.eval(source) -> Result<Value>     // evaluator re-entrancy (io::load)
//   ctx.create_stream(source) -> Value    // stream factory (A1/A2)
//   ctx.store() -> &dyn Store             // persistence
//   ctx.next_id() -> u64                  // id generation
//   ctx.clock()/ctx.rng() (capability-scoped; gated by B)
```

Builtins may **return `Value::Deferred`** — e.g. `curl.get` returns a stream
computation (A₁) or a promise, rather than blocking. The VM composes and forces at
the edge.

### Stream factory seam

```rust
pub trait StreamFactory: Send + Sync {
    /// Build the source computation for a stream of this StreamSource kind.
    fn create(&self, source: &StreamSource, ctx: &StepCtx) -> Box<dyn Computation>;
}
```

`curl`/`human`/`sse` register a factory keyed by the `StreamSource` variant they
produce. **Both** fresh creation and `SuspendedStream` resume resolve their
computation through the registry — one dispatch path, matching the A₁ snapshot/resume
model.

### Args and forcing

Args arrive as `Value`s that may be `Value::Deferred`. Two options for who forces
them (open item, leaning toward the second):

1. **Eager:** the VM forces all args to ready values before `call` — simplest, but
   forfeits laziness at the builtin boundary.
2. **Lazy:** the builtin receives possibly-`Deferred` args and forces only what it
   needs via the driver exposed on `ctx` — enables pipelining (e.g. a builtin that
   forwards a stream without materializing it). Preferred; matches the A₀ intent.

### Capability gating

The registry is the gate:
- `HostRegistry::full()` (std) registers the tokio-backed `curl`/`human`/`sse`
  factories + all effectful builtins + `clock`/`rng`.
- `HostRegistry::minimal()` (no_std/restricted) registers only pure builtins and the
  synchronous stream surface. Async/IO host concerns are simply absent — no `#[cfg]`
  in the VM's dispatch.

## Phased migration (behavior-preserving)

1. Introduce `HostRegistry` + `BuiltinFn` + `StreamFactory` alongside the existing
   `match` dispatch. No behavior change.
2. Port each of the 7 builtins to `BuiltinFn` and register it; switch its `vm.rs`
   arm to registry dispatch, one builtin at a time, its tests green before the arm
   is removed. (`io::load` lands with the `ctx.eval` re-entrancy; `curl`/`human`/`sse`
   land as `StreamFactory`s once A₁'s stream computations exist.)
3. Remove the residual hard-coded arms once all 7 route through the registry.

Depends on A₀ (for `StepCtx`/`Value::Deferred`) and A₁ (for the stream factories'
return type).

## Non-goals

- **No dynamic library / `.so` plugin loading.** "Plugin" = Rust-level registration
  (in-tree builtins + downstream crates composing a `HostRegistry`).
- Not A₀ (value model) or A₁ (streams).
- Not folding `Store` into `HostRegistry` — persistence stays injected as
  `Arc<dyn Store>` (could join later).
- No FMPL syntax or builtin-semantics change.

## Testing

- Behavior preservation: every existing builtin/stream test stays green; each
  builtin's tests pass on the registry path before its `match` arm is deleted.
- A `HostRegistry::minimal()` evaluates a representative pure program with no
  effectful builtins registered (the no_std seed).
- `io::load` re-entrancy through `ctx.eval` produces the same result as today's
  closure path; `SuspendedStream` resume routes through a registered factory.

## Open items

- The exact `StepCtx` contract (defined in A₀; enumerated here per builtin so the
  port has no surprises).
- Eager vs lazy arg forcing (see above) — pick lazy unless a hot-path cost shows.
- `VmError` surface for builtin failures vs the current error path.

## References

- [computable-value-model.md](./computable-value-model.md) — A₀ (`StepCtx`, `Value::Deferred`)
- [stream-computation.md](./stream-computation.md) — A₁ (stream computations the factories return)
- `fmpl-persistence/src/store.rs` — the `Store` seam this mirrors
- `fmpl-core/src/vm.rs` (hard-coded builtin arms), `fmpl-core/src/builtins/` (the 7 builtins)
