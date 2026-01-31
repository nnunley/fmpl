//! FMPL Core Library
//!
//! This crate provides the core components of the FMPL language:
//! - Lexer (tokenization using logos)
//! - Parser (recursive descent producing AST)
//! - Compiler (AST to Indexed RPN bytecode)
//! - VM (bytecode execution)
//! - Object database (in-memory prototype-based objects)
//! - Grammar (OMeta-style extensible PEG grammars)

pub mod ast;
pub mod builtins;
pub mod bytecode;
pub mod compiler;
pub mod debug;
pub mod error;
pub mod grammar;
pub mod instructions;
pub mod json;
pub mod lexer;
pub mod object;
pub mod parser;
pub mod repr;
pub mod stream;
pub mod tuplespace;
pub mod value;
pub mod vm;

pub use ast::Expr;
pub use compiler::{CompiledCode, Compiler};
pub use error::{Error, Result};
pub use grammar::{Grammar, GrammarRegistry, Pattern, Rule};
pub use lexer::{Lexer, Token};
pub use object::{Object, ObjectDb, ObjectId};
pub use parser::Parser;
pub use repr::{SourceRepr, object_source_repr};
pub use stream::{SinkHandle, StreamEvent, StreamHandle};
pub use value::Value;
pub use vm::Vm;

/// Evaluate FMPL source code and return the result.
///
/// Uses the generated scannerless parser from fmpl_parser.fmpl.
/// Set the environment variable `FMPL_USE_LEGACY_PARSER=1` to use the
/// legacy hand-written recursive descent parser.
pub fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let use_legacy = std::env::var("FMPL_USE_LEGACY_PARSER")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    let ast = if use_legacy {
        let tokens = Lexer::new(source).tokenize()?;
        Parser::with_source(&tokens, source).parse()?
    } else {
        parser::generated_parse(source)?
    };
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

/// Check if source code is syntactically complete.
/// Returns Ok(true) if the code is complete and can be evaluated.
/// Returns Ok(false) if more input is needed (e.g., unclosed brackets).
/// Returns Err if there's a syntax error that can't be fixed by more input.
pub fn is_complete(source: &str) -> Result<bool> {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(t) => t,
        Err(e) => {
            // Lexer errors at the very end might indicate incomplete input
            if e.is_incomplete() {
                return Ok(false);
            }
            return Err(e);
        }
    };

    let token_count = tokens.len();

    match Parser::with_source(&tokens, source).parse() {
        Ok(_) => Ok(true),
        Err(e) if e.is_incomplete() => Ok(false),
        Err(Error::Parser { token, message }) => {
            // If the error is at or near the end and mentions expected tokens,
            // it might be incomplete input rather than a syntax error
            if token >= token_count.saturating_sub(1) {
                // Error at the last token or beyond - likely incomplete
                let incomplete_messages = [
                    "expected RBracket",
                    "expected RParen",
                    "expected RBrace",
                    "expected",        // Any "expected X" at end of input
                    "unmatched brace", // Grammar body not closed
                ];
                if incomplete_messages.iter().any(|m| message.contains(m)) {
                    return Ok(false);
                }
            }
            Err(Error::Parser { token, message })
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_complete() {
        // Complete expressions
        assert!(is_complete("1 + 2").unwrap());
        assert!(is_complete("[1, 2]").unwrap());
        assert!(is_complete("let x = 1").unwrap());

        // Incomplete expressions
        assert!(!is_complete("[").unwrap());
        assert!(!is_complete("[1,").unwrap());
        assert!(!is_complete("let x = [").unwrap());
        assert!(!is_complete("let x = [\n1,").unwrap());

        // Multiline complete
        assert!(is_complete("[\n1,\n2\n]").unwrap());
    }
}
