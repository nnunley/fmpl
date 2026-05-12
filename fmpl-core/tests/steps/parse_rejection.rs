//! `parse_rejection` step-def: assert that a source string is rejected by
//! the Rust parser.
//!
//! Consumed by SCENARIO-0104 / SCENARIO-0105 cases. Each case declares:
//!
//! ```text
//! - source: `:Foo(1)`
//!   expect_error_contains:
//!     - `[:`
//!     - instead
//! ```
//!
//! Required field:
//!   - `source` (String) — the FMPL source to feed to the parser.
//!
//! Optional fields:
//!   - `expect_rejected` (Bool, default true) — if false, asserts success
//!     (callers should prefer the `parse_success` step-def for that).
//!   - `expect_error_contains` (List<String>) — each phrase must appear in
//!     the formatted error message (`format!("{e:?}")`). Used for the
//!     message-quality invariants in SCENARIO-0104 case 6 and SCENARIO-0105
//!     case 4 (rejection must point at the canonical `[:Tag, ...]` form).

use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;
use fmpl_scenario_runner::corpus::{Card, Case};
use fmpl_scenario_runner::error::StepError;
use fmpl_scenario_runner::step_def::{StepDef, StepDefRegistration};

pub struct ParseRejection;

impl StepDef for ParseRejection {
    fn action_type(&self) -> &'static str {
        "parse_rejection"
    }

    fn run(&self, _card: &Card, case: &Case) -> Result<(), StepError> {
        let source = case
            .fields
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                StepError::new("parse_rejection: case missing required field `source`")
            })?;

        let expect_rejected = case
            .fields
            .get("expect_rejected")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let result: fmpl_core::error::Result<fmpl_core::ast::Expr> = (|| {
            let tokens = Lexer::new(source).tokenize()?;
            Parser::with_source(&tokens, source).parse()
        })();

        if expect_rejected {
            match result {
                Ok(ast) => Err(StepError::new(format!(
                    "parse_rejection: expected rejection of `{source}`, but parse succeeded with AST: {ast:?}"
                ))),
                Err(e) => {
                    let msg = format!("{e:?}");
                    if let Some(phrases) = case
                        .fields
                        .get("expect_error_contains")
                        .and_then(|v| v.as_list())
                    {
                        for phrase in phrases {
                            let needle = phrase.as_str().ok_or_else(|| {
                                StepError::new(
                                    "parse_rejection: expect_error_contains entries must be strings",
                                )
                            })?;
                            if !msg.contains(needle) {
                                return Err(StepError::new(format!(
                                    "parse_rejection: error message of `{source}` does not contain {needle:?}.\nActual error: {msg}"
                                )));
                            }
                        }
                    }
                    Ok(())
                }
            }
        } else {
            result.map(|_| ()).map_err(|e| {
                StepError::new(format!(
                    "parse_rejection: expect_rejected=false for `{source}`, but parse failed: {e:?}"
                ))
            })
        }
    }
}

inventory::submit! { StepDefRegistration(&ParseRejection) }
