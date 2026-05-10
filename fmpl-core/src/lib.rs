//! FMPL Core Library
//!
//! This crate provides the core components of the FMPL language:
//! - Lexer (tokenization using logos)
//! - Parser (recursive descent producing AST)
//! - Compiler (AST to Indexed RPN bytecode)
//! - VM (bytecode execution)
//! - Object database (in-memory prototype-based objects)
//! - Grammar (OMeta-style extensible PEG grammars)

#[macro_use]
pub mod macros;
pub mod ast;
pub mod builtins;
pub mod bytecode;
pub mod compiler;
#[cfg(feature = "cross_compile")]
pub mod cross_compile;
pub mod debug;
pub mod diagnostics;
pub mod error;
pub mod grammar;
pub mod instructions;
pub mod ir_builder;
pub mod json;
pub mod lexer;
pub mod object;
pub mod parse_stream;
pub mod parser;
pub mod pattern;
pub mod repr;
pub mod stream;
pub mod tuplespace;
pub mod types;
pub mod value;
pub mod vm;

pub use ast::Expr;
pub use compiler::{CompiledCode, Compiler, Instruction};
pub use error::{Error, Result};
pub use grammar::{Grammar, GrammarRegistry, Rule};
pub use lexer::{Lexer, Token};
pub use object::{Object, ObjectDb, ObjectId};
pub use parse_stream::ParseStream;
pub use parser::Parser;
pub use pattern::Pattern;
pub use repr::{SourceRepr, object_source_repr};
pub use stream::{SinkHandle, StreamEvent, StreamHandle};
pub use value::Value;
pub use vm::Vm;

/// Evaluate FMPL source code and return the result.
///
/// Pipeline selection (in priority order):
/// - `FMPL_USE_FMPL_COMPILER=1` — route source through the FMPL pipeline
///   (ast::parse → ast_to_ir.fmpl → ir::compile → code::eval).
/// - `FMPL_USE_LEGACY_PARSER=1` — use the hand-written parser instead of
///   the generated parser. Compiler and VM are unchanged.
/// - default — use the generated scannerless parser → native Compiler → VM.
pub fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    if std::env::var("FMPL_USE_FMPL_COMPILER")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return eval_via_fmpl_pipeline(vm, source);
    }

    eval_via_native(vm, source)
}

/// Evaluate via the native pipeline: parser → Compiler (Rust) → bytecode → VM.
///
/// Defaults to the legacy hand-written parser because the generated parser
/// can drift across rebuilds of fmpl-bootstrap (see bootstrap_determinism.rs).
/// Set `FMPL_USE_GENERATED_PARSER=1` to opt into the generated parser when
/// you've verified bootstrap output is current.
///
/// The compiler (`crate::compiler::Compiler`) and VM are the same
/// Rust-implemented runtime regardless of parser choice.
pub fn eval_via_native(vm: &mut Vm, source: &str) -> Result<Value> {
    let use_generated = std::env::var("FMPL_USE_GENERATED_PARSER")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    let ast = if use_generated {
        parser::generated_parse(source)?
    } else {
        let tokens = Lexer::new(source).tokenize()?;
        Parser::with_source(&tokens, source).parse()?
    };
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

/// Backwards-compatible alias for `eval_via_native`.
///
/// Older code referred to this path as the "Rust compiler" path, contrasting
/// with the FMPL pipeline. The name was misleading because the only thing
/// "Rust" about it was the parser-and-compiler implementation language —
/// `eval_via_fmpl_pipeline` also ultimately runs through the same compiler
/// and VM. Prefer `eval_via_native` in new code.
pub fn eval_via_rust_compiler(vm: &mut Vm, source: &str) -> Result<Value> {
    eval_via_native(vm, source)
}

/// Evaluate via the FMPL pipeline:
/// ast::parse → ast_to_ir.fmpl → ir::compile → code::eval.
///
/// Loads prelude.fmpl and ast_to_ir.fmpl into the VM on first call (cached
/// per-VM via a sentinel binding). User source compiles through this
/// pipeline; the wrapper code that drives the pipeline uses the legacy
/// parser to avoid sensitivity to generated-parser regressions.
pub fn eval_via_fmpl_pipeline(vm: &mut Vm, source: &str) -> Result<Value> {
    use smol_str::SmolStr;

    let bootstrap_marker = SmolStr::new("__fmpl_pipeline_bootstrapped");
    let already_bootstrapped = eval_via_legacy_parser(vm, bootstrap_marker.as_str())
        .ok()
        .filter(|v| !matches!(v, Value::Null))
        .is_some();
    if !already_bootstrapped {
        eval_via_legacy_parser(vm, r#"io::load("lib/core/prelude.fmpl")"#)?;
        eval_via_legacy_parser(vm, r#"io::load("lib/core/ast_to_ir.fmpl")"#)?;
        // ITER-0004c item 4: load ast_optimizer.fmpl. The file ends with a
        // bare module-map literal (no internal `let ast_optimizer = ...`),
        // so we must wrap with `let ast_optimizer = ...` to capture the map
        // under a name. Bracket-index `ast_optimizer["optimize"]` is the
        // verified-working access form (string-keyed lookup).
        eval_via_legacy_parser(
            vm,
            r#"let ast_optimizer = io::load("lib/core/ast_optimizer.fmpl")"#,
        )?;
        eval_via_legacy_parser(vm, &format!("let {} = true", bootstrap_marker))?;
    }

    // ITER-0004c item 4: thread ast_optimizer["optimize"] between ast::parse
    // and ast_to_ir.expr. Pipeline order:
    //   ast::parse → ast_optimizer["optimize"] → ast_to_ir.expr → ir::compile → code::eval
    let pipeline_source = format!(
        r#"let (ast = ast::parse({:?})) let (opt = ast_optimizer["optimize"](ast)) let (ir = opt @ ast_to_ir.expr) let (code = ir::compile(ir)) code::eval(code)"#,
        source
    );
    eval_via_legacy_parser(vm, &pipeline_source)
}

/// Evaluate using the legacy hand-written parser unconditionally.
/// Used internally for bootstrap-sensitive code paths and externally as
/// an escape hatch when the generated parser has regressed.
pub fn eval_via_legacy_parser(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

/// Check if source code is syntactically complete.
/// Returns Ok(true) if the code is complete and can be evaluated.
/// Returns Ok(false) if more input is needed (e.g., unclosed brackets).
/// Returns Err if there's a syntax error that can't be fixed by more input.
pub fn is_complete(source: &str) -> Result<bool> {
    match parser::generated_parse(source) {
        Ok(_) => return Ok(true),
        Err(Error::UnexpectedEof) => return Ok(false),
        Err(Error::Parser { token, message })
            if likely_incomplete_generated(source, token, &message) =>
        {
            return Ok(false);
        }
        Err(_) => {
            // Fall back to legacy completeness detection for compatibility while
            // generated parser parity is still in progress.
        }
    }

    is_complete_legacy(source)
}

fn likely_incomplete_generated(source: &str, token: usize, message: &str) -> bool {
    let at_or_near_end = token >= source.len().saturating_sub(1);
    if !at_or_near_end {
        return false;
    }

    let incomplete_markers = [
        "unexpected input at position",
        "expected",
        "unterminated",
        "unexpected eof",
    ];
    incomplete_markers
        .iter()
        .any(|marker| message.contains(marker))
}

fn is_complete_legacy(source: &str) -> Result<bool> {
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
