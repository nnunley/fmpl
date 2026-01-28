//! Lexer for FMPL using logos.

use crate::error::{Error, Result};
use logos::Logos;
use smol_str::SmolStr;

/// A positioned token with source location.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// FMPL tokens.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r]+")]
#[logos(skip r"--[^\n]*")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    // Keywords
    #[token("object")]
    Object,
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("do")]
    Do,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("fold")]
    Fold,
    #[token("foldr")]
    Foldr,
    #[token("map")]
    Map,
    #[token("filter")]
    Filter,
    #[token("lambda")]
    Lambda,
    #[token("return")]
    Return,
    #[token("spawn")]
    Spawn,
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
    #[token("throw")]
    Throw,
    #[token("match")]
    Match,
    #[token("when")]
    When,
    #[token("as")]
    As,
    #[token("stream")]
    Stream,
    #[token("grammar")]
    Grammar,
    #[token("yield")]
    Yield,

    // Built-in references
    #[token("self")]
    Self_,
    #[token("parent")]
    Parent,
    #[token("caller")]
    Caller,
    #[token("user")]
    User,
    #[token("args")]
    Args,
    #[token("null")]
    Null,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // Scope markers
    #[token("#private")]
    Private,
    #[token("#public")]
    Public,
    #[token("#protected")]
    Protected,
    #[token("#facets")]
    Facets,

    // Identifiers and literals
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| SmolStr::new(lex.slice()))]
    Ident(SmolStr),

    #[regex(r"\^[a-zA-Z_][a-zA-Z0-9_]*", |lex| SmolStr::new(&lex.slice()[1..]))]
    ObjTag(SmolStr),

    #[regex(r"@[a-zA-Z_][a-zA-Z0-9_]*", |lex| SmolStr::new(&lex.slice()[1..]))]
    FnTag(SmolStr),

    // Symbol: either identifier-style (:foo) or operator-style (:+, :==, etc.)
    #[regex(r":([a-zA-Z_][a-zA-Z0-9_]*|[+\-*/%<>=!|&]+)", |lex| SmolStr::new(&lex.slice()[1..]))]
    Symbol(SmolStr),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        // Process escape sequences
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        let mut last_was_backslash = false;

        while let Some(c) = chars.next() {
            if last_was_backslash {
                match c {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '\'' => result.push('\''),
                    '0' => result.push('\0'),
                    _ => {
                        // Unknown escape, keep as-is
                        result.push('\\');
                        result.push(c);
                    }
                }
                last_was_backslash = false;
            } else if c == '\\' {
                last_was_backslash = true;
            } else {
                result.push(c);
            }
        }

        if last_was_backslash {
            result.push('\\');
        }

        SmolStr::new(result)
    })]
    String(SmolStr),

    // Operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("=")]
    Eq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,

    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,

    #[token("|>")]
    Pipe,
    #[token("=>")]
    Arrow,
    #[token("<-")]
    AsyncCall,
    #[token("$")]
    SyncCall,
    #[token("::")]
    ColonColon,
    #[token("<:")]
    Inherits,
    #[token("..")]
    DotDot,

    #[token("\\")]
    Backslash,
    #[token("~")]
    Tilde,
    #[token("&")]
    Ampersand,
    #[token("@")]
    At,
    #[token("^")]
    Caret,
    #[token("?")]
    Question,

    // Delimiters
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,

    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
    #[token(";")]
    Semi,
    #[token("|")]
    Bar,
    #[token("_", priority = 3)]
    Underscore,
}

/// Lexer wrapper that produces positioned tokens.
pub struct Lexer<'a> {
    source: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Tokenize the source and return a vector of spanned tokens.
    pub fn tokenize(&self) -> Result<Vec<SpannedToken>> {
        let mut tokens = Vec::new();
        let mut lexer = Token::lexer(self.source);

        while let Some(result) = lexer.next() {
            match result {
                Ok(token) => {
                    tokens.push(SpannedToken {
                        token,
                        span: lexer.span(),
                    });
                }
                Err(()) => {
                    return Err(Error::Lexer {
                        position: lexer.span().start,
                        message: format!("unexpected character: {:?}", &self.source[lexer.span()]),
                    });
                }
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let tokens = Lexer::new("let x = 42").tokenize().unwrap();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].token, Token::Let);
        assert!(matches!(tokens[1].token, Token::Ident(_)));
        assert_eq!(tokens[2].token, Token::Eq);
        assert_eq!(tokens[3].token, Token::Int(42));
    }

    #[test]
    fn test_object_def() {
        let tokens = Lexer::new("object ^foo { }").tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Object);
        assert!(matches!(tokens[1].token, Token::ObjTag(_)));
        assert_eq!(tokens[2].token, Token::LBrace);
        assert_eq!(tokens[3].token, Token::RBrace);
    }

    #[test]
    fn test_string_literal() {
        let tokens = Lexer::new(r#""hello world""#).tokenize().unwrap();
        assert!(matches!(&tokens[0].token, Token::String(s) if s == "hello world"));
    }

    #[test]
    fn test_symbols() {
        let tokens = Lexer::new(":foo :bar").tokenize().unwrap();
        assert!(matches!(&tokens[0].token, Token::Symbol(s) if s == "foo"));
        assert!(matches!(&tokens[1].token, Token::Symbol(s) if s == "bar"));
    }

    #[test]
    fn test_operator_symbols() {
        // Test operator symbol literals like :+, :-, :==, etc.
        let tokens = Lexer::new(":+ :- :* :/ :== :!= :<= :>= :&& :||")
            .tokenize()
            .unwrap();
        assert!(matches!(&tokens[0].token, Token::Symbol(s) if s == "+"));
        assert!(matches!(&tokens[1].token, Token::Symbol(s) if s == "-"));
        assert!(matches!(&tokens[2].token, Token::Symbol(s) if s == "*"));
        assert!(matches!(&tokens[3].token, Token::Symbol(s) if s == "/"));
        assert!(matches!(&tokens[4].token, Token::Symbol(s) if s == "=="));
        assert!(matches!(&tokens[5].token, Token::Symbol(s) if s == "!="));
        assert!(matches!(&tokens[6].token, Token::Symbol(s) if s == "<="));
        assert!(matches!(&tokens[7].token, Token::Symbol(s) if s == ">="));
        assert!(matches!(&tokens[8].token, Token::Symbol(s) if s == "&&"));
        assert!(matches!(&tokens[9].token, Token::Symbol(s) if s == "||"));
    }

    #[test]
    fn test_operators() {
        let tokens = Lexer::new("|> => <- $ ::").tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Pipe);
        assert_eq!(tokens[1].token, Token::Arrow);
        assert_eq!(tokens[2].token, Token::AsyncCall);
        assert_eq!(tokens[3].token, Token::SyncCall);
        assert_eq!(tokens[4].token, Token::ColonColon);
    }

    #[test]
    fn test_comments() {
        let tokens = Lexer::new("x -- this is a comment\ny").tokenize().unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].token, Token::Ident(_)));
        assert!(matches!(tokens[1].token, Token::Ident(_)));
    }

    #[test]
    fn test_try_catch_tokens() {
        let tokens = Lexer::new("try { } catch e { }").tokenize().unwrap();
        assert_eq!(tokens[0].token, Token::Try);
        assert_eq!(tokens[1].token, Token::LBrace);
        assert_eq!(tokens[2].token, Token::RBrace);
        assert_eq!(tokens[3].token, Token::Catch);
    }
}
