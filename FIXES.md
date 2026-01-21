# Documentation and Spec Fixes

This file contains a checklist of issues to address in the `docs` and `specs` directories to improve consistency, accuracy, and organization.

## High-Priority Issues

### Inconsistencies and Outdated Information

- [x] **`specs/README.md` vs. `docs/plans`**: The `specs/README.md` file lists several implementation plans as "Complete", but the corresponding plan documents in `docs/plans` are not updated to reflect this. For example, `streaming-grammar-push-model-implementation.md` is listed as complete, but the plan document itself is not updated.
- [ ] **`specs/pattern-matching.md`**: This spec claims that map and list patterns are not supported in `@` match expressions, but this seems to be a key feature of the "unified grammar" vision. This needs to be clarified and the spec updated to reflect the current implementation or the intended design.
- [x] **`docs/design/project-overview-draft.md`**: This document had an outdated "Current Implementation Status" section. ✓ Fixed: Updated to reflect that streaming grammar, async operators, and ParseState serialization are complete; moved pattern matching in `@` expressions to In Progress; added Multi-VAT coordination to Planned.
- [x] **`specs/reviewed-files.md`**: This file is out of date and does not reflect the current review status of the documentation. It should be updated or removed. ✓ Fixed: Removed as it is a metadata tracking file that provides no value to users or developers.
- [x] **Project Name**: The project is referred to as "[Project Name TBD]" in several places. A decision should be made on the project's name and the documentation updated accordingly. ✓ Fixed: Replaced with "FMPL" in specs/README.md, specs/fmpl-cli.md, specs/fmpl-core.md, and docs/design/project-overview-draft.md

### Missing Documentation

- [ ] **Missing Specs**: There are no specs for the `lib` directory, which contains `anthropic.fmpl`, `compaction.fmpl`, `llm-common.fmpl`, and `ollama.fmpl`.
- [ ] **Missing Implementation Plans**: There are no implementation plans for several of the features described in the design documents, such as the multi-VAT architecture, the tuple space, and the PASETO-based security model.

### Build Instructions

- [x] **`AGENTS.md` missing build instructions**: The `.github/copilot-instructions.md` file contained comprehensive build instructions, architecture details, and development workflow information that was not in `AGENTS.md`. This has been consolidated into `AGENTS.md`.

## Medium-Priority Issues

### Broken Links and Incorrect File Paths

- [x] **`specs/README.md`**: The links to the implementation plans in `specs/README.md` are incorrect. They point to markdown files in the `docs/plans` directory, but with a `.md` extension at the end of the filename which is not correct.
- [ ] **General Review**: A general review of all markdown files for broken links and incorrect file paths should be performed.

### Clarity and Organization

- [ ] **Consolidate Specs**: The `specs` directory contains several files that could be consolidated. For example, the `streaming-grammar.md` spec could be merged into the `grammar-system.md` spec.
- [ ] **Standardize Document Structure**: The documents in `docs/plans` and `docs/design` have inconsistent structures. A standard structure for design documents and implementation plans should be defined and applied.
- [ ] **Update `ralph.yml`**: The `ralph.yml` file seems to be a configuration file for a development tool. It should be reviewed and updated to reflect the current project structure.

## Low-Priority Issues

- [ ] **Spelling and Grammar**: A general review of all documentation for spelling and grammar errors should be performed.
- [ ] **Update `TUTORIAL.md`**: The `TUTORIAL.md` file is likely out of date. It should be reviewed and updated to reflect the current state of the language and tools.
