## Why
Parser generation and bootstrap workflows still rely on legacy parser paths beyond the intended stage0 bootstrap boundary. This blocks the self-hosting objective where generated parser output is authoritative for stage1+ regeneration.

## What Changes
- Define explicit staged bootstrap invariants:
  - stage0 may use legacy parser,
  - stage1+ must use generated parser paths.
- Add build/CI enforcement for generated parser freshness and deterministic regeneration.
- Refactor bootstrap tooling to make parser mode selection explicit and auditable.
- Add integration tests validating stage0 -> stage1 regeneration and successful build/parse verification.
- Add guardrails preventing expansion of legacy parser use in runtime/default test paths.
- Update documentation in `docs/` to describe the staged bootstrap model and developer workflows.

## Impact
- Affected specs:
  - `parser-bootstrap`
  - (follow-on) `fmpl-core` baseline capability via implementation alignment
- Affected code:
  - `fmpl-core/build.rs`
  - `fmpl-bootstrap/src/main.rs`
  - `fmpl-core/src/lib.rs`
  - parser/bootstrap integration tests under `fmpl-core/tests/`
- Affected docs:
  - `docs/plans/2026-01-29-scannerless-fmpl-parser.md`
  - `docs/design/project-overview-draft.md`
  - additional docs pages that currently imply broader legacy-parser usage
