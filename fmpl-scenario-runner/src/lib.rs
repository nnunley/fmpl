//! Data-driven scenario runner for FMPL behavior-corpus tests.
//!
//! Public API surface described in
//! `docs/superpowers/specs/2026-05-12-scenario-runner-design.md`.

pub mod corpus;
pub mod error;
pub mod step_def;

pub use corpus::{Card, Case, Value, parse_corpus};
pub use error::{CorpusError, DispatchError, StepError};
pub use step_def::{StepDef, StepDefRegistration, dispatch};
