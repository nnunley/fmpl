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
use crate::pattern::{CharPattern, RepeatKind};
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
            // Use parse_pattern_without_action so we don't consume the => action here
            let pattern = self.parse_pattern_without_action()?;
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
                Pattern::Guard {
                    pattern: Box::new(pattern),
                    predicate: guard_expr,
                }
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
            cases.push(Pattern::Action {
                pattern: Box::new(final_pattern),
                action,
            });

            // Optional semicolon or comma between cases
            self.skip_whitespace();
            if self.peek_char() == Some(';') || self.peek_char() == Some(',') {
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
            // Match blocks use traditional PEG semantics (no backtracking)
            Pattern::Choice(cases.into_iter().map(|p| (p, false)).collect())
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

            // Consume optional semicolon or comma between rules
            self.skip_whitespace();
            if self.peek_char() == Some(';') || self.peek_char() == Some(',') {
                self.advance();
            }
        }

        Ok(())
    }

    /// Parse a single rule: `?name = pattern (=> action)?` or `name(params) = expr`
    fn parse_rule(&mut self) -> Result<(SmolStr, Rule)> {
        // Check for `?` backtracking marker
        let backtracking = if self.peek_char() == Some('?') {
            self.advance();
            true
        } else {
            false
        };
        self.skip_whitespace();

        let name = self.parse_ident()?;
        self.skip_whitespace();

        // Check for function-style rule: name(params) = expr
        if self.peek_char() == Some('(') {
            self.advance(); // consume '('
            self.skip_whitespace();

            // Parse parameter list
            let mut params = Vec::new();
            if self.peek_char() != Some(')') {
                params.push(self.parse_ident()?);
                self.skip_whitespace();

                while self.peek_char() == Some(',') {
                    self.advance();
                    self.skip_whitespace();
                    params.push(self.parse_ident()?);
                    self.skip_whitespace();
                }
            }

            self.expect_char(')')?;
            self.skip_whitespace();
            self.expect_char('=')?;
            self.skip_whitespace();

            // Parse the body as an FMPL expression
            let body = self.parse_action()?;

            let rule = Rule::function(params.into_iter().map(SmolStr::from).collect(), body);

            return Ok((name, rule));
        }

        self.expect_char('=')?;
        self.skip_whitespace();

        // Check if RHS is a lambda expression (starts with \)
        // In this case, the rule is a helper function that returns the lambda
        if self.peek_char() == Some('\\') {
            // Parse the lambda as an FMPL expression
            let body = self.parse_action()?;
            let rule = Rule {
                pattern: Pattern::Empty, // Empty pattern always succeeds
                action: Some(body),
                backtracking,
                ..Default::default()
            };
            return Ok((name, rule));
        }

        // Parse pattern with optional when guard
        let pattern = self.parse_pattern()?;
        self.skip_whitespace();

        // Actions are now handled at the alternative level (inside parse_choice -> parse_alternative)
        // so Rule.action is no longer used. Actions are embedded in Pattern::Action.
        let rule = Rule {
            pattern,
            backtracking,
            ..Default::default()
        };

        Ok((name, rule))
    }

    /// Parse a pattern (ordered choice at top level).
    fn parse_pattern(&mut self) -> Result<Pattern> {
        self.parse_choice(true) // allow actions in grammar rules
    }

    /// Parse pattern without consuming actions - for match blocks where actions are handled separately
    fn parse_pattern_without_action(&mut self) -> Result<Pattern> {
        self.parse_choice(false)
    }

    /// Parse choice: `?a | ?b | c`
    /// Alternatives can be prefixed with `?` to enable backtracking for that alternative.
    /// Each alternative can have its own action: `"a" => 1 | "b" => 2` (if allow_action is true)
    fn parse_choice(&mut self, allow_action: bool) -> Result<Pattern> {
        // Check for `?` marker on first alternative
        let first_uses_backtracking = if self.peek_char() == Some('?') {
            self.advance();
            true
        } else {
            false
        };

        // Parse first alternative (sequence + optional action)
        let first_alt = self.parse_alternative(allow_action)?;
        let mut alternatives = vec![(first_alt, first_uses_backtracking)]; // (pattern, uses_backtracking)

        loop {
            self.skip_whitespace();
            // Only | is a choice separator (comma separates named rules, not alternatives)
            if self.peek_char() == Some('|') && !self.peek_str("|>") {
                self.advance();
                self.skip_whitespace();

                // Check for `?` marker on this alternative
                let uses_backtracking = if self.peek_char() == Some('?') {
                    self.advance();
                    true
                } else {
                    false
                };

                let pattern = self.parse_alternative(allow_action)?;
                alternatives.push((pattern, uses_backtracking));
            } else {
                break;
            }
        }

        if alternatives.len() == 1 {
            Ok(alternatives.pop().unwrap().0)
        } else {
            Ok(Pattern::Choice(alternatives))
        }
    }

    /// Parse a single alternative in a choice: `sequence (when guard)? (=> action)?`
    /// If allow_action is false, don't consume the action (for match blocks)
    fn parse_alternative(&mut self, allow_action: bool) -> Result<Pattern> {
        let pattern = self.parse_sequence()?;
        self.skip_whitespace();

        // Check for when guard on this alternative (only when allow_action, because
        // match blocks handle their own when guards)
        let pattern = if allow_action && self.peek_keyword("when") {
            self.advance_by(4); // consume "when"
            self.skip_whitespace();

            // Parse the guard expression - read until we see "=>" or a terminating character
            let guard_start = self.pos;
            while !self.is_eof() {
                if self.peek_str("=>")
                    || self.peek_char() == Some(';')
                    || self.peek_char() == Some('}')
                    || self.peek_char() == Some('|')
                    || self.peek_char() == Some('\n')
                {
                    break;
                }
                self.advance();
            }
            let guard_source = &self.source[guard_start..self.pos].trim();

            // Parse the guard as an FMPL expression
            let lexer = Lexer::new(guard_source);
            let tokens = lexer.tokenize()?;
            let mut expr_parser = ExprParser::new(&tokens);
            let guard_expr = expr_parser.parse()?;

            Pattern::Guard {
                pattern: Box::new(pattern),
                predicate: guard_expr,
            }
        } else {
            pattern
        };

        self.skip_whitespace();

        // Check for action on this alternative (only in grammar rules, not match blocks)
        if allow_action && self.peek_str("=>") {
            self.advance_by(2);
            self.skip_whitespace();
            let action = self.parse_alternative_action()?;
            Ok(Pattern::Action {
                pattern: Box::new(pattern),
                action,
            })
        } else {
            Ok(pattern)
        }
    }

    /// Parse an action within an alternative (stops at | ; } or newline followed by identifier)
    fn parse_alternative_action(&mut self) -> Result<Expr> {
        let start = self.pos;
        let mut brace_depth = 0;
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let mut in_string = false;
        let mut escape_next = false;
        let mut in_symbol = false; // Track if we're inside a symbol literal like :||

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

            // Track symbol literal state
            // A symbol starts with : followed by any non-whitespace, non-delimiter character
            // Inside a symbol, | and other special chars are allowed
            if c == ':' && !in_symbol {
                // Check if this is the start of a symbol
                let next_pos = self.pos + 1;
                if next_pos < self.source.len() {
                    let next_c = self.source[next_pos..next_pos + 1].chars().next();
                    // Symbol starts if : is followed by non-whitespace and certain non-delimiters
                    // Note: | IS allowed in symbols (e.g., :||, :|), so don't exclude it here
                    if next_c != Some(' ')
                        && next_c != Some('(')
                        && next_c != Some('[')
                        && next_c != Some('{')
                        && next_c != Some(',')
                        && next_c != Some(';')
                        && next_c != Some('\n')
                        && next_c != Some('\t')
                    {
                        in_symbol = true;
                    }
                }
            }

            // End symbol at whitespace, (, [, {, comma, semicolon, or newline
            // But NOT at | since | is allowed inside symbols
            if in_symbol {
                if c == ' '
                    || c == '('
                    || c == '['
                    || c == '{'
                    || c == ','
                    || c == ';'
                    || c == '\n'
                    || c == '\t'
                {
                    in_symbol = false;
                }
            }

            match c {
                '{' => brace_depth += 1,
                '}' if brace_depth > 0 => brace_depth -= 1,
                '}' => break, // End of grammar
                '(' => paren_depth += 1,
                ')' if paren_depth > 0 => paren_depth -= 1,
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                '|' if brace_depth == 0
                    && paren_depth == 0
                    && bracket_depth == 0
                    && !self.peek_str("|>")
                    && !in_symbol =>
                {
                    break;
                } // Choice separator (not in symbol)
                ',' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 && !in_symbol => {
                    break;
                } // Rule separator (not in symbol)
                ';' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 && !in_symbol => {
                    break;
                } // Rule terminator (not in symbol)
                '\n' => {
                    // Check if there's a rule starting on next line
                    let saved_pos = self.pos;
                    self.advance(); // consume newline
                    self.skip_whitespace();
                    if self.is_at_rule_start() || self.peek_char() == Some('}') {
                        self.pos = saved_pos;
                        break;
                    }
                    continue;
                }
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
        // DEBUG: print action source
        let tokens = Lexer::new(action_src).tokenize()?;
        let expr = ExprParser::with_source(&tokens, action_src).parse()?;
        Ok(expr)
    }

    /// Parse sequence: `a b c` or `a:x when guard b:y when guard`
    /// Each element can have its own `when` guard for CSP-style constraints.
    fn parse_sequence(&mut self) -> Result<Pattern> {
        let mut items = Vec::new();

        loop {
            self.skip_whitespace();
            // Stop at choice separator, action arrow, rule end, semicolon, or grammar end
            if self.is_eof()
                || self.peek_char() == Some('|')
                || self.peek_char() == Some('}')
                || self.peek_char() == Some(')')
                || self.peek_char() == Some(';')
                || self.peek_str("=>")
                || self.is_at_rule_start()
            {
                break;
            }

            // Check if this is a 'when' at the start of position - that means it's an alternative-level guard
            if self.peek_keyword("when") && items.is_empty() {
                // 'when' at the very start means this is an alternative-level guard, not sequence-level
                break;
            }

            let mut elem = self.parse_prefix()?;

            // Check for per-element when guard: element when guard
            self.skip_whitespace();
            if self.peek_keyword("when") {
                self.advance_by(4); // consume "when"
                self.skip_whitespace();

                // Parse guard until next element, choice, action, or rule end
                let guard_start = self.pos;
                let mut paren_depth = 0;
                let mut bracket_depth = 0;
                while !self.is_eof() {
                    let c = self.peek_char().unwrap();
                    match c {
                        '(' => paren_depth += 1,
                        ')' if paren_depth > 0 => paren_depth -= 1,
                        '[' => bracket_depth += 1,
                        ']' if bracket_depth > 0 => bracket_depth -= 1,
                        '|' | ';' | '}' if paren_depth == 0 && bracket_depth == 0 => break,
                        _ => {}
                    }
                    // Check for => (action arrow) at depth 0
                    if paren_depth == 0 && bracket_depth == 0 && self.peek_str("=>") {
                        break;
                    }
                    // Check for start of next sequence element (identifier not followed by 'in')
                    // But NOT if we're after a '.' (method call) or a comparison operator
                    if paren_depth == 0 && bracket_depth == 0 {
                        // Look for pattern starts: identifier, string literal, character class, etc.
                        // Skip this check if the previous character was '.' (method call context)
                        let prev_char = if self.pos > guard_start {
                            self.source[..self.pos].chars().last()
                        } else {
                            None
                        };
                        let in_method_call = prev_char == Some('.');

                        if c.is_alphabetic() && !self.peek_keyword("when") && !in_method_call {
                            // Could be start of next pattern - peek ahead to see if it's an identifier
                            // followed by something that looks like a pattern
                            let saved = self.pos;
                            // Try to parse as an expression or pattern start
                            // If the identifier is followed by : then whitespace/pattern, it's a new element
                            // But if it's followed by ( directly (method call), it's part of the guard expression
                            while let Some(ch) = self.peek_char() {
                                if ch.is_alphanumeric() || ch == '_' {
                                    self.advance();
                                } else {
                                    break;
                                }
                            }
                            let after_ident = self.peek_char();
                            self.skip_whitespace();
                            let after_ws = self.peek_char();
                            self.pos = saved;

                            // Only break for pattern-like constructs:
                            // - identifier followed by ':' is binding like foo:x
                            // - identifier followed by whitespace then '[' or '"' is a pattern
                            // But NOT identifier followed by '(' (that's a function call)
                            if after_ident == Some(':') {
                                // Binding syntax
                                break;
                            } else if after_ident != Some('(')
                                && (after_ws == Some('[') || after_ws == Some('"'))
                            {
                                // Pattern element
                                break;
                            }
                            // Otherwise continue as part of the guard expression
                        }
                    }
                    self.advance();
                }
                let guard_source = &self.source[guard_start..self.pos].trim();

                if !guard_source.is_empty() {
                    let lexer = Lexer::new(guard_source);
                    let tokens = lexer.tokenize()?;
                    let mut expr_parser = ExprParser::new(&tokens);
                    let guard_expr = expr_parser.parse()?;

                    elem = Pattern::Guard {
                        pattern: Box::new(elem),
                        predicate: guard_expr,
                    };
                }
            }

            items.push(elem);
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
                    pattern = Pattern::Repeat {
                        pattern: Box::new(pattern),
                        kind: RepeatKind::ZeroOrMore,
                    };
                }
                Some('+') => {
                    self.advance();
                    pattern = Pattern::Repeat {
                        pattern: Box::new(pattern),
                        kind: RepeatKind::OneOrMore,
                    };
                }
                Some('?') => {
                    self.advance();
                    pattern = Pattern::Optional(Box::new(pattern));
                }
                Some(':') => {
                    self.advance();
                    // Check for choice point marker: digit:?s
                    let is_choice = if self.peek_char() == Some('?') {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    let name = self.parse_ident()?;
                    pattern = Pattern::Bind {
                        name: name,
                        pattern: Box::new(pattern),
                        is_choice: is_choice,
                    };
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
                    Ok(Pattern::Char(CharPattern::Exact(s.chars().next().unwrap())))
                } else {
                    Ok(Pattern::StringLiteral(SmolStr::new(s)))
                }
            }
            Some('\'') => {
                let s = self.parse_char_literal()?;
                Ok(Pattern::Char(CharPattern::Exact(s)))
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
                // - Char class: has ranges with '-' (e.g., [0-9], [a-z]), or just chars [abc]
                //   Also includes escaped chars and quotes: ["\\] matches " or \
                // - List pattern: has comma-separated values (e.g., [x, y], [1, 2])
                // - Ambiguous: [123] could be "char class with 3 chars" or "list with 1 element"
                //   We treat single digits/chars without commas as char classes in grammar context
                // Note: " and ' inside [...] are character class members, not list pattern indicators
                // E.g., ["\\] matches the character " or \, and ['] matches the character '
                let is_list_pattern = if let Some(next) = self.peek_char() {
                    match next {
                        ']' | '[' | '%' | '_' | ':' | '|' => true, // | indicates rest pattern
                        c if c.is_alphabetic() => {
                            // Check for range pattern [a-z] vs list [x, y]
                            let mut lookahead_pos = self.pos;
                            let mut found_comma = false;
                            let mut found_pipe = false; // | indicates rest pattern

                            // Look ahead up to 20 characters to detect the pattern
                            for _ in 0..20 {
                                if let Some(c) = self.source[lookahead_pos..].chars().next() {
                                    if c == ',' {
                                        found_comma = true;
                                        break;
                                    } else if c == '|' {
                                        found_pipe = true;
                                        break;
                                    } else if c == ']' {
                                        break;
                                    }
                                    lookahead_pos += c.len_utf8();
                                } else {
                                    break;
                                }
                            }

                            // It's a list pattern ONLY if: has comma or has pipe (rest)
                            // Otherwise treat as char class (even without ranges, [abc] is a char class)
                            found_comma || found_pipe
                        }
                        c if c.is_ascii_digit() => {
                            // Check if this looks like a range (char class) or a list element
                            // Look ahead: if we see ',' or '|', it's a list pattern
                            let mut lookahead_pos = self.pos;
                            let mut found_comma = false;
                            let mut found_pipe = false; // | indicates rest pattern
                            // Look ahead up to 20 characters to detect the pattern
                            for _ in 0..20 {
                                if let Some(c) = self.source[lookahead_pos..].chars().next() {
                                    if c == ',' {
                                        found_comma = true;
                                        break;
                                    } else if c == '|' {
                                        found_pipe = true;
                                        break;
                                    } else if c == ']' {
                                        break;
                                    }
                                    lookahead_pos += c.len_utf8();
                                } else {
                                    break;
                                }
                            }
                            // It's a list pattern ONLY if: has comma or has pipe (rest)
                            // Otherwise treat as char class (even [0] is char class in grammar context)
                            found_comma || found_pipe
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
                    Ok(Pattern::ApplyRule(SmolStr::new(&name)))
                }
            }
            Some(c) if c.is_alphabetic() => {
                let name = self.parse_ident()?;
                Ok(Pattern::ApplyRule(name))
            }
            Some(':') => {
                // Constructor pattern: :Tag(patterns...) or symbol literal :Tag/:+
                self.advance(); // consume ':'
                let tag = self.parse_symbol_name()?;
                self.skip_whitespace();
                if self.peek_char() == Some('(') {
                    self.advance(); // consume '('
                    self.skip_whitespace();
                    let mut patterns = Vec::new();
                    if self.peek_char() != Some(')') {
                        // Use value_pattern parsing for constructor children -
                        // this treats bare identifiers as bindings, not rule references
                        patterns.push(self.parse_tag_child_pattern()?);
                        self.skip_whitespace();
                        while self.peek_char() == Some(',') {
                            self.advance();
                            self.skip_whitespace();
                            if self.peek_char() == Some(')') {
                                break; // trailing comma
                            }
                            patterns.push(self.parse_tag_child_pattern()?);
                            self.skip_whitespace();
                        }
                    }
                    self.expect_char(')')?;
                    Ok(Pattern::TagMatch(tag, patterns))
                } else {
                    // Just :Tag/:+ without parens - match symbol value
                    Ok(Pattern::SymbolLiteral(tag))
                }
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
            Ok(Pattern::Char(CharPattern::NegatedClass(ranges)))
        } else {
            Ok(Pattern::Char(CharPattern::Class(ranges)))
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
                rest_pattern = Some(Box::new(Pattern::Bind {
                    name: rest_ident,
                    pattern: Box::new(Pattern::Any),
                    is_choice: false,
                }));
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
                rest_pattern = Some(Box::new(Pattern::Bind {
                    name: rest_ident,
                    pattern: Box::new(Pattern::Any),
                    is_choice: false,
                }));
                break;
            }
        }

        self.expect_char(']')?;
        Ok(Pattern::ListMatch(patterns, rest_pattern))
    }

    /// Parse a pattern inside a TagMatch (constructor pattern).
    /// Like value patterns, bare identifiers are bindings, not rule references.
    /// But this also supports nested TagMatch patterns like :Binary(op, :Int(a), :Int(b)).
    fn parse_tag_child_pattern(&mut self) -> Result<Pattern> {
        self.skip_whitespace();

        match self.peek_char() {
            Some('_') => {
                self.advance();
                Ok(Pattern::Any)
            }
            Some(':') => {
                // Could be a symbol literal :foo/:+ or a nested TagMatch :Tag(...)
                self.advance();
                let name = self.parse_symbol_name()?;
                self.skip_whitespace();
                if self.peek_char() == Some('(') {
                    // Nested TagMatch: :Tag(patterns...)
                    self.advance(); // consume '('
                    self.skip_whitespace();
                    let mut patterns = Vec::new();
                    if self.peek_char() != Some(')') {
                        patterns.push(self.parse_tag_child_pattern()?);
                        self.skip_whitespace();
                        while self.peek_char() == Some(',') {
                            self.advance();
                            self.skip_whitespace();
                            if self.peek_char() == Some(')') {
                                break; // trailing comma
                            }
                            patterns.push(self.parse_tag_child_pattern()?);
                            self.skip_whitespace();
                        }
                    }
                    self.expect_char(')')?;
                    Ok(Pattern::TagMatch(name, patterns))
                } else {
                    // Symbol literal :foo/:+ - matches Value::Symbol("foo")/Value::Symbol("+")
                    Ok(Pattern::SymbolLiteral(name))
                }
            }
            Some('[') => {
                // List pattern
                self.advance();
                self.parse_list_pattern()
            }
            Some('%') => {
                // Map pattern
                self.parse_map_pattern()
            }
            Some(c) if c.is_alphabetic() => {
                let name = self.parse_ident()?;
                // Check for suffix operators: rule*:binding, rule+:binding, rule:binding
                match self.peek_char() {
                    Some('*') => {
                        self.advance(); // consume '*'
                        let repeat = Pattern::Repeat {
                            pattern: Box::new(Pattern::ApplyRule(name)),
                            kind: RepeatKind::ZeroOrMore,
                        };
                        if self.peek_char() == Some(':') {
                            self.advance(); // consume ':'
                            let bind_name = self.parse_ident()?;
                            Ok(Pattern::Bind {
                                name: bind_name,
                                pattern: Box::new(repeat),
                                is_choice: false,
                            })
                        } else {
                            Ok(repeat)
                        }
                    }
                    Some('+') => {
                        self.advance(); // consume '+'
                        let repeat = Pattern::Repeat {
                            pattern: Box::new(Pattern::ApplyRule(name)),
                            kind: RepeatKind::OneOrMore,
                        };
                        if self.peek_char() == Some(':') {
                            self.advance(); // consume ':'
                            let bind_name = self.parse_ident()?;
                            Ok(Pattern::Bind {
                                name: bind_name,
                                pattern: Box::new(repeat),
                                is_choice: false,
                            })
                        } else {
                            Ok(repeat)
                        }
                    }
                    Some(':') => {
                        self.advance(); // consume ':'
                        let bind_name = self.parse_ident()?;
                        Ok(Pattern::Bind {
                            name: bind_name,
                            pattern: Box::new(Pattern::ApplyRule(name)),
                            is_choice: false,
                        })
                    }
                    _ => {
                        // Bare identifier - treat as binding (not rule reference)
                        Ok(Pattern::Bind {
                            name,
                            pattern: Box::new(Pattern::Any),
                            is_choice: false,
                        })
                    }
                }
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
                message: format!("unexpected character in constructor pattern: {:?}", c),
            }),
            None => Err(Error::UnexpectedEof),
        }
    }

    /// Parse a value pattern (used inside map/list patterns).
    /// Follows OMeta semantics: pattern[:binding] where pattern can be a rule, literal, or _.
    fn parse_value_pattern(&mut self) -> Result<Pattern> {
        self.skip_whitespace();

        // First, parse the pattern (rule, literal, wildcard, etc.)
        let pattern = match self.peek_char() {
            Some('_') => {
                self.advance();
                // Check if next char is alphanumeric (identifier like _foo)
                if self
                    .peek_char()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    // It's an identifier starting with _, continue parsing
                    let mut name = String::from("_");
                    while let Some(c) = self.peek_char() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let rule_pattern = Pattern::ApplyRule(SmolStr::new(&name));
                    // Check for * or + suffix (repetition)
                    match self.peek_char() {
                        Some('*') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(rule_pattern),
                                kind: RepeatKind::ZeroOrMore,
                            }
                        }
                        Some('+') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(rule_pattern),
                                kind: RepeatKind::OneOrMore,
                            }
                        }
                        _ => rule_pattern,
                    }
                } else {
                    // Just `_` - wildcard/any pattern, check for * or + suffix
                    match self.peek_char() {
                        Some('*') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(Pattern::Any),
                                kind: RepeatKind::ZeroOrMore,
                            }
                        }
                        Some('+') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(Pattern::Any),
                                kind: RepeatKind::OneOrMore,
                            }
                        }
                        _ => Pattern::Any,
                    }
                }
            }
            Some(':') => {
                // Could be :Tag(...) constructor or :symbol literal
                self.advance();
                let name = self.parse_symbol_name()?;
                self.skip_whitespace();
                if self.peek_char() == Some('(') {
                    // Tag match: :Tag(children...)
                    self.advance(); // consume '('
                    self.skip_whitespace();
                    let mut patterns = Vec::new();
                    if self.peek_char() != Some(')') {
                        patterns.push(self.parse_tag_child_pattern()?);
                        self.skip_whitespace();
                        while self.peek_char() == Some(',') {
                            self.advance();
                            self.skip_whitespace();
                            if self.peek_char() == Some(')') {
                                break;
                            }
                            patterns.push(self.parse_tag_child_pattern()?);
                            self.skip_whitespace();
                        }
                    }
                    self.expect_char(')')?;
                    Pattern::TagMatch(name, patterns)
                } else {
                    // Symbol literal :foo
                    Pattern::SymbolMatch(name)
                }
            }
            Some('[') => {
                // For value patterns (inside map/list), we only support list patterns, not char classes
                // Char classes are only supported in top-level grammar patterns
                self.advance();
                self.parse_list_pattern()?
            }
            Some('%') => self.parse_map_pattern()?,
            Some(c) if c.is_alphabetic() || c == '_' => {
                // Bare identifier - check for keywords first, then treat as rule reference
                let name = self.parse_ident()?;

                // Handle boolean literals
                if name.as_str() == "true" {
                    Pattern::MatchValue(Value::Bool(true))
                } else if name.as_str() == "false" {
                    Pattern::MatchValue(Value::Bool(false))
                } else if name.as_str() == "null" {
                    Pattern::MatchValue(Value::Null)
                } else {
                    let rule_pattern = Pattern::ApplyRule(name);
                    // Check for * or + suffix (repetition)
                    match self.peek_char() {
                        Some('*') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(rule_pattern),
                                kind: RepeatKind::ZeroOrMore,
                            }
                        }
                        Some('+') => {
                            self.advance();
                            Pattern::Repeat {
                                pattern: Box::new(rule_pattern),
                                kind: RepeatKind::OneOrMore,
                            }
                        }
                        _ => rule_pattern,
                    }
                }
            }
            Some('"') => {
                let s = self.parse_string()?;
                Pattern::MatchValue(Value::String(s.into()))
            }
            Some('\'') => {
                let s = self.parse_char_literal()?;
                let mut str_buf = String::new();
                str_buf.push(s);
                Pattern::MatchValue(Value::String(str_buf.into()))
            }
            Some(c) if c.is_ascii_digit() || c == '-' => {
                // Parse a number literal
                let num_str = self.parse_number_literal()?;
                if num_str.contains('.') {
                    let f: f64 = num_str.parse().map_err(|_| Error::Parser {
                        token: self.pos,
                        message: format!("invalid float literal: {}", num_str),
                    })?;
                    Pattern::MatchValue(Value::Float(f))
                } else {
                    let i: i64 = num_str.parse().map_err(|_| Error::Parser {
                        token: self.pos,
                        message: format!("invalid int literal: {}", num_str),
                    })?;
                    Pattern::MatchValue(Value::Int(i))
                }
            }
            Some(c) => {
                return Err(Error::Parser {
                    token: self.pos,
                    message: format!("unexpected character in value pattern: {:?}", c),
                });
            }
            None => {
                return Err(Error::UnexpectedEof);
            }
        };

        // Check for optional :binding suffix
        self.skip_whitespace();
        if self.peek_char() == Some(':') {
            self.advance();
            self.skip_whitespace();
            let binding_name = self.parse_ident()?;
            Ok(Pattern::Bind {
                name: binding_name,
                pattern: Box::new(pattern),
                is_choice: false,
            })
        } else {
            Ok(pattern)
        }
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
                '|' if brace_depth == 0 && paren_depth == 0 && !self.peek_str("|>") => break, // Choice separator
                ';' if brace_depth == 0 && paren_depth == 0 => break, // Rule terminator
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
        let mut bracket_depth = 0;
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
                '[' => bracket_depth += 1,
                ']' if bracket_depth > 0 => bracket_depth -= 1,
                ';' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => break, // Next case
                ',' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => break, // Next case (comma separator)
                '\n' if brace_depth == 0 && paren_depth == 0 => {
                    // Check if the next non-whitespace char starts a new pattern
                    // Patterns can start with: :Symbol, _, [, %, ident, ', "
                    let remaining = &self.source[self.pos + 1..];
                    let trimmed = remaining.trim_start_matches(|c: char| c == ' ' || c == '\t');
                    if let Some(first) = trimmed.chars().next() {
                        // Break if we see a pattern-starting character at start of line
                        if first == ':'
                            || first == '_'
                            || first == '['
                            || first == '%'
                            || first.is_alphabetic()
                        {
                            break;
                        }
                    }
                }
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

    /// Parse a symbol name (after the colon).
    /// Supports both identifier-style (:foo) and operator-style (:+, :==, etc.)
    fn parse_symbol_name(&mut self) -> Result<SmolStr> {
        let start = self.pos;

        // First check if it's an operator symbol
        if let Some(c) = self.peek_char() {
            if matches!(
                c,
                '+' | '-' | '*' | '/' | '%' | '<' | '>' | '=' | '!' | '|' | '&'
            ) {
                // Parse operator characters
                while let Some(c) = self.peek_char() {
                    if matches!(
                        c,
                        '+' | '-' | '*' | '/' | '%' | '<' | '>' | '=' | '!' | '|' | '&'
                    ) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.pos > start {
                    return Ok(SmolStr::new(&self.source[start..self.pos]));
                }
            }
        }

        // Otherwise parse as identifier
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
                message: "expected symbol name".to_string(),
            });
        }

        Ok(SmolStr::new(&self.source[start..self.pos]))
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '#' {
                // Skip # comment to end of line
                while let Some(c) = self.peek_char() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else if self.peek_str("--") {
                // Skip -- comment to end of line (FMPL style)
                while let Some(c) = self.peek_char() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else if self.peek_str("//") {
                // Skip // comment to end of line (C style)
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

    fn peek_keyword(&self, s: &str) -> bool {
        if !self.peek_str(s) {
            return false;
        }
        // Check that after the keyword is either EOF or non-identifier character
        let next_pos = self.pos + s.len();
        if next_pos >= self.source.len() {
            return true;
        }
        let next_char = self.source[next_pos..].chars().next().unwrap();
        !next_char.is_alphanumeric() && next_char != '_'
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
    }

    #[test]
    fn test_comma_rule_separator() {
        // Comma separates named rules, not alternatives within a rule
        let src = r#"{ main = "hi" => 1, other = "bye" => 2 }"#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse_anonymous().unwrap();

        assert_eq!(grammar.name.as_str(), "<anonymous>");
        assert!(grammar.rules.contains_key("main"));
        assert!(grammar.rules.contains_key("other"));
    }

    #[test]
    fn test_pipe_alternatives() {
        let src = r#"{ main = "hi" => 1 | "bye" => 2 }"#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse_anonymous().unwrap();

        assert_eq!(grammar.name.as_str(), "<anonymous>");
        assert!(grammar.rules.contains_key("main"));
    }

    #[test]
    fn test_match_block_comma() {
        let src = r#"{ "hi" => 1, "bye" => 2 }"#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse_match_block().unwrap();

        assert_eq!(grammar.name.as_str(), "<match>");
        assert!(grammar.rules.contains_key("main"));
    }

    #[test]
    fn test_match_block_pipe() {
        // Note: match blocks don't use |, they use implicit choice
        // So we test semicolon separator instead
        let src = r#"{ "hi" => 1; "bye" => 2 }"#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse_match_block().unwrap();

        assert_eq!(grammar.name.as_str(), "<match>");
        assert!(grammar.rules.contains_key("main"));
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
            Pattern::Char(CharPattern::Class(ranges)) => assert_eq!(ranges.len(), 3),
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

        // Actions are now embedded in Pattern::Action, not in Rule.action
        match &rule.pattern {
            Pattern::Action { pattern: _, action } => match action {
                Expr::GrammarLiteral(inner) => {
                    assert!(inner.rules.contains_key("inner"));
                }
                _ => panic!("expected grammar literal action, got {:?}", action),
            },
            _ => panic!(
                "expected Pattern::Action with grammar literal, got {:?}",
                rule.pattern
            ),
        }
    }

    #[test]
    fn test_float_rule_with_special_char_class() {
        // Test a rule similar to the one in fmpl_grammar.fmpl that was failing
        let src = r#"
            grammar test {
                digit = [0-9];
                _ = [ ]*;
                float = digit+ "." digit* ([eE] [+-]? digit+)? _
                      | digit+ [eE] [+-]? digit+ _
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();

        assert!(grammar.rules.contains_key("digit"));
        assert!(grammar.rules.contains_key("float"));
    }

    #[test]
    fn test_parse_fmpl_grammar_file() {
        let src = include_str!("../../tests/fmpl/fmpl_grammar.fmpl");
        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();

        assert_eq!(grammar.name.as_str(), "fmpl");
        // Verify key rules exist
        assert!(grammar.rules.contains_key("space"));
        assert!(grammar.rules.contains_key("alpha"));
        assert!(grammar.rules.contains_key("digit"));
        assert!(grammar.rules.contains_key("ident"));
        assert!(grammar.rules.contains_key("expr"));
        assert!(grammar.rules.contains_key("stmt"));
        assert!(grammar.rules.contains_key("code"));
        // Grammar should have many rules
        assert!(
            grammar.rules.len() > 30,
            "expected 30+ rules, got {}",
            grammar.rules.len()
        );
    }

    #[test]
    fn test_parse_star_quantifier_in_tag_child() {
        // expr*:args inside a TagMatch should parse as Bind("args", Repeat(ApplyRule("expr"), ZeroOrMore))
        let src = r#"
            grammar test::star {
                expr = :Tagged(tag, expr*:args) => :MakeTagged(tag, args)
                     | :Call(expr:func, expr*:a) => :Call(func, a)
                     | :List(expr*:items) => :MakeList(items)
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        assert!(
            grammar.rules.contains_key("expr"),
            "grammar should have 'expr' rule"
        );
    }

    #[test]
    fn test_parse_rule_binding_in_tag_child() {
        // expr:l inside a TagMatch should parse as Bind("l", ApplyRule("expr"))
        let src = r#"
            grammar test::bind {
                expr = :Binary(:+, expr:l, expr:r) => :Add(l, r)
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        assert!(
            grammar.rules.contains_key("expr"),
            "grammar should have 'expr' rule"
        );
    }

    #[test]
    fn test_parse_tag_in_list_pattern() {
        // [:Binding(name, expr:value)] should parse a TagMatch inside a list pattern
        let src = r#"
            grammar test::tag_in_list {
                expr = :Let([:Binding(name, expr:value)], expr:body) => :Let(name, value, body)
            }
        "#;

        let mut parser = GrammarParser::new(src);
        let grammar = parser.parse().unwrap();
        assert!(
            grammar.rules.contains_key("expr"),
            "grammar should have 'expr' rule"
        );
    }
}
