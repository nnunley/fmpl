//! FMPL Core Library
//!
//! This crate provides the core components of the FMPL language:
//! - Lexer (tokenization using logos)
//! - Parser (recursive descent producing AST)
//! - Compiler (AST to Indexed RPN bytecode)
//! - VM (bytecode execution)
//! - Object database (in-memory prototype-based objects)
//! - Grammar (OMeta-style extensible PEG grammars)

// The fjall dependency is native-only (see Cargo.toml), so the persistence
// feature cannot be satisfied on wasm — fail loudly instead of half-compiling.
#[cfg(all(feature = "persistence", target_arch = "wasm32"))]
compile_error!("the `persistence` feature is not supported on wasm32 targets");

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
pub mod parser_epoch;
pub mod pattern;
pub mod persistence;
pub mod repr;
pub mod stream;
pub mod tuplespace;
pub mod types;
pub mod value;
pub mod vm;
pub mod vm_version;

pub use vm_version::{VM_VERSION, VM_VERSION_MAJOR, VM_VERSION_MINOR, VM_VERSION_PATCH};

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

/// Compile `source` through the native pipeline, persist the resulting
/// `CompiledCode` to `bytecode_store` at `key` (stamping the envelope's
/// `source_hash` against an entry in `source_store`), then execute the
/// code on `vm` and return the result.
///
/// This is the **persistence-aware sibling** of [`eval`]. `eval()` is
/// unchanged; production callers that don't want persistence keep
/// calling it. Callers that want compile-time bytecode persisted under
/// a recoverable source reference call this instead.
///
/// # Pipeline choice
///
/// Internally takes the **native compile path** (lexer → legacy parser
/// → Compiler → VM), not the `FMPL_USE_FMPL_COMPILER` pipeline. The
/// FMPL pipeline routes user source through `ast_to_ir.fmpl` via
/// `eval_via_legacy_parser` on a derived driver string; persisting that
/// `CompiledCode` would stamp the driver string's hash, not the user's
/// source. Persisting against the user's source is what
/// `source_hash`-based recovery needs, so we compile-once via the
/// native path. The `FMPL_USE_GENERATED_PARSER` opt-in for the
/// generated parser is honored — it changes the parser, not the
/// compiler — so the existing parity gates apply to persisted output.
///
/// # Errors
///
/// - Lex/parse/compile errors surface as `Error::Lexer` / `Error::Parser`
///   / `Error::Compiler`.
/// - Source-store insert and envelope-store insert failures surface as
///   `Error::BytecodePersistenceError`.
/// - Runtime errors during VM execution surface as `Error::Runtime`
///   (whatever the VM normally returns).
///
/// On error the persisted-bytecode state may be partially populated
/// (e.g. source bytes inserted but envelope insert failed). Callers
/// that care about atomicity must layer their own transactional wrapper.
#[cfg(feature = "persistence")]
pub fn eval_persistent(
    vm: &mut Vm,
    source: &str,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
    key: &str,
) -> Result<Value> {
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
    code.save_to_store(bytecode_store, source_store, key, Some(source.as_bytes()))?;
    vm.run(&code)
}

/// Walk `bytecode_store` and rebuild every envelope whose payload is
/// incompatible with the current VM major version by recompiling its
/// source from `source_store`. Returns the recovery taxonomy
/// ([`fmpl_persistence::RecoveryStats`]) so the caller can observe how
/// many records were recovered, were unrecoverable, or failed to
/// recompile.
///
/// This is the **fmpl-core-side orchestrator** that closes
/// fmpl-persistence's `recover_incompatible` over the running VM:
/// reuses the existing closure seam at `fmpl_persistence::recovery`,
/// converts the iteration `&[u8]` key back to a `&str`, and routes
/// through [`eval_persistent`] so the rebuilt record is stamped with a
/// fresh envelope (current VM version, same source_hash) and the
/// resulting value lands on `vm`.
///
/// Per-record errors (UTF-8 failure on the key, recompile failure)
/// are surfaced through `RecoveryError::recompile(...)` and counted
/// into `RecoveryStats.recompile_failed`. Backend (store I/O) errors
/// short-circuit and propagate as `Error::BytecodePersistenceError`.
///
/// # Why no new trait
///
/// `fmpl-persistence::recover_incompatible` already exposes a closure
/// (`FnMut(&[u8], &[u8]) -> Result<(), RecoveryError>`) as its
/// inversion-of-control seam. The project pattern at this layer is
/// closure parameters, not traits — wrapping the closure in a trait
/// would add ceremony without changing the inversion-of-control shape.
#[cfg(feature = "persistence")]
pub fn recover_and_rebind(
    vm: &mut Vm,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
) -> Result<fmpl_persistence::RecoveryStats> {
    use fmpl_persistence::{RecoveryError, recover_incompatible};

    let stats = recover_incompatible(
        bytecode_store,
        source_store,
        VM_VERSION_MAJOR,
        |key_bytes, src_bytes| {
            let key = std::str::from_utf8(key_bytes).map_err(RecoveryError::recompile)?;
            let source = std::str::from_utf8(src_bytes).map_err(RecoveryError::recompile)?;
            eval_persistent(vm, source, bytecode_store, source_store, key)
                .map(|_| ())
                .map_err(|e| RecoveryError::recompile(std::io::Error::other(e.to_string())))
        },
    )
    .map_err(|e| Error::BytecodePersistenceError(format!("recover_incompatible: {e}")))?;
    Ok(stats)
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
