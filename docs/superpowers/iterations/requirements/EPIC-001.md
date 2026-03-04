# EPIC-001 — Parser Cutover

**Summary:** Parser Cutover
**Stories:** STORY-0001, STORY-0002, STORY-0003, STORY-0004
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`
**Status:** 0/4 done

## STORY-0001

**Epic:** EPIC-001 — Parser Cutover
**Title:** Replace Rust lexer and parser with FMPL scannerless PEG parser

**As a** FMPL developer
**I want** the compilation pipeline to use fmpl_parser.fmpl instead of the Rust lexer and parser
**So that** the parser is self-hosted and can be developed interactively in the REPL

**Acceptance criteria:**
- AC-1: fmpl_parser.fmpl produces identical AST tagged values for all existing test cases compared to the Rust lexer+parser · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0001`
- AC-2: REPL uses the FMPL parser by default when processing input · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0001`
- AC-3: Web server uses the FMPL parser by default when processing input · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0001`
- AC-4: fmpl-bootstrap crate retains the Rust parser for seed generation · impact:`local` · seam:`integration` · scenario:`SCENARIO-0001`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:190-205`

**Status:** pending

## STORY-0002

**Epic:** EPIC-001 — Parser Cutover
**Title:** Add parse_with_grammar compilation path

**As a** FMPL developer
**I want** a parse_with_grammar path in the compilation pipeline that invokes fmpl_parser.fmpl
**So that** both parsers can run side-by-side for parity testing before cutover

**Acceptance criteria:**
- AC-1: A parse_with_grammar function exists that takes FMPL source and returns AST tagged values using fmpl_parser.fmpl · impact:`local` · seam:`integration` · scenario:`SCENARIO-0002`
- AC-2: A flag or configuration option selects which parser to use (Rust or FMPL), defaulting to FMPL once parity is achieved · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0002`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:194-199`

**Status:** pending

## STORY-0003

**Epic:** EPIC-001 — Parser Cutover
**Title:** Bridge AST tagged values and ast::Expr representations

**As a** FMPL developer
**I want** a bridge between tagged value ASTs (from the FMPL parser) and ast::Expr (from the Rust parser)
**So that** the rest of the compilation pipeline can consume either representation

**Acceptance criteria:**
- AC-1: Tagged value ASTs produced by fmpl_parser.fmpl can be converted to a form consumable by the existing Rust compiler pipeline · impact:`local` · seam:`integration` · scenario:`SCENARIO-0001`
- AC-2: Both parsers run on the test suite and produce diffable AST output showing any mismatches · impact:`local` · seam:`integration` · scenario:`SCENARIO-0001`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:196-198`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:311-313`

**Status:** pending

## STORY-0004

**Epic:** EPIC-001 — Parser Cutover
**Title:** Retire Rust lexer and parser from main compilation path

**As a** FMPL developer
**I want** the Rust lexer and parser to be removed from the main compilation path and retained only in fmpl-bootstrap
**So that** the codebase is simplified and the FMPL parser is the single source of truth

**Acceptance criteria:**
- AC-1: Rust lexer and parser code is only compiled in the fmpl-bootstrap crate, not in fmpl-core's default features · impact:`local` · seam:`integration`
- AC-2: All tests pass without the Rust parser available in the main path · impact:`cross-surface` · seam:`integration`

**Sources:**

**Status:** pending

## STORY-0038

**Epic:** EPIC-008 — Generated Parser Default
**Title:** Verify REPL and web server use generated parser by default

**As a** FMPL developer
**I want** the REPL and web server to automatically use the generated parser
**So that** switching the parser default propagates to all entry points without code changes

**Acceptance criteria:**
- AC-1: fmpl-cli REPL calls fmpl_core::eval() and uses the generated parser without any special configuration · impact:`local` · seam:`app-level` · scenario:`SCENARIO-0015`
- AC-2: fmpl-web server calls fmpl_core::eval() and uses the generated parser without any special configuration · impact:`local` · seam:`app-level` · scenario:`SCENARIO-0015`
- AC-3: Running '1 + 2 * 3' in the REPL returns 7 · impact:`local` · seam:`app-level` · scenario:`SCENARIO-0015`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:26-51`

**Status:** pending
