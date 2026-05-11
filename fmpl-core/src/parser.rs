//! Recursive descent parser for FMPL.
//!
//! This module contains both the legacy hand-written parser and the
//! generated parser (included at build time).

use crate::ast::*;
use crate::error::Error;
use crate::grammar::Grammar;
use crate::grammar::parser::GrammarParser;
use crate::lexer::{SpannedToken, Token};
// Re-exported into the generated parser module via `use super::*`.
#[allow(unused_imports)]
use crate::value::Value;
use smol_str::SmolStr;
#[allow(unused_imports)]
use std::sync::Arc;

// Local Result type for parser (same as crate::error::Result)
type Result<T> = std::result::Result<T, Error>;

// Include the generated parser at build time
// The generated_parse function provides an alternative entry point
include!(concat!(env!("OUT_DIR"), "/generated_parser.rs"));

// Compile-time check: the generator's epoch must match the source-of-record.
// If this fails, regenerate the parser (rebuild fmpl-bootstrap, then
// `cargo clean -p fmpl-core && cargo build`). See parser_epoch.rs.
//
// Wrapped in a cfg gate so the fallback parser (which omits
// GENERATED_PARSER_EPOCH) still builds — the build script panics with a
// clearer message when the real generator is available and produces stale
// output.
#[cfg(has_generated_parser_epoch)]
const _: () = {
    assert!(
        crate::parser_epoch::PARSER_EPOCH == GENERATED_PARSER_EPOCH,
        "parser-generator epoch mismatch: source-of-record disagrees with cached generated parser. Rebuild fmpl-bootstrap and regenerate (cargo clean -p fmpl-core)."
    );
};

/// Parser state.
pub struct Parser<'a> {
    tokens: &'a [SpannedToken],
    source: Option<&'a str>,
    pos: usize,
    /// True while parsing the body of a `pat => body` arm inside
    /// `parse_inline_pattern_block`. When set, `parse_postfix` treats a
    /// `[`-led token that is preceded by a newline in the source as the
    /// start of the next case (and stops consuming the body) rather than
    /// as an `Expr::Index` postfix on the current expression.
    in_pattern_case_body: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [SpannedToken]) -> Self {
        Self {
            tokens,
            source: None,
            pos: 0,
            in_pattern_case_body: false,
        }
    }

    pub fn with_source(tokens: &'a [SpannedToken], source: &'a str) -> Self {
        Self {
            tokens,
            source: Some(source),
            pos: 0,
            in_pattern_case_body: false,
        }
    }

    /// Return true if the source bytes between the previous token's end and
    /// the current token's start contain a newline. Returns false if either
    /// there's no source available or there's no preceding token (we're at
    /// the very start of input).
    fn newline_before_current_token(&self) -> bool {
        let Some(source) = self.source else {
            return false;
        };
        if self.pos == 0 || self.pos >= self.tokens.len() {
            return false;
        }
        let prev_end = self.tokens[self.pos - 1].span.end;
        let curr_start = self.tokens[self.pos].span.start;
        if curr_start <= prev_end {
            return false;
        }
        source
            .get(prev_end..curr_start)
            .is_some_and(|gap| gap.contains('\n'))
    }

    /// Parse a complete program (sequence of expressions/definitions).
    pub fn parse(&mut self) -> Result<Expr> {
        let mut exprs = Vec::new();

        while !self.is_at_end() {
            exprs.push(self.parse_toplevel()?);
            self.skip_semis();
        }

        if exprs.len() == 1 {
            Ok(exprs.pop().unwrap())
        } else {
            Ok(Expr::Sequence(exprs))
        }
    }

    /// Parse a top-level item (object def or expression).
    fn parse_toplevel(&mut self) -> Result<Expr> {
        if self.check(&Token::Object) {
            self.parse_object_def()
        } else {
            self.parse_expr()
        }
    }

    /// Parse an object definition.
    fn parse_object_def(&mut self) -> Result<Expr> {
        self.expect(&Token::Object)?;

        // Parse name (may be qualified or ^tag for bcom constructors)
        let is_constructor = matches!(self.peek_token(), Some(Token::ObjTag(_)));
        let name = if let Some(Token::ObjTag(s)) = self.peek_token().cloned() {
            self.advance();
            QualifiedName::simple(s)
        } else {
            self.parse_qualified_name()?
        };

        // Parse optional parameters
        let has_params = self.check(&Token::LParen);
        let params = if has_params {
            self.parse_param_names()?
        } else {
            Vec::new()
        };

        // Parse optional parent list (after params, before body)
        let parents = Vec::new(); // TODO: parse parent inheritance

        // Parse body
        self.expect(&Token::LBrace)?;
        let (bindings, facets) = self.parse_object_body()?;
        self.expect(&Token::RBrace)?;

        Ok(Expr::ObjectDef(ObjectDef {
            name,
            params,
            parents,
            bindings,
            facets,
            is_constructor,
        }))
    }

    /// Parse object body (bindings and facets).
    fn parse_object_body(&mut self) -> Result<(Vec<Binding>, Vec<FacetDef>)> {
        let mut bindings = Vec::new();
        let mut facets = Vec::new();
        let mut current_visibility = Visibility::Private;
        let mut in_facets = false;

        while !self.check(&Token::RBrace) && !self.is_at_end() {
            // Check for visibility markers
            if self.check(&Token::Dot) {
                self.advance();
                match self.peek_token() {
                    Some(Token::Private) => {
                        self.advance();
                        current_visibility = Visibility::Private;
                        in_facets = false;
                    }
                    Some(Token::Public) => {
                        self.advance();
                        current_visibility = Visibility::Public;
                        in_facets = false;
                    }
                    Some(Token::Protected) => {
                        self.advance();
                        current_visibility = Visibility::Protected;
                        in_facets = false;
                    }
                    Some(Token::Facets) => {
                        self.advance();
                        in_facets = true;
                    }
                    _ => {
                        return Err(self.error("expected visibility marker"));
                    }
                }
                self.skip_semis();
                continue;
            }

            if in_facets {
                // Parse facet definition
                let facet = self.parse_facet_def()?;
                facets.push(facet);
            } else {
                // Parse regular binding
                let binding = self.parse_binding(current_visibility)?;
                bindings.push(binding);
            }
            self.skip_semis();
        }

        Ok((bindings, facets))
    }

    /// Parse a binding (property or method).
    fn parse_binding(&mut self, visibility: Visibility) -> Result<Binding> {
        let name = self.expect_ident()?;

        // Check for method params
        let has_params = self.check(&Token::LParen);
        let params = if has_params {
            self.parse_param_names()?
        } else {
            Vec::new()
        };

        self.expect(&Token::Colon)?;
        let value = self.parse_expr()?;

        Ok(Binding {
            name,
            params,
            has_params,
            value,
            visibility,
        })
    }

    /// Parse a facet definition.
    fn parse_facet_def(&mut self) -> Result<FacetDef> {
        let name = self.expect_ident()?;

        // Check for terminal marker (!)
        let terminal = if self.check(&Token::Bang) {
            self.advance();
            true
        } else {
            false
        };

        self.expect(&Token::Colon)?;
        self.expect(&Token::LBracket)?;

        let mut members = Vec::new();
        while !self.check(&Token::RBracket) && !self.is_at_end() {
            members.push(self.expect_ident()?);
            if !self.check(&Token::RBracket) {
                self.expect(&Token::Comma)?;
            }
        }
        self.expect(&Token::RBracket)?;

        Ok(FacetDef {
            name,
            members,
            terminal,
        })
    }

    /// Parse parameter names.
    fn parse_param_names(&mut self) -> Result<Vec<SmolStr>> {
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();

        while !self.check(&Token::RParen) && !self.is_at_end() {
            params.push(self.expect_ident()?);
            if !self.check(&Token::RParen) {
                // Allow comma or space separation
                if self.check(&Token::Comma) {
                    self.advance();
                }
            }
        }
        self.expect(&Token::RParen)?;
        Ok(params)
    }

    /// Parse a qualified name (foo::bar::baz or ::foo::bar).
    fn parse_qualified_name(&mut self) -> Result<QualifiedName> {
        let mut parts = if self.check(&Token::ColonColon) {
            // Global qualified name: ::foo::bar
            self.advance();
            vec![SmolStr::new("")]
        } else {
            // Regular qualified name: foo::bar
            vec![self.expect_ident_or_keyword()?]
        };

        // Continue parsing additional ::name parts
        while self.check(&Token::ColonColon) {
            self.advance();
            parts.push(self.expect_ident_or_keyword()?);
        }

        Ok(QualifiedName { parts })
    }

    /// Parse an expression.
    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_assignment()
    }

    /// Parse assignment (lowest precedence).
    /// Assignment is right-associative: a = b = c means a = (b = c)
    fn parse_assignment(&mut self) -> Result<Expr> {
        let left = self.parse_pipe()?;

        // Check if this is an assignment
        if self.check(&Token::Eq) {
            self.advance();
            let right = self.parse_assignment()?; // Right-associative
            return Ok(Expr::Assignment(Box::new(left), Box::new(right)));
        }

        Ok(left)
    }

    /// Parse pipe and grammar application operators (lowest precedence).
    fn parse_pipe(&mut self) -> Result<Expr> {
        let mut left = self.parse_or()?;

        loop {
            if self.check(&Token::Pipe) {
                self.advance();
                let right = self.parse_or()?;
                left = Expr::Binary(Box::new(left), BinOp::Pipe, Box::new(right));
            } else if self.check(&Token::At) {
                // Grammar application: expr @ grammar_expr.rule
                // Or anonymous block: expr @ { pattern => action; ... }
                // Or inline pattern block: expr @ { %{a: b} => b, _ => default }
                self.advance();

                if self.check(&Token::LBrace) {
                    // Check if this looks like an inline pattern block (AST patterns)
                    // vs a grammar block (PEG patterns)
                    if self.is_inline_pattern_block() {
                        // Inline pattern block: expr @ { %{a: b} => b, _ => default }
                        let cases = self.parse_inline_pattern_block()?;
                        left = Expr::InlinePatternBlock {
                            input: Box::new(left),
                            cases,
                        };
                    } else {
                        // Anonymous grammar block: expr @ { digit* => result; ... }
                        let anon_grammar = self.parse_anonymous_grammar_block()?;
                        left = Expr::GrammarApply {
                            input: Box::new(left),
                            grammar: Box::new(Expr::GrammarLiteral(anon_grammar)),
                            rule: SmolStr::new("main"), // Anonymous blocks use "main" rule
                        };
                    }
                } else {
                    // Named grammar application: expr @ grammar_expr.rule
                    let grammar_expr = self.parse_postfix()?;

                    // The grammar_expr should end with .rule access
                    // Extract the rule name from the last PropAccess
                    let (grammar, rule) = match grammar_expr {
                        Expr::PropAccess(base, prop) => (*base, prop),
                        Expr::Qualified(qn) if qn.parts.len() >= 2 => {
                            // Handle qualified::name.rule case
                            // Last part is the rule name
                            let rule = qn.parts.last().unwrap().clone();
                            let grammar_parts = qn.parts[..qn.parts.len() - 1].to_vec();
                            (
                                Expr::Qualified(QualifiedName {
                                    parts: grammar_parts,
                                }),
                                rule,
                            )
                        }
                        _ => return Err(self.error("grammar application requires grammar.rule")),
                    };

                    left = Expr::GrammarApply {
                        input: Box::new(left),
                        grammar: Box::new(grammar),
                        rule,
                    };
                }
            } else if self.check(&Token::Inherits) {
                // Grammar extension: base <: { rules }
                self.advance();
                let rules = self.parse_grammar_body()?;
                left = Expr::GrammarExtend {
                    base: Box::new(left),
                    rules,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parse or (||).
    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;

        while self.check(&Token::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right));
        }

        Ok(left)
    }

    /// Parse and (&&).
    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;

        while self.check(&Token::AndAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right));
        }

        Ok(left)
    }

    /// Parse equality (== !=).
    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;

        loop {
            let op = if self.check(&Token::EqEq) {
                BinOp::Eq
            } else if self.check(&Token::NotEq) {
                BinOp::NotEq
            } else {
                break;
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    /// Parse comparison (< > <= >= in).
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_term()?;

        loop {
            let op = if self.check(&Token::Lt) {
                BinOp::Lt
            } else if self.check(&Token::Gt) {
                BinOp::Gt
            } else if self.check(&Token::LtEq) {
                BinOp::LtEq
            } else if self.check(&Token::GtEq) {
                BinOp::GtEq
            } else if self.check(&Token::In) {
                BinOp::In
            } else {
                break;
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    /// Parse term (+ -).
    fn parse_term(&mut self) -> Result<Expr> {
        let mut left = self.parse_factor()?;

        loop {
            let op = if self.check(&Token::Plus) {
                BinOp::Add
            } else if self.check(&Token::Minus) {
                BinOp::Sub
            } else {
                break;
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    /// Parse factor (* / %).
    fn parse_factor(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            let op = if self.check(&Token::Star) {
                BinOp::Mul
            } else if self.check(&Token::Slash) {
                BinOp::Div
            } else if self.check(&Token::Percent)
                && !self
                    .peek_ahead(1)
                    .is_some_and(|t| matches!(t.token, Token::LBrace))
            {
                // Guard: %{ is a map literal, not modulo followed by block
                BinOp::Mod
            } else {
                break;
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }

        Ok(left)
    }

    /// Parse unary (- !).
    fn parse_unary(&mut self) -> Result<Expr> {
        if self.check(&Token::Minus) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr)));
        }

        if self.check(&Token::Bang) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)));
        }

        if self.check(&Token::SyncCall) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::SyncCall(Box::new(expr)));
        }

        if self.check(&Token::AsyncCall) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::AsyncCall(Box::new(expr)));
        }

        self.parse_postfix()
    }

    /// Parse postfix (calls, property access, indexing).
    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(&Token::LParen) {
                // Function call
                let args = self.parse_args()?;
                expr = Expr::Call(Box::new(expr), args);
            } else if self.check(&Token::Dot) {
                self.advance();

                // Check for .as(:facet)
                if self.check(&Token::As) {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    let facet = self.expect_symbol()?;
                    self.expect(&Token::RParen)?;
                    expr = Expr::FacetAccess(Box::new(expr), facet);
                } else {
                    // Property or method access
                    // Use expect_ident_or_keyword to allow keywords like `in`, `map`, etc. as method names
                    let name = self.expect_ident_or_keyword()?;
                    if self.check(&Token::LParen) {
                        let args = self.parse_args()?;
                        expr = Expr::MethodCall(Box::new(expr), name, args);
                    } else {
                        expr = Expr::PropAccess(Box::new(expr), name);
                    }
                }
            } else if self.check(&Token::LBracket) {
                // Inside a `pat => body` arm of an inline pattern block, a
                // `[`-led token preceded by a newline starts the next case,
                // not an `Expr::Index` postfix on the current body. Bail out
                // of the postfix loop so the caller's loop in
                // `parse_inline_pattern_block` sees the `[` next.
                if self.in_pattern_case_body && self.newline_before_current_token() {
                    break;
                }
                self.advance();

                // Check for slice starting with .. (open start)
                if self.check(&Token::DotDot) {
                    self.advance();
                    // Check for open end too (full slice [..])
                    let end = if self.check(&Token::RBracket) {
                        None
                    } else {
                        Some(Box::new(self.parse_expr()?))
                    };
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Slice(Box::new(expr), None, end);
                } else {
                    let index = self.parse_expr()?;

                    // Check for slice
                    if self.check(&Token::DotDot) {
                        self.advance();
                        // Check for open end (slice from index to end)
                        let end = if self.check(&Token::RBracket) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        self.expect(&Token::RBracket)?;
                        expr = Expr::Slice(Box::new(expr), Some(Box::new(index)), end);
                    } else {
                        self.expect(&Token::RBracket)?;
                        expr = Expr::Index(Box::new(expr), Box::new(index));
                    }
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Parse function arguments.
    fn parse_args(&mut self) -> Result<Vec<Arg>> {
        self.expect(&Token::LParen)?;
        let mut args = Vec::new();

        while !self.check(&Token::RParen) && !self.is_at_end() {
            if self.check(&Token::Underscore) {
                self.advance();
                args.push(Arg::Placeholder);
            } else {
                args.push(Arg::Expr(self.parse_expr()?));
            }

            if !self.check(&Token::RParen) {
                self.expect(&Token::Comma)?;
            }
        }
        self.expect(&Token::RParen)?;
        Ok(args)
    }

    /// Parse primary expression.
    fn parse_primary(&mut self) -> Result<Expr> {
        let token = self.peek_token().ok_or(Error::UnexpectedEof)?;

        match token {
            Token::Int(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Int(n))
            }
            Token::Float(f) => {
                let f = *f;
                self.advance();
                Ok(Expr::Float(f))
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::String(s))
            }
            Token::Symbol(s) => {
                let s = s.clone();
                self.advance();
                // Reject legacy :Symbol(args) constructor syntax (DESIGN-002).
                if self.check(&Token::LParen) {
                    return Err(self.error(&format!(
                        "legacy :{}(...) constructor syntax is not supported; use [:{}] or [:{}, ...] instead",
                        s, s, s
                    )));
                }
                Ok(Expr::Symbol(s))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            Token::Self_ => {
                self.advance();
                Ok(Expr::Self_)
            }
            Token::Parent => {
                self.advance();
                Ok(Expr::Parent)
            }
            Token::Caller => {
                self.advance();
                Ok(Expr::Caller)
            }
            Token::User => {
                self.advance();
                Ok(Expr::User)
            }
            Token::Args => {
                self.advance();
                Ok(Expr::Args)
            }
            Token::Ident(s) => {
                let s = s.clone();
                self.advance();

                // Check for qualified name
                if self.check(&Token::ColonColon) {
                    let mut parts = vec![s];
                    while self.check(&Token::ColonColon) {
                        self.advance();
                        parts.push(self.expect_ident_or_keyword()?);
                    }
                    Ok(Expr::Qualified(QualifiedName { parts }))
                } else {
                    Ok(Expr::Ident(s))
                }
            }
            Token::ObjTag(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::ObjTag(s))
            }
            Token::FnTag(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::FnTag(s))
            }
            Token::Underscore => {
                self.advance();
                Ok(Expr::Placeholder)
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => self.parse_list(),
            Token::LBrace => self.parse_sequence(),
            Token::Percent => self.parse_map(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Do => self.parse_do_while(),
            Token::For => self.parse_for(),
            Token::Fold => self.parse_fold(),
            Token::Foldr => self.parse_foldr(),
            Token::Map => self.parse_map_each(),
            Token::Filter => self.parse_filter(),
            Token::Let => self.parse_let(),
            Token::Lambda => self.parse_lambda(),
            Token::Backslash => self.parse_short_lambda(),
            Token::Return => self.parse_return(),
            Token::Yield => self.parse_yield(),
            Token::Spawn => self.parse_spawn(),
            Token::New => self.parse_new(),
            Token::Match => self.parse_match(),
            Token::Try => self.parse_try_catch(),
            Token::Throw => self.parse_throw(),
            Token::Stream => {
                // Check if this is stream::... (module access)
                if self.peek_ahead_is_coloncolon() {
                    self.advance();
                    let mut parts = vec![SmolStr::new("stream")];
                    while self.check(&Token::ColonColon) {
                        self.advance();
                        parts.push(self.expect_ident_or_keyword()?);
                    }
                    Ok(Expr::Qualified(QualifiedName { parts }))
                } else if self.peek_ahead(1).map(|t| &t.token) == Some(&Token::LBrace) {
                    // stream { expr } - stream literal
                    self.parse_stream_literal()
                } else {
                    // stream as a bare identifier - refers to the stream builtin
                    self.advance();
                    Ok(Expr::Ident(SmolStr::new("stream")))
                }
            }
            Token::Grammar => {
                // Check if this is grammar::... (module access)
                if self.peek_ahead_is_coloncolon() {
                    self.advance();
                    let mut parts = vec![SmolStr::new("grammar")];
                    while self.check(&Token::ColonColon) {
                        self.advance();
                        parts.push(self.expect_ident_or_keyword()?);
                    }
                    Ok(Expr::Qualified(QualifiedName { parts }))
                } else if self.peek_ahead(1).map(|t| &t.token) == Some(&Token::LBrace) {
                    // grammar { rules } - anonymous grammar literal
                    self.parse_grammar_literal()
                } else if self
                    .peek_ahead(1)
                    .map(|t| matches!(&t.token, Token::Ident(_)))
                    .unwrap_or(false)
                {
                    // grammar Name { rules } - named grammar
                    self.parse_named_grammar()
                } else {
                    // grammar as a bare identifier - refers to the grammar builtin
                    self.advance();
                    Ok(Expr::Ident(SmolStr::new("grammar")))
                }
            }
            Token::ColonColon => {
                // Global qualified name: ::foo::bar
                self.advance();
                let mut parts = vec![SmolStr::new("")]; // Empty first part for global namespace
                parts.push(self.expect_ident_or_keyword()?);
                while self.check(&Token::ColonColon) {
                    self.advance();
                    parts.push(self.expect_ident_or_keyword()?);
                }
                Ok(Expr::Qualified(QualifiedName { parts }))
            }
            _ => Err(self.error(&format!("unexpected token: {:?}", token))),
        }
    }

    /// Parse grammar literal: `grammar { rules }`
    fn parse_grammar_literal(&mut self) -> Result<Expr> {
        self.expect(&Token::Grammar)?;

        // We need source access to parse grammar body
        let source = self
            .source
            .ok_or_else(|| self.error("grammar literals require source access"))?;

        // Find the opening brace token and its position
        let lbrace_pos = self.pos;
        self.expect(&Token::LBrace)?;
        let start = self.tokens[lbrace_pos].span.start;

        // Find matching closing brace (handling nesting)
        let mut depth = 1;
        while depth > 0 && !self.is_at_end() {
            match self.peek_token() {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                self.advance();
            }
        }

        if depth != 0 {
            return Err(self.error("unmatched brace in grammar literal"));
        }

        // Get the end position (including the closing brace)
        let end = self.tokens[self.pos].span.end;
        self.advance(); // consume the closing brace

        // Extract the grammar body source (from { to })
        let grammar_source = &source[start..end];

        // Parse using GrammarParser
        let mut grammar_parser = GrammarParser::new(grammar_source);
        let grammar = grammar_parser.parse_anonymous()?;

        Ok(Expr::GrammarLiteral(grammar))
    }

    /// Parse named grammar: `grammar Name { rules }` or `grammar Name <: Parent { rules }`
    fn parse_named_grammar(&mut self) -> Result<Expr> {
        // We need source access to parse grammar body
        let source = self
            .source
            .ok_or_else(|| self.error("named grammars require source access"))?;

        // Get the start position of "grammar" keyword
        let grammar_start = self.tokens[self.pos].span.start;
        self.expect(&Token::Grammar)?;

        // Expect the grammar name
        let _name = self.expect_ident()?;

        // Check for optional inheritance: <: ParentName
        if self.check(&Token::Inherits) {
            self.advance(); // consume <:
            // Consume the parent name (could be qualified like parent::name)
            self.expect_ident_or_keyword()?;
            while self.check(&Token::ColonColon) {
                self.advance();
                self.expect_ident_or_keyword()?;
            }
        }

        // Find the opening brace token
        self.expect(&Token::LBrace)?;

        // Find matching closing brace (handling nesting)
        let mut depth = 1;
        while depth > 0 && !self.is_at_end() {
            match self.peek_token() {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                self.advance();
            }
        }

        if depth != 0 {
            return Err(self.error("unmatched brace in named grammar"));
        }

        // Get the end position (including the closing brace)
        let end = self.tokens[self.pos].span.end;
        self.advance(); // consume the closing brace

        // Extract the full grammar source (from "grammar" to "}")
        let grammar_source = &source[grammar_start..end];

        // Parse using GrammarParser.parse() which expects "grammar Name { ... }"
        let mut grammar_parser = GrammarParser::new(grammar_source);
        let grammar = grammar_parser.parse()?;

        Ok(Expr::GrammarLiteral(grammar))
    }

    /// Parse stream literal: `stream { expr }`
    fn parse_stream_literal(&mut self) -> Result<Expr> {
        self.expect(&Token::Stream)?;
        let expr = self.parse_sequence()?;
        Ok(Expr::StreamLiteral(Box::new(expr)))
    }

    /// Parse grammar body: `{ rules }` (used by grammar literals and extensions)
    fn parse_grammar_body(&mut self) -> Result<Grammar> {
        // We need source access to parse grammar body
        let source = self
            .source
            .ok_or_else(|| self.error("grammar extensions require source access"))?;

        // Find the opening brace token and its position
        let lbrace_pos = self.pos;
        self.expect(&Token::LBrace)?;
        let start = self.tokens[lbrace_pos].span.start;

        // Find matching closing brace (handling nesting)
        let mut depth = 1;
        while depth > 0 && !self.is_at_end() {
            match self.peek_token() {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                self.advance();
            }
        }

        if depth != 0 {
            return Err(self.error("unmatched brace in grammar"));
        }

        // Get the end position (including the closing brace)
        let end = self.tokens[self.pos].span.end;
        self.advance(); // consume the closing brace

        // Extract the grammar body source (from { to })
        let grammar_source = &source[start..end];

        // Parse using GrammarParser
        let mut grammar_parser = GrammarParser::new(grammar_source);
        grammar_parser.parse_anonymous()
    }

    /// Parse anonymous grammar block for @ operator: `{ pattern => action; ... }`
    fn parse_anonymous_grammar_block(&mut self) -> Result<Grammar> {
        // We need source access to parse grammar body
        let source = self
            .source
            .ok_or_else(|| self.error("anonymous grammar blocks require source access"))?;

        // Find the opening brace token and its position
        let lbrace_pos = self.pos;
        self.expect(&Token::LBrace)?;
        let start = self.tokens[lbrace_pos].span.start;

        // Find matching closing brace (handling nesting)
        let mut depth = 1;
        while depth > 0 && !self.is_at_end() {
            match self.peek_token() {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                self.advance();
            }
        }

        if depth != 0 {
            return Err(self.error("unmatched brace in grammar block"));
        }

        // Get the end position (including the closing brace)
        let end = self.tokens[self.pos].span.end;
        self.advance(); // consume the closing brace

        // Extract the grammar body source (from { to })
        let grammar_source = &source[start..end];

        // Parse using GrammarParser's match block parser
        let mut grammar_parser = GrammarParser::new(grammar_source);
        grammar_parser.parse_match_block()
    }

    /// Parse inline pattern block for @ operator: `{ pattern => body, ... }`
    /// Uses AST patterns (like match expressions), not grammar patterns.
    fn parse_inline_pattern_block(&mut self) -> Result<Vec<PatternCase>> {
        self.expect(&Token::LBrace)?;

        let mut cases = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            cases.push(self.parse_pattern_case()?);
            // Allow comma or semicolon as separator
            if self.check(&Token::Comma) || self.check(&Token::Semi) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        Ok(cases)
    }

    /// Parse a single pattern case: pattern [when guard] => body
    fn parse_pattern_case(&mut self) -> Result<PatternCase> {
        let pattern = self.parse_pattern()?;

        let guard = if self.check(&Token::When) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(&Token::Arrow)?;
        let prev_flag = self.in_pattern_case_body;
        self.in_pattern_case_body = true;
        let body = self.parse_expr();
        self.in_pattern_case_body = prev_flag;
        let body = body?;

        Ok(PatternCase {
            pattern,
            guard,
            body: Box::new(body),
        })
    }

    /// Check if the next tokens look like an inline pattern block (AST patterns)
    /// rather than a grammar block. Returns true if we should use inline pattern parsing.
    ///
    /// The key challenge is distinguishing:
    /// - AST patterns: `%{key: x}`, `[a, b]`, `_`, `x`, `42`, `:Symbol`
    /// - Grammar patterns: `%{key: _:x}`, `[a-z]+`, `_:binding`, `"literal"`, `.`
    ///
    /// CONSERVATIVE APPROACH: Only trigger inline pattern parsing for patterns that
    /// are unambiguously AST patterns. Default to grammar parsing for backward
    /// compatibility with existing code.
    ///
    /// Triggers for inline pattern parsing:
    /// - `%{key: identifier` where identifier is followed by `}` or `,` (not `:`)
    /// - `_` followed immediately by `=>` or `when` (not `_:binding`)
    /// - Identifier followed immediately by `=>` or `when` (simple variable pattern)
    /// - `:Symbol` followed by `(` (constructor pattern) or `=>` (symbol match)
    /// - Integer/Float followed by `=>` (literal pattern)
    /// - `[` followed by identifier and `,` (list pattern, not `[a-z]` char class)
    ///
    /// Default: grammar parsing
    fn is_inline_pattern_block(&self) -> bool {
        // Look at token after the opening brace
        let Some(first) = self.peek_ahead(1) else {
            return false;
        };

        match &first.token {
            // %{ could be AST map pattern or grammar map pattern
            // Be VERY conservative: only treat as AST if value is identifier NOT followed by :
            // This avoids `%{key: _:binding}` being parsed as AST
            Token::Percent => {
                // Token sequence: % { key : value
                // pos+1: %
                // pos+2: {
                // pos+3: key
                // pos+4: :
                // pos+5: value_start
                // pos+6: after_value (} or , for AST, : for grammar _:binding)
                if let Some(t6) = self.peek_ahead(6) {
                    // If token after value is `:`, it's grammar _:binding syntax
                    if matches!(t6.token, Token::Colon) {
                        return false;
                    }
                    // If token after value is `}` or `,`, it's AST pattern
                    if matches!(t6.token, Token::RBrace | Token::Comma) {
                        return true;
                    }
                }
                // Default: grammar
                false
            }

            // _ (wildcard): inline pattern — compile_match handles Wildcard directly
            Token::Underscore => self
                .peek_ahead(2)
                .map(|t| matches!(t.token, Token::Arrow | Token::When))
                .unwrap_or(false),

            // :Symbol patterns:
            // - `:Symbol =>` or `:Symbol when` is inline pattern (symbol match)
            // - `:Symbol(...)` is constructor pattern — compile_match handles recursively
            Token::Symbol(_) => {
                if let Some(t2) = self.peek_ahead(2) {
                    matches!(t2.token, Token::Arrow | Token::When | Token::LParen)
                } else {
                    false
                }
            }

            // Identifier is inline if followed by => or when
            Token::Ident(_) => self
                .peek_ahead(2)
                .map(|t| matches!(t.token, Token::Arrow | Token::When))
                .unwrap_or(false),

            // [ could be list pattern or character class
            // List pattern: [a, b] -> has comma after first element
            // List pattern: [:Tag, ...] -> has comma after symbol
            // Character class: [a-z] -> has dash for ranges
            Token::LBracket => {
                if let Some(second) = self.peek_ahead(2)
                    && let Some(third) = self.peek_ahead(3)
                {
                    // [a, ...] or [:Tag, ...] is list pattern
                    if matches!(second.token, Token::Ident(_) | Token::Symbol(_))
                        && matches!(third.token, Token::Comma)
                    {
                        return true;
                    }
                    // [a] => or [:Tag] => is list pattern (single element)
                    if matches!(second.token, Token::Ident(_) | Token::Symbol(_))
                        && matches!(third.token, Token::RBracket)
                        && let Some(fourth) = self.peek_ahead(4)
                        && matches!(fourth.token, Token::Arrow | Token::When)
                    {
                        return true;
                    }
                }
                false
            }

            // Integer/Float literals followed by => are inline patterns
            Token::Int(_) | Token::Float(_) => self
                .peek_ahead(2)
                .map(|t| matches!(t.token, Token::Arrow | Token::When))
                .unwrap_or(false),

            // String literals are ambiguous - default to grammar
            Token::String(_) => false,

            // Everything else: default to grammar parsing
            _ => false,
        }
    }

    /// Parse list literal or list comprehension.
    /// List literal: [expr1, expr2, ...]
    /// List comprehension: [expr for var in iterable if condition]
    fn parse_list(&mut self) -> Result<Expr> {
        self.expect(&Token::LBracket)?;

        if self.check(&Token::RBracket) {
            self.advance();
            return Ok(Expr::List(Vec::new()));
        }

        let first = self.parse_expr()?;

        // Check for [head | tail] syntax
        if self.check(&Token::Bar) {
            self.advance();
            let tail = self.parse_expr()?;
            self.expect(&Token::RBracket)?;
            return Ok(Expr::ListCons(Box::new(first), Box::new(tail)));
        }

        // Check for list comprehension: [expr for var in iterable if condition]
        if self.check(&Token::For) {
            self.advance();
            // Parse variable name
            let elem_var = self.expect_ident()?;
            self.expect(&Token::In)?;
            // Parse iterable
            let iterable = self.parse_expr()?;

            // Optional filter condition
            let filter_condition = if self.check(&Token::If) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.expect(&Token::RBracket)?;

            // Return MapEach or Filter based on whether there's a condition
            return match filter_condition {
                Some(condition) => {
                    // [expr for x in iterable if condition] is a filter
                    // But we need to transform the element, not just filter
                    // So this is actually: map expr, then filter
                    // For now, let's implement it as: filter with transformation
                    // We'll need to handle both transformation and filtering
                    // The simplest approach: map the transformation, then filter
                    // But that requires two passes. Let's think...

                    // Actually, the syntax [x for x in list if pred] means:
                    // "give me x where x is in list and pred(x) is true"
                    // So first_expr is the value to output, elem_var is the loop variable
                    // If there's a condition, filter by it

                    // For [x * 2 for x in [1,2,3] if x % 2 == 0]:
                    // - We want to transform (x * 2) and filter (x % 2 == 0)
                    // - This is map + filter combined

                    // For now, let's implement simple cases:
                    // [x for x in list] -> map
                    // [x for x in list if pred] -> filter (no transformation)
                    // [expr for x in list if pred] -> not yet supported

                    // Check if first expression is just the variable reference
                    if let Expr::Ident(ref name) = first
                        && name == &elem_var
                    {
                        // Simple filter: [x for x in list if pred]
                        return Ok(Expr::Filter {
                            elem_var,
                            iterable: Box::new(iterable),
                            body: Box::new(condition),
                        });
                    }

                    // For [expr for x in list if pred], we need to combine map and filter
                    // This requires either:
                    // 1. Two passes (map then filter)
                    // 2. Inline the transformation and filtering in one loop
                    // For now, return an error
                    return Err(Error::Parser {
                        token: 0,
                        message: "map + filter comprehensions not yet supported, use map then filter separately".to_string(),
                    });
                }
                None => {
                    // [expr for x in iterable] is a map
                    Ok(Expr::MapEach {
                        elem_var,
                        iterable: Box::new(iterable),
                        body: Box::new(first),
                    })
                }
            };
        }

        // Regular list literal
        let mut items = vec![first];
        while self.check(&Token::Comma) {
            self.advance();
            if self.check(&Token::RBracket) {
                break; // trailing comma
            }
            items.push(self.parse_expr()?);
        }
        self.expect(&Token::RBracket)?;
        Ok(Expr::List(items))
    }

    /// Parse sequence (block).
    fn parse_sequence(&mut self) -> Result<Expr> {
        self.expect(&Token::LBrace)?;
        let mut exprs = Vec::new();

        while !self.check(&Token::RBrace) && !self.is_at_end() {
            exprs.push(self.parse_expr()?);
            if self.check(&Token::Semi) {
                self.advance();
            } else if !self.check(&Token::RBrace) {
                // Allow implicit semicolons before }
                break;
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Sequence(exprs))
    }

    /// Parse map literal.
    fn parse_map(&mut self) -> Result<Expr> {
        self.expect(&Token::Percent)?;
        self.expect(&Token::LBrace)?;

        if self.check(&Token::RBrace) {
            self.advance();
            return Ok(Expr::Map(Vec::new()));
        }

        let mut entries = Vec::new();
        loop {
            let entry = self.parse_map_entry()?;
            entries.push(entry);

            if self.check(&Token::Comma) {
                self.advance();
                if self.check(&Token::RBrace) {
                    break; // trailing comma
                }
            } else {
                break;
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Map(entries))
    }

    /// Parse a single map entry.
    fn parse_map_entry(&mut self) -> Result<MapEntry> {
        // Check for symbol literal key (:symbol: value)
        // The lexer tokenizes ":type" as Token::Symbol("type")
        if self.check_symbol_key() {
            let name = match self.peek_token() {
                Some(Token::Symbol(s)) => s.clone(),
                _ => return Err(self.error("expected symbol key")),
            };
            self.advance(); // symbol token
            self.expect(&Token::Colon)?;
            let value = self.parse_expr()?;
            return Ok(MapEntry::Symbol(name, value));
        }

        // Check for identifier key (name: value)
        // Allow both Ident and keyword tokens as map keys
        let is_symbol_key = match self.peek_token() {
            Some(Token::Ident(_)) => true,
            Some(token) => {
                Self::is_keyword_token(token)
                    && self.peek_ahead(1).map(|t| &t.token) == Some(&Token::Colon)
            }
            None => false,
        };

        if is_symbol_key {
            let name = match self.peek_token() {
                Some(Token::Ident(name)) => name.clone(),
                Some(token) => Self::token_name(token),
                None => return Err(self.error("expected identifier")),
            };
            self.advance(); // ident or keyword
            self.advance(); // colon
            let value = self.parse_expr()?;
            return Ok(MapEntry::Symbol(name, value));
        }

        // Computed key (expr => value)
        let key = self.parse_expr()?;
        self.expect(&Token::Arrow)?;
        let value = self.parse_expr()?;
        Ok(MapEntry::Computed(key, value))
    }

    /// Get the string name of a keyword token.
    fn token_name(token: &Token) -> SmolStr {
        match token {
            Token::Object => "object".into(),
            Token::Let => "let".into(),
            Token::If => "if".into(),
            Token::Then => "then".into(),
            Token::Else => "else".into(),
            Token::While => "while".into(),
            Token::Do => "do".into(),
            Token::Lambda => "lambda".into(),
            Token::Return => "return".into(),
            Token::Spawn => "spawn".into(),
            Token::New => "new".into(),
            Token::Try => "try".into(),
            Token::Catch => "catch".into(),
            Token::Throw => "throw".into(),
            Token::Match => "match".into(),
            Token::When => "when".into(),
            Token::As => "as".into(),
            Token::Stream => "stream".into(),
            Token::Grammar => "grammar".into(),
            Token::Self_ => "self".into(),
            Token::Parent => "parent".into(),
            Token::Caller => "caller".into(),
            Token::User => "user".into(),
            Token::Args => "args".into(),
            Token::Null => "null".into(),
            Token::True => "true".into(),
            Token::False => "false".into(),
            _ => "".into(),
        }
    }

    /// Check if a token is a keyword that can be used as a map key.
    fn is_keyword_token(token: &Token) -> bool {
        matches!(
            token,
            Token::Object
                | Token::Let
                | Token::If
                | Token::Then
                | Token::Else
                | Token::While
                | Token::Do
                | Token::Lambda
                | Token::Return
                | Token::Yield
                | Token::Spawn
                | Token::New
                | Token::Try
                | Token::Catch
                | Token::Throw
                | Token::Match
                | Token::When
                | Token::As
                | Token::Stream
                | Token::Grammar
                | Token::Self_
                | Token::Parent
                | Token::Caller
                | Token::User
                | Token::Args
                | Token::Null
                | Token::True
                | Token::False
        )
    }

    /// Parse if expression.
    fn parse_if(&mut self) -> Result<Expr> {
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Then)?;
        let then_branch = self.parse_expr()?;

        let else_branch = if self.check(&Token::Else) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        Ok(Expr::If(Box::new(cond), Box::new(then_branch), else_branch))
    }

    /// Parse while loop.
    fn parse_while(&mut self) -> Result<Expr> {
        self.expect(&Token::While)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::Do)?;
        let body = self.parse_expr()?;
        Ok(Expr::While(Box::new(cond), Box::new(body)))
    }

    /// Parse do-while loop.
    fn parse_do_while(&mut self) -> Result<Expr> {
        self.expect(&Token::Do)?;
        let body = self.parse_expr()?;
        self.expect(&Token::While)?;
        let cond = self.parse_expr()?;
        Ok(Expr::DoWhile(Box::new(body), Box::new(cond)))
    }

    /// Parse for loop: for pattern in iterable { body }
    fn parse_for(&mut self) -> Result<Expr> {
        self.expect(&Token::For)?;

        // Parse the pattern (left side of 'in')
        let pattern = self.parse_pattern()?;

        self.expect(&Token::In)?;

        // Parse the iterable
        let iterable = self.parse_expr()?;

        // Expect body: either { body } or just body
        // This matches the grammar: for x in expr { body } or for x in expr body
        let body = if self.check(&Token::LBrace) {
            self.advance();
            let body_expr = self.parse_expr()?;
            self.expect(&Token::RBrace)?;
            body_expr
        } else {
            self.parse_expr()?
        };

        Ok(Expr::For(pattern, Box::new(iterable), Box::new(body)))
    }

    /// Parse fold left: fold func, initial, iterable
    /// Syntax: fold (\acc \elem acc + elem), 0, [1, 2, 3]
    fn parse_fold(&mut self) -> Result<Expr> {
        self.expect(&Token::Fold)?;

        // Parse the function (lambda or function reference)
        let func = self.parse_expr()?;

        // Expect comma
        self.expect(&Token::Comma)?;

        // Parse initial value
        let initial = self.parse_expr()?;

        // Expect comma
        self.expect(&Token::Comma)?;

        // Parse the iterable
        let iterable = self.parse_expr()?;

        // We'll store the function and let the compiler handle calling it
        // The accumulator variable name will be internal (_acc)
        Ok(Expr::Fold {
            initial: Box::new(initial),
            acc_var: SmolStr::new("_acc"),
            iterable: Box::new(iterable),
            body: Box::new(func),
        })
    }

    /// Parse fold right: foldr func, initial, iterable
    /// Syntax: foldr (\acc \elem elem * acc), 1, [1, 2, 3]
    fn parse_foldr(&mut self) -> Result<Expr> {
        self.expect(&Token::Foldr)?;

        // Parse the function (lambda or function reference)
        let func = self.parse_expr()?;

        // Expect comma
        self.expect(&Token::Comma)?;

        // Parse initial value
        let initial = self.parse_expr()?;

        // Expect comma
        self.expect(&Token::Comma)?;

        // Parse the iterable
        let iterable = self.parse_expr()?;

        // We'll store the function and let the compiler handle calling it
        // The accumulator variable name will be internal (_acc)
        Ok(Expr::Foldr {
            initial: Box::new(initial),
            acc_var: SmolStr::new("_acc"),
            iterable: Box::new(iterable),
            body: Box::new(func),
        })
    }

    /// Parse map: map <function> <iterable>
    /// Syntax: map \x x * 2, [1, 2, 3]
    /// Or: map \x x * 2 in [1, 2, 3]
    fn parse_map_each(&mut self) -> Result<Expr> {
        self.expect(&Token::Map)?;

        // Parse the function (lambda or function reference)
        let func = self.parse_expr()?;

        // Parse separator: comma or 'in'
        if !self.check(&Token::Comma) && !self.check(&Token::In) {
            return Err(Error::Parser {
                token: 0, // position doesn't matter much here
                message: "expected ',' or 'in' after function in map".to_string(),
            });
        }
        self.advance();

        // Parse the iterable
        let iterable = self.parse_expr()?;

        Ok(Expr::MapEach {
            elem_var: SmolStr::new("_elem"), // Will be bound internally
            iterable: Box::new(iterable),
            body: Box::new(func), // The function becomes the body
        })
    }

    /// Parse filter: filter <predicate> <iterable>
    /// Syntax: filter \x x % 2 == 0, [1, 2, 3, 4, 5]
    fn parse_filter(&mut self) -> Result<Expr> {
        self.expect(&Token::Filter)?;

        // Parse the predicate (lambda or function reference)
        let pred = self.parse_expr()?;

        // Parse separator: comma or 'in'
        if !self.check(&Token::Comma) && !self.check(&Token::In) {
            return Err(Error::Parser {
                token: 0,
                message: "expected ',' or 'in' after predicate in filter".to_string(),
            });
        }
        self.advance();

        // Parse the iterable
        let iterable = self.parse_expr()?;

        Ok(Expr::Filter {
            elem_var: SmolStr::new("_elem"), // Will be bound internally
            iterable: Box::new(iterable),
            body: Box::new(pred), // The predicate becomes the body
        })
    }

    /// Parse let expression.
    fn parse_let(&mut self) -> Result<Expr> {
        self.expect(&Token::Let)?;

        // Check if this is expression-style: let (bindings) in body
        // or statement-style: let name = expr
        if self.check(&Token::LParen) {
            // Expression-style: let (bindings) in body
            self.advance();

            let mut bindings = Vec::new();
            while !self.check(&Token::RParen) && !self.is_at_end() {
                // Check if this is a pattern or simple binding
                let binding = if self.check(&Token::Percent) || self.check(&Token::LBracket) {
                    // Pattern destructuring: %{...} = expr or [...] = expr
                    let pattern = self.parse_pattern()?;
                    self.expect(&Token::Eq)?;
                    let init = self.parse_expr()?;
                    LetBinding::Destructure(pattern, Box::new(init))
                } else {
                    // Simple binding: name = expr or just name
                    let name = self.expect_ident()?;
                    let init = if self.check(&Token::Eq) {
                        self.advance();
                        Some(Box::new(self.parse_expr()?))
                    } else {
                        None
                    };
                    LetBinding::Simple(name, init)
                };

                bindings.push(binding);

                if !self.check(&Token::RParen) && self.check(&Token::Comma) {
                    self.advance();
                }
            }
            self.expect(&Token::RParen)?;

            let body = self.parse_expr()?;
            Ok(Expr::Let(bindings, Box::new(body)))
        } else {
            // Statement-style: let name = expr
            // Binds to current scope and returns the value
            let name = self.expect_ident()?;
            self.expect(&Token::Eq)?;
            let init = Box::new(self.parse_expr()?);

            Ok(Expr::LetStmt(name, init))
        }
    }

    /// Parse lambda expression.
    fn parse_lambda(&mut self) -> Result<Expr> {
        self.expect(&Token::Lambda)?;
        let params = self.parse_param_names()?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda(params, Box::new(body)))
    }

    /// Parse short lambda (\x expr) or multi-param short lambda (\x, y expr).
    fn parse_short_lambda(&mut self) -> Result<Expr> {
        self.expect(&Token::Backslash)?;
        let first_param = self.expect_ident_or_underscore()?;

        // Check for comma-separated additional params
        if self.check(&Token::Comma) {
            let mut params = vec![first_param];
            while self.check(&Token::Comma) {
                self.advance(); // consume comma
                params.push(self.expect_ident_or_underscore()?);
            }
            let body = self.parse_expr()?;
            Ok(Expr::Lambda(params, Box::new(body)))
        } else {
            let body = self.parse_expr()?;
            Ok(Expr::ShortLambda(first_param, Box::new(body)))
        }
    }

    /// Parse return statement.
    fn parse_return(&mut self) -> Result<Expr> {
        self.expect(&Token::Return)?;

        // Check if there's an expression following
        if self.is_at_end() || self.check(&Token::Semi) || self.check(&Token::RBrace) {
            return Ok(Expr::Return(None));
        }

        let expr = self.parse_expr()?;
        Ok(Expr::Return(Some(Box::new(expr))))
    }

    /// Parse yield expression.
    fn parse_yield(&mut self) -> Result<Expr> {
        self.expect(&Token::Yield)?;

        // yield requires an expression
        let expr = self.parse_expr()?;
        Ok(Expr::Yield(Box::new(expr)))
    }

    /// Parse spawn expression.
    fn parse_spawn(&mut self) -> Result<Expr> {
        self.expect(&Token::Spawn)?;
        // Use parse_primary instead of parse_unary to avoid consuming
        // the parentheses as part of a function call on the constructor
        let constructor = self.parse_primary()?;
        let args = if self.check(&Token::LParen) {
            self.parse_args()?
        } else {
            // Allow bare arguments for common case
            let mut args = Vec::new();
            while !self.is_at_end()
                && !self.check(&Token::Semi)
                && !self.check(&Token::RBrace)
                && !self.check(&Token::RParen)
            {
                args.push(Arg::Expr(self.parse_unary()?));
            }
            args
        };
        Ok(Expr::Spawn(Box::new(constructor), args))
    }

    /// Parse new constructor expression: `new ^ctor(args)`.
    /// Semantically synchronous object construction (vs spawn for concurrent).
    /// Currently compiles to the same Spawn instruction.
    fn parse_new(&mut self) -> Result<Expr> {
        self.expect(&Token::New)?;
        let constructor = self.parse_primary()?;
        let args = if self.check(&Token::LParen) {
            self.parse_args()?
        } else {
            vec![]
        };
        Ok(Expr::Spawn(Box::new(constructor), args))
    }

    /// Parse match expression.
    fn parse_match(&mut self) -> Result<Expr> {
        self.expect(&Token::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(&Token::LBrace)?;

        let mut cases = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            cases.push(self.parse_match_case()?);
            self.skip_semis();
        }
        self.expect(&Token::RBrace)?;

        Ok(Expr::Match(Box::new(scrutinee), cases))
    }

    /// Parse a match case.
    fn parse_match_case(&mut self) -> Result<MatchCase> {
        let pattern = self.parse_pattern()?;

        let guard = if self.check(&Token::When) {
            self.advance();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(&Token::Arrow)?;
        let body = self.parse_expr()?;

        Ok(MatchCase {
            pattern,
            guard,
            body: Box::new(body),
        })
    }

    /// Parse try/catch expression.
    fn parse_try_catch(&mut self) -> Result<Expr> {
        self.expect(&Token::Try)?;
        self.expect(&Token::LBrace)?;

        let mut body_exprs = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            body_exprs.push(self.parse_expr()?);
            if self.check(&Token::Semi) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        let body = if body_exprs.len() == 1 {
            body_exprs.pop().unwrap()
        } else {
            Expr::Sequence(body_exprs)
        };

        self.expect(&Token::Catch)?;

        let error_binding = match self.peek_token() {
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                name
            }
            _ => return Err(self.error("expected identifier after 'catch'")),
        };

        self.expect(&Token::LBrace)?;

        let mut catch_exprs = Vec::new();
        while !self.check(&Token::RBrace) && !self.is_at_end() {
            catch_exprs.push(self.parse_expr()?);
            if self.check(&Token::Semi) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        let catch_body = if catch_exprs.len() == 1 {
            catch_exprs.pop().unwrap()
        } else {
            Expr::Sequence(catch_exprs)
        };

        Ok(Expr::TryCatch {
            body: Box::new(body),
            error_binding,
            catch_body: Box::new(catch_body),
        })
    }

    /// Parse throw expression.
    fn parse_throw(&mut self) -> Result<Expr> {
        self.expect(&Token::Throw)?;
        let expr = self.parse_expr()?;
        Ok(Expr::Throw(Box::new(expr)))
    }

    /// Parse a pattern.
    fn parse_pattern(&mut self) -> Result<Pattern> {
        let token = self.peek_token().ok_or(Error::UnexpectedEof)?;

        let pattern = match token {
            Token::Underscore => {
                self.advance();
                Pattern::Wildcard
            }
            Token::Int(n) => {
                let n = *n;
                self.advance();
                Pattern::Int(n)
            }
            Token::Float(f) => {
                let f = *f;
                self.advance();
                Pattern::Float(f)
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                Pattern::String(s)
            }
            Token::Symbol(s) => {
                let s = s.clone();
                self.advance();
                // Reject legacy :Symbol(patterns...) constructor syntax in
                // pattern position (DESIGN-002, mirrors parse_primary at :619).
                if self.check(&Token::LParen) {
                    return Err(self.error(&format!(
                        "legacy :{}(...) constructor pattern is not supported; use [:{}] or [:{}, ...] instead",
                        s, s, s
                    )));
                }
                Pattern::Symbol(s)
            }
            Token::Ident(s) => {
                let s = s.clone();
                self.advance();
                Pattern::Var(s)
            }
            Token::LBracket => {
                self.advance();
                let mut patterns = Vec::new();
                let mut tail = None;

                while !self.check(&Token::RBracket) && !self.is_at_end() {
                    if self.check(&Token::Bar) {
                        self.advance();
                        tail = Some(self.expect_ident()?);
                        break;
                    }
                    patterns.push(self.parse_pattern()?);
                    if !self.check(&Token::RBracket) && !self.check(&Token::Bar) {
                        self.expect(&Token::Comma)?;
                    }
                }
                self.expect(&Token::RBracket)?;
                Pattern::List(patterns, tail)
            }
            Token::Percent => {
                self.advance();
                self.expect(&Token::LBrace)?;
                let mut entries = Vec::new();

                while !self.check(&Token::RBrace) && !self.is_at_end() {
                    let key = self.expect_ident()?;
                    self.expect(&Token::Colon)?;
                    let value = self.parse_pattern()?;
                    entries.push((key, value));

                    if !self.check(&Token::RBrace) {
                        self.expect(&Token::Comma)?;
                    }
                }
                self.expect(&Token::RBrace)?;
                Pattern::Map(entries)
            }
            _ => return Err(self.error("expected pattern")),
        };

        // Check for `as` binding
        if self.check(&Token::As) {
            self.advance();
            let name = self.expect_ident()?;
            return Ok(Pattern::As(Box::new(pattern), name));
        }

        Ok(pattern)
    }

    // --- Helper methods ---

    fn peek_token(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|st| &st.token)
    }

    fn peek_ahead(&self, n: usize) -> Option<&SpannedToken> {
        self.tokens.get(self.pos + n)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn check(&self, token: &Token) -> bool {
        self.peek_token() == Some(token)
    }

    /// Check if the next token is ColonColon (for module qualification like stream::foo)
    fn peek_ahead_is_coloncolon(&self) -> bool {
        self.peek_ahead(1).map(|t| &t.token) == Some(&Token::ColonColon)
    }

    /// Check if current position is a symbol key (Symbol followed by Colon)
    fn check_symbol_key(&self) -> bool {
        matches!(self.peek_token(), Some(Token::Symbol(_)))
            && self.peek_ahead(1).map(|t| &t.token) == Some(&Token::Colon)
    }

    fn expect(&mut self, token: &Token) -> Result<()> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(&format!("expected {:?}", token)))
        }
    }

    fn expect_ident(&mut self) -> Result<SmolStr> {
        match self.peek_token() {
            Some(Token::Ident(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            _ => Err(self.error("expected identifier")),
        }
    }

    /// Expect an identifier or keyword (for method/property names after `.`).
    /// Keywords like `in`, `map`, `filter` can be used as method names.
    fn expect_ident_or_keyword(&mut self) -> Result<SmolStr> {
        let result = match self.peek_token() {
            Some(Token::Ident(s)) => Ok(s.clone()),
            // Allow keywords as method names
            Some(Token::In) => Ok(SmolStr::new("in")),
            Some(Token::Map) => Ok(SmolStr::new("map")),
            Some(Token::Filter) => Ok(SmolStr::new("filter")),
            Some(Token::Fold) => Ok(SmolStr::new("fold")),
            Some(Token::Foldr) => Ok(SmolStr::new("foldr")),
            Some(Token::Match) => Ok(SmolStr::new("match")),
            Some(Token::When) => Ok(SmolStr::new("when")),
            Some(Token::As) => Ok(SmolStr::new("as")),
            Some(Token::Return) => Ok(SmolStr::new("return")),
            Some(Token::Yield) => Ok(SmolStr::new("yield")),
            Some(Token::Spawn) => Ok(SmolStr::new("spawn")),
            Some(Token::New) => Ok(SmolStr::new("new")),
            Some(Token::Do) => Ok(SmolStr::new("do")),
            Some(Token::While) => Ok(SmolStr::new("while")),
            Some(Token::For) => Ok(SmolStr::new("for")),
            Some(Token::If) => Ok(SmolStr::new("if")),
            Some(Token::Then) => Ok(SmolStr::new("then")),
            Some(Token::Else) => Ok(SmolStr::new("else")),
            Some(Token::Let) => Ok(SmolStr::new("let")),
            Some(Token::Try) => Ok(SmolStr::new("try")),
            Some(Token::Catch) => Ok(SmolStr::new("catch")),
            Some(Token::Throw) => Ok(SmolStr::new("throw")),
            Some(Token::Object) => Ok(SmolStr::new("object")),
            Some(Token::Lambda) => Ok(SmolStr::new("lambda")),
            Some(Token::Stream) => Ok(SmolStr::new("stream")),
            Some(Token::Grammar) => Ok(SmolStr::new("grammar")),
            Some(Token::Self_) => Ok(SmolStr::new("self")),
            Some(Token::Parent) => Ok(SmolStr::new("parent")),
            Some(Token::Caller) => Ok(SmolStr::new("caller")),
            Some(Token::User) => Ok(SmolStr::new("user")),
            Some(Token::Args) => Ok(SmolStr::new("args")),
            Some(Token::Null) => Ok(SmolStr::new("null")),
            Some(Token::True) => Ok(SmolStr::new("true")),
            Some(Token::False) => Ok(SmolStr::new("false")),
            _ => Err(self.error("expected identifier")),
        };
        if result.is_ok() {
            self.advance();
        }
        result
    }

    /// Expect an identifier or underscore (for lambda parameters that can use _ as wildcard)
    fn expect_ident_or_underscore(&mut self) -> Result<SmolStr> {
        match self.peek_token() {
            Some(Token::Ident(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            Some(Token::Underscore) => {
                self.advance();
                Ok(SmolStr::new("_"))
            }
            _ => Err(self.error("expected identifier or underscore")),
        }
    }

    fn expect_symbol(&mut self) -> Result<SmolStr> {
        match self.peek_token() {
            Some(Token::Symbol(s)) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            _ => Err(self.error("expected symbol")),
        }
    }

    fn skip_semis(&mut self) {
        while self.check(&Token::Semi) {
            self.advance();
        }
    }

    fn error(&self, message: &str) -> Error {
        // Include current token information in error message for better debugging
        let token_info = if let Some(token) = self.peek_token() {
            format!(
                " (at position {}, token {}: {:?})",
                self.pos, self.pos, token
            )
        } else if let Some(st) = self.tokens.get(self.pos) {
            format!(
                " (at position {}, token {}: {:?})",
                self.pos, self.pos, st.token
            )
        } else {
            format!(
                " (at position {}, no token available, total tokens: {})",
                self.pos,
                self.tokens.len()
            )
        };
        Error::Parser {
            token: self.pos,
            message: format!("{}{}", message, token_info),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Result<Expr> {
        let tokens = Lexer::new(source).tokenize()?;
        Parser::with_source(&tokens, source).parse()
    }

    #[test]
    fn test_basic_expr() {
        let expr = parse("1 + 2 * 3").unwrap();
        assert!(matches!(expr, Expr::Binary(_, BinOp::Add, _)));
    }

    #[test]
    fn test_let_binding() {
        let expr = parse("let (x = 42) x + 1").unwrap();
        assert!(matches!(expr, Expr::Let(_, _)));
    }

    #[test]
    fn test_if_expr() {
        let expr = parse("if x > 0 then x else -x").unwrap();
        assert!(matches!(expr, Expr::If(_, _, Some(_))));
    }

    #[test]
    fn test_lambda() {
        let expr = parse("lambda (x, y) x + y").unwrap();
        assert!(matches!(expr, Expr::Lambda(_, _)));
    }

    #[test]
    fn test_list() {
        let expr = parse("[1, 2, 3]").unwrap();
        if let Expr::List(items) = expr {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_map() {
        let expr = parse("%{foo: 1, bar: 2}").unwrap();
        assert!(matches!(expr, Expr::Map(_)));
    }

    #[test]
    fn test_method_call() {
        let expr = parse("obj.method(1, 2)").unwrap();
        assert!(matches!(expr, Expr::MethodCall(_, _, _)));
    }

    #[test]
    fn test_pipe() {
        let expr = parse("x |> f |> g").unwrap();
        assert!(matches!(expr, Expr::Binary(_, BinOp::Pipe, _)));
    }

    #[test]
    fn test_grammar_literal() {
        let expr = parse("grammar { digit = [0-9] }").unwrap();
        match expr {
            Expr::GrammarLiteral(grammar) => {
                assert!(grammar.rules.contains_key("digit"));
            }
            _ => panic!("expected GrammarLiteral, got {:?}", expr),
        }
    }

    #[test]
    fn test_grammar_extension() {
        let expr = parse("base <: { hex = [0-9a-f] }").unwrap();
        match expr {
            Expr::GrammarExtend { base, rules } => {
                // base should be an identifier
                assert!(matches!(*base, Expr::Ident(ref name) if name == "base"));
                // rules should have the hex rule
                assert!(rules.rules.contains_key("hex"));
            }
            _ => panic!("expected GrammarExtend, got {:?}", expr),
        }
    }

    #[test]
    fn test_stream_literal() {
        let expr = parse("stream { 1 + 2 }").unwrap();
        assert!(matches!(expr, Expr::StreamLiteral(_)));
    }

    #[test]
    fn test_parse_try_catch() {
        let expr = parse("try { 42 } catch e { 0 }").unwrap();
        assert!(matches!(expr, Expr::TryCatch { .. }));
    }

    #[test]
    fn test_qualified_name_with_global_prefix() {
        // Test that ::foo::bar parses correctly
        let expr = parse("::__builtin_curl").unwrap();
        assert!(matches!(expr, Expr::Qualified(_)));
        if let Expr::Qualified(qn) = expr {
            assert_eq!(qn.parts, ["", "__builtin_curl"]);
        } else {
            panic!("expected Qualified name");
        }
    }

    #[test]
    fn test_qualified_name_with_global_prefix_method_call() {
        // Test that ::foo::bar.method() parses correctly
        let expr = parse("::__builtin_curl.get()").unwrap();
        assert!(matches!(expr, Expr::MethodCall(_, _, _)));
    }

    #[test]
    fn test_parse_try_catch_with_expression() {
        let expr = parse("try { 1 + 2 } catch err { err }").unwrap();
        if let Expr::TryCatch { error_binding, .. } = expr {
            assert_eq!(error_binding.as_str(), "err");
        } else {
            panic!("expected TryCatch");
        }
    }

    #[test]
    fn test_try_catch_is_expression() {
        // try/catch can be used as a value
        let expr = parse("let (x = try { 42 } catch e { 0 }) x").unwrap();
        assert!(matches!(expr, Expr::Let(_, _)));
    }

    #[test]
    fn test_parse_let_destructure_map() {
        let expr = parse("let (%{x: a, y: b} = point) a + b").unwrap();
        if let Expr::Let(bindings, _) = expr {
            assert!(matches!(bindings[0], LetBinding::Destructure(_, _)));
        } else {
            panic!("expected Let");
        }
    }

    #[test]
    fn test_parse_let_destructure_list() {
        let expr = parse("let ([first, second] = items) first").unwrap();
        if let Expr::Let(bindings, _) = expr {
            assert!(matches!(bindings[0], LetBinding::Destructure(_, _)));
        } else {
            panic!("expected Let");
        }
    }

    #[test]
    fn test_nested_short_lambda() {
        let expr = parse(r#"\x \y x + y"#).unwrap();
        if let Expr::ShortLambda(_, body) = expr {
            if let Expr::ShortLambda(_, inner_body) = *body {
                assert!(matches!(*inner_body, Expr::Binary(_, _, _)));
            } else {
                panic!("expected inner ShortLambda");
            }
        } else {
            panic!("expected ShortLambda");
        }
    }

    #[test]
    fn test_parenthesized_nested_short_lambda() {
        let expr = parse(r#"(\x \y x + y)"#).unwrap();
        // Should be the same as unparsed version
        if let Expr::ShortLambda(_, body) = expr {
            if let Expr::ShortLambda(_, inner_body) = *body {
                assert!(matches!(*inner_body, Expr::Binary(_, _, _)));
            } else {
                panic!("expected inner ShortLambda");
            }
        } else {
            panic!("expected ShortLambda");
        }
    }

    #[test]
    fn test_lambda_followed_by_number() {
        let expr = parse(r#"(\x x + 1) 0"#).unwrap();
        // Lambda followed by a number - should parse as sequence
        if let Expr::Sequence(exprs) = expr {
            assert_eq!(exprs.len(), 2);
        } else {
            panic!("expected Sequence");
        }
    }

    #[test]
    fn test_fold_with_curried_lambda() {
        let expr = parse(r#"fold (\acc \elem acc + elem), 0, [1, 2, 3]"#).unwrap();
        if let Expr::Fold {
            initial,
            acc_var,
            iterable,
            body,
        } = expr
        {
            assert!(matches!(*initial, Expr::Int(0)));
            assert_eq!(acc_var, "_acc");
            assert!(matches!(*iterable, Expr::List(_)));
            // Body should be a nested ShortLambda
            if let Expr::ShortLambda(_, inner_body) = *body {
                if let Expr::ShortLambda(_, innermost_body) = *inner_body {
                    assert!(matches!(*innermost_body, Expr::Binary(_, _, _)));
                } else {
                    panic!("expected inner ShortLambda in fold body");
                }
            } else {
                panic!("expected ShortLambda as fold body");
            }
        } else {
            panic!("expected Fold");
        }
    }

    #[test]
    fn test_curried_lambda_syntax() {
        // Verify that curried lambdas parse correctly
        let expr = parse(r#"\acc \elem acc + elem"#).unwrap();
        if let Expr::ShortLambda(_, body) = expr {
            if let Expr::ShortLambda(_, inner_body) = *body {
                assert!(matches!(*inner_body, Expr::Binary(_, _, _)));
            } else {
                panic!("expected inner ShortLambda");
            }
        } else {
            panic!("expected ShortLambda");
        }
    }
}
