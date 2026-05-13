# Progress

**Phase:** ITER-0005b-FIX-A **DONE 2026-05-14T19:35 EDT, post-audit AUDIT-CLEAN 2026-05-14T19:50 EDT.** Closing PAR sentinel sweep clean (22 pass / 0 fail / 4 skip on long-standing TBD-command rows). Sentinel `persistence_schema_format_anti_rot` is GREEN. ITER-0005c unblocked. Post-iteration PAR audit (Reviewers A + B, parallel adversarial) found one Critical + one Serious — both corrected inline: (1) sentinel-sweep block in iteration-log now contains verbatim script output (FIX-MECH verifiability contract upheld), (2) dead `COMMANDS` array in run_sentinels.sh removed. Reviewer B also flagged a dangling `R-M-M-2` process tag at recovery.rs:99 — pre-existing from ITER-0005b, routed to ITER-PROCESS-TAGS' inventory.

## What ITER-0005b-FIX-A delivered

- **T1 — FIX-1** typed re-export laundering: `recovery.rs`'s test module now calls `envelope::write_compiled_code(...)` instead of `write(... PayloadKind::CompiledCode ...)` directly. `rg "PayloadKind::" fmpl-persistence/src/recovery.rs` returns empty. The schema-format anti-rot sentinel is green. Side-discovery: the inline `#[cfg(test)] mod tests` in recovery.rs had E0423/E0433 build errors before FIX-1 — the tests weren't compiling, the string-scan sentinel was the only thing catching the leak. After FIX-1: tests compile and run.
- **T2 — FIX-5** unused API deletion: `recover_incompatible_from_path` removed from `recovery.rs`. Zero callers verified pre-deletion.
- **T3 — FIX-4** corpus rows verified pre-done (NO-OP). SCENARIO-0100 + SCENARIO-0102 rows already had concrete commands at iteration start.
- **T4 — FIX-7** roadmap.md ITER-0005b status amended (AC-2 + AC-6 marked re-opened). iteration-log.md historical text was already corrected during ITER-0005b close-out.
- **T5 — FIX-MECH (Option-α)** sentinel-sweep script shipped at `docs/superpowers/iterations/scripts/run_sentinels.sh`. Includes a build-prerequisites preamble (`FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap` + `touch fmpl-core/build.rs` + `cargo build -p fmpl-core`) so the script is environment-honest. Closing PAR contract: every future iteration's closing PAR entry must include a `### Sentinel sweep (closing-PAR)` block with this script's output.
- **T6 — Wrap** closing PAR sweep captured in iteration-log; ITER-0005c's `Depends on:` line now includes `ITER-0005b-FIX-A`.

## Verification at the closed state

- `cargo build --workspace --all-features` clean.
- `cargo clippy --all-targets --all-features -- -D warnings` clean.
- fmpl-core: 1292/1292 passing (unchanged).
- fmpl-persistence: **103 passing** (was 102 passing + 1 failing pre-iteration; +1 net).
- fmpl-workspace-tests: 3/3 passing.
- Workspace total: **1446 passing, 1 failing, 181 ignored.** The 1 failure is `fmpl-web::storylet_http::test_multi_session_isolation` — a pre-existing ITER-0005a.6 regression that the 0005a.6 closing PAR did not catch (the 0005a.6 verification gates listed fmpl-core / fmpl-persistence / fmpl-workspace-tests only). Documented as a follow-up gap; out of scope for FIX-A.

## Iterations status

**Done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), **0005b-FIX-A**.

**Pending (next-action priority order):**

1. **ITER-0005a.2** — STORY-0099 AC-5 write-side sweep (small; unblocked; independent of FIX-B). NOTE: appears in pending list from prior progress but verify against current roadmap status — may already be done as part of the 0005a.{0,1,2,3,5,6} run, in which case skip.
2. **ITER-0005b-FIX-B** — AC-2 + AC-6 evidence-seam decisions (architectural). Requires its own pre-iter PAR with paths picked (2A vs 2B, 6A vs 6B). Sibling-project study of moor-echo + cairn recommended before path commitment.
3. **ITER-0005c** — bytecode persistence proof case. **Unblocked now that FIX-A closed.** Its design is affected by ITER-0005b-FIX-B's eval-seam decision; running FIX-B before 0005c is preferred but not required.
4. **ITER-0005b-OBJ** — Grammar/Object source_hash threading.
5. **ITER-0005b-GC** — source store GC keyspace-scan orchestration.
6. **ITER-0005b-AST-SLOT** — Lambda + Object + Grammar AST slot (blocks SYNTH).
7. **ITER-0005b-SYNTH** — constructor synthesizer (blocked by AST-SLOT).
8. **ITER-0005d** — remaining payload classes.
9. **ITER-0005e** — VM snapshot + tracer substrate.
10. **ITER-0005f** — feature flag wiring + final polish.
11. **ITER-PROCESS-TAGS** — project-wide process-tag sweep + structural proof test (low-priority; non-blocking).

## Discovered follow-up gaps from FIX-A

1. **fmpl-web `test_multi_session_isolation` Backend(Locked) failure** — pre-existing ITER-0005a.6 regression. Triage as a small housekeeping iteration or roll into ITER-PROCESS-TAGS.
2. **Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.** Either map them to runnable tests or downgrade cadence.
3. **EPIC-003 "Status: 0/11 done" counter is stale** — should reflect STORY-0099 closure.
4. **Process-tag references in `recovery.rs` doc comments** (lines 13-23) — already on ITER-PROCESS-TAGS' inventory.

## Recommended next iteration

The audit step for ITER-0005b-FIX-A (PAR paired auditors, three-tier) should run next per the iterative-development loop. After audit clean: pick next pending. The candidate immediately next is **ITER-0005b-FIX-B** (architectural seam decisions) since it's pre-committed to a sibling-study + pre-iter-PAR flow; alternatively, **ITER-0005a.2** can run in parallel if verified still pending.

## Ratchet status (post-FIX-A)

1. `no_legacy_fmpl_syntax` gate — stable. ✅
2. Sentinel test corpus — 1292 fmpl-core + 103 fmpl-persistence + 3 fmpl-workspace-tests = 1398 tracked-crate tests passing.
3. Parser-epoch system — PARSER_EPOCH = 5. ✅
4. Structural invariants — green. ✅
5. Canonical-pipeline parity (SCENARIO-0108) — green. ✅
6. No-fjall-in-consumers — workspace-wide gate green. ✅
7. AC-5 writer-bypass gate — green. ✅
8. AC-6 anti-rot ratchet — green. ✅
9. **Schema-format anti-rot — GREEN** ✅ (FIX-1 landed; recovery.rs no longer references `PayloadKind`).
10. Content-addressed source-store dedup (SCENARIO-0100) — green at cadence=iteration (will be promoted to sentinel after FIX-B closes the seam decisions).
11. Recovery-from-incompatible-payload (SCENARIO-0102) — green at cadence=iteration (will be promoted to sentinel after FIX-B closes the seam decisions).
12. **NEW** Sentinel-sweep mechanical defense (FIX-MECH) — operational. Every iteration's closing PAR must capture a sentinel-sweep block in its iteration-log entry.
