//! Parser for grammar definitions.
//!
//! Parses grammar syntax like:
//! ```fmpl
//! grammar mud::commands <: base::parser {
//!     verb = word:v &{ valid_verb(v) } => v
//!     command = "take" spaces noun:obj => %{action: :take}
//! }
//! ```

use super::{CharRange, Grammar, Pattern, Rule};
use crate::ast::Expr;
use crate::error::{Error, Result};
use crate::lexer::Lexer;
use crate::parser::Parser as ExprParser;
use crate::value::Value;
use smol_str::SmolStr;

/// Parser for grammar definitions.
pub struct GrammarParser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> GrammarParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    /// Parse a complete grammar definition.
    pub fn parse(&mut self) -> Result<Grammar> {
        self.skip_whitespace();

        // Expect "grammar"
        self.expect_keyword("grammar")?;
        self.skip_whitespace();

        // Parse grammar name (qualified)
        let name = self.parse_qualified_name()?;
        self.skip_whitespace();

        // Optional parent grammar
        let parent = if self.peek_str("<:") {
            self.advance_by(2);
            self.skip_whitespace();
            Some(self.parse_qualified_name()?)
        } else {
            None
        };

        self.skip_whitespace();
        self.expect_char('{')?;

        let mut grammar = match parent {
            Some(p) => Grammar::with_parent(name, p),
            None => Grammar::new(name),
        };

        self.parse_rules_into(&mut grammar)?;
        Ok(grammar)
    }

    /// Parse an anonymous grammar: `{ rules }` (for grammar literals).
    pub fn parse_anonymous(&mut self) -> Result<Grammar> {
        self.skip_whitespace();
        self.expect_char('{')?;

        let mut grammar = Grammar::new(SmolStr::new("<anonymous>"));
        self.parse_rules_into(&mut grammar)?;
        Ok(grammar)
    }

    /// Parse an anonymous match block: `{ pattern => action; pattern => action; ... }`
    /// Creates a grammar with a single "main" rule that is a Choice of all patterns.
    pub fn parse_match_block(&mut self) -> Result<Grammar> {
        self.skip_whitespace();
        self.expect_char('{')?;

        let mut cases = Vec::new();

        loop {
            self.skip_whitespace();
            if self.peek_char() == Some('}') {
                self.advance();
                break;
            }
            if self.is_eof() {
                return Err(Error::Parser {
                    token: self.pos,
                    message: "unexpected end of match block".to_string(),
                });
            }

            // Parse pattern => action (or pattern when guard => action)
            let pattern = self.parse_pattern()?;
            self.skip_whitespace();

            // Check for optional guard: pattern when guard => action
            let final_pattern = if self.peek_str("when") {
                self.advance_by(4); // consume "when"
                self.skip_whitespace();

                // Parse the guard expression - read until we see "=>"
                let guard_start = self.pos;
                while !self.is_eof() && !self.peek_str("=>") {
                    self.advance();
                }
                let guard_source = &self.source[guard_start..self.pos];

                // Parse the guard as an FMPL expression
                let lexer = Lexer::new(guard_source);
                let tokens = lexer.tokenize()?;
                let mut expr_parser = ExprParser::new(&tokens);
                let guard_expr = expr_parser.parse()?;

                // The guard is an FMPL expression that should evaluate to a boolean
                // We wrap the pattern with a Guard that matches first, then checks the guard
                Pattern::Guard(Box::new(pattern), guard_expr)
            } else {
                pattern
            };

            self.skip_whitespace();

            if !self.peek_str("=>") {
                return Err(Error::Parser {
                    token: self.pos,
                    message: "expected '=>' in match case".to_string(),
                });
            }
            self.advance_by(2);
            self.skip_whitespace();

            let action = self.parse_match_action()?;
            cases.push(Pattern::Action(Box::new(final_pattern), action));

            // Optional semicolon between cases
            self.skip_whitespace();
            if self.peek_char() == Some(';') {
                self.advance();
            }
        }

        if cases.is_empty() {
            return Err(Error::Parser {
                token: self.pos,
                message: "match block must have at least one case".to_string(),
            });
        }

        let mut grammar = Grammar::new(SmolStr::new("<match>"));
        let main_pattern = if cases.len() == 1 {
            cases.into_iter().next().unwrap()
        } else {
            Pattern::Choice(cases)
        };
        grammar.add_rule(SmolStr::new("main"), Rule::new(main_pattern));
        Ok(grammar)
    }

    /// Parse rules into an existing grammar until closing brace.
    fn parse_rules_into(&mut self, grammar: &mut Grammar) -> Result<()> {
        loop {
            self.skip_whitespace();
            if self.peek_char() == Some('}') {
                self.advance();
                break;
            }
            if self.is_eof() {
                return Err(Error::Parser {
                    token: self.pos,
                    message: "unexpected end of grammar".to_string(),
                });
            }

            let (rule_name, rule) = self.parse_rule()?;
            grammar.add_rule(rule_name, rule);

            // Consume optional semicolon between rules
            self.skip_whitespace();
            if self.peek_char() == Some(';') {
                self.advance();
            }
        }

        Ok(())
    }

    /// Parse a single rule: `name = pattern (=> action)?`
    fn parse_rule(&mut self) -> Result<(SmolStr, Rule)> {
        let name = self.parse_ident()?;
        self.skip_whitespace();
        self.expect_char('=')?;
        self.skip_whitespace();

        let pattern = self.parse_pattern()?;
        self.skip_whitespace();

        let action = if self.peek_str("=>") {
            self.advance_by(2);
            self.skip_whitespace();
            Some(self.parse_action()?)
        } else {
            None
        };

        let rule = match action {
            Some(a) => Rule::with_action(pattern, a),
            None => Rule::new(pattern),
        };

        Ok((name, rule))
    }

    /// Parse a pattern (ordered choice at top level).
    fn parse_pattern(&mut self) -> Result<Pattern> {
        self.parse_choice()
    }

    /// Parse choice: `a | b | c`
    fn parse_choice(&mut self) -> Result<Pattern> {
        let mut alternatives = vec![self.parse_sequence()?];

        loop {
            self.skip_whitespace();
            if self.peek_char() == Some('|') && !self.peek_str("|>") {
                self.advance();
                self.skip_whitespace();
                alternatives.push(self.parse_sequence()?);
            } else {
                break;
            }
        }

        if alternatives.len() == 1 {
            Ok(alternatives.pop().unwrap())
        } else {
            Ok(Pattern::Choice(alternatives))
        }
    }

    /// Parse sequence: `a b c`
    fn parse_sequence(&mut self) -> Result<Pattern> {
        let mut items = Vec::new();

        loop {
            self.skip_whitespace();
            // Stop at choice separator, action arrow, rule end, semicolon, guard keyword, or grammar end
            if self.is_eof()
                || self.peek_char() == Some('|')
                || self.peek_char() == Some('}')
                || self.peek_char() == Some(')')
                || self.peek_char() == Some(';')
                || self.peek_str("=>")
                || self.peek_str("when")
                || self.is_at_rule_start()
            {
                break;
            }

            items.push(self.parse_prefix()?);
        }

        match items.len() {
            0 => Ok(Pattern::Empty),
            1 => Ok(items.pop().unwrap()),
            _ => Ok(Pattern::Seq(items)),
        }
    }

    /// Parse prefix operators: `&pattern`, `~pattern`
    fn parse_prefix(&mut self) -> Result<Pattern> {
        self.skip_whitespace();
        match self.peek_char() {
            Some('&') if !self.peek_str("&&") => {
                self.advance();
                if self.peek_char() == Some('{') {
                    // Semantic predicate: &{ expr }
                    self.advance();
                    let expr = self.parse_action()?;
                    self.skip_whitespace();
                    self.expect_char('}')?;
                    Ok(Pattern::Predicate(expr))
                } else {
                    let inner = self.parse_suffix()?;
                    Ok(Pattern::Lookahead(Box::new(inner)))
                }
            }
            Some('~') => {
                self.advance();
                let inner = self.parse_suffix()?;
                Ok(Pattern::Not(Box::new(inner)))
            }
            _ => self.parse_suffix(),
        }
    }

    /// Parse suffix operators: `pattern*`, `pattern+`, `pattern?`, `pattern:name`
    fn parse_suffix(&mut self) -> Result<Pattern> {
        let mut pattern = self.parse_primary()?;

        loop {
            match self.peek_char() {
                Some('*') => {
                    self.advance();
                    pattern = Pattern::Star(Box::new(pattern));
                }
                Some('+') => {
                    self.advance();
                    pattern = Pattern::Plus(Box::new(pattern));
                }
                Some('?') => {
                    self.advance();
                    pattern = Pattern::Optional(Box::new(pattern));
                }
                Some(':') => {
                    self.advance();
                    let name = self.parse_ident()?;
                    pattern = Pattern::Bind(Box::new(pattern), name);
                }
                _ => break,
            }
        }

        Ok(pattern)
    }

    /// Parse primary patterns: literals, rule refs, groups, char classes, map/list patterns.
    fn parse_primary(&mut self) -> Result<Pattern> {
        self.skip_whitespace();

        match self.peek_char() {
            Some('.') => {
                self.advance();
                Ok(Pattern::Any)
            }
            Some('"') => {
                let s = self.parse_string()?;
                if s.len() == 1 {
                    Ok(Pattern::Char(s.chars().next().unwrap()))
                } else {
                    Ok(Pattern::Literal(SmolStr::new(s)))
                }
            }
            Some('\'') => {
                let s = self.parse_char_literal()?;
                Ok(Pattern::Char(s))
            }
            Some('[') => {
                // Check if this is a list pattern (for value matching) or char class
                // List pattern: [pattern, pattern, ...] - matches list values
                // Char class: [a-zA-Z] - matches single character

                // Use a simple heuristic: if we see clear list markers immediately after '[', it's a list pattern
                // Save position for potential rollback
                let saved_pos = self.pos;
                self.advance(); // consume the '['
                self.skip_whitespace();

                // Distinguish between char class [a-z] and list pattern [x, y]
                // - Char class: has ranges with '-' (e.g., [0-9], [a-z])
                // - List pattern: has comma-separated values (e.g., [x, y], [1, 2])
                // - Ambiguous: [123] could be "char class with 3 chars" or "list with 1 element"
                //   We treat single digits/chars as list patterns for value matching
                let is_list_pattern = if let Some(next) = self.peek_char() {
                    match next {
                        ']' | '[' | '%' | '_' | ':' | '"' | '\'' => true,
                        c if c.is_alphabetic() => true,
                        c if c.is_ascii_digit() => {
                            // Check if this looks like a range (char class) or a list element
                            // Look ahead: if we see '-' after digit, it's a range (char class)
                            // If we see ',' or ']' or another digit/identifier, it's a list
                            let mut lookahead_pos = self.pos;
                            let mut found_range = false;
                            let mut found_comma = false;
                            let mut found_end = false;
                            // Look ahead up to 10 characters to detect the pattern
                            for _ in 0..10 {
                                if let Some(c) = self.source[lookahead_pos..].chars().next() {
                                    if c == '-' && !found_range {
                                        found_range = true;
                                    } else if c == ',' {
                                        found_comma = true;
                                        break;
                                    } else if c == ']' {
                                        found_end = true;
                                        break;
                                    }
                                    lookahead_pos += c.len_utf8();
                                } else {
                                    break;
                                }
                            }
                            // It's a list pattern if: has comma, OR (no range AND single element)
                            // It's a char class if: has range AND no comma
                            found_comma || (!found_range && found_end)
                        }
                        _ => false,
                    }
                } else {
                    true // EOF, default to list pattern
                };

                if is_list_pattern {
                    self.parse_list_pattern()
                } else {
                    // Rollback and parse as char class
                    self.pos = saved_pos;
                    self.parse_char_class()
                }
            }
            Some('%') => {
                // Map pattern for value matching: %{key: pattern, key2: pattern2}
                self.parse_map_pattern()
            }
            Some('(') => {
                self.advance();
                self.skip_whitespace();
                let inner = self.parse_pattern()?;
                self.skip_whitespace();
                self.expect_char(')')?;
                Ok(inner)
            }
            Some('<') => {
                // Super call: <rule>
                self.advance();
                let name = self.parse_ident()?;
                self.expect_char('>')?;
                Ok(Pattern::Super(name))
            }
            Some('_') => {
                // Check if it's just `_` (wildcard) or an identifier starting with `_`
                self.advance();
                if self
                    .peek_char()
                    .is_none_or(|c| !c.is_alphanumeric() && c != '_')
                {
                    // Just `_` alone - this is the wildcard/any pattern
                    Ok(Pattern::Any)
                } else {
                    // It's an identifier starting with `_`, continue parsing
                    let mut name = String::from("_");
                    while let Some(c) = self.peek_char() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    Ok(Pattern::Rule(SmolStr::new(&name)))
                }
            }
            Some(c) if c.is_alphabetic() => {
                let name = self.parse_ident()?;
                Ok(Pattern::Rule(name))
            }
            Some(c) => Err(Error::Parser {
                token: self.pos,
                message: format!("unexpected character in pattern: {:?}", c),
            }),
            None => Err(Error::UnexpectedEof),
        }
    }

    /// Parse a character class: `[a-zA-Z0-9]` or `[^...]`
    fn parse_char_class(&mut self) -> Result<Pattern> {
        self.expect_char('[')?;
        let negated = if self.peek_char() == Some('^') {
            self.advance();
            true
        } else {
            false
        };

        let mut ranges = Vec::new();

        while self.peek_char() != Some(']') && !self.is_eof() {
            let c = self.advance().ok_or(Error::UnexpectedEof)?;
            if c == '\\' {
                let escaped = self.parse_escape()?;
                if self.peek_char() == Some('-') && self.peek_ahead(1) != Some(']') {
                    self.advance();
                    let end = self.advance().ok_or(Error::UnexpectedEof)?;
                    let end = if end == '\\' {
                        self.parse_escape()?
                    } else {
                        end
                    };
                    ranges.push(CharRange::Range(escaped, end));
                } else {
                    ranges.push(CharRange::Char(escaped));
                }
            } else if self.peek_char() == Some('-') && self.peek_ahead(1) != Some(']') {
                self.advance();
                let end = self.advance().ok_or(Error::UnexpectedEof)?;
                let end = if end == '\\' {
                    self.parse_escape()?
                } else {
                    end
                };
                ranges.push(CharRange::Range(c, end));
            } else {
                ranges.push(CharRange::Char(c));
            }
        }

        self.expect_char(']')?;

        if negated {
            Ok(Pattern::NegCharClass(ranges))
        } else {
            Ok(Pattern::CharClass(ranges))
        }
    }

    /// Parse an escape sequence.
    fn parse_escape(&mut self) -> Result<char> {
        match self.advance() {
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('\\') => Ok('\\'),
            Some('"') => Ok('"'),
            Some('\'') => Ok('\''),
            Some('[') => Ok('['),
            Some(']') => Ok(']'),
            Some(c) => Ok(c),
            None => Err(Error::UnexpectedEof),
        }
    }

    /// Parse a string literal.
    fn parse_string(&mut self) -> Result<String> {
        self.expect_char('"')?;
        let mut s = String::new();
        while self.peek_char() != Some('"') {
            match self.advance() {
                Some('\\') => s.push(self.parse_escape()?),
                Some(c) => s.push(c),
                None => return Err(Error::UnexpectedEof),
            }
        }
        self.expect_char('"')?;
        Ok(s)
    }

    /// Parse a character literal.
    fn parse_char_literal(&mut self) -> Result<char> {
        self.expect_char('\'')?;
        let c = if self.peek_char() == Some('\\') {
            self.advance();
            self.parse_escape()?
        } else {
            self.advance().ok_or(Error::UnexpectedEof)?
        };
        self.expect_char('\'')?;
        Ok(c)
    }

    /// Parse a map pattern for value matching: %{key: pattern, key2: pattern2}
    fn parse_map_pattern(&mut self) -> Result<Pattern> {
        self.expect_char('%')?;
        self.expect_char('{')?;
        self.skip_whitespace();

        let mut entries = Vec::new();

        while self.peek_char() != Some('}') && !self.is_eof() {
            self.skip_whitespace();

            // Parse the key (identifier or symbol)
            let key = if self.peek_char() == Some(':') {
                // Symbol literal as key
                self.advance();
                self.parse_ident()?
            } else {
                // Regular identifier as key
                self.parse_ident()?
            };

            self.skip_whitespace();
            self.expect_char(':')?;
            self.skip_whitespace();

            // Parse the value pattern (using value pattern rules)
            let value_pattern = self.parse_value_pattern()?;

            entries.push((key, value_pattern));

            self.skip_whitespace();

            // Check for comma separator or end
            if self.peek_char() == Some(',') {
                self.advance();
            }
        }

        self.expect_char('}')?;
        Ok(Pattern::MapMatch(entries))
    }

    /// Parse a list pattern for value matching: [pattern, pattern, ...]
    fn parse_list_pattern(&mut self) -> Result<Pattern> {
        // The opening '[' has already been consumed
        self.skip_whitespace();

        let mut patterns = Vec::new();
        let mut rest_pattern = None;

        while self.peek_char() != Some(']') && !self.is_eof() {
            self.skip_whitespace();

            // Check for rest pattern (| tail)
            if self.peek_char() == Some('|') {
                self.advance();
                self.skip_whitespace();
                let rest_ident = self.parse_ident()?;
                rest_pattern = Some(Box::new(Pattern::Bind(Box::new(Pattern::Any), rest_ident)));
                break;
            }

            // Parse element pattern (using value pattern rules)
            patterns.push(self.parse_value_pattern()?);

            self.skip_whitespace();

            // Check for comma separator or rest pattern
            if self.peek_char() == Some(',') {
                self.advance();
                self.skip_whitespace();
            } else if self.peek_char() == Some('|') {
                // Rest pattern after elements
                self.advance();
                self.skip_whitespace();
                let rest_ident = self.parse_ident()?;
                rest_pattern = Some(Box::new(Pattern::Bind(Box::new(Pattern::Any), rest_ident)));
                break;
            }
        }

        self.expect_char(']')?;
        Ok(Pattern::ListMatch(patterns, rest_pattern))
    }

    /// Parse a value pattern (used inside map/list patterns).
    /// Value patterns treat bare identifiers as bindings (not rule references).
    fn parse_value_pattern(&mut self) -> Result<Pattern> {
        self.skip_whitespace();

        let pattern = match self.peek_char() {
            Some('_') => {
                self.advance();
                Ok(Pattern::Any)
            }
            Some(':') => {
                // Symbol literal
                self.advance();
                let name = self.parse_ident()?;
                Ok(Pattern::SymbolMatch(name))
            }
            Some('[') => {
                self.advance();
                self.parse_list_pattern()
            }
            Some('%') => self.parse_map_pattern(),
            Some(c) if c.is_alphabetic() || c == '_' => {
                // Bare identifier - treat as binding (not rule reference)
                let name = self.parse_ident()?;
                Ok(Pattern::Bind(Box::new(Pattern::Any), name))
            }
            Some('"') => {
                let s = self.parse_string()?;
                Ok(Pattern::MatchValue(Value::String(s.into())))
            }
            Some('\'') => {
                let s = self.parse_char_literal()?;
                let mut str_buf = String::new();
                str_buf.push(s);
                Ok(Pattern::MatchValue(Value::String(str_buf.into())))
            }
            Some(c) if c.is_ascii_digit() || c == '-' => {
                // Parse a number literal
                let num_str = self.parse_number_literal()?;
                if num_str.contains('.') {
                    let f: f64 = num_str.parse().map_err(|_| Error::Parser {
                        token: self.pos,
                        message: format!("invalid float literal: {}", num_str),
                    })?;
                    Ok(Pattern::MatchValue(Value::Float(f)))
                } else {
                    let i: i64 = num_str.parse().map_err(|_| Error::Parser {
                        token: self.pos,
                        message: format!("invalid int literal: {}", num_str),
                    })?;
                    Ok(Pattern::MatchValue(Value::Int(i)))
                }
            }
            Some(c) => Err(Error::Parser {
                token: self.pos,
                message: format!("unexpected character in value pattern: {:?}", c),
            }),
            None => Err(Error::UnexpectedEof),
        };

        // Don't apply postfix modifiers to value patterns (they don't make sense for bindings)
        pattern
    }

    /// Parse a number literal (int or float).
    fn parse_number_literal(&mut self) -> Result<String> {
        let start = self.pos;

        // Handle negative sign
        if self.peek_char() == Some('-') {
            self.advance();
        }

        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.advance();
            } else {
                break;
            }
        }

        let end = self.pos;
        if end == start {
            return Err(Error::Parser {
                token: self.pos,
                message: "expected number".to_string(),
            });
        }

        Ok(self.source[start..end].to_string())
    }

    /// Parse a semantic action (FMPL expression).
    fn parse_action(&mut self) -> Result<Expr> {
        // Find the end of the action - next rule or end of grammar
        let start = self.pos;
        let mut brace_depth = 0;
        let mut paren_depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        while !self.is_eof() {
            let c = self.peek_char().unwrap();

            if escape_next {
                escape_next = false;
                self.advance();
                continue;
            }

            if c == '\\' {
                escape_next = true;
                self.advance();
                continue;
            }

            if c == '"' {
                in_string = !in_string;
                self.advance();
                continue;
            }

            if in_string {
                self.advance();
                continue;
            }

            match c {
                '{' => brace_depth += 1,
                '}' if brace_depth > 0 => brace_depth -= 1,
                '}' => break, // End of grammar
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                _ => {}
            }

            // Check if we're at a new rule (ident = ...)
            if brace_depth == 0 && paren_depth == 0 && self.is_at_rule_start() {
                break;
            }

            self.advance();
        }

        let action_src = &self.source[start..self.pos].trim();
        if action_src.is_empty() {
            return Err(Error::Parser {
                token: start,
                message: "empty semantic action".to_string(),
            });
        }

        // Parse as FMPL expression
        let tokens = Lexer::new(action_src).tokenize()?;
        let expr = ExprParser::with_source(&tokens, action_src).parse()?;
        Ok(expr)
    }

    /// Parse a semantic action for match blocks (stops at ; or } at depth 0).
    fn parse_match_action(&mut self) -> Result<Expr> {
        let start = self.pos;
        let mut brace_depth = 0;
        let mut paren_depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        while !self.is_eof() {
            let c = self.peek_char().unwrap();

            if escape_next {
                escape_next = false;
                self.advance();
                continue;
            }

            if c == '\\' {
                escape_next = true;
                self.advance();
                continue;
            }

            if c == '"' {
                in_string = !in_string;
                self.advance();
                continue;
            }

            if in_string {
                self.advance();
                continue;
            }

            match c {
                '{' => brace_depth += 1,
                '}' if brace_depth > 0 => brace_depth -= 1,
                '}' => break, // End of match block
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                ';' if brace_depth == 0 && paren_depth == 0 => break, // Next case
                _ => {}
            }

            self.advance();
        }

        let action_src = &self.source[start..self.pos].trim();
        if action_src.is_empty() {
            return Err(Error::Parser {
                token: start,
                message: "empty semantic action".to_string(),
            });
        }

        // Parse as FMPL expression
        let tokens = Lexer::new(action_src).tokenize()?;
        let expr = ExprParser::with_source(&tokens, action_src).parse()?;
        Ok(expr)
    }

    /// Check if we're at the start of a new rule (identifier followed by =).
    fn is_at_rule_start(&self) -> bool {
        let remaining = &self.source[self.pos..];
        let trimmed = remaining.trim_start();

        // Find potential identifier
        let ident_end = trimmed
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(trimmed.len());

        if ident_end == 0 {
            return false;
        }

        let after_ident = trimmed[ident_end..].trim_start();
        after_ident.starts_with('=') && !after_ident.starts_with("=>")
    }

    /// Parse a qualified name like `foo::bar::baz`.
    fn parse_qualified_name(&mut self) -> Result<SmolStr> {
        let mut name = self.parse_ident()?.to_string();

        while self.peek_str("::") {
            self.advance_by(2);
            name.push_str("::");
            name.push_str(&self.parse_ident()?);
        }

        Ok(SmolStr::new(name))
    }

    /// Parse an identifier.
    fn parse_ident(&mut self) -> Result<SmolStr> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        if self.pos == start {
            return Err(Error::Parser {
                token: self.pos,
                message: "expected identifier".to_string(),
            });
        }

        Ok(SmolStr::new(&self.source[start..self.pos]))
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '#' {
                // Skip comment to end of line
                while let Some(c) = self.peek_char() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(n)
    }

    fn peek_str(&self, s: &str) -> bool {
        self.source[self.pos..].starts_with(s)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn advance_by(&mut self, n: usize) {
        for _ in 0..n {
            if let Some(c) = self.peek_char() {
                self.pos += c.len_utf8();
            }
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.peek_char() {
            Some(c) if c == expected => {
                self.advance();
                Ok(())
            }
            Some(c) => Err(Error::Parser {
                token: self.pos,
                message: format!("expected {:?}, got {:?}", expected, c),
            }),
            None => Err(Error::UnexpectedEof),
        }
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<()> {
        if self.source[self.pos..].starts_with(kw) {
            let after = self.source.get(self.pos + kw.len()..);
            if let Some(rest) = after
                && let Some(c) = rest.chars().next()
                && (c.is_alphanumeric() || c == '_')
            {
                return Err(Error::Parser {
                    token: self.pos,
                    message: format!("expected keyword {:?}", kw),
                });
            }
            self.pos += kw.len();
            Ok(())
        } else {
            Err(Error::Parser {
                token: self.pos,
                message: format!("expected keyword {:?}", kw),
            })
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_grammar() {
        let src = r#"
            grammar test::simple {
                digit = [0-9]
                number = digit+
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();

        assert_eq!(grammar.name.as_str(), "test::simple");
        assert!(grammar.parent.is_none());
        assert!(grammar.rules.contains_key("digit"));
        assert!(grammar.rules.contains_key("number"));
    }

    #[test]
    fn test_parse_grammar_with_parent() {
        let src = r#"
            grammar mud::commands <: base::parser {
                verb = word
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();

        assert_eq!(grammar.name.as_str(), "mud::commands");
        assert_eq!(grammar.parent.as_deref(), Some("base::parser"));
    }

    #[test]
    fn test_parse_choice() {
        let src = r#"
            grammar test::choice {
                vowel = 'a' | 'e' | 'i' | 'o' | 'u'
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        let rule = grammar.rules.get("vowel").unwrap();

        match &rule.pattern {
            Pattern::Choice(alts) => assert_eq!(alts.len(), 5),
            _ => panic!("expected Choice pattern"),
        }
    }

    #[test]
    fn test_parse_sequence_with_binding() {
        let src = r#"
            grammar test::bind {
                pair = word:a spaces word:b
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        let rule = grammar.rules.get("pair").unwrap();

        match &rule.pattern {
            Pattern::Seq(items) => assert_eq!(items.len(), 3),
            _ => panic!("expected Seq pattern"),
        }
    }

    #[test]
    fn test_parse_char_class() {
        let src = r#"
            grammar test::class {
                hex = [0-9a-fA-F]
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        let rule = grammar.rules.get("hex").unwrap();

        match &rule.pattern {
            Pattern::CharClass(ranges) => assert_eq!(ranges.len(), 3),
            _ => panic!("expected CharClass pattern"),
        }
    }

    #[test]
    fn test_parse_lookahead_and_not() {
        let src = r#"
            grammar test::lookahead {
                notslash = ~'/' .
                followed = &digit word
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();

        assert!(grammar.rules.contains_key("notslash"));
        assert!(grammar.rules.contains_key("followed"));
    }

    #[test]
    fn test_parse_super_call() {
        let src = r#"
            grammar test::super_ <: base::parser {
                word = <word> '!'
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        let rule = grammar.rules.get("word").unwrap();

        match &rule.pattern {
            Pattern::Seq(items) => {
                assert!(matches!(&items[0], Pattern::Super(n) if n == "word"));
            }
            _ => panic!("expected Seq pattern"),
        }
    }

    #[test]
    fn test_parse_action_with_grammar_literal() {
        let src = r#"
            grammar test::action {
                digit = [0-9] => grammar { inner = [0-9] }
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        let rule = grammar.rules.get("digit").unwrap();

        match rule.action.as_ref() {
            Some(Expr::GrammarLiteral(inner)) => {
                assert!(inner.rules.contains_key("inner"));
            }
            _ => panic!("expected grammar literal action"),
        }
    }
}
