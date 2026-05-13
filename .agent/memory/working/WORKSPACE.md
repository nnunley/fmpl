# Workspace (live task state)

## Current task
**No active task.** ITER-0005b closed 2026-05-14T12:55 EDT (partial STORY-0100 closure; closing PAR dispatch pending — see `iteration-log.md` for the deferred-ACs map).

## State at `@` (uncommitted working-copy)

Contains everything from the prior commit (Task #21 + ITER-0005a.6) PLUS ITER-0005b's deliverable:
- fmpl-persistence/src/hash_compute.rs (NEW)
- fmpl-persistence/src/source_store.rs (NEW)
- fmpl-persistence/src/recovery.rs (NEW)
- fmpl-persistence/tests/source_store.rs (NEW)
- fmpl-persistence/tests/scenario_0100_content_addressed_source.rs (NEW)
- fmpl-persistence/tests/scenario_0102_recover_incompatible.rs (NEW)
- fmpl-persistence/tests/bytecode_persistence.rs (updated: 7 call sites + 2 new tests)
- fmpl-core/src/compiler.rs (`CompiledCode::save_to_store` signature change, gated `#[cfg(feature = "persistence")]`)
- fmpl-persistence/Cargo.toml (dev-deps activate fmpl-core's `persistence` feature)
- docs/superpowers/specs/2026-05-14-iter-0005b-plan.md (the iteration plan)
- docs/superpowers/specs/2026-05-14-lambda-ast-slot.md (design note for future iteration)

## Verification at the closed state

- `cargo build --workspace --all-features` clean
- `cargo clippy --all-targets --all-features -- -D warnings` clean
- fmpl-core: 1292/1292
- fmpl-persistence: 101 passing (was 73; +28 from ITER-0005b)
- fmpl-workspace-tests: 3/3
- All invariant gates green

## Next iteration candidates

By priority:
1. **ITER-0005b closing PAR** — dispatch 2 reviewers per the lesson from 0005a.5/0005a.6.
2. **ITER-0005c** — bytecode persistence proof case; now unblocked.
3. **ITER-0005b-AST-SLOT** — Lambda + Object + Grammar gain source AST slot; unblocks 0005b-SYNTH.
4. **ITER-0005b-OBJ** — Grammar/Object source_hash threading.
5. **ITER-0005b-GC** — source store GC keyspace-scan orchestration.
6. **ITER-0005a.2** — small AC-5 write-side sweep.

## Next step (resume instructions)

1. Read `docs/superpowers/iterations/progress.md` for current state
2. Read `docs/superpowers/iterations/iteration-log.md` (ITER-0005b section) for the latest iteration's narrative + lessons (especially: pre-iter PAR earning its keep again; the user-directed deferral of the Lambda AST slot)
3. Dispatch closing PAR on ITER-0005b before declaring victory
4. Then pick the next iteration

Sentinels GREEN. No regression to recover from.
