//! Parser debugging utilities for FMPL.
//!
//! Provides detailed inspection of parser state, tokenization, and AST structure
//! for debugging parsing issues.

use crate::lexer::Lexer;
use crate::parser::Parser;
use std::fmt;

/// Detailed tokenization information for debugging.
#[derive(Debug, Clone)]
pub struct TokenDebugInfo {
    pub position: usize,
    pub token: String,
    pub span_start: usize,
    pub span_end: usize,
    pub source_preview: String,
}

/// Parse trace entry for step-by-step debugging.
#[derive(Debug, Clone)]
pub struct ParseTrace {
    pub step: usize,
    pub parser_position: usize,
    pub action: String,
    pub current_token: Option<String>,
}

/// Parse debug result containing all diagnostic information.
#[derive(Debug, Clone)]
pub struct ParseDebug {
    pub tokens: Vec<TokenDebugInfo>,
    pub trace: Vec<ParseTrace>,
    pub error_message: Option<String>,
    pub success: bool,
}

impl fmt::Display for TokenDebugInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:3}] {:20} @ {:4}:{:<4} | {}",
            self.position, self.token, self.span_start, self.span_end, self.source_preview
        )
    }
}

impl fmt::Display for ParseTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Step {:3}: pos={:<3} | {:20} | token={:?}",
            self.step, self.parser_position, self.action, self.current_token
        )
    }
}

impl fmt::Display for ParseDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Parse Debug Result ===")?;
        writeln!(f, "Success: {}", self.success)?;

        if let Some(ref error) = self.error_message {
            writeln!(f, "Error: {}", error)?;
        }

        writeln!(f, "\n=== Tokenization ({}) tokens ===", self.tokens.len())?;
        for token in &self.tokens {
            writeln!(f, "{}", token)?;
        }

        if !self.trace.is_empty() {
            writeln!(f, "\n=== Parse Trace ({}) steps ===", self.trace.len())?;
            for trace in &self.trace {
                writeln!(f, "{}", trace)?;
            }
        }

        Ok(())
    }
}

/// Debug tokenization of source code.
pub fn debug_tokenize(source: &str) -> Vec<TokenDebugInfo> {
    let lexer = Lexer::new(source);
    match lexer.tokenize() {
        Ok(tokens) => {
            tokens
                .iter()
                .enumerate()
                .map(|(i, st)| {
                    let source_preview = &source[st.span.clone()];
                    let preview_len = 30.min(source_preview.len());
                    let preview = if source_preview.len() > preview_len {
                        format!("{}...", &source_preview[..preview_len])
                    } else {
                        source_preview.to_string()
                    };
                    // Escape newlines for display
                    let preview = preview.replace('\n', "\\n").replace('\r', "\\r");

                    TokenDebugInfo {
                        position: i,
                        token: format!("{:?}", st.token),
                        span_start: st.span.start,
                        span_end: st.span.end,
                        source_preview: preview,
                    }
                })
                .collect()
        }
        Err(e) => {
            vec![TokenDebugInfo {
                position: 0,
                token: format!("ERROR: {}", e),
                span_start: 0,
                span_end: 0,
                source_preview: String::new(),
            }]
        }
    }
}

/// Debug parse of source code with detailed tracing.
pub fn debug_parse(source: &str, _with_trace: bool) -> ParseDebug {
    let tokens_info = debug_tokenize(source);

    // Try to parse
    let lexer = Lexer::new(source);
    let parse_result = lexer
        .tokenize()
        .and_then(|tokens| Parser::with_source(&tokens, source).parse());

    match parse_result {
        Ok(_) => ParseDebug {
            tokens: tokens_info,
            trace: vec![], // TODO: collect trace if with_trace
            error_message: None,
            success: true,
        },
        Err(e) => ParseDebug {
            tokens: tokens_info,
            trace: vec![],
            error_message: Some(format!("{}", e)),
            success: false,
        },
    }
}

/// Format source with line numbers for easier debugging.
pub fn format_with_lines(source: &str) -> String {
    source
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{:4}: {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Show a context window around a position in source.
pub fn show_context(source: &str, position: usize, window: usize) -> String {
    let start = position.saturating_sub(window);
    let _end = (position + window).min(source.len());

    let mut result = String::new();
    result.push_str(&format!("Position {} ({}):\n", position, source.len()));

    if start > 0 {
        result.push_str(&format!("...{}\n", &source[start..position]));
    } else {
        result.push_str(&format!("{}\n", &source[..position]));
    }

    // Show marker under the position
    let marker = if position < source.len() {
        let _char_width = source[position..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        format!("{}^", " ".repeat(position.saturating_sub(start)))
    } else {
        "<EOF>".to_string()
    };

    result.push_str(&marker);
    result.push('\n');

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_tokenize() {
        let source = "let x = 42";
        let debug = debug_tokenize(source);
        assert_eq!(debug.len(), 4);
        assert_eq!(debug[0].token, "Let");
        assert_eq!(debug[1].token, "Ident(\"x\")");
    }

    #[test]
    fn test_debug_parse_success() {
        let source = "1 + 1";
        let debug = debug_parse(source, false);
        assert!(debug.success);
        assert!(debug.error_message.is_none());
    }

    #[test]
    fn test_debug_parse_error() {
        let source = "1 +";
        let debug = debug_parse(source, false);
        assert!(!debug.success);
        assert!(debug.error_message.is_some());
    }
}
