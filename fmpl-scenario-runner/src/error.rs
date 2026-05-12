//! Error types for the scenario runner.
//!
//! - `StepError`: a single step-def returned Err during execution.
//! - `DispatchError`: dispatch failed (unknown action_type, or wraps a StepError).
//! - `CorpusError`: markdown corpus parsing failed.
//!
//! All three have `Display` impls because the codegen-generated #[test]
//! functions use `{}` format on DispatchError in panic messages.

use std::fmt;

/// Returned by a step-def when an assertion fails.
#[derive(Debug)]
pub struct StepError {
    pub message: String,
}

impl StepError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Returned by `dispatch` when the action_type isn't registered or the
/// matched step-def returned Err.
#[derive(Debug)]
pub enum DispatchError {
    /// action_type didn't match any registered StepDef
    Unknown(String),
    /// the matched step-def returned Err
    Step(StepError),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DispatchError::Unknown(action_type) => write!(
                f,
                "unknown action_type {action_type:?} — register a StepDef impl in tests/steps/"
            ),
            DispatchError::Step(step_error) => write!(f, "{step_error}"),
        }
    }
}

/// Returned by `parse_corpus` when the markdown corpus is malformed.
#[derive(Debug)]
pub enum CorpusError {
    Io(std::io::Error),
    Malformed {
        line: usize,
        message: String,
    },
    DuplicateId {
        id: String,
        first_line: usize,
        dup_line: usize,
    },
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorpusError::Io(e) => write!(f, "io error: {e}"),
            CorpusError::Malformed { line, message } => {
                write!(f, "malformed card at line {line}: {message}")
            }
            CorpusError::DuplicateId {
                id,
                first_line,
                dup_line,
            } => write!(
                f,
                "duplicate scenario id {id} at line {dup_line} (first defined at line {first_line})"
            ),
        }
    }
}

// Optional: error trait
impl std::error::Error for StepError {}
impl std::error::Error for DispatchError {}
impl std::error::Error for CorpusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CorpusError::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_error_displays_message() {
        let e = StepError::new("expected X, got Y");
        assert_eq!(format!("{e}"), "expected X, got Y");
    }

    #[test]
    fn dispatch_error_unknown_displays_action_type() {
        let e = DispatchError::Unknown("foo_bar".to_string());
        let s = format!("{e}");
        assert!(s.contains("foo_bar"), "got: {s}");
        assert!(s.contains("tests/steps/"), "got: {s}");
    }

    #[test]
    fn dispatch_error_step_wraps_step_error_display() {
        let e = DispatchError::Step(StepError::new("inner message"));
        assert_eq!(format!("{e}"), "inner message");
    }

    #[test]
    fn corpus_error_malformed_includes_line_and_message() {
        let e = CorpusError::Malformed {
            line: 42,
            message: "missing field".to_string(),
        };
        let s = format!("{e}");
        assert!(s.contains("42"), "got: {s}");
        assert!(s.contains("missing field"), "got: {s}");
    }

    #[test]
    fn corpus_error_duplicate_id_includes_both_lines() {
        let e = CorpusError::DuplicateId {
            id: "SCENARIO-0001".to_string(),
            first_line: 10,
            dup_line: 50,
        };
        let s = format!("{e}");
        assert!(s.contains("SCENARIO-0001"), "got: {s}");
        assert!(s.contains("10"), "got: {s}");
        assert!(s.contains("50"), "got: {s}");
    }
}
