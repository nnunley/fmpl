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
pub mod error;
pub mod grammar;
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
pub fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}
