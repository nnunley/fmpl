//! Recursive descent parser for FMPL.

use crate::ast::*;
use crate::error::{Error, Result};
use crate::grammar::Grammar;
use crate::grammar::parser::GrammarParser;
use crate::lexer::{SpannedToken, Token};
use smol_str::SmolStr;

/// Parser state.
pub struct Parser<'a> {
    tokens: &'a [SpannedToken],
    source: Option<&'a str>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [SpannedToken]) -> Self {
        Self {
            tokens,
            source: None,
            pos: 0,
        }
    }

    pub fn with_source(tokens: &'a [SpannedToken], source: &'a str) -> Self {
        Self {
            tokens,
            source: Some(source),
            pos: 0,
        }
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

        // Parse name (may be qualified or ^tag)
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

    /// Parse a qualified name (foo::bar::baz).
    fn parse_qualified_name(&mut self) -> Result<QualifiedName> {
        let first = self.expect_ident()?;
        let mut parts = vec![first];

        while self.check(&Token::ColonColon) {
            self.advance();
            parts.push(self.expect_ident()?);
        }

        Ok(QualifiedName { parts })
    }

    /// Parse an expression.
    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_pipe()
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
                self.advance();

                if self.check(&Token::LBrace) {
                    // Anonymous grammar block: expr @ { pattern => action; ... }
                    let anon_grammar = self.parse_anonymous_grammar_block()?;
                    left = Expr::GrammarApply {
                        input: Box::new(left),
                        grammar: Box::new(Expr::GrammarLiteral(anon_grammar)),
                        rule: SmolStr::new("main"), // Anonymous blocks use "main" rule
                    };
                } else {
                    // Named grammar application: expr @ grammar_expr.rule
                    let grammar_expr = self.parse_postfix()?;

                    // The grammar_expr should end with .rule access
                    // Extract the rule name from the last PropAccess
                    let (grammar, rule) = match grammar_expr {
                        Expr::PropAccess(base, prop) => (*base, prop),
                        Expr::Qualified(qn) => {
                            // Handle qualified::name.rule case
                            if qn.parts.len() >= 2 {
                                // Last part is the rule name
                                let rule = qn.parts.last().unwrap().clone();
                                let grammar_parts = qn.parts[..qn.parts.len() - 1].to_vec();
                                (
                                    Expr::Qualified(QualifiedName {
                                        parts: grammar_parts,
                                    }),
                                    rule,
                                )
                            } else {
                                return Err(self.error("grammar application requires grammar.rule"));
                            }
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

    /// Parse comparison (< > <= >=).
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
            } else if self.check(&Token::Percent) {
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
                if self.check_ident("as") {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    let facet = self.expect_symbol()?;
                    self.expect(&Token::RParen)?;
                    expr = Expr::FacetAccess(Box::new(expr), facet);
                } else {
                    // Property or method access
                    let name = self.expect_ident()?;
                    if self.check(&Token::LParen) {
                        let args = self.parse_args()?;
                        expr = Expr::MethodCall(Box::new(expr), name, args);
                    } else {
                        expr = Expr::PropAccess(Box::new(expr), name);
                    }
                }
            } else if self.check(&Token::LBracket) {
                self.advance();
                let index = self.parse_expr()?;

                // Check for slice
                if self.check(&Token::DotDot) {
                    self.advance();
                    let end = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Slice(Box::new(expr), Box::new(index), Box::new(end));
                } else {
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
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
                        parts.push(self.expect_ident()?);
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
            Token::Let => self.parse_let(),
            Token::Lambda => self.parse_lambda(),
            Token::Backslash => self.parse_short_lambda(),
            Token::Return => self.parse_return(),
            Token::Spawn => self.parse_spawn(),
            Token::Match => self.parse_match(),
            Token::Try => self.parse_try_catch(),
            Token::Throw => self.parse_throw(),
            Token::Stream => self.parse_stream_literal(),
            Token::Grammar => self.parse_grammar_literal(),
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

    /// Parse list literal.
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
        // Check for symbol key (name: value)
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
                | Token::Spawn
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

    /// Parse short lambda (\x expr).
    fn parse_short_lambda(&mut self) -> Result<Expr> {
        self.expect(&Token::Backslash)?;
        let param = self.expect_ident()?;
        let body = self.parse_expr()?;
        Ok(Expr::ShortLambda(param, Box::new(body)))
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

    fn check_ident(&self, name: &str) -> bool {
        matches!(self.peek_token(), Some(Token::Ident(s)) if s == name)
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
        Error::Parser {
            token: self.pos,
            message: message.to_string(),
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
}
