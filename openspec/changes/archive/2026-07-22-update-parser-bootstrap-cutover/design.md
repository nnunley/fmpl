## Context
FMPL now has a generated parser path and substantial FMPL-authored parser infrastructure, but bootstrap and regeneration still include legacy-parser dependencies beyond stage0. Existing documentation in `docs/` describes scannerless parser progress but does not fully codify strict staged cutover rules.

## Goals / Non-Goals

- Goals:
  - Make parser bootstrap stages explicit, enforceable, and testable.
  - Ensure generated parser is authoritative for stage1+ regeneration.
  - Add deterministic and freshness guarantees to prevent silent parser drift.
  - Document operational workflow in both OpenSpec and `docs/` so another agent can continue implementation without ambiguity.

- Non-Goals:
  - Full execution_tape semantic parity in this change.
  - Removing legacy parser implementation code entirely.
  - Migrating all parser feature gaps in one proposal.

## Stage Model

### Stage0 (`legacy-stage0`)
- Purpose: produce initial generated parser artifact when no trustworthy generated artifact is available.
- Allowed: legacy parser usage in bootstrap path.
- Disallowed: using stage0 mode as the default for ongoing regeneration.

### Stage1+ (`generated-stage1`)
- Purpose: normal regeneration and bootstrap continuation.
- Required: generated parser path for parser regeneration and validation.
- Allowed fallback: explicit, opt-in, auditable legacy fallback only.

## Enforcement Strategy

### Build-time checks
- Freshness gate: parser source/generator changes require regenerated output.
- Determinism gate: repeated regeneration with identical inputs must produce byte-identical output.

### Test/CI guardrails
- Prevent new non-allowlisted legacy parser callsites in runtime/default tests.
- Integration tests for stage0 -> stage1 sequence and resulting compile/eval viability.

### Runtime path alignment
- Keep compatibility APIs where needed but avoid default paths that route through legacy parser.
- Align completeness checking behavior with generated-parser workflow.

## Docs Review and Alignment
The following docs must be reviewed/updated in this change to prevent contradictory operational guidance:

- `docs/plans/2026-01-29-scannerless-fmpl-parser.md`
  - Add explicit staged bootstrap status and cutover constraints.
- `docs/design/project-overview-draft.md`
  - Reflect generated-parser-first bootstrap posture.
- Additional parser/bootstrap references in `docs/`:
  - ensure no guidance implies routine legacy parser usage beyond stage0.

## Risks / Trade-offs
- Risk: generated parser still lacks full parity for some test surfaces.
  - Mitigation: maintain narrowly scoped allowlist and track reductions as follow-on tasks.
- Risk: deterministic checks can fail due to incidental non-determinism in generator output ordering.
  - Mitigation: normalize generation ordering and include reproducibility assertions.
- Risk: docs drift from implementation.
  - Mitigation: docs update tasks are in-scope and required for sign-off.

## Migration Plan
1. Add bootstrap mode model and flags.
2. Add build-time freshness/determinism gates.
3. Add CI/test guardrails and integration tests.
4. Update docs to match operational truth.
5. Validate OpenSpec + tests and publish residual exceptions.

## Open Questions
- Should stage0 artifacts be committed and pinned, or produced in CI bootstrap jobs only?
- What is the acceptable temporary allowlist for legacy parser usage while parity gaps are closed?
