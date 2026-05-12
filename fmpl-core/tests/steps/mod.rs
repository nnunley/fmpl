//! Step-definition implementations for the data-driven scenario runner.
//!
//! Each submodule declares a struct implementing `StepDef` (from
//! `fmpl_scenario_runner::step_def`) and registers it via `inventory::submit!`.
//! The codegen-emitted `#[test]` functions in `scenario_runner.rs` call
//! `fmpl_scenario_runner::step_def::dispatch(card, case)` which walks the
//! inventory and finds the step-def whose `action_type` matches the case's
//! resolved action.
//!
//! `tests/scenario_runner.rs` (created in ITER-0004d.4 T9) declares both
//! `mod common;` and `mod steps;` so that:
//!   - the `grep_invariant` submodule below can reach
//!     `crate::common::comment_strip::*`, and
//!   - the `inventory::submit!` registrations here are link-reachable from
//!     the test binary.
//!
//! Until T9 lands, `tests/steps/` is unreachable from any test crate. The
//! crate-level `#![allow(dead_code, unused_imports)]` below suppresses the
//! resulting "unused" warnings; T9 wires this into `scenario_runner.rs` and
//! the suppressions can stay (they're harmless once the module is in use).

#![allow(dead_code, unused_imports)]

pub mod grep_invariant;
pub mod parse_rejection;
pub mod parse_success;
