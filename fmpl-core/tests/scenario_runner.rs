//! Data-driven scenario test runner (ITER-0004d.4).
//!
//! This file is intentionally TINY. The real test functions are generated
//! at build time by `fmpl-core/build.rs::generate_scenario_tests` from the
//! markdown corpus at `docs/superpowers/iterations/behavior-scenarios.md`.
//!
//! Architecture:
//! - `docs/superpowers/iterations/behavior-scenarios.md` is the source of
//!   truth. Each scenario card has a `**Action type:**` line and a
//!   `**Cases:**` block per the data-driven runner's card format.
//! - `fmpl-core/build.rs` calls `fmpl_scenario_runner::corpus::parse_corpus`
//!   at build time and emits one `#[test]` per case into
//!   `OUT_DIR/scenarios_generated.rs`.
//! - The generated tests call `fmpl_scenario_runner::step_def::dispatch`
//!   which walks the `inventory::iter::<StepDefRegistration>` registry.
//! - The step-defs in `tests/steps/*.rs` register themselves via
//!   `inventory::submit!`. The `mod steps;` declaration here brings them
//!   into the test binary's link set.
//! - The `mod common;` declaration brings in the shared comment-strip
//!   helper that `grep_invariant` uses.

mod common;
mod steps;

include!(concat!(env!("OUT_DIR"), "/scenarios_generated.rs"));
