# Progress

**Phase:** FIX-B
**Task:** 6/6 (DONE)
**Last event:** 2026-05-15T01:30Z — ITER-0005b-FIX-B closed. STORY-0100 AC-2 + AC-6 closed. Sentinel sweep clean (23 pass / 0 fail / 4 skip — same long-standing TBD-row skips as FIX-A). fmpl-persistence 107 passing (+4 vs baseline of 103); fmpl-core 1292 passing (unchanged); clippy --all-features clean; workspace --all-features build clean.

**Iteration:** ITER-0005b-FIX-B — AC-2 + AC-6 evidence-seam closure (one iteration, two ordered ACs).

**Open decisions made by this iteration owner:**

1. **T0-IMPL dispatch**: native-pipeline only (not wrap of `eval()`). The FMPL pipeline routes user source through `ast_to_ir.fmpl` via `eval_via_legacy_parser` on a derived driver string; persisting that `CompiledCode` would stamp the driver string's hash, not the user's source — defeating recovery. Native compile path is what `source_hash` recovery actually needs.
2. **T3 logging vs amend**: chose **option (b) — amend wording**. AC-6 text changed from "logs the recovery attempt" → "the recovery attempt is reflected in `RecoveryStats::recovered_from_source`". Rationale: adding `tracing` pulls a new dep into fmpl-persistence for a debug-only observable; the project pattern at this layer is "stats reflect" via typed counters; both pre-iter PAR reviewers said either is defensible.

**Iterations status — done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), 0005b-FIX-A, **0005b-FIX-B**. **In flight:** none.

**Pending (priority order, FIX-B unblocks ITER-0005c):**

1. **ITER-0005c** — bytecode persistence proof case (next; FIX-B closed STORY-0100 AC-2 + AC-6 which 0005c may consume).
2. ITER-0005b-OBJ — Grammar/Object source_hash threading.
3. ITER-0005b-GC — source store GC keyspace-scan orchestration.
4. ITER-0005b-AST-SLOT — Lambda + Object + Grammar AST slot.
5. ITER-0005b-SYNTH — constructor synthesizer.
6. ITER-0005d — remaining payload classes.
7. ITER-0005e — VM snapshot + tracer substrate.
8. ITER-0005f — feature flag wiring + final polish.
9. ITER-PROCESS-TAGS — project-wide process-tag sweep + structural proof test.

## Discovered follow-up gaps (carried from FIX-A; FIX-B did not close any)

1. fmpl-web `test_multi_session_isolation` Backend(Locked) failure — pre-existing ITER-0005a.6 regression.
2. Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.
3. EPIC-003 "Status: 0/11 done" counter is stale.
4. Process-tag references in `recovery.rs` doc comments (on ITER-PROCESS-TAGS' inventory).

## New follow-up gaps from FIX-B

5. **`save_to_store` `?Sized` relaxation** — FIX-B added `+ ?Sized` to `CompiledCode::save_to_store`'s generic Store bound. If a future iteration needs `&dyn Store` through `ObjectDb::save_to_store` or `ParseState::save_to_store`, those bounds may need the same relaxation. Pre-emptively adding it everywhere is reasonable; not in FIX-B's scope to fan out.
6. **Iteration-log validator regex doesn't match sub-iter naming** — validator uses `## ITER-(\d+)` which collapses `ITER-0005a.1`, `ITER-0005a.2`, ..., `ITER-0005b-FIX-A`, `ITER-0005b-FIX-B` all into a single `ITER-0005` section. Pre-existing validator limitation; surfaced (not introduced) by FIX-B. Tooling fix, low priority.
