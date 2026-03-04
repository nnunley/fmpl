//! Code generation context and configuration.
//!
//! Different code generation targets have different requirements:
//! - Parser generation: integrates with fmpl_core::Value (returns Result<Value>)
//! - Standalone programs: uses simplified Value (operations panic on error)
//! - Future: WebAssembly, other languages, etc.

/// Code generation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenTarget {
    /// Generate parser code for fmpl-core integration.
    /// Uses fmpl_core::Value which has fallible operations (Result<Value>).
    Parser,

    /// Generate standalone Rust program.
    /// Uses simplified Value with infallible operations (panics on error).
    Standalone,
    // Future targets:
    // WebAssembly,
    // JavaScript,
    // Python,
}

/// Code generation configuration.
#[derive(Debug, Clone)]
pub struct CodegenContext {
    /// Target for code generation.
    pub target: CodegenTarget,

    /// Whether to include debug tracing.
    pub debug: bool,

    /// Indentation level (for pretty-printing).
    pub indent: usize,
}

impl CodegenContext {
    /// Create context for parser code generation.
    pub fn parser() -> Self {
        Self {
            target: CodegenTarget::Parser,
            debug: false,
            indent: 0,
        }
    }

    /// Create context for standalone program generation.
    pub fn standalone() -> Self {
        Self {
            target: CodegenTarget::Standalone,
            debug: false,
            indent: 0,
        }
    }

    /// Enable debug tracing.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Check if this is parser code generation (needs Result unwrapping).
    pub fn is_parser(&self) -> bool {
        self.target == CodegenTarget::Parser
    }

    /// Check if this is standalone code generation (operations are infallible).
    pub fn is_standalone(&self) -> bool {
        self.target == CodegenTarget::Standalone
    }

    /// Generate result unwrapping code for parser mode.
    /// Returns ".unwrap()" for parser, "" for standalone.
    pub fn unwrap_call(&self) -> &'static str {
        if self.is_parser() { ".unwrap()" } else { "" }
    }

    /// Generate error handling for operations.
    /// - Parser mode: uses unwrap()
    /// - Standalone mode: operation returns value directly
    pub fn operation_result(&self, expr: &str) -> String {
        if self.is_parser() {
            format!("{}.unwrap()", expr)
        } else {
            expr.to_string()
        }
    }
}

/// Helper functions for generating Rust code.
pub struct RustBuilder;

impl RustBuilder {
    /// Generate a method call with appropriate error handling.
    pub fn method_call(
        context: &CodegenContext,
        receiver: &str,
        method: &str,
        arg: &str,
    ) -> String {
        let call = format!("({}).{}(&{})", receiver, method, arg);
        context.operation_result(&call)
    }

    /// Generate a binary operation.
    pub fn binary_op(context: &CodegenContext, op: &str, lhs: &str, rhs: &str) -> String {
        let call = format!("({}).{}(&{})", lhs, op, rhs);
        context.operation_result(&call)
    }

    /// Generate a unary operation.
    pub fn unary_op(context: &CodegenContext, op: &str, operand: &str) -> String {
        let call = format!("({}).{}()", operand, op);
        context.operation_result(&call)
    }

    /// Wrap expression in reference if needed.
    pub fn ref_expr(expr: &str) -> String {
        // If expr already has outer parens or is a simple variable, just add &
        format!("&{}", expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_targets() {
        let parser_ctx = CodegenContext::parser();
        let standalone_ctx = CodegenContext::standalone();

        assert!(parser_ctx.is_parser());
        assert!(!parser_ctx.is_standalone());
        assert_eq!(parser_ctx.unwrap_call(), ".unwrap()");

        assert!(standalone_ctx.is_standalone());
        assert!(!standalone_ctx.is_parser());
        assert_eq!(standalone_ctx.unwrap_call(), "");
    }

    #[test]
    fn test_builder_binary_op() {
        let parser_ctx = CodegenContext::parser();
        let standalone_ctx = CodegenContext::standalone();

        assert_eq!(
            RustBuilder::binary_op(&parser_ctx, "add", "a", "b"),
            "(a).add(&b).unwrap()"
        );

        assert_eq!(
            RustBuilder::binary_op(&standalone_ctx, "add", "a", "b"),
            "(a).add(&b)"
        );
    }
}
