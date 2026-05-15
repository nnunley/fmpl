# Workspace (live task state)

## Current task
**No active task.**

Recent closures (2026-05-16):
- ITER-TUPLE-PERSIST closed 00:04 EDT — Durable TupleSpace (STORY-0103) landed end-to-end.
- ITER-0005b-FIX-B-GAP-1 closed via iterative-development orchestrator — cardinality stress evidence for STORY-0100 AC-6.
- ITER-0005b-FIX-B-GAP-2 closed via iterative-development orchestrator — non-UTF-8 source bytes error-path symmetric evidence.

All three audited clean. Sentinel sweep stands at 169 passed, 3 ignored, 0 failed.

## State at `@` (uncommitted working-copy)

Contains everything from the prior commit (ITER-0005b family closed)
PLUS ITER-TUPLE-PERSIST's deliverable, PLUS the agentic-stack demo harness
work (separately discussed below):

ITER-TUPLE-PERSIST (Durable TupleSpace):
- fmpl-persistence/src/schema.rs — PayloadKind::Tuple = 0x0A added
- fmpl-core/src/tuplespace/mod.rs — Tuple gains serde + store_key
- fmpl-core/src/tuplespace/store.rs — TupleSpace::open + write-through + delete-on-consume
- fmpl-core/src/vm.rs — tuplespace.open builtin + out() shape changed to single tagged map
- fmpl-core/tests/tuplespace_vm.rs — rewritten to new out() shape + 3 new error-case tests
- fmpl-core/tests/tuplespace_facet.rs — 1 source-string update
- fmpl-core/tests/durable_tuplespace_round_trip.rs (NEW) — 4 round-trip tests
- demo/tavern.fmpl — out() callsites updated
- docs/superpowers/specs/2026-05-15-iter-tuple-persist.md (the scope doc)

Demo harness (built earlier in this session, not strictly ITER-TUPLE-PERSIST):
- fmpl-cli/src/main.rs — script-friendly REPL mode, .store-* / .fetch / .open-store commands, source-on-let tracking
- fmpl-cli/Cargo.toml — adds fmpl-persistence + serde_json deps
- fmpl-persistence/examples/source_handoff.rs (NEW)
- demo/harness.py (NEW) — YAML-driven multi-REPL harness via pexpect+PopenSpawn
- demo/scenarios/three_hashes.yaml (NEW)
- demo/scenarios/tavern.yaml (NEW)
- demo/scenarios/durable_tuplespace.yaml (NEW — ITER-TUPLE-PERSIST demo)
- demo/transcript-*.txt + demo/typescript-*.bsd — canonical run artifacts
- demo/README.md — documents both demos and harness

## Verification at the closed state

- `cargo build -p fmpl-core --features persistence` clean
- `cargo build -p fmpl-cli` clean
- fmpl-persistence lib tests: 57/57
- fmpl-core lib tests --features persistence: 328/328 + 1 ignored
- fmpl-core integration: tuplespace_vm 8/8, tuplespace_facet 5/5, tuplespace 7/7, durable_tuplespace_round_trip 4/4
- Demo scenarios: three_hashes 34/34, tavern 38/38, durable_tuplespace 34/34 — all 0 failures
- One pre-existing unrelated test (`canonical_pipeline_must_be_active`) fails because fmpl-bootstrap isn't built — NOT a regression

## Naming note (resolved 2026-05-16)

This iteration was authored under the name `ITER-0005c` initially.
The roadmap had reserved `ITER-0005c` for the planned bytecode-
persistence-proof-case (the next step in the 0005a/b/c/d/e/f
numbered Store-substrate payload-class sweep). The collision was
caught at close-out; the iteration was renamed to
`ITER-TUPLE-PERSIST` (descriptive suffix, sits outside the numbered
sequence because its scope is orthogonal to that sweep).

Scope doc: `docs/superpowers/specs/2026-05-15-iter-tuple-persist.md`

The originally-planned `ITER-0005c` (bytecode persistence proof
case) remains pending; it's still the next numbered iteration in
the 0005a→f sequence per `roadmap.md`.

## Next iteration candidates

By priority:
1. **ITER-TUPLE-PERSIST closing PAR** — dispatch 2 reviewers (lesson from
   0005a.5/0005a.6/0005b). I drove straight through; an outside read
   would catch what I missed.
2. **`rdp` semantics fix** — VM dispatch errors on no-match instead
   of returning Null as the comment claims. ITER-TUPLE-PERSIST's tests work
   around it; a future iteration should fix the surface OR amend the
   comment. Small but real.
3. **TupleSpaceFacet AC-6 unification** — the facet's `out` still
   takes two args; the unification with the new tagged-map shape was
   deliberately deferred this iteration (facets have separate dispatch
   and no durable-write semantics yet).
4. **FMPL FFI for store::source/value/bytecode/fetch** — surfaced
   earlier in this session as deferred; would let scenarios compose
   store ops from inside FMPL code instead of only as REPL dot-
   commands.
5. **ITER-0005b-AST-SLOT** — Lambda + Object + Grammar gain source
   AST slot; unblocks 0005b-SYNTH.
6. **Persistent subscriptions for TupleSpace** — explicitly out of
   ITER-TUPLE-PERSIST scope. Follows the Stream/Sink Suspended→reconnect
   pattern.
7. **Multi-writer / cross-process simultaneous access** — fjall's
   single-writer lock applies. Broker process or alternative backend.
8. **GC for orphaned durable tuples** — partial-failure phantoms
   from interrupted `in` calls.

## Next step (resume instructions)

1. Read `docs/superpowers/iterations/progress.md` for current state
2. Read `docs/superpowers/iterations/iteration-log.md` ITER-TUPLE-PERSIST
   section for narrative + lessons
3. Read `docs/superpowers/specs/2026-05-15-iter-0005c-tuplespace-
   persistence.md` for the scope decisions
4. Dispatch closing PAR on ITER-TUPLE-PERSIST before declaring fully closed
5. Then pick the next iteration from candidates above

Sentinels GREEN. No regression to recover from.
