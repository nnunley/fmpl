# Progress

**Phase:** ITER-0005b-FIX-B starting (sibling-study + pre-iter PAR resolved 2026-05-14; entering implementation).

**Iteration:** ITER-0005b-FIX-B — AC-2 + AC-6 evidence-seam closure (one iteration, two ordered ACs).

**Pre-iter context (all complete):**

- Sibling study: `~/development/moor-echo` (Path B advocate) + `~/development/cairn` (Path A advocate) captured at `docs/superpowers/specs/2026-05-14-fix-b-seam-paths.md`.
- Pre-iter PAR (Reviewers A + B, paired adversarial): paths chosen — **AC-2 = Path 2A (sibling-entry `eval_persistent`)**; **AC-6 = Path 6A (orchestrator `recover_and_rebind` in fmpl-core; no new trait — closure reuse)**.
- Resolved build sequence T0-T6 captured in roadmap scope card lines 2160+.

**Resolved shapes:**

```rust
// T0-IMPL: sibling entry, eval() untouched
#[cfg(feature = "persistence")]
pub fn eval_persistent(
    vm: &mut Vm,
    source: &str,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
    key: &str,
) -> Result<Value>;

// T2: orchestrator in fmpl-core, no new trait
#[cfg(feature = "persistence")]
pub fn recover_and_rebind(
    vm: &mut Vm,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
) -> Result<fmpl_persistence::RecoveryStats>;
```

**Open decisions deferred to iteration owner:**

1. T0-IMPL: does `eval_persistent` wrap `eval()` or duplicate `eval_via_*` dispatch?
2. T3: AC-6 logging — add tracing hook to `recover_incompatible`, or amend AC-6 wording?

**Sentinel corpus baseline:** PENDING (running-an-iteration step 3).

**Last event:** 2026-05-15T03:55:00Z — orchestrator resumed in Resume mode; FIX-B scope card updated with PAR resolutions; entering running-an-iteration.

**Critical context for next session:** Discord-bot demo deliverable slipped on 2026-05-14 EDT (the day its timing gate opened). Captured in auto-memory at `project_discord_bot_slip_2026_05_15.md`. Surface as top priority at next session start before resuming fmpl iterations.

## Iterations status

**Done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), **0005b-FIX-A**.

**In flight:** **ITER-0005b-FIX-B** (just kicked off).

**Pending (priority order after FIX-B):**

1. **ITER-0005c** — bytecode persistence proof case.
2. **ITER-0005b-OBJ** — Grammar/Object source_hash threading.
3. **ITER-0005b-GC** — source store GC keyspace-scan orchestration.
4. **ITER-0005b-AST-SLOT** — Lambda + Object + Grammar AST slot.
5. **ITER-0005b-SYNTH** — constructor synthesizer.
6. **ITER-0005d** — remaining payload classes.
7. **ITER-0005e** — VM snapshot + tracer substrate.
8. **ITER-0005f** — feature flag wiring + final polish.
9. **ITER-PROCESS-TAGS** — project-wide process-tag sweep + structural proof test.

## Discovered follow-up gaps (carried from FIX-A)

1. **fmpl-web `test_multi_session_isolation` Backend(Locked) failure** — pre-existing ITER-0005a.6 regression.
2. **Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.**
3. **EPIC-003 "Status: 0/11 done" counter is stale.**
4. **Process-tag references in `recovery.rs` doc comments** (on ITER-PROCESS-TAGS' inventory).
