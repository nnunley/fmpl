# Progress

## RESUME INSTRUCTIONS FOR NEXT SESSION

A previous session left the orchestrator at a clean iteration boundary. To resume autonomous iterative-development:

1. Invoke the `iterative-development:iterative-development` skill (or its sub-skills directly).
2. The orchestrator reads `roadmap.md` to find the next pending iteration. Current next-pending sequence:
   1. **ITER-0005c** — bytecode persistence proof case. Real iteration (~60-90 min).
   2. **ITER-0005d, 0005e, 0005f** — broader payload-class sweep + VM snapshot + feature flag wiring.
   3. **ITER-PROCESS-TAGS** — low priority sweep.
3. The autonomous loop runs all pending iterations in order until done. No human checkpoints; PAR catches issues. See `iterative-development.md`'s "Escalation Policy" — catastrophe-only.

Baseline state at resume (2026-05-16 02:48 EDT):
- Sentinel sweep: 26 pass, 0 fail, 4 skip (long-standing TBDs SCENARIO-0012/0013/0020/0021).
- `cargo test -p fmpl-persistence --features fjall-backend`: 110 passed, 17 suites.
- `cargo clippy -p fmpl-persistence --lib --features fjall-backend -- -D warnings`: No issues.
- Pre-existing clippy `collapsible_if` at `fmpl-core/src/tuplespace/store.rs:325` (from ITER-TUPLE-PERSIST, in working tree as uncommitted change). Recorded as carried gap; not a regression from any closed iteration.
- 89 cited stories exist in requirements (citation check OK).
- Four iterations closed cleanly this session: ITER-TUPLE-PERSIST, GAP-1, GAP-2, **SWEEP-ASSERTIONS**. Each audited PAR-clean.

**Phase:** ITER-SWEEP-ASSERTIONS DONE 2026-05-16. Pending: closing PAR aggregation across the broader 0005b-FIX-B family (each individual iteration already audited PAR-clean per its iteration-log entry).
**Last event:** 2026-05-16T02:47 EDT — ITER-SWEEP-ASSERTIONS closed via the iterative-development orchestrator: 9 test bodies strengthened to full 6-counter exhaustion across both integration tier (`recover_and_rebind_unit.rs`) and in-module tier (`recovery.rs::tests`). Pre-iteration PAR caught a convergent Serious finding (in-module asymmetry originally out of scope); scope revised mid-iteration to add SWEEP-B1..B7; pre-iteration PAR round 2 APPROVE. Verification gates: 110 fmpl-persistence tests, sentinel sweep byte-identical to baseline (26/0/4), clippy clean on the files touched.

**Prior phase:** ITER-0005b-FIX-B-GAP-2 DONE 2026-05-16. +1 sentinel test closing STORY-0100 AC-6's error-path symmetric gap.

**Prior phase:** ITER-0005b-FIX-B-GAP-1 DONE 2026-05-16 — SCENARIO-0102 promoted to sentinel, +2 sentinel-cadence stress scenarios.

**Prior phase:** ITER-TUPLE-PERSIST (Durable TupleSpace) DONE 2026-05-16T00:04 EDT — PayloadKind::Tuple = 0x0A, TupleSpace::open(path), per-tuple `durable: true` write-through, in/inp delete-on-consume, FMPL `tuplespace.open` builtin, AC-6 API break to `space.out(%{...})`, integration tests, demo YAML scenario.

**Note on naming:** ITER-TUPLE-PERSIST authored under `ITER-0005c` initially; renamed post-implementation because the roadmap had reserved `ITER-0005c` for the planned bytecode-persistence-proof-case. The descriptive suffix signals orthogonality to the 0005a/b/c/d/e/f numbered sequence. Scope doc: `docs/superpowers/specs/2026-05-15-iter-tuple-persist.md`.

## Earlier phase (preserved)

**Prior phase:** ITER-0005b-FIX-B **DONE with follow-up gap iterations** — closing PAR aggregation complete, 2 gap stories spawned. See iteration-log for full transcript.

## Iterations status

**Done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), 0005b-FIX-A, 0005b-FIX-B (with follow-up gaps), 0005b-FIX-B-GAP-1, 0005b-FIX-B-GAP-2, ITER-TUPLE-PERSIST, **ITER-SWEEP-ASSERTIONS**.

**In flight:** none.

**Pending (priority order):**
1. **ITER-0005c** — bytecode persistence proof case (unblocked).
2. ITER-0005b-OBJ — Grammar/Object source_hash threading.
3. ITER-0005b-GC — source store GC keyspace-scan orchestration.
4. ITER-0005b-AST-SLOT — Lambda + Object + Grammar AST slot.
5. ITER-0005b-SYNTH — constructor synthesizer.
6. ITER-0005d — remaining payload classes.
7. ITER-0005e — VM snapshot + tracer substrate.
8. ITER-0005f — feature flag wiring + final polish.
9. ITER-PROCESS-TAGS — project-wide process-tag sweep + structural proof test.

## Discovered follow-up gaps (carried)

1. fmpl-web `test_multi_session_isolation` Backend(Locked) failure — pre-existing ITER-0005a.6 regression.
2. Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.
3. EPIC-003 "Status: 0/11 done" counter is stale (STORY-0099 + STORY-0100 now closed).
4. Process-tag references in `recovery.rs` doc comments + scenario test file module docs (on ITER-PROCESS-TAGS' inventory).
5. **`save_to_store` `?Sized` relaxation** — FIX-B added `+ ?Sized` to `CompiledCode::save_to_store`'s generic Store bound. If a future iteration needs `&dyn Store` through `ObjectDb::save_to_store` or `ParseState::save_to_store`, those bounds may need the same relaxation.
6. **Iteration-log validator regex doesn't match sub-iter naming** — validator uses `## ITER-(\d+)` which collapses `ITER-0005a.1`, `ITER-0005a.2`, ..., `ITER-0005b-FIX-A`, `ITER-0005b-FIX-B`, `ITER-SWEEP-ASSERTIONS` all into a single section. Pre-existing validator limitation. Low priority.
7. **Pre-existing clippy `collapsible_if` at `fmpl-core/src/tuplespace/store.rs:325`** — from ITER-TUPLE-PERSIST work, in working tree as uncommitted change. Surface for cleanup; not a regression.
8. **`scenario_0102_recover_incompatible.rs::scenario_0102_composes_with_iter_store_for_full_keyspace_coverage`** asserts only 2 of 6 counters — weakest shape in the SCENARIO-0102 family. Surfaced by SWEEP-ASSERTIONS pre-iteration PAR; deferred because the test is at a different seam (journey composition test, not `recover_and_rebind` unit test).
9. **Rebind-side-effect assertion on non-UTF-8 recompile-failure paths.** `recover_and_rebind_counts_non_utf8_{key,source}_as_recompile_failure` do not assert "future-major envelope is still in place at the original key" (the happy-path tests do). Different invariant axis from counter-discrimination. Surfaced by SWEEP-ASSERTIONS pre-iteration PAR.
10. **Helper-extraction refactor opportunity.** Five tests in `recover_and_rebind_unit.rs` share `(tempdir + FjallStore + SourceStore + future_vm + write)` setup. Extracting a shared `setup_orchestrator_fixtures()` helper would reduce repetition. Different axis from assertion-strengthening; surfaced by SWEEP-ASSERTIONS pre-iteration PAR and explicitly out of scope.
