# Scenario Runner — Design Spec (ITER-0004d.4)

**Date:** 2026-05-12 (revised 2026-05-12 to add bootstrap-durability scope; revised again 2026-05-12 by PAR scope review to defer FMPL-side runner to ITER-0004d.5)
**Owner:** ITER-0004d.4 (Rust-side runner only — FMPL-side bootstrap-durability surface deferred to ITER-0004d.5)
**Status:** Design — pending writing-plans
**Origin:** User feedback during ITER-0004d.1 T19 review on 2026-05-12. The per-scenario Rust test pattern in `fmpl-core/tests/structural_invariants.rs` is stylish but redundant against the scenario cards in `behavior-scenarios.md`. A cucumber / FitNesse-SLIM-style data-driven runner where the scenario card IS the source of truth would (a) make cards directly executable, (b) collapse per-test boilerplate, and (c) let future scenarios land as card-authoring tasks rather than test-writing tasks.

**Revision 2026-05-12 (post-design-review):** the user added a durability requirement — scenario cards must survive the bootstrap process. The original spec aimed to land both surfaces in v1.

**Revision 2026-05-12 (PAR scope review):** both PAR reviewers independently flagged that the FMPL-side runner (Deliverable B: `lib/tests/scenarios/scenarios.fmpl`, `fmpl_emit.rs` compiler, `dispatch.fmpl` FMPL dispatcher, `scenario_runner_bootstrap.rs` test target) is a separate substantial deliverable that should split out of this iteration. Key reasoning: (a) `grep_invariant` cannot be implemented FMPL-side until `io::read_dir` exists, so a v1 FMPL-side runner ships as a partial stub with limited coverage — making the "bootstrap durability" claim overstated for v1; (b) the Rust runner alone (Deliverable A) is a complete, well-bounded iteration with concrete acceptance criteria. Decision: **ITER-0004d.4 ships Deliverable A only.** Deliverable B becomes a new ITER-0004d.5 iteration in the roadmap, gated on `io::read_dir` landing.

## Goal

Make `docs/superpowers/iterations/behavior-scenarios.md` directly executable via a thin Rust runner.

Each scenario card carries enough structured data (action type, inputs, expectations) that a thin Rust driver dispatches each case to a step-definition and surfaces per-case pass/fail with line-span back-references into the markdown.

The first three consumers — SCENARIO-0104, SCENARIO-0105, SCENARIO-0106 (all from ITER-0004d.1) — migrate from `fmpl-core/tests/structural_invariants.rs` into the runner. That file is deleted once the runner covers the same evidence at the Rust surface.

**Deferred to ITER-0004d.5:** the FMPL-side runner that compiles the corpus to `lib/tests/scenarios/scenarios.fmpl` and re-executes against the regenerated parser. Architecture preserves room for it (the corpus parser produces a `Vec<Card>` that an `fmpl_emit.rs` module can serialize); no architectural lock-in is introduced.

## Durability target (deferred to ITER-0004d.5)

Originally the v1 durability target was "parser regeneration": same pass/fail outcomes from the canonical generated parser vs the source-tree parser. This is now ITER-0004d.5's target. ITER-0004d.4 ships the Rust runner only; durability is verified via the existing SCENARIO-0108 (`canonical_pipeline_parity.rs`), which already proves the canonical pipeline is behaviorally equivalent for SCENARIO-0104/0105 inputs.

Two later targets remain out of scope:

- **Self-compile cycle (ITER-0006).** The corpus validates that stage-N+1 of self-compile behaves identically to stage-N. Requires ITER-0006 to land first.
- **Fjall snapshot persistence (ITER-0005).** Architecturally compatible — when ITER-0005 lands, `scenarios.fmpl` (from ITER-0004d.5) is a regular FMPL value handled by the Fjall snapshot machinery without scenario-specific work.

## Non-goals

- **FMPL-side runner and bootstrap-durability surface (deferred to ITER-0004d.5).** No `lib/tests/scenarios/scenarios.fmpl`, no `fmpl_emit.rs`, no `scenario_runner_bootstrap.rs` in v1. The architecture preserves room for these (Card/Case types serializable to FMPL value form is straightforward), but the v1 iteration does not ship them.
- Migrating scenarios SCENARIO-0001..0077 (most have no step-def coverage today). Migration is opt-in.
- Self-compile cycle durability (waits on ITER-0006).
- Fjall-snapshot durability (waits on ITER-0005).
- A TUI / visual reporter. `cargo test` output is sufficient.
- Parameterized fixture-style step-defs beyond what the three concrete consumers need.

## Architecture

### Components

```
fmpl-scenario-runner/                  ← new workspace crate (library)
  Cargo.toml                           ← deps: inventory (0.3.x — current stable)
  src/
    lib.rs                             ← re-exports public API
    corpus.rs                          ← markdown corpus parser
    step_def.rs                        ← trait + inventory registry (Rust surface)
    error.rs                           ← StepError, DispatchError (Display impl on both)

fmpl-core/
  Cargo.toml                           ← [dev-dependencies] fmpl-scenario-runner
                                          [build-dependencies] fmpl-scenario-runner
  build.rs                             ← extended with codegen step writing
                                          OUT_DIR/scenarios_generated.rs (Rust)
  tests/
    scenario_runner.rs                 ← Rust-surface test target; include!s the
                                          generated file; declares `mod steps;`
    steps/                             ← step-def impls (live with the test binary)
      mod.rs                           ← `pub mod parse_rejection; ...`
      parse_rejection.rs               ← struct ParseRejection; impl StepDef
      parse_success.rs                 ← struct ParseSuccess;   impl StepDef
      grep_invariant.rs                ← struct GrepInvariantAbsent;
                                          struct GrepInvariantPresent;
    common/
      comment_strip.rs                 ← moved from structural_invariants.rs;
                                          shared //-line-comment strip helper
    structural_invariants.rs           ← MOSTLY DELETED at iteration end (see
                                          "Special-case migrations" below for
                                          g3_postlude_arms_fire_on_poison_nodes
                                          which gets its own home).
    postlude_arm_contract.rs           ← NEW. Holds g3_postlude_arms_fire_on_poison_nodes
                                          (a test that asserts IS_GENERATED_PARSER
                                          and calls generated_parse — too special-case
                                          to fit cleanly into the scenario card format).

DEFERRED to ITER-0004d.5:
  fmpl-scenario-runner/src/fmpl_emit.rs     (compile Vec<Card> → list-shape FMPL value)
  fmpl-core/tests/scenario_runner_bootstrap.rs  (drives bootstrap, re-runs corpus)
  lib/tests/scenarios/scenarios.fmpl        (compiled corpus artifact)
  lib/tests/scenarios/dispatch.fmpl         (FMPL-side dispatcher)
```

### Public API (`fmpl-scenario-runner`)

```rust
// corpus.rs
pub fn parse_corpus(path: &Path) -> Result<Vec<Card>, CorpusError>;

pub struct Card {
    pub id: String,                    // "SCENARIO-0104"
    pub title: String,
    pub kind: Option<String>,          // "invariant" | "contract" | ...
    pub seam: Option<String>,          // "unit" | "integration" | ...
    pub action_type: Option<String>,   // default for cases without override
    pub cases: Vec<Case>,
    pub owning_stories: Vec<String>,
    pub sources: Vec<String>,
    pub line_start: usize,             // 1-based, inclusive
    pub line_end: usize,
}

pub struct Case {
    pub action: String,                // resolved action type (case override or card default)
    pub fields: BTreeMap<String, Value>,
    pub line_start: usize,
    pub line_end: usize,
}

pub enum Value {
    String(String),
    Bool(bool),
    Int(i64),
    List(Vec<Value>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str>;
    pub fn as_bool(&self) -> Option<bool>;
    pub fn as_int(&self) -> Option<i64>;
    pub fn as_list(&self) -> Option<&[Value]>;
}

// step_def.rs
pub trait StepDef: Sync {
    fn action_type(&self) -> &'static str;
    fn run(&self, card: &Card, case: &Case) -> Result<(), StepError>;
}

pub struct StepDefRegistration(pub &'static dyn StepDef);

inventory::collect!(StepDefRegistration);

pub fn dispatch(card: &Card, case: &Case) -> Result<(), DispatchError>;
    // Walks inventory::iter::<StepDefRegistration>(), picks by action_type.
    // Returns DispatchError::Unknown if no step-def matches.
    // Returns DispatchError::Step(StepError) if the step-def returned Err.

// error.rs
pub struct StepError { pub message: String }
impl StepError { pub fn new(msg: impl Into<String>) -> Self }
impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub enum DispatchError {
    Unknown(String),         // action_type not registered
    Step(StepError),
}
// PAR-revised: the codegen uses `{}` format on DispatchError in
// generated #[test] panic messages. Display impl is REQUIRED.
impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchError::Unknown(action_type) => {
                write!(f, "unknown action_type {action_type:?} — register a StepDef impl in tests/steps/")
            }
            DispatchError::Step(step_error) => write!(f, "{step_error}"),
        }
    }
}

pub enum CorpusError {
    Io(std::io::Error),
    Malformed { line: usize, message: String },
    DuplicateId { id: String, first_line: usize, dup_line: usize },
}
impl std::fmt::Display for CorpusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorpusError::Io(e) => write!(f, "io error: {e}"),
            CorpusError::Malformed { line, message } => {
                write!(f, "malformed card at line {line}: {message}")
            }
            CorpusError::DuplicateId { id, first_line, dup_line } => {
                write!(f, "duplicate scenario id {id} at line {dup_line} (first defined at line {first_line})")
            }
        }
    }
}
```

### Test-binary glue

```rust
// fmpl-core/tests/scenario_runner.rs
mod steps;  // imports each step-def submodule so inventory::submit! is reachable

// The build.rs writes scenarios_generated.rs containing per-case #[test] fns
// plus a static SCENARIO_CORPUS for shared lookup.
include!(concat!(env!("OUT_DIR"), "/scenarios_generated.rs"));
```

```rust
// fmpl-core/tests/steps/mod.rs
pub mod parse_rejection;
pub mod parse_success;
pub mod grep_invariant;
```

```rust
// fmpl-core/tests/steps/parse_rejection.rs (sketch)
use fmpl_scenario_runner::{Card, Case, StepDef, StepDefRegistration, StepError};
use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;

pub struct ParseRejection;

impl StepDef for ParseRejection {
    fn action_type(&self) -> &'static str { "parse_rejection" }
    fn run(&self, _card: &Card, case: &Case) -> Result<(), StepError> {
        let source = case.fields.get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StepError::new("case missing required field: source"))?;

        let expect_rejected = case.fields.get("expect_rejected")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let result = (|| -> Result<_, fmpl_core::error::Error> {
            let tokens = Lexer::new(source).tokenize()?;
            Parser::with_source(&tokens, source).parse()
        })();

        if expect_rejected {
            match result {
                Ok(ast) => Err(StepError::new(format!(
                    "expected parse of `{source}` to be rejected, \
                     but parse succeeded with AST: {ast:?}"
                ))),
                Err(e) => {
                    if let Some(phrases) = case.fields.get("expect_error_contains").and_then(|v| v.as_list()) {
                        let msg = format!("{e:?}");
                        for phrase in phrases {
                            let needle = phrase.as_str().ok_or_else(|| StepError::new(
                                "expect_error_contains entries must be strings"
                            ))?;
                            if !msg.contains(needle) {
                                return Err(StepError::new(format!(
                                    "parse rejected, but error message did not contain {needle:?}.\nActual: {msg}"
                                )));
                            }
                        }
                    }
                    Ok(())
                }
            }
        } else {
            result.map(|_| ()).map_err(|e| StepError::new(format!(
                "expected parse of `{source}` to succeed, got: {e:?}"
            )))
        }
    }
}

inventory::submit! { StepDefRegistration(&ParseRejection) }
```

### Codegen (`fmpl-core/build.rs` extension)

```rust
// Pseudocode added to fmpl-core/build.rs:
fn generate_scenario_tests() -> std::io::Result<()> {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let corpus_path = Path::new(&manifest)
        .parent().unwrap()
        .join("docs/superpowers/iterations/behavior-scenarios.md");
    // Rust 2024 edition syntax (matches the rest of fmpl-core/build.rs).
    println!("cargo::rerun-if-changed={}", corpus_path.display());

    let cards = fmpl_scenario_runner::corpus::parse_corpus(&corpus_path)
        .map_err(|e| std::io::Error::other(format!("corpus: {e:?}")))?;

    let mut out = String::new();
    out.push_str("// AUTO-GENERATED by fmpl-core/build.rs — DO NOT EDIT\n\n");
    out.push_str("use fmpl_scenario_runner::{Card, Case, dispatch};\n\n");

    for card in &cards {
        if card.action_type.is_none() {
            // Skipped: card has no default action type. Cases with explicit
            // overrides could still run, but for simplicity we skip the whole
            // card. (Could revisit in a future iteration.)
            continue;
        }
        for (i, _case) in card.cases.iter().enumerate() {
            let fn_name = format!("scenario_{}_case_{}",
                card.id.trim_start_matches("SCENARIO-"),
                i);
            writeln!(out,
                r#"
#[test]
fn {fn_name}() {{
    let cards = corpus();
    let card = cards.iter().find(|c| c.id == "{id}").unwrap();
    let case = &card.cases[{i}];
    if let Err(e) = dispatch(card, case) {{
        panic!(
            "behavior-scenarios.md:{{}}-{{}} ({id} case {i}): {{}}",
            card.line_start, card.line_end, e
        );
    }}
}}
"#,
                fn_name = fn_name, id = card.id, i = i)?;
        }
    }

    // Helper that lazy-parses the corpus once per test binary.
    //
    // PAR-revised: use env!("CARGO_MANIFEST_DIR") embedded at the test
    // binary's compile time, NOT a runtime relative path. Matches the
    // pattern used in structural_invariants.rs:35-37. A runtime relative
    // path like "../docs/..." would fail if the test binary is invoked
    // with a non-standard CWD (e.g., directly from target/debug/deps/).
    out.push_str(r#"
fn corpus() -> &'static [Card] {
    static CORPUS: std::sync::OnceLock<Vec<Card>> = std::sync::OnceLock::new();
    CORPUS.get_or_init(|| {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CARGO_MANIFEST_DIR has a parent")
            .join("docs/superpowers/iterations/behavior-scenarios.md");
        fmpl_scenario_runner::corpus::parse_corpus(&path).expect("corpus parse")
    })
}
"#);

    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("scenarios_generated.rs");
    fs::write(out_path, out)?;
    Ok(())
}
```

**PAR-revised codegen notes:**
- The codegen emits `env!("CARGO_MANIFEST_DIR")` as a literal string into the generated file. `env!` resolves at the TEST BINARY's compile time (not at build.rs's compile time), giving an absolute path embedded in the test binary. This is the project-standard pattern (see `structural_invariants.rs:35-37`).
- `cargo::rerun-if-changed` uses the Rust 2024 edition double-colon syntax matching the existing build.rs (`fmpl-core/build.rs` consistently uses `cargo::` per PAR finding S3).
- The generated `#[test]` uses `{}` format on `DispatchError`, which requires a `Display` impl. See the next section.

## Data flow

```
docs/superpowers/iterations/behavior-scenarios.md  (source of truth)
                  │
                  ▼  cargo build (fmpl-core test target)
            fmpl-core/build.rs
                  │  invokes fmpl_scenario_runner::corpus::parse_corpus
                  ▼
            Vec<Card>
                  │  build.rs emits one #[test] per (card, case_index)
                  ▼
        OUT_DIR/scenarios_generated.rs
                  │  cargo test compiles + links
                  ▼
        scenario_runner test binary
          ├── tests/scenario_runner.rs   (include!s the generated file)
          ├── tests/steps/{*}.rs          (step-defs; inventory::submit!)
          └── inventory::iter populates the registry at static-init
                  │  each #[test] calls dispatch(card, case)
                  ▼
        Step-def runs the case; returns Result<(), StepError>
          ├── Ok:  test passes; line span shows in test output
          └── Err: panic!("behavior-scenarios.md:NN-MM (SCENARIO-NNNN case M): {err}")
```

## Card format

The runner accepts cards in `behavior-scenarios.md` with this shape:

```markdown
## SCENARIO-NNNN — Title

**Kind:** invariant | contract | surface | failure-recovery
**Proof seam:** unit | integration | e2e | app-level | process-level
**Owning stories:** STORY-NNNN, STORY-MMMM
**Action type:** `parse_rejection`         ← optional; absent ⇒ skipped

**Preconditions:**
- Free-form narrative bullets (informational; runner ignores)

**Action:**
- Free-form narrative bullets (informational; runner ignores)

**Cases:**
- action: `parse_rejection`
  source: `:Foo(1)`
  expect_rejected: true
  expect_error_contains:
    - `use [:Tag]`
    - `instead`
- action: `parse_success`
  source: `:Foo`
- source: `:Bar(1, 2, 3)`               ← inherits action_type from card default

**Expected observables:**
- Free-form narrative bullets (informational; runner ignores)

**Automation status:** implemented
**Execution command:** `cargo test -p fmpl-core --test scenario_runner scenario_NNNN`

**Sources:**
- file:line references
```

### Card-format rules

- `**Action type:**` at the card top is the **default** action type for cases that don't override. It is also the discoverability flag: a card without it is skipped by the runner.
- Each case in `**Cases:**` MAY override with its own `action:` key. If absent, the case inherits the card's default action type (the value of `**Action type:**` at the card top). A case is well-formed if it has either an explicit `action:` key OR the card has a default `**Action type:**`; a case with neither is a corpus error.
- Cases are bullet-list items. A case begins with a `- ` bullet at the indent level under `**Cases:**`. Subsequent more-indented bullets and `key: value` lines belong to that case until the next `- ` at the same indent (or the end of the `**Cases:**` block).
- Keys are `snake_case`. Values are:
  - **Backtick-quoted strings** (`` `:Foo(1)` ``) — preserves whitespace and special characters.
  - **Bare strings** (without backticks) — trimmed.
  - **Booleans** (`true` / `false`).
  - **Integers** (digit sequences).
  - **Indented sub-bullets** — a list of values.
- Line spans `(line_start, line_end)` are inclusive 1-based. Card span runs from its `##` heading to the line before the next `##` (or EOF). Case span runs from its `- ` bullet through the last sub-bullet.

### Cases-shape decision (locked from clarifying answers)

The user's choice for SCENARIO-0106 was **one card with multiple action-type cases**. The card format above accommodates this: each case in `**Cases:**` carries its own `action:` field. The card-level `**Action type:**` is the default for cases that don't specify their own.

## Step definitions

Three step-defs ship with the iteration. Each implements `trait StepDef` and registers via `inventory::submit!`.

### `parse_rejection`

```
Inputs:
  source:                 String       (required)
Expectations:
  expect_rejected:        bool         (default true)
  expect_error_contains:  Vec<String>  (default [])

Behavior:
  1. Tokenize `source` via Lexer.
  2. Parse via Parser.
  3. If expect_rejected:
       - Err   → for each phrase in expect_error_contains, assert it appears
                 in format!("{err:?}").
       - Ok    → fail with the AST in the message.
     Else (expect_rejected = false):
       - Ok    → pass.
       - Err   → fail with the error in the message.
```

### `parse_success`

```
Inputs:
  source:                 String       (required)

Behavior:
  1. Tokenize `source` via Lexer.
  2. Parse via Parser.
  3. Assert Ok; on Err, fail with the error in the message.
```

(Distinct from `parse_rejection` with `expect_rejected: false` for discoverability — control cases read more clearly as `action: parse_success` than as `action: parse_rejection / expect_rejected: false`.)

### `grep_invariant` (two action types: `expect_absent`, `expect_present`)

```
Common inputs:
  needle:                 String       (required)
  scope:                  String       (required; path relative to repo root,
                                        either a file or a directory)
expect_absent expectations:
  (none; the implied expectation is "0 matches")
expect_present expectations:
  min_count:              usize        (default 1)

Behavior:
  1. Resolve `scope`. If a file: load it. If a directory: recursively collect
     all `.rs` files under it.
  2. For each file, walk lines. Strip `//`-line-comments (per the helper moved
     from structural_invariants.rs). Count whole-word matches of `needle`.
  3. For `expect_absent`: assert total count == 0. On failure, list every hit
     as `path:line: text`.
  4. For `expect_present`: assert total count >= min_count. On failure, give
     count + the searched scope.
```

The `comment_strip` helper from `structural_invariants.rs` moves to `fmpl-core/tests/common/comment_strip.rs` so both the (transitional) old test file and the new step-def can call it.

## Error handling

### Three failure modes

| Mode | Cause | Behavior |
|---|---|---|
| **Corpus parse error** | Malformed card (missing required field, bad indent, syntax error) | `build.rs` panics with `[corpus:NN-MM] error: <description>`. Build fails; no tests run. |
| **Dispatch error** | Card has `**Action type:** foo` but no step-def registered for `foo` | The generated `#[test]` panics immediately with `unknown action_type "foo"`. Other scenarios still run. |
| **Case failure** | Step-def returned `Err(StepError)` | Normal `#[test]` panic with the formatted message. Other tests unaffected. |

### Failure output format

```
behavior-scenarios.md:2149-2180 (SCENARIO-0104 case 0):
  expected parse of `:Foo(1)` to be rejected, but parse succeeded with AST:
    Expr::Tagged("Foo", [Expr::Int(1)])
```

First line is machine-parseable (`file:span (id case N): prefix`). Body is the step-def's message, indented two spaces.

### Test name convention

```
scenario_NNNN_case_M
```

`M` is the zero-based case index within the card's `**Cases:**` list. `cargo test scenario_0104` filters to all cases of SCENARIO-0104.

### Skipped-scenarios summary

A `corpus_health_check` test always passes but writes to stderr:

```
[scenario_runner] skipped: 77 cards have no **Action type:** (run with
                  FMPL_SCENARIO_LIST_SKIPPED=1 to see them all)
```

Informational; does not affect test pass/fail.

### Compile-time validation

`build.rs` performs static checks beyond parsing:

- Every card with `**Action type:**` must have at least one case.
- Every case's resolved `action` (override or card default) must be a non-empty string.
- Duplicate scenario IDs are a corpus error.
- The inventory step-def registry isn't visible to build.rs (runtime-only), so the build-time check cannot verify "every action_type has a step-def". That validation happens at runtime via dispatch errors.

## Testing strategy

### `fmpl-scenario-runner` (the crate itself)

- `tests/corpus_parse.rs` — fixture-driven corpus parser tests:
  - Minimal valid card (1 case, default action)
  - Card with mixed-action cases (matches SCENARIO-0106 shape)
  - Card without `**Action type:**` (parses, marked skipped)
  - Malformed-card fixtures (one per error type): missing Cases, duplicate id, bad indent, unterminated case
  - Card with all field types (string, bool, int, list-of-strings)
- `tests/step_dispatch.rs` — exercises StepDef trait + inventory:
  - Registration works.
  - Dispatch picks the right step-def by action_type.
  - Unknown action_type returns DispatchError::Unknown.

### `fmpl-core` integration

- `fmpl-core/tests/build_codegen_check.rs` — parses the real `behavior-scenarios.md`; asserts ≥3 runnable cards exist; asserts the codegen output contains one `#[test]` per case.
- `fmpl-core/tests/scenario_runner.rs` — the test target itself; once SCENARIO-0104/0105/0106 are migrated, this binary produces the 17 evidence tests.
- Step-defs have their own unit tests in their `tests/steps/*.rs` modules against synthetic inputs.

### Sentinel verification

- After migration, `cargo test -p fmpl-core --test scenario_runner` covers ALL of SCENARIO-0104/0105/0106's existing evidence (PAR-revised: ≥19 cases, not 17 — `structural_invariants.rs` has grown since the original spec was written).

### Test count reconciliation (PAR-revised)

The original spec claimed "17 tests" — this was wrong. Current `structural_invariants.rs` (per `cargo test -p fmpl-core --test structural_invariants` = 19 passed) breakdown:
- SCENARIO-0104: 6 tests (single-arg, multi-arg, let-rhs, bare-symbol control, list-form control, hint-quality)
- SCENARIO-0105: 4 tests (match-arm, let-destructure, list-pattern control, hint-quality)
- SCENARIO-0106: 8 greps (#1-#5: deleted variants absent; #6: Instruction::MakeListNode absent from compiler.rs; #7: ExtractListChild present in compiler.rs; #8: LegacyTagCtor name-coupling)
- `g3_postlude_arms_fire_on_poison_nodes`: 1 test (G3 from ITER-0004d.3a; SPECIAL — see migration plan below)

Total: 19 tests in the file. After ITER-0004d.4, the scenario runner must cover ALL of them OR the special case must be explicitly relocated (g3-test, see below).

### NEW in this iteration (PAR-revised): SCENARIO-0106 grep #9

ITER-0004h audit flagged: `Type::Tagged` was deleted with no structural-invariant ratchet. The user explicitly asked for this ratchet to be authored as a scenario card via the new runner, not as a 9th grep test in `structural_invariants.rs`. So ITER-0004d.4 ADDS a new case to SCENARIO-0106:

```
- action: `expect_absent`
  needle: `Type::Tagged`
  scope: `fmpl-core/src/`
```

Acceptance criterion: SCENARIO-0106 grep #9 (`Type::Tagged` absent from `fmpl-core/src/`) is authored as the 9th case in the SCENARIO-0106 card and runs successfully via the runner.

### Special-case migration: `g3_postlude_arms_fire_on_poison_nodes`

This test (added by ITER-0004d.3a as the falsifiability-under-fallback safety net) does NOT fit the scenario card format cleanly because:
- It asserts `fmpl_core::parser::IS_GENERATED_PARSER` (a precondition check, not a scenario case)
- It calls `generated_parse` directly (would need a `generated_parse_rejection` step-def vs the existing `parse_rejection` which uses `Parser::with_source`)
- It has `#[allow(clippy::assertions_on_constants)]`

**Decision:** the test moves to a new file `fmpl-core/tests/postlude_arm_contract.rs` rather than being deleted with the rest of `structural_invariants.rs`. The file holds just this one test plus a brief doc comment explaining why it's separate. A future iteration can add a `generated_parse_rejection` step-def and migrate this test if desired; v1 keeps it as a standalone test.

## Order of work (PAR-revised)

1. Scaffold `fmpl-scenario-runner` crate: Cargo.toml (deps: `inventory = "0.3"`), lib.rs stub, workspace member registration.
2. Implement `corpus.rs` with fixture-driven TDD.
3. Implement `step_def.rs` (trait + inventory plumbing) with synthetic step-def tests.
4. Implement `error.rs` (StepError, DispatchError, CorpusError — ALL three with `Display` impls per PAR finding #6).
5. Add the codegen path to `fmpl-core/build.rs`. Use `env!("CARGO_MANIFEST_DIR")` (compile-time absolute path) for the `corpus()` helper, NOT a runtime relative path. Use `cargo::rerun-if-changed` (double-colon, Rust 2024) consistent with existing build.rs.
6. Move comment-strip helper from `structural_invariants.rs` to `tests/common/comment_strip.rs`.
7. Implement the three step-defs in `tests/steps/`: parse_rejection, parse_success, grep_invariant (handles both `expect_absent` and `expect_present` action types).
8. Migrate SCENARIO-0104, 0105, 0106 cards to the new structured format. **For SCENARIO-0106 specifically: 8 existing cases + 1 new case for grep #9 (`Type::Tagged` absent from `fmpl-core/src/`). Total 9 cases for that card.**
9. Relocate `g3_postlude_arms_fire_on_poison_nodes` to `fmpl-core/tests/postlude_arm_contract.rs`.
10. Run `scenario_runner`; verify ≥20 evidence tests pass (19 existing + 1 new grep #9, plus the relocated g3-test in its own binary).
11. Delete the migrated body of `structural_invariants.rs` (but NOT the g3 test, which moved in step 9).
12. Update `behavior-corpus.md` execution commands: SCENARIO-0104/0105/0106 point at `scenario_runner`; SCENARIO-0106 includes a note about grep #9 being added in this iteration; the postlude-arm test gets its own corpus entry if not already covered.
13. Update `no_legacy_fmpl_syntax.rs` exclusions: add `scenario_runner.rs` and `postlude_arm_contract.rs` (the latter intentionally contains `":Foo(1)"` parser fixtures).
14. Update progress.md and iteration-log.md.

## Acceptance criteria (PAR-revised)

**Functional:**
- `cargo test -p fmpl-core --test scenario_runner` reports ≥20 passing tests, 0 failed: 6 (SCENARIO-0104) + 4 (SCENARIO-0105) + 9 (SCENARIO-0106 including new grep #9) + 1 (corpus_health_check skipped-summary informational test) = 20+.
- `cargo test -p fmpl-core --test scenario_runner scenario_0104` filters correctly to the 6 SCENARIO-0104 cases.
- `cargo test -p fmpl-core --test postlude_arm_contract` reports 1 passing test (the relocated g3-test).
- A failing case prints `behavior-scenarios.md:NN-MM (SCENARIO-NNNN case M): <msg>` with a clear message.
- `structural_invariants.rs` is deleted (the g3-test moved to its own file).
- `fmpl-core/tests/common/comment_strip.rs` retains the comment-strip helper.
- `fmpl-scenario-runner` crate has its own passing tests (corpus parser tests + step-def dispatch tests).

**SCENARIO-0106 grep #9 specific:**
- The SCENARIO-0106 card in `behavior-scenarios.md` includes a 9th case: `action: expect_absent, needle: Type::Tagged, scope: fmpl-core/src/`.
- That case runs and passes via the scenario runner (which proves the ITER-0004h `Type::Tagged` deletion is ratcheted).

**Build hygiene:**
- Full workspace `cargo test` is green.
- `cargo test -p fmpl-core --test no_legacy_fmpl_syntax` still passes (excludes updated for `scenario_runner.rs` and `postlude_arm_contract.rs`).
- `cargo clippy --all-targets --quiet -- -D warnings` clean (pre-commit hook requirement).
- All other sentinels untouched: ast_to_ir_parity, scenario_0103, tavern_demo, no_legacy_fmpl_syntax, canonical_pipeline_parity, opcode_rename_evidence, type_inference, diagnostics_fmpl_source_scan.

## Risks and mitigations

- **`inventory` cross-crate visibility.** Step-defs in `tests/steps/*.rs` only register if the test binary's `mod steps;` declaration is present. Mitigation: `scenario_runner.rs` declares `mod steps;` at the top; the build.rs codegen does not assume otherwise.
- **Corpus parser brittleness on existing cards.** Most of the ~80 existing cards do not have `**Action type:**` and use free-form narrative. The parser must tolerate this and skip such cards without erroring. Mitigation: parse cards leniently; only complain about a card if it declares `**Action type:**` AND then has malformed `**Cases:**`.
- **`cargo:rerun-if-changed` performance.** Rebuilding test infrastructure every time `behavior-scenarios.md` changes is desired but cheap (a single ~60KB file scan). Acceptable cost.
- **Step-def-to-card field-mismatch errors at runtime.** A step-def that expects `source` but the case has `src` produces a runtime failure rather than a compile-time one. Mitigation: each step-def emits a clear `case missing required field: <name>` StepError; this is a small price for the data-driven design.

## Out of scope (deferred)

- Migration of scenarios beyond SCENARIO-0104/0105/0106. Migration is opt-in; future iterations can add cards as their action types become defined.
- A grammar-based scenario parser written in FMPL (on-brand with DESIGN-001 metacircular; a future iteration).
- Parameterized step-defs with fixture libraries.
- Output formatters beyond `cargo test` default.
- Coverage gates (e.g., "every scenario in the corpus must have an action_type by 2026-06-01"). Future iteration.

## Origin and references

- ITER-0004d.1 T19 user review (2026-05-12): "the tests in rust would be required to be even more minimal than what you've got there, as it should just be a driver for the scenario list, like the cucumber system, or the fitnesse.org SLIM framework."
- ITER-0004d.1 T19 user request (2026-05-12): "The scenario runner should probably also emit the span of line numbers for the test cases running."
- `fmpl-core/tests/structural_invariants.rs` — first consumer; deleted at iteration end.
- `docs/superpowers/iterations/behavior-scenarios.md` — corpus source.
- `docs/superpowers/iterations/behavior-corpus.md` — execution index updated by iteration.
- `docs/superpowers/iterations/roadmap.md` — ITER-0004d.4 entry contains additional rationale.
