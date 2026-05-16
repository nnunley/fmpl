# Progress

## RESUME INSTRUCTIONS FOR NEXT SESSION

ITER-0005c (bytecode persistence proof case) closed 2026-05-16 late evening EDT after 4 rounds of pre-iteration PAR (R1+R2+R3 each REVISE with progressively smaller findings; R4 scope converged on textual fixes and proceeded). Next pending iterations:

1. **ITER-0005b-OBJ / 0005b-GC / 0005b-AST-SLOT / 0005b-SYNTH** — STORY-0100 follow-on threads (Grammar/Object source_hash, source store GC, AST slot, constructor synthesizer).
2. **ITER-0005d** — remaining payload classes (objects, grammars, GrammarRegistry, memo tables). Its scope card MUST revisit drop+reopen vs subprocess as a per-payload-class decision (per ITER-0005c carried gap #3).
3. **ITER-0005e** — VM snapshot + tracer substrate.
4. **ITER-0005f** — feature flag wiring + final polish.
5. **ITER-PROCESS-TAGS** — low-priority sweep.

Baseline state at ITER-0005c close (2026-05-16 late evening EDT):
- Sentinel sweep: 26 pass, 0 fail, 4 skip (long-standing TBDs SCENARIO-0012/0013/0020/0021) — byte-identical to pre-ITER-0005c baseline.
- `cargo test -p fmpl-persistence --features fjall-backend`: 112 passed (was 110; +2 from ITER-0005c drop+reopen tests).
- `cargo test -p fmpl-core --features persistence --lib`: 328 passed, 1 ignored.
- `cargo clippy --workspace --all-features -- -D warnings`: No issues.
- 89 cited stories exist in requirements (citation check OK).
- REVIEW_QUEUE: empty (drained pre-iteration; 35 candidates rejected as auto-cluster telemetry).

**Phase:** ITER-0005c DONE + AUDIT-CLEAN 2026-05-16 late evening EDT.
**Last event:** ITER-0005c closing PAR (auditing-progress, paired auditors A+B) returned GAPS FOUND on documentation traceability — convergent: receiving scope cards (ITER-0005d, 0005e) hadn't acknowledged inherited carried gaps; progress.md gap list missing 2 of 5 items; ITER-0005c roadmap card body retained pre-R4 Build order. All addressed inline (no follow-up iteration spawned). Code/evidence/sentinel sweep CLEAN throughout. Final deliverables: STORY-0014 AC-1 calibrated (`journey` → `integration` impact with cross-process gap named); SCENARIO-0018 Action/Expected augmented to require drop+reopen + envelope `source_hash` recovery; `CompiledCode::load_from_store` relaxed to `S: Store + ?Sized` with the drop+reopen test as its first dyn-Store consumer; two new integration tests (`bytecode_survives_drop_and_reopen`, `nested_code_survives_drop_and_reopen`). Pre-iteration commit `f299c8e` drained 35-candidate REVIEW_QUEUE.

**Prior phase:** ITER-SWEEP-ASSERTIONS DONE + AUDIT-CLEAN 2026-05-16.
**Last event:** 2026-05-16T03:0X EDT — ITER-SWEEP-ASSERTIONS auditing-progress PAR (Auditor A + Auditor B, paired adversarial) returned unanimous CLEAN across all three tiers (Tier 1 deep evidence, Tier 2 impacted behavior, Tier 3 sentinel corpus). Auditor B's discrimination-strength probe (the dimension SWEEP was scoped around) found no tautologically-zero assertions; each new zero-counter equality discriminates at least one identifiable wrong-implementation routing path. Pre-iteration PAR caught a convergent Serious finding (in-module asymmetry originally out of scope); scope revised mid-iteration to add SWEEP-B1..B7; pre-iteration PAR round 2 APPROVE. Verification gates: 110 fmpl-persistence tests, sentinel sweep byte-identical to baseline (26/0/4), clippy clean workspace-wide. Bundled commit `pywkzxor` aggregates four iterations (TUPLE-PERSIST + GAP-1 + GAP-2 + SWEEP-ASSERTIONS).

**Prior phase:** ITER-0005b-FIX-B-GAP-2 DONE 2026-05-16. +1 sentinel test closing STORY-0100 AC-6's error-path symmetric gap.

**Prior phase:** ITER-0005b-FIX-B-GAP-1 DONE 2026-05-16 — SCENARIO-0102 promoted to sentinel, +2 sentinel-cadence stress scenarios.

**Prior phase:** ITER-TUPLE-PERSIST (Durable TupleSpace) DONE 2026-05-16T00:04 EDT — PayloadKind::Tuple = 0x0A, TupleSpace::open(path), per-tuple `durable: true` write-through, in/inp delete-on-consume, FMPL `tuplespace.open` builtin, AC-6 API break to `space.out(%{...})`, integration tests, demo YAML scenario.

**Note on naming:** ITER-TUPLE-PERSIST authored under `ITER-0005c` initially; renamed post-implementation because the roadmap had reserved `ITER-0005c` for the planned bytecode-persistence-proof-case. The descriptive suffix signals orthogonality to the 0005a/b/c/d/e/f numbered sequence. Scope doc: `docs/superpowers/specs/2026-05-15-iter-tuple-persist.md`.

## Earlier phase (preserved)

**Prior phase:** ITER-0005b-FIX-B **DONE with follow-up gap iterations** — closing PAR aggregation complete, 2 gap stories spawned. See iteration-log for full transcript.

## Iterations status

**Done:** ITER-0004, 0004a-d, 0005a, 0005a.{0,1,2,3,5,6}, 0005b (partial), 0005b-FIX-A, 0005b-FIX-B (with follow-up gaps), 0005b-FIX-B-GAP-1, 0005b-FIX-B-GAP-2, ITER-TUPLE-PERSIST, ITER-SWEEP-ASSERTIONS, **ITER-0005c**.

**In flight:** none.

**Pending (priority order):**
1. ITER-0005b-OBJ — Grammar/Object source_hash threading.
2. ITER-0005b-GC — source store GC keyspace-scan orchestration.
3. ITER-0005b-AST-SLOT — Lambda + Object + Grammar AST slot.
4. ITER-0005b-SYNTH — constructor synthesizer.
5. ITER-0005d — remaining payload classes. **Scope card must revisit drop+reopen vs subprocess per-payload-class (ITER-0005c carried gap #3).**
6. ITER-0005e — VM snapshot + tracer substrate.
7. ITER-0005f — feature flag wiring + final polish.
8. ITER-PROCESS-TAGS — project-wide process-tag sweep + structural proof test.

## Discovered follow-up gaps (carried)

1. fmpl-web `test_multi_session_isolation` Backend(Locked) failure — pre-existing ITER-0005a.6 regression.
2. Long-standing TBD-command sentinels: SCENARIO-0012, 0013, 0020, 0021.
3. ~~EPIC-003 "Status: 0/11 done" counter is stale (STORY-0099 + STORY-0100 now closed).~~ RESOLVED 2026-07-22 — counter corrected to 2/11.
4. Process-tag references in `recovery.rs` doc comments + scenario test file module docs (on ITER-PROCESS-TAGS' inventory).
5. **`save_to_store` `?Sized` relaxation** — FIX-B added `+ ?Sized` to `CompiledCode::save_to_store`'s generic Store bound. If a future iteration needs `&dyn Store` through `ObjectDb::save_to_store` or `ParseState::save_to_store`, those bounds may need the same relaxation.
6. **Iteration-log validator regex doesn't match sub-iter naming** — validator uses `## ITER-(\d+)` which collapses `ITER-0005a.1`, `ITER-0005a.2`, ..., `ITER-0005b-FIX-A`, `ITER-0005b-FIX-B`, `ITER-SWEEP-ASSERTIONS` all into a single section. Pre-existing validator limitation. Low priority.
7. **Pre-existing clippy `collapsible_if` at `fmpl-core/src/tuplespace/store.rs:325`** — from ITER-TUPLE-PERSIST work, in working tree as uncommitted change. Surface for cleanup; not a regression.
8. **`scenario_0102_recover_incompatible.rs::scenario_0102_composes_with_iter_store_for_full_keyspace_coverage`** asserts only 2 of 6 counters — weakest shape in the SCENARIO-0102 family. Surfaced by SWEEP-ASSERTIONS pre-iteration PAR; deferred because the test is at a different seam (journey composition test, not `recover_and_rebind` unit test).
9. **Rebind-side-effect assertion on non-UTF-8 recompile-failure paths.** `recover_and_rebind_counts_non_utf8_{key,source}_as_recompile_failure` do not assert "future-major envelope is still in place at the original key" (the happy-path tests do). Different invariant axis from counter-discrimination. Surfaced by SWEEP-ASSERTIONS pre-iteration PAR.
10. **Helper-extraction refactor opportunity.** Five tests in `recover_and_rebind_unit.rs` share `(tempdir + FjallStore + SourceStore + future_vm + write)` setup. Extracting a shared `setup_orchestrator_fixtures()` helper would reduce repetition. Different axis from assertion-strengthening; surfaced by SWEEP-ASSERTIONS pre-iteration PAR and explicitly out of scope.
11. **Cross-process bytecode load proof** (ITER-0005c carried gap #1). STORY-0014's design source `bootstrap-design.md:223-235` references "session-to-session restart" semantics. ITER-0005c proves only same-process drop+reopen, calibrating AC-1's `impact` from `journey` to `integration`. A future subprocess-sentinel iteration (likely co-scoped with ITER-0005f or as a sibling of ITER-0005e VM-snapshot tests) owns cross-process evidence.
12. **ObjectDb / ParseState `load_from_store` `?Sized` relaxation** (ITER-0005c carried gap #4). Un-relaxed today; ITER-0005c relaxed only `CompiledCode::load_from_store` because that's the only peer with a real `&dyn Store` consumer in scope. Per `feedback_ship_infrastructure_with_first_consumer`. Owner: ITER-0005d (its scope card revisits per-payload-class).
13. **Save-side `?Sized` asymmetry on ObjectDb/ParseState** (ITER-0005c carried gap #5). `CompiledCode::save_to_store` is already `+ ?Sized` (from prior iteration). The other two save peers are not. Acceptable today (no save-side `&dyn Store` consumer). Owner: ITER-0005d.
14. **`TODO(ITER-0005a.4)` manual prefix-strip** at `fmpl-core/src/compiler.rs:755-759` (ITER-0005c carried gap #2; pre-existing). The `CompiledCode::load_from_store` body uses a manual `let payload = &b[ENVELOPE_HEADER_SIZE..]` slice instead of routing through `loader::decode`. Will be closed when ITER-0005a.4 lands the `DecodedRecord` consolidation. Owner: ITER-0005a.4.
15. **ITER-0005e snapshot template question** (ITER-0005c carried gap #3). Single-keyspace drop+reopen (the ITER-0005c precedent) doesn't extend cleanly to ITER-0005e's multi-keyspace snapshot API (`Vm::snapshot(dir)` writes object db, bytecode store, source store, grammar registry, memo tables atomically). ITER-0005e's pre-iteration PAR must pick between (a) multi-keyspace drop+reopen, (b) atomic-rename `tmp_dir → final_dir`, or (c) manifest+versioned-pointer pattern. Owner: ITER-0005e (now noted in its scope card).
