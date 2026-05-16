## 1. Stage Invariants and Modes
- [x] 1.1 Define canonical stage model in code/docs (`legacy-stage0`, `generated-stage1`).
- [x] 1.2 Add explicit parser mode selection to `fmpl-bootstrap` CLI and default behavior.
- [x] 1.3 Ensure mode selection is surfaced in bootstrap logs for CI diagnostics.

## 2. Build Pipeline Enforcement
- [x] 2.1 Update `fmpl-core/build.rs` to prefer generated-stage1 regeneration flow.
- [x] 2.2 Require explicit opt-in for legacy fallback mode in build pipeline.
- [x] 2.3 Add generated parser freshness check that fails build/CI when stale.
- [x] 2.4 Add deterministic regeneration check (same inputs => byte-identical `generated_parser.rs`).

## 3. Runtime/REPL Path Cleanup
- [x] 3.1 Remove legacy-only completeness assumptions from `is_complete` and align behavior with generated parser workflow.
- [ ] 3.2 Add CI/test guard preventing new `eval_via_legacy_parser` callsites outside allowlist.

## 4. Integration Tests
- [ ] 4.1 Add stage0 bootstrap integration test (legacy-allowed bootstrap artifact generation).
- [ ] 4.2 Add stage1 regeneration integration test (generated parser path only).
- [ ] 4.3 Add end-to-end test proving regenerated parser compiles and evaluates a representative corpus.

## 5. Documentation Updates (docs/ review)
- [ ] 5.1 Update `docs/plans/2026-01-29-scannerless-fmpl-parser.md` with staged bootstrap status and cutover constraints.
- [ ] 5.2 Update `docs/design/project-overview-draft.md` to reflect generated-parser-first bootstrap posture.
- [ ] 5.3 Add a concise operator/developer runbook for bootstrap regeneration commands and troubleshooting.

## 6. Validation and Handoff
- [ ] 6.1 Run `openspec validate update-parser-bootstrap-cutover --strict`.
- [ ] 6.2 Verify `cargo test -p fmpl-core` passes with new bootstrap/parser checks.
- [ ] 6.3 Record final cutover checklist and residual exceptions (if any).
