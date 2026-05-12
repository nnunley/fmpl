//! `parse_success` step-def: assert that a source string parses successfully
//! through the Rust parser.
//!
//! Consumed by the control cases of SCENARIO-0104 (`:Foo` bare symbol,
//! `[:Foo, 1, 2]` list form) and SCENARIO-0105 (`[:Pair, a, b]` list-pattern
//! form). Required field:
//!   - `source` (String) — the FMPL source the parser must accept.
//!
//! The AST result is discarded; only acceptance is verified.

use fmpl_core::lexer::Lexer;
use fmpl_core::parser::Parser;
use fmpl_scenario_runner::corpus::{Card, Case};
use fmpl_scenario_runner::error::StepError;
use fmpl_scenario_runner::step_def::{StepDef, StepDefRegistration};

pub struct ParseSuccess;

impl StepDef for ParseSuccess {
    fn action_type(&self) -> &'static str {
        "parse_success"
    }

    fn run(&self, _card: &Card, case: &Case) -> Result<(), StepError> {
        let source = case
            .fields
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StepError::new("parse_success: case missing required field `source`"))?;

        let result: fmpl_core::error::Result<fmpl_core::ast::Expr> = (|| {
            let tokens = Lexer::new(source).tokenize()?;
            Parser::with_source(&tokens, source).parse()
        })();

        result.map(|_| ()).map_err(|e| {
            StepError::new(format!(
                "parse_success: expected `{source}` to parse, but got: {e:?}"
            ))
        })
    }
}

inventory::submit! { StepDefRegistration(&ParseSuccess) }
