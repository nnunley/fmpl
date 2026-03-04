//! Error types for FMPL.

use thiserror::Error;

/// Result type alias for FMPL operations.
pub type Result<T> = std::result::Result<T, Error>;

/// FMPL error types.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    #[error("Lexer error at position {position}: {message}")]
    Lexer { position: usize, message: String },

    #[error("Parser error at token {token}: {message}")]
    Parser { token: usize, message: String },

    #[error("Unexpected end of input")]
    UnexpectedEof,

    #[error("Compiler error: {0}")]
    Compiler(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Type error: expected {expected}, got {got}")]
    Type { expected: String, got: String },

    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Undefined property: {0}")]
    UndefinedProperty(String),

    #[error("Undefined method: {0}")]
    UndefinedMethod(String),

    #[error("Object not found: {0}")]
    ObjectNotFound(u64),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Stack underflow")]
    StackUnderflow,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Grammar not found: {0}")]
    GrammarNotFound(String),

    #[error("Rule not found: {rule} in grammar {grammar}")]
    RuleNotFound { grammar: String, rule: String },

    #[error("Parse failed at position {position}: {message}")]
    ParseFailed { position: usize, message: String },

    #[error("Object persistence error: {0}")]
    ObjectPersistenceError(String),
}

impl Error {
    /// Returns true if this error indicates incomplete input that might be continued.
    pub fn is_incomplete(&self) -> bool {
        matches!(self, Error::UnexpectedEof)
    }
}
