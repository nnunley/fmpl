# Progress

**Phase:** ITER-0005b-FIX-B **DONE with follow-up gap iterations** — closing PAR aggregation complete, 2 gap stories spawned.
**Last event:** 2026-05-15T01:55Z — closing PAR (Reviewers A + B, paired adversarial) returned. Reviewer A: CLEAN. Reviewer B: GAPS-FOUND (1 Serious + 1 Minor). PAR aggregation rule applied → take worst severity → verdict **GAPS-FOUND**. Gap iterations ITER-0005b-FIX-B-GAP-1 + GAP-2 added to roadmap.

**Iteration:** ITER-0005b-FIX-B — AC-2 + AC-6 evidence-seam closure (one iteration, two ordered ACs).

## Closing PAR outcome (2026-05-15)

**Aggregated verdict:** GAPS-FOUND.

**Critical:** none.

**Serious (Reviewer B unique — Reviewer A did not probe this dimension):**

- Iterator-during-mutation aliasing in `recover_and_rebind` not stress-tested at cardinality > 1. SCENARIO-0102 covers only N=1; the iterator mutates the store it's iterating; fjall's snapshot semantics under same-keyspace insertion are inherited, not asserted. → **ITER-0005b-FIX-B-GAP-1**.

**Minor (both reviewers; overlap on env-var asymmetry):**

- `eval_persistent` silently ignores `FMPL_USE_FMPL_COMPILER=1` (documented but no warning). Non-blocking.
- Non-UTF-8 source bytes path uncovered (mirror of the existing key test). → **ITER-0005b-FIX-B-GAP-2**.
- Process tags in scenario test file module docs — pre-existing project convention; defer to `ITER-PROCESS-TAGS` sweep. Not a FIX-B-specific finding.

**What both reviewers confirmed (no findings):**

- AC-2 evidence at journey seam: real compile pipeline + source_hash round-trip. Strong.
- AC-6 evidence at cross-surface seam (cardinality=1 only): real recompile + bind-and-execute-to-`Value::Int(3)`. Faithful.
- Native-pipeline-only decision: sound + documented.
- `?Sized` relaxation: load-bearing + safe.
- Sentinel sweep verbatim block: byte-equal to fresh run.
- Test counts: fmpl-persistence 107 (+4), fmpl-core 1292 (unchanged).

## Open decisions made by this iteration owner

1. **T0-IMPL dispatch**: native-pipeline only (not wrap of `eval()`). The FMPL pipeline routes user source through `ast_to_ir.fmpl` via `eval_via_legacy_parser` on a derived driver string; persisting that `CompiledCode` would stamp the driver string's hash, not the user's source — defeating recovery. Native compile path is what `source_hash` recovery actually needs.
2. **T3 logging vs amend**: chose **option (b) — amend wording**. AC-6 text changed from "logs the recovery attempt" → "the recovery attempt is reflected in `RecoveryStats::recovered_from_source`". Rationale: adding `tracing` pulls a new dep into fmpl-persistence for a debug-only observable; the project pattern at this layer is "stats reflect" via typed counters; both pre-iter PAR reviewers said either is defensible.

## Iterations status

**Done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), 0005b-FIX-A, **0005b-FIX-B (with follow-up gaps)**.

**In flight:** none.

**Pending (priority order):**

1. **ITER-0005b-FIX-B-GAP-1** — multi-incompatible-record stress (Serious gap; small; unblocked). NEW from closing PAR.
2. **ITER-0005b-FIX-B-GAP-2** — non-UTF-8 source bytes test (Minor symmetric gap; small; can bundle with GAP-1). NEW from closing PAR.
3. **ITER-0005c** — bytecode persistence proof case (unblocked by FIX-B core closure; touches disjoint surfaces from GAP-1/GAP-2 so can run in parallel).
4. ITER-0005b-OBJ — Grammar/Object source_hash threading.
5. ITER-0005b-GC — source store GC keyspace-scan orchestration.
6. ITER-0005b-AST-SLOT — Lambda + Object + Grammar AST slot.
7. ITER-0005b-SYNTH — constructor synthesizer.
8. ITER-0005d — remaining payload classes.
9. ITER-0005e — VM snapshot + tracer substrate.
10. ITER-0005f — feature flag wiring + final polish.
11. ITER-PROCESS-TAGS — project-wide process-tag sweep + structural proof test.

**TOP PRIORITY for next session start:** Discord-bot demo (slipped 2026-05-14; demo was the scheduled deliverable for the day its timing gate opened). Surface via memory `project_discord_bot_slip_2026_05_15.md` BEFORE resuming the orchestrator on fmpl iterations.

## Discovered follow-up gaps (carried)

1. fmpl-web `test_multi_session_isolation` Backend(Locked) failure — pre-existing ITER-0005a.6 regression.
2. Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.
3. EPIC-003 "Status: 0/11 done" counter is stale (STORY-0099 + STORY-0100 now closed).
4. Process-tag references in `recovery.rs` doc comments + scenario test file module docs (on ITER-PROCESS-TAGS' inventory).
5. **`save_to_store` `?Sized` relaxation** — FIX-B added `+ ?Sized` to `CompiledCode::save_to_store`'s generic Store bound. If a future iteration needs `&dyn Store` through `ObjectDb::save_to_store` or `ParseState::save_to_store`, those bounds may need the same relaxation. Pre-emptively adding it everywhere is reasonable; not in FIX-B's scope to fan out.
6. **Iteration-log validator regex doesn't match sub-iter naming** — validator uses `## ITER-(\d+)` which collapses `ITER-0005a.1`, `ITER-0005a.2`, ..., `ITER-0005b-FIX-A`, `ITER-0005b-FIX-B` all into a single `ITER-0005` section. Pre-existing validator limitation; surfaced (not introduced) by FIX-B. Tooling fix, low priority.
