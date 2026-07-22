# ITER-TUPLE-PERSIST — Durable TupleSpace

**Date:** 2026-05-15
**Status:** delivered (2026-05-16T00:04 EDT)
**Prior:** ITER-0005b (closed 2026-05-14 — content-addressed source store)
**Anticipated by:** `roadmap.md` lines noting `PayloadKind::Tuple = 0x0A`
as reserved for a future durable-tuplespace iteration.
**Bridges:** the `specs/tuplespace.md:111-115` aspirational
`durable: true` design and the `fmpl-persistence::Store` substrate.

**Naming note:** this iteration was authored under the name
`ITER-0005c` initially. The roadmap had already reserved `ITER-0005c`
for the **bytecode persistence proof case** as a planned next step in
the 0005a→f payload-class sweep. To avoid the name collision, this
work was renamed to `ITER-TUPLE-PERSIST` post-implementation — it
sits outside the 0005a/b/c/d/e/f numbered sequence because its scope
(coordination primitive durability) is orthogonal to that sequence's
focus (Store-substrate payload class breadth).

## Why this iteration

The TupleSpace is FMPL's coordination primitive. Today it is pure
RAM: `BTreeMap<(namespace, type, seq), Tuple>` inside an
`Arc<Mutex<...>>`. The design spec at `specs/tuplespace.md:111-115`
documents `durable: true` as the per-tuple opt-in for persistence —
but that's aspirational; no `Tuple` field implements it.

Meanwhile, `fmpl-persistence` shipped a clean `Store` trait
(ITER-0005a.5), a content-addressed `SourceStore` and an envelope
writer (ITER-0005b), and the `Value` enum has full serde coverage
including custom handlers for live resource handles. The substrate
to wire TupleSpace through is in place.

The agentic-stack demo (`demo/scenarios/three_hashes.yaml`) shows the
substrate working live across process restart. The natural next step
is to make the coordination primitive itself persist — closing the
seam between Acts 1 and 2 of that demo.

## Scope decisions

**Honored from spec:**
- Per-tuple `durable: true` flag controls whether each `out` writes
  through to the backing store. In-memory `BTreeMap` stays
  authoritative for queries; the store is a write-through durability
  layer. (Per `roadmap.md`: "pattern-match query layer sits outside
  the Store trait.")

**Added beyond spec:**
- `tuplespace.open(path)` per-space constructor that supplies the
  backing store. The spec's per-tuple `durable: true` requires a
  store to exist — opening one is how that's provisioned. In-memory
  spaces (`tuplespace.new()`) error on `durable: true` (no store to
  write to).

**API break (intentional):** the existing `space.out(type, data)`
two-arg form is replaced with `space.out(%{type:..., data:...,
durable:...?})` taking a single tagged map — matches the spec.
Justified because FMPL is pre-1.0 and we are the only consumers;
3 callsites and 12 internal sites updated in this iteration.

## Story

**STORY-0103: Durable TupleSpace.** A user opens a tuplespace at a
filesystem path; tuples marked `durable: true` survive process
restart; in-memory query semantics are unchanged.

## Acceptance Criteria

1. **AC-1 (PayloadKind):** `fmpl-persistence::PayloadKind` gains
   `Tuple = 0x0A` variant. Existing tests that assert `0x0A` is an
   unrecognized variant are updated.

2. **AC-2 (Tuple serde):** `Tuple` derives `Serialize +
   Deserialize`. Round-trips bytes-identically for the variants used
   by current consumers (Int/Float/String/Symbol/List/Map data).

3. **AC-3 (Per-space store):** `TupleSpace` gains an optional
   `Store`-trait-object backing. `new()` keeps it `None` (purely
   in-memory). New `open(path)` constructor opens a `FjallStore`
   and replays existing tuples into the in-memory `BTreeMap` on
   construction.

4. **AC-4 (Out write-through):** `out(tuple)` writes the tuple to
   the in-memory map (existing behavior) AND, when the tuple is
   `durable: true` and a backing store is present, writes an
   envelope-wrapped record under a composite key
   `namespace || type || seq`. `durable: true` with no backing
   store is a hard error. `durable: false` with a backing store is
   a no-op on the store (in-memory only).

5. **AC-5 (Destructive consume removes from store):** `in` and
   `inp` (the destructive reads) remove the matched record from the
   backing store as well as from the in-memory map. Crash between
   the two is acceptable to leave a phantom on disk; replay-on-open
   tolerates phantoms (future GC iteration cleans them).

6. **AC-6 (Out call shape — spec alignment):** the FMPL surface
   becomes `space.out(%{type: :T, data: ..., durable: true})` per
   spec. The two-arg form `space.out(:T, data)` is removed.

7. **AC-7 (Round-trip scenario):** an integration test under
   `fmpl-core/tests/` (or `fmpl-persistence/tests/`) opens a
   tuplespace at a tempdir, `out`s a durable tuple, drops the space,
   reopens the same path, and `rd`s the tuple back — payload
   byte-identical.

8. **AC-8 (Demo scenario):** `demo/scenarios/durable_tuplespace.yaml`
   under the existing YAML harness: Alice opens, outs durable tuples,
   closes; Bob reopens, rds them back; harness audit-asserts payload
   equality.

## Out of scope

- **Persistent subscriptions.** Subscriptions stay in-memory; they
  follow the existing Stream/Sink Suspended→reconnect pattern as a
  future iteration. Reconnecting a stream across process restart is
  its own design problem.
- **Pattern-match-via-storage.** Queries go through the in-memory
  `BTreeMap`. The roadmap explicitly notes this would need a
  separate abstraction "NOT part of the Store trait."
- **GC for orphaned tuples.** If a destructive `in` writes to the
  store, then crashes before removing the in-memory entry (or vice
  versa), a phantom can result. Replay-on-open tolerates them
  (matching them at read time costs the same as a real match;
  they're invisible to the user). Cleanup is a follow-up iteration.
- **Multi-writer / cross-process simultaneous access.** Fjall's
  single-writer lock applies just as it did for SourceStore. Two
  REPLs sequence; they do not share a live persistent tuplespace.
  Broker design is a separate iteration.
- **Migration from existing in-memory-only spaces.** No migration
  tool. The change in `out` call shape (`space.out(:T, data)` →
  `space.out(%{type: :T, data: ...})`) is a hard API break;
  callsites are updated by hand in this iteration.

## Build sequence

| Task | What |
|------|------|
| T1 | Add `PayloadKind::Tuple = 0x0A`. Update the skip-list test. |
| T2 | Derive `Serialize + Deserialize` on `Tuple`. Round-trip unit tests. |
| T3 | Add `store: Option<Arc<dyn Store + Send + Sync>>` field on `TupleSpace`. Add `TupleSpace::open(path)`. Implement replay-on-open. |
| T4 | Wire `out` write-through when `durable: true`. Error if `durable: true` and no store. |
| T5 | Wire `in`/`inp` destructive-consume to also remove from the store. |
| T6 | Add `tuplespace.open(path)` FMPL builtin in `vm.rs` (alongside `tuplespace.new`). |
| T7 | Update VM dispatch of `space.out` to take a single tagged map. Update all callsites: `demo/tavern.fmpl`, `tuplespace_vm.rs` tests, anything else `grep` finds. |
| T8 | Author the integration test (AC-7) and the harness scenario (AC-8). |

## Invariants to preserve

- In-memory query semantics are unchanged. `rd`, `in`, `inp`, `rdp`
  return what they returned before, given identical input.
- `tuplespace.new()` (no path) continues to work exactly as today —
  no I/O, no store, no surprises.
- The `Store` trait surface stays narrow. Tuple removal piggybacks on
  the existing `FjallStore::keyspace()` escape hatch that
  `SourceStore::compact` already uses; we do not promote `remove` to
  the trait.

## Lessons applied

- **`feedback_ship_infrastructure_with_first_consumer.md`** —
  every piece of new infrastructure here has a concrete consumer
  the day it lands: the demo scenario AC-8 + the integration test
  AC-7 + the natural eventual consumer (any FMPL coordination
  program that wants restart-survivability).
- **`feedback_parity_gates_surface_bugs.md`** — AC-7 (the
  round-trip) is the parity gate. If it fails on first run, the
  fix is in the impl, not in narrowing the test.
- **`feedback_split_iterations_on_reader_writer_asymmetry.md`** —
  reader (replay-on-open) and writer (`out` write-through) are in
  this iteration, but multi-writer (cross-process) is split out
  cleanly.
