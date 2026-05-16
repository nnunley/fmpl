# parser-bootstrap Specification

## Purpose
TBD - created by archiving change update-parser-bootstrap-cutover. Update Purpose after archive.
## Requirements
### Requirement: Parser Bootstrap Uses Generated Stage By Default
The bootstrap workflow MUST default to `generated-stage1` and only use `legacy-stage0` when explicitly selected.

#### Scenario: Default bootstrap stage is generated-stage1
- **WHEN** `fmpl-bootstrap` runs without `--stage`
- **THEN** it uses `generated-stage1`
- **AND** it logs the selected stage mode

