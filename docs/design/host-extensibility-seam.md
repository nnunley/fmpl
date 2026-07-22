# Host Extensibility Seam

**Status**: Draft — being reframed onto A₀
**Date**: 2026-07-22
**Related**: [computable-value-model.md](./computable-value-model.md), [async-streams.md](../../specs/async-streams.md), [persistence.md](../../specs/persistence.md), [vm.md](../../specs/vm.md)

> **Reframe (2026-07-22, post-pushback):** this document predates the
> [computable-value model (A₀)](./computable-value-model.md). The work splits into
> **A₁ — streams** (a stream is a `Computation`; the `StreamProvider` `snapshot()`
> below is a computation continuation) and **A₂ — builtin registry** (`HostCtx`
> becomes the A₀ `StepCtx`, carrying the `io::load` evaluator re-entrancy). Both sit
> on A₀. The registry/seam ideas here stand; the value-model plumbing moves to A₀.
> This doc will be split into A₁ and A₂ specs.

## Overview

A registration seam that lets builtins and plugins provide host implementations —
effectful builtins and stream providers — instead of the VM hard-coding them. The
VM holds a `HostRegistry` (injected the same way it holds a persistence `Store`),
and effectful builtins and stream backends are resolved through it rather than
through `match` arms and hand-rolled `tokio` channels.

This is sub-project **A** of a three-part effort. It is behavior-preserving on its
own, and it is the foundation for **B** (`#![no_std]` fmpl-core) and **C**
(execution_tape as an opt-in VM backend). See [Relationship to B and C](#relationship-to-sub-projects-b-and-c).

## Motivation

Today the VM's host interface is closed and hard-wired:

- **Builtins are hard-coded.** `vm.rs` dispatches effectful builtins through
  literal `match` arms — `CurlBuiltin::get(...)`, `HumanBuiltin::approve(...)`,
  `RandBuiltin::float()`, keyed on `(namespace, name)` string pairs. There is no
  way for a downstream crate (or a capability-restricted build) to add, replace,
  or omit a host builtin.
- **Streams are hand-rolled and tokio-bound.** Each async builtin constructs its
  own `tokio::sync::mpsc` channel plus a `StreamHandle` (see
  `builtins/curl.rs`). The value model carries two parallel stream variants —
  `Value::Stream` (lazy, synchronous) and `Value::AsyncStream` (tokio) — and the
  VM branches on which one it has. Streams cannot be provided by anything other
  than the built-in tokio path.
- **No capability story.** Because the tokio and I/O couplings are woven directly
  into the VM's dispatch, there is no clean place to say "this build has no OS /
  no async runtime; use only the synchronous providers."

The persistence layer already solved the analogous problem with the `Store` trait
(`fmpl-persistence`): a `Send + Sync` trait, dependency-injected as
`Arc<dyn Store>`, with backends (e.g. `FjallStore`) implementing it behind a
feature. This design applies the same pattern to the VM's host surface.

## Design

### `HostRegistry` — the seam

The VM owns a `HostRegistry`, injected at construction exactly like a `Store`. It
holds two maps of registered implementations:

```rust
pub struct HostRegistry {
    /// Effectful builtins, keyed by (namespace, name).
    builtins: HashMap<(SmolStr, SmolStr), Arc<dyn BuiltinFn>>,
    /// Stream provider factories, keyed by StreamSource variant.
    stream_factories: HashMap<StreamSourceKind, Arc<dyn StreamFactory>>,
}
```

`HostRegistry` is populated by composition, not global mutable state. A
`HostRegistry::full()` constructor registers the standard std-backed set;
capability-restricted callers compose a smaller registry (see
[Capability gating](#capability-gating)). Registration happens once, at VM setup;
the registry is then read-only for the VM's lifetime.

### Builtin seam

Effectful builtins implement a single trait and register under their
`(namespace, name)` key:

```rust
pub trait BuiltinFn: Send + Sync {
    fn call(&self, ctx: &mut HostCtx, args: &[Value]) -> Result<Value, VmError>;
}
```

`vm.rs` replaces the hard-coded `match` arms for effectful builtins with a single
dispatch through the registry: `self.registry.call(ns, name, ctx, args)`. If no
builtin is registered for a key, the VM returns the same "unknown builtin" error
it does today.

**Scope:** the seam covers the **effectful / host builtins** — `curl`, `human`,
`io`, `env`, `sse`, `time`, `rand`. Pure-computational operations that are VM
opcodes stay in the VM; they are not host concerns and gain nothing from
indirection.

### Stream seam

Streams become a single provider abstraction. A provider yields events
synchronously; async providers bridge to the runtime internally (the existing
`recv_blocking` adapter):

```rust
pub trait StreamProvider: Send + Sync {
    /// Pull the next event. Synchronous surface; async impls block internally.
    fn next_event(&mut self) -> Option<StreamEvent>;
    /// Durable-suspension metadata, for serialization and resume.
    fn source_meta(&self) -> &StreamSource;
}

/// Mints one provider per stream (unlike Store, which is a single shared instance).
pub trait StreamFactory: Send + Sync {
    fn create(&self, source: &StreamSource, ctx: &HostCtx) -> Box<dyn StreamProvider>;
}
```

Two providers ship built-in:

- **`ThunkProvider`** — synchronous, `alloc`-only. Yields from a lazily-iterated
  collection `Value` or a preloaded event buffer. This is the future no_std path.
- **`AsyncStreamProvider`** — wraps the existing tokio `StreamHandle`. This is the
  std path; it is where all `tokio`/`mpsc` coupling is confined.

The effectful stream builtins (`curl`, `human`, `sse`) register a `StreamFactory`
keyed by the `StreamSource` variant they produce (`HttpGet`, `HumanApproval`,
etc.). **Both** fresh creation and `SuspendedStream` resume/reconnect resolve
their provider through the registry — there is one dispatch path, not a
construct-here / resume-there split.

### Value unification

`Value::AsyncStream` is removed. All streams are:

```rust
Value::Stream(Arc<StreamCell<Box<dyn StreamProvider>>>)
```

- The lazy `StreamOp` pipeline (`Map`/`Filter`/`FlatMap`/`Take`/…) is unchanged
  and runs over whatever the provider yields — it is already backend-agnostic.
- `Value::SuspendedStream(StreamSource)` and `StreamSource`/`SinkSource`
  serialization are preserved; resume rebuilds a provider through the registry.
- `StreamCell<T>` is a type alias — `Arc<Mutex<T>>` today. It exists so sub-project
  B can swap the lock for a no_std-compatible one in a single place.

### `HostCtx` boundary

`HostCtx` is the capability handle passed to `BuiltinFn::call` and
`StreamFactory::create`. It exposes exactly what host implementations legitimately
need — creating streams through the registry, access to the injected `Store`, ID
generation — and nothing else. Keeping this boundary narrow is what lets a builtin
be understood and tested without reaching into VM internals.

### Capability gating

The registry *is* the capability gate. Rather than scattering
`#[cfg(not(target_arch = "wasm32"))]` through the VM, a build registers only the
providers and builtins its target supports:

- **Full (std):** `HostRegistry::full()` registers `ThunkProvider`,
  `AsyncStreamProvider`, and the tokio-backed `curl`/`human`/`sse` factories plus
  the effectful builtins.
- **Minimal / restricted:** register only `ThunkProvider` and the pure builtins.
  Async/I/O host concerns are simply absent — no conditional compilation in the
  VM's dispatch.

This is the mechanism sub-project B uses to make the async/I/O surface `std`-only
without threading `cfg` attributes through call sites.

## Non-goals (this sub-project)

- **No dynamic plugin loading.** "Plugin" means Rust-level registration — in-tree
  builtins and downstream crates that build a `HostRegistry`. No `.so`/`dylib`
  loading, no ABI.
- **No `std` feature / no_std work.** That is sub-project B. This design only makes
  the async/I/O surface a single registrable thing B can gate by composition.
- **No `Store` migration.** Persistence stays injected as `Arc<dyn Store>`; the
  registry could subsume it later, but not here.
- **No FMPL syntax or builtin-semantics change.** Purely a change to how host
  implementations are wired.

## Testing

- **Behavior preservation:** the existing stream and builtin test suites
  (map/filter/pipe/take, curl/human/sse, tuplespace streams) stay green — this is
  a refactor, not a behavior change.
- **Serialization round-trip:** `SuspendedStream` resume through the registry
  produces the same behavior as today.
- **New — synchronous provider proof:** a test drives a stream end-to-end through
  `ThunkProvider` with zero tokio on the path, and a minimal `HostRegistry`
  (thunk + pure builtins only) evaluates a representative program. This is the
  seed of the no_std proof and the concrete evidence that the async surface is
  fully confined behind the registry.

## Relationship to sub-projects B and C

- **B — `#![no_std]` fmpl-core.** Builds directly on this seam: async/I/O host
  concerns are already registry-composed, so B's `std` feature gates *which
  registry you build* rather than editing dispatch. B additionally handles the
  non-stream std couplings (`HashMap` hasher → `hashbrown`, `Mutex` → no_std lock
  via `StreamCell` and peers, `time`, direct io/fs).
- **C — execution_tape opt-in.** Independent; a small feature wiring the existing
  `cross_compile.rs` optional dependency. Unaffected by this seam.

## Examples

Registering a custom builtin and stream provider from a downstream crate:

```rust
let mut registry = HostRegistry::full();
registry.register_builtin("metrics", "emit", Arc::new(MetricsEmit::new(sink)));
registry.register_stream_factory(StreamSourceKind::WebSocket, Arc::new(WsFactory));
let vm = Vm::with_registry(store, registry);
```

A capability-restricted (future no_std) registry — only synchronous providers:

```rust
let mut registry = HostRegistry::minimal(); // ThunkProvider + pure builtins
// no curl/human/sse, no AsyncStreamProvider — async host surface simply absent
let vm = Vm::with_registry(store, registry);
```

From FMPL, nothing changes — the same surface dispatches through the registry:

```fmpl
let events = stream { space.match(%{type: :log}) }
events |> filter(|e| e.level == :error) |> handle
```

## References

- [async-streams.md](../../specs/async-streams.md) — stream semantics and the pipe operator
- [persistence.md](../../specs/persistence.md) — the `Store` seam this design mirrors
- [vm.md](../../specs/vm.md) — VM dispatch and value model
- `fmpl-persistence/src/store.rs` — `Store: Send + Sync` reference implementation of the pattern
