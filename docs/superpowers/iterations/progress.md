# Progress

**Phase:** FIX-B
**Task:** 2/6
**Last event:** 2026-05-15T00:30Z — T0-IMPL + T1 done. `eval_persistent` lands in `fmpl-core/src/lib.rs` under `#[cfg(feature = "persistence")]`; native-pipeline-only (FMPL pipeline excluded — would stamp driver-string hash, not user source). SCENARIO-0101-eval-persist green (2 tests, both pass). fmpl-core 1292 passing, fmpl-persistence 105 passing (+2). behavior-corpus + behavior-scenarios updated. `save_to_store` Sized bound relaxed to `?Sized` so `&dyn Store` works through the new sibling entry.

**Iteration:** ITER-0005b-FIX-B — AC-2 + AC-6 evidence-seam closure (one iteration, two ordered ACs).

**Open decisions made by this iteration owner:**

1. **T0-IMPL dispatch**: native-pipeline only (not wrap of `eval()`). The FMPL pipeline routes user source through `ast_to_ir.fmpl` via `eval_via_legacy_parser` on a derived driver string; persisting that `CompiledCode` would stamp the driver string's hash, not the user's source — defeating recovery. Native compile path is what `source_hash` recovery actually needs.
2. **T3 logging vs amend**: not yet made (T3 still pending).

**Iterations status — done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), 0005b-FIX-A. **In flight:** **ITER-0005b-FIX-B** (T0-IMPL + T1 done; T2-T6 in progress).

**Pending (priority order after FIX-B):**

1. ITER-0005c — bytecode persistence proof case.
2. ITER-0005b-OBJ — Grammar/Object source_hash threading.
3. ITER-0005b-GC — source store GC keyspace-scan orchestration.
4. ITER-0005b-AST-SLOT — Lambda + Object + Grammar AST slot.
5. ITER-0005b-SYNTH — constructor synthesizer.
6. ITER-0005d — remaining payload classes.
7. ITER-0005e — VM snapshot + tracer substrate.
8. ITER-0005f — feature flag wiring + final polish.
9. ITER-PROCESS-TAGS — project-wide process-tag sweep + structural proof test.

## Discovered follow-up gaps (carried from FIX-A)

1. fmpl-web `test_multi_session_isolation` Backend(Locked) failure — pre-existing ITER-0005a.6 regression.
2. Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.
3. EPIC-003 "Status: 0/11 done" counter is stale.
4. Process-tag references in `recovery.rs` doc comments (on ITER-PROCESS-TAGS' inventory).
