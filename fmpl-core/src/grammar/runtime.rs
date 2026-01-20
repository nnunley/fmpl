//! PEG runtime engine with packrat memoization.
//!
//! Executes grammar patterns against input, producing FMPL values.
//! Uses packrat parsing for linear-time performance on left-recursive grammars.
//!
//! Supports three input modes:
//! - Text: Character-by-character string parsing
//! - Binary: Byte stream parsing for protocols/file formats
//! - Values: Object stream parsing for AST transformation

use super::{Grammar, GrammarRegistry, Input, ParseResult, Pattern, Rule};
use crate::error::{Error, Result};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Key for memoization cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MemoKey {
    rule_name: SmolStr,
    position: usize,
}

/// Cached parse result.
#[derive(Debug, Clone)]
enum MemoEntry {
    /// Parsing in progress (for left recursion detection).
    InProgress,
    /// Completed with result.
    Done(ParseResult),
}

/// Action evaluator callback type.
/// Takes the action expression and bindings, returns the evaluated result.
pub type ActionEvaluator<'e> =
    Box<dyn FnMut(&crate::ast::Expr, &HashMap<SmolStr, Value>) -> Result<Value> + 'e>;

/// PEG parser runtime.
pub struct PegRuntime<'a, 'e> {
    input: Input,
    registry: &'a GrammarRegistry,
    grammar: Arc<Grammar>,
    action_evaluator: Option<ActionEvaluator<'e>>,
    memo: HashMap<MemoKey, MemoEntry>,
    bindings: HashMap<SmolStr, Value>,
}

impl<'a, 'e> PegRuntime<'a, 'e> {
    /// Create a new runtime for parsing text with a specific grammar.
    pub fn new(input: &str, registry: &'a GrammarRegistry, grammar: Arc<Grammar>) -> Self {
        Self {
            input: Input::text(input),
            registry,
            grammar,
            action_evaluator: None,
            memo: HashMap::new(),
            bindings: HashMap::new(),
        }
    }

    /// Create a new runtime for parsing binary data.
    pub fn new_binary(
        input: Vec<u8>,
        registry: &'a GrammarRegistry,
        grammar: Arc<Grammar>,
    ) -> Self {
        Self {
            input: Input::binary(input),
            registry,
            grammar,
            action_evaluator: None,
            memo: HashMap::new(),
            bindings: HashMap::new(),
        }
    }

    /// Create a new runtime for parsing value streams (tree parsing).
    pub fn new_values(
        input: Vec<Value>,
        registry: &'a GrammarRegistry,
        grammar: Arc<Grammar>,
    ) -> Self {
        Self {
            input: Input::values(input),
            registry,
            grammar,
            action_evaluator: None,
            memo: HashMap::new(),
            bindings: HashMap::new(),
        }
    }

    /// Set an action evaluator callback for semantic actions.
    pub fn with_action_evaluator(mut self, evaluator: ActionEvaluator<'e>) -> Self {
        self.action_evaluator = Some(evaluator);
        self
    }

    /// Parse input starting at position 0 with the given rule.
    pub fn parse(&mut self, rule_name: &str) -> Result<ParseResult> {
        self.parse_at(rule_name, 0)
    }

    /// Parse input starting at the given position with the given rule.
    pub fn parse_at(&mut self, rule_name: &str, pos: usize) -> Result<ParseResult> {
        self.bindings.clear();
        self.apply_rule(&SmolStr::new(rule_name), pos)
    }

    /// Apply a named rule at a position.
    fn apply_rule(&mut self, rule_name: &SmolStr, pos: usize) -> Result<ParseResult> {
        let key = MemoKey {
            rule_name: rule_name.clone(),
            position: pos,
        };

        // Check memo cache
        if let Some(entry) = self.memo.get(&key) {
            return match entry {
                MemoEntry::InProgress => {
                    // Left recursion detected - fail this branch
                    Ok(ParseResult::Failure)
                }
                MemoEntry::Done(result) => Ok(result.clone()),
            };
        }

        // Mark as in progress for left recursion detection
        self.memo.insert(key.clone(), MemoEntry::InProgress);

        // Find the rule
        let rule = self.find_rule(rule_name)?;
        let result = self.match_pattern(&rule.pattern, pos)?;

        // Handle semantic action if present
        let result = if let ParseResult::Success(matched, end_pos) = &result {
            if let Some(action) = &rule.action {
                // Store the matched value for action evaluation
                let action_result = self.evaluate_action(action, matched.clone())?;
                ParseResult::Success(action_result, *end_pos)
            } else {
                result
            }
        } else {
            result
        };

        // Update memo with final result
        self.memo.insert(key, MemoEntry::Done(result.clone()));
        Ok(result)
    }

    /// Find a rule by name, checking current grammar and parents.
    fn find_rule(&self, name: &SmolStr) -> Result<Rule> {
        // Check current grammar
        if let Some(rule) = self.grammar.rules.get(name) {
            return Ok(rule.clone());
        }

        // Check direct parent_grammar chain (for first-class grammars)
        let mut parent_grammar = self.grammar.parent_grammar.clone();
        while let Some(pg) = parent_grammar {
            if let Some(rule) = pg.rules.get(name) {
                return Ok(rule.clone());
            }
            parent_grammar = pg.parent_grammar.clone();
        }

        // Check named parent chain via registry (for registered grammars)
        let mut parent_name = self.grammar.parent.clone();
        while let Some(pname) = parent_name {
            if let Some(parent) = self.registry.get(&pname) {
                if let Some(rule) = parent.rules.get(name) {
                    return Ok(rule.clone());
                }
                parent_name = parent.parent.clone();
            } else {
                break;
            }
        }

        Err(Error::Runtime(format!(
            "undefined rule: {} in grammar {}",
            name, self.grammar.name
        )))
    }

    /// Find a rule from parent grammar (for super calls).
    fn find_parent_rule(&self, name: &SmolStr) -> Result<Rule> {
        // First check direct parent_grammar
        if let Some(pg) = &self.grammar.parent_grammar {
            return pg.rules.get(name).cloned().ok_or_else(|| {
                Error::Runtime(format!(
                    "rule {} not found in parent grammar {}",
                    name, pg.name
                ))
            });
        }

        // Fall back to named parent via registry
        let parent_name = self.grammar.parent.as_ref().ok_or_else(|| {
            Error::Runtime(format!(
                "super call in grammar {} which has no parent",
                self.grammar.name
            ))
        })?;

        let parent = self
            .registry
            .get(parent_name)
            .ok_or_else(|| Error::Runtime(format!("parent grammar not found: {}", parent_name)))?;

        parent.rules.get(name).cloned().ok_or_else(|| {
            Error::Runtime(format!(
                "rule {} not found in parent grammar {}",
                name, parent_name
            ))
        })
    }

    /// Match a pattern at a position.
    fn match_pattern(&mut self, pattern: &Pattern, pos: usize) -> Result<ParseResult> {
        match pattern {
            Pattern::Empty => Ok(ParseResult::Success(Value::Null, pos)),

            Pattern::Any => match &self.input {
                Input::Text(_) => {
                    if let Some(c) = self.char_at(pos) {
                        Ok(ParseResult::Success(
                            Value::String(SmolStr::new(c.to_string())),
                            pos + c.len_utf8(),
                        ))
                    } else {
                        Ok(ParseResult::Failure)
                    }
                }
                Input::Binary(_) => {
                    if let Some(b) = self.byte_at(pos) {
                        Ok(ParseResult::Success(Value::Int(b as i64), pos + 1))
                    } else {
                        Ok(ParseResult::Failure)
                    }
                }
                Input::Values(_) => {
                    if let Some(v) = self.value_at(pos) {
                        Ok(ParseResult::Success(v.clone(), pos + 1))
                    } else {
                        Ok(ParseResult::Failure)
                    }
                }
            },

            Pattern::Char(expected) => {
                if self.char_at(pos) == Some(*expected) {
                    Ok(ParseResult::Success(
                        Value::String(SmolStr::new(expected.to_string())),
                        pos + expected.len_utf8(),
                    ))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Literal(s) => {
                if let Some(text) = self.text_from(pos)
                    && text.starts_with(s.as_str())
                {
                    return Ok(ParseResult::Success(
                        Value::String(s.clone()),
                        pos + s.len(),
                    ));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::CharClass(ranges) => {
                if let Some(c) = self.char_at(pos)
                    && ranges.iter().any(|r| r.matches(c))
                {
                    return Ok(ParseResult::Success(
                        Value::String(SmolStr::new(c.to_string())),
                        pos + c.len_utf8(),
                    ));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::NegCharClass(ranges) => {
                if let Some(c) = self.char_at(pos)
                    && !ranges.iter().any(|r| r.matches(c))
                {
                    return Ok(ParseResult::Success(
                        Value::String(SmolStr::new(c.to_string())),
                        pos + c.len_utf8(),
                    ));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::Rule(name) => self.apply_rule(name, pos),

            Pattern::Super(name) => {
                let rule = self.find_parent_rule(name)?;
                self.match_pattern(&rule.pattern, pos)
            }

            Pattern::Seq(patterns) => {
                let mut current_pos = pos;
                let mut values = Vec::new();

                for p in patterns {
                    match self.match_pattern(p, current_pos)? {
                        ParseResult::Success(v, new_pos) => {
                            values.push(v);
                            current_pos = new_pos;
                        }
                        ParseResult::Failure => return Ok(ParseResult::Failure),
                    }
                }

                // Return concatenated string or list of values
                let result = if values.iter().all(|v| matches!(v, Value::String(_))) {
                    let s: String = values
                        .iter()
                        .filter_map(|v| {
                            if let Value::String(s) = v {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Value::String(SmolStr::new(s))
                } else {
                    Value::List(Arc::new(values))
                };

                Ok(ParseResult::Success(result, current_pos))
            }

            Pattern::Choice(alternatives) => {
                for alt in alternatives {
                    match self.match_pattern(alt, pos)? {
                        ParseResult::Success(v, new_pos) => {
                            return Ok(ParseResult::Success(v, new_pos));
                        }
                        ParseResult::Failure => continue,
                    }
                }
                Ok(ParseResult::Failure)
            }

            Pattern::Star(inner) => {
                let mut current_pos = pos;
                let mut values = Vec::new();

                loop {
                    match self.match_pattern(inner, current_pos)? {
                        ParseResult::Success(v, new_pos) if new_pos > current_pos => {
                            values.push(v);
                            current_pos = new_pos;
                        }
                        _ => break,
                    }
                }

                // Return concatenated string or list
                let result = if values.iter().all(|v| matches!(v, Value::String(_))) {
                    let s: String = values
                        .iter()
                        .filter_map(|v| {
                            if let Value::String(s) = v {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Value::String(SmolStr::new(s))
                } else {
                    Value::List(Arc::new(values))
                };

                Ok(ParseResult::Success(result, current_pos))
            }

            Pattern::Plus(inner) => {
                // Must match at least once
                let first = self.match_pattern(inner, pos)?;
                match first {
                    ParseResult::Failure => Ok(ParseResult::Failure),
                    ParseResult::Success(v, mut current_pos) => {
                        let mut values = vec![v];

                        loop {
                            match self.match_pattern(inner, current_pos)? {
                                ParseResult::Success(v, new_pos) if new_pos > current_pos => {
                                    values.push(v);
                                    current_pos = new_pos;
                                }
                                _ => break,
                            }
                        }

                        let result = if values.iter().all(|v| matches!(v, Value::String(_))) {
                            let s: String = values
                                .iter()
                                .filter_map(|v| {
                                    if let Value::String(s) = v {
                                        Some(s.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            Value::String(SmolStr::new(s))
                        } else {
                            Value::List(Arc::new(values))
                        };

                        Ok(ParseResult::Success(result, current_pos))
                    }
                }
            }

            Pattern::Optional(inner) => match self.match_pattern(inner, pos)? {
                ParseResult::Success(v, new_pos) => Ok(ParseResult::Success(v, new_pos)),
                ParseResult::Failure => Ok(ParseResult::Success(Value::Null, pos)),
            },

            Pattern::Lookahead(inner) => {
                // Match but don't consume
                match self.match_pattern(inner, pos)? {
                    ParseResult::Success(_, _) => Ok(ParseResult::Success(Value::Null, pos)),
                    ParseResult::Failure => Ok(ParseResult::Failure),
                }
            }

            Pattern::Not(inner) => {
                // Succeed if inner fails, don't consume
                match self.match_pattern(inner, pos)? {
                    ParseResult::Success(_, _) => Ok(ParseResult::Failure),
                    ParseResult::Failure => Ok(ParseResult::Success(Value::Null, pos)),
                }
            }

            Pattern::Bind(inner, name) => match self.match_pattern(inner, pos)? {
                ParseResult::Success(v, new_pos) => {
                    self.bindings.insert(name.clone(), v.clone());
                    Ok(ParseResult::Success(v, new_pos))
                }
                ParseResult::Failure => Ok(ParseResult::Failure),
            },

            Pattern::Predicate(expr) => {
                // Evaluate expression, succeed if truthy
                let result = self.evaluate_predicate(expr)?;
                if result.is_truthy() {
                    Ok(ParseResult::Success(Value::Null, pos))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Action(inner, action) => match self.match_pattern(inner, pos)? {
                ParseResult::Success(matched, new_pos) => {
                    let result = self.evaluate_action(action, matched)?;
                    Ok(ParseResult::Success(result, new_pos))
                }
                ParseResult::Failure => Ok(ParseResult::Failure),
            },

            // === Binary Patterns ===
            Pattern::Byte(expected) => {
                if self.byte_at(pos) == Some(*expected) {
                    Ok(ParseResult::Success(Value::Int(*expected as i64), pos + 1))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::ByteRange(lo, hi) => {
                if let Some(b) = self.byte_at(pos)
                    && b >= *lo
                    && b <= *hi
                {
                    return Ok(ParseResult::Success(Value::Int(b as i64), pos + 1));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::Bytes(n) => {
                if let Some(bytes) = self.bytes_at(pos, *n) {
                    let values: Vec<Value> = bytes.iter().map(|b| Value::Int(*b as i64)).collect();
                    Ok(ParseResult::Success(Value::List(Arc::new(values)), pos + n))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::UInt8 => {
                if let Some(b) = self.byte_at(pos) {
                    Ok(ParseResult::Success(Value::Int(b as i64), pos + 1))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::UInt16BE => {
                if let Some(bytes) = self.bytes_at(pos, 2) {
                    let value = u16::from_be_bytes([bytes[0], bytes[1]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 2))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::UInt16LE => {
                if let Some(bytes) = self.bytes_at(pos, 2) {
                    let value = u16::from_le_bytes([bytes[0], bytes[1]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 2))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::UInt32BE => {
                if let Some(bytes) = self.bytes_at(pos, 4) {
                    let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 4))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::UInt32LE => {
                if let Some(bytes) = self.bytes_at(pos, 4) {
                    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 4))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Int8 => {
                if let Some(b) = self.byte_at(pos) {
                    Ok(ParseResult::Success(Value::Int(b as i8 as i64), pos + 1))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Int16BE => {
                if let Some(bytes) = self.bytes_at(pos, 2) {
                    let value = i16::from_be_bytes([bytes[0], bytes[1]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 2))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Int16LE => {
                if let Some(bytes) = self.bytes_at(pos, 2) {
                    let value = i16::from_le_bytes([bytes[0], bytes[1]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 2))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Int32BE => {
                if let Some(bytes) = self.bytes_at(pos, 4) {
                    let value = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 4))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            Pattern::Int32LE => {
                if let Some(bytes) = self.bytes_at(pos, 4) {
                    let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    Ok(ParseResult::Success(Value::Int(value as i64), pos + 4))
                } else {
                    Ok(ParseResult::Failure)
                }
            }

            // === Object/Value Patterns ===
            Pattern::MatchValue(expected) => {
                if let Some(v) = self.value_at(pos)
                    && v == expected
                {
                    return Ok(ParseResult::Success(v.clone(), pos + 1));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::MatchType(type_name) => {
                if let Some(v) = self.value_at(pos) {
                    let matches = match type_name.as_str() {
                        "null" => matches!(v, Value::Null),
                        "bool" => matches!(v, Value::Bool(_)),
                        "int" => matches!(v, Value::Int(_)),
                        "float" => matches!(v, Value::Float(_)),
                        "string" => matches!(v, Value::String(_)),
                        "symbol" => matches!(v, Value::Symbol(_)),
                        "list" => matches!(v, Value::List(_)),
                        "map" => matches!(v, Value::Map(_)),
                        "object" => matches!(v, Value::Object(_)),
                        _ => false,
                    };
                    if matches {
                        return Ok(ParseResult::Success(v.clone(), pos + 1));
                    }
                }
                Ok(ParseResult::Failure)
            }

            Pattern::SymbolMatch(name) => {
                if let Some(Value::Symbol(sym)) = self.value_at(pos)
                    && sym.as_str() == name.as_str()
                {
                    return Ok(ParseResult::Success(Value::Symbol(sym.clone()), pos + 1));
                }
                Ok(ParseResult::Failure)
            }

            Pattern::ListMatch(patterns, rest) => {
                // Clone the list items to avoid borrow checker issues
                let items = match self.value_at(pos) {
                    Some(Value::List(items)) => (*items).clone(),
                    _ => return Ok(ParseResult::Failure),
                };

                // Match patterns against list elements
                if patterns.len() > items.len() && rest.is_none() {
                    return Ok(ParseResult::Failure);
                }

                let mut matched_values = Vec::new();
                let mut item_idx = 0;

                for pat in patterns {
                    if item_idx >= items.len() {
                        return Ok(ParseResult::Failure);
                    }
                    // Create a sub-runtime for matching the element
                    let sub_input = Input::values(vec![items[item_idx].clone()]);
                    let result = self.match_value_pattern(pat, &sub_input, 0)?;
                    match result {
                        ParseResult::Success(v, _) => {
                            matched_values.push(v);
                            item_idx += 1;
                        }
                        ParseResult::Failure => return Ok(ParseResult::Failure),
                    }
                }

                // Handle rest pattern
                if let Some(rest_pattern) = rest {
                    let remaining: Vec<Value> = items[item_idx..].to_vec();
                    let sub_input = Input::values(remaining);
                    let result = self.match_value_pattern(rest_pattern, &sub_input, 0)?;
                    match result {
                        ParseResult::Success(v, _) => matched_values.push(v),
                        ParseResult::Failure => return Ok(ParseResult::Failure),
                    }
                } else if item_idx < items.len() {
                    // Not all items matched and no rest pattern
                    return Ok(ParseResult::Failure);
                }

                Ok(ParseResult::Success(
                    Value::List(Arc::new(matched_values)),
                    pos + 1,
                ))
            }

            Pattern::MapMatch(entries) => {
                // Clone the map to avoid borrow checker issues
                let map = match self.value_at(pos) {
                    Some(Value::Map(map)) => (*map).clone(),
                    _ => return Ok(ParseResult::Failure),
                };

                let mut matched_values = Vec::new();

                for (key, pat) in entries {
                    if let Some(value) = map.get(key.as_str()) {
                        let sub_input = Input::values(vec![value.clone()]);
                        let result = self.match_value_pattern(pat, &sub_input, 0)?;
                        match result {
                            ParseResult::Success(v, _) => matched_values.push(v),
                            ParseResult::Failure => return Ok(ParseResult::Failure),
                        }
                    } else {
                        return Ok(ParseResult::Failure);
                    }
                }

                Ok(ParseResult::Success(
                    Value::List(Arc::new(matched_values)),
                    pos + 1,
                ))
            }

            Pattern::Apply(inner) => {
                // Clone value to avoid borrow checker issues
                let value = match self.value_at(pos) {
                    Some(v) => v.clone(),
                    None => return Ok(ParseResult::Failure),
                };

                // Apply pattern to current value (descend into it)
                let sub_input = Input::from_value(value);
                let sub_len = sub_input.len();
                let result = self.match_value_pattern(inner, &sub_input, 0)?;
                match result {
                    ParseResult::Success(v, end_pos) => {
                        // Check if entire sub-input was consumed
                        if end_pos == sub_len {
                            Ok(ParseResult::Success(v, pos + 1))
                        } else {
                            Ok(ParseResult::Failure)
                        }
                    }
                    ParseResult::Failure => Ok(ParseResult::Failure),
                }
            }

            Pattern::End => {
                if self.is_at_end(pos) {
                    Ok(ParseResult::Success(Value::Null, pos))
                } else {
                    Ok(ParseResult::Failure)
                }
            }
        }
    }

    // === Input access helpers ===

    /// Get character at position (for text input).
    fn char_at(&self, pos: usize) -> Option<char> {
        match &self.input {
            Input::Text(s) => s[pos..].chars().next(),
            _ => None,
        }
    }

    /// Get byte at position (for binary input).
    fn byte_at(&self, pos: usize) -> Option<u8> {
        match &self.input {
            Input::Binary(bytes) => bytes.get(pos).copied(),
            Input::Text(s) => s.as_bytes().get(pos).copied(),
            _ => None,
        }
    }

    /// Get value at position (for value stream input).
    fn value_at(&self, pos: usize) -> Option<&Value> {
        match &self.input {
            Input::Values(values) => values.get(pos),
            _ => None,
        }
    }

    /// Check if position is at end of input.
    fn is_at_end(&self, pos: usize) -> bool {
        pos >= self.input.len()
    }

    /// Get text slice starting at position.
    fn text_from(&self, pos: usize) -> Option<&str> {
        match &self.input {
            Input::Text(s) => s.get(pos..),
            _ => None,
        }
    }

    /// Read n bytes starting at position.
    fn bytes_at(&self, pos: usize, n: usize) -> Option<&[u8]> {
        match &self.input {
            Input::Binary(bytes) => bytes.get(pos..pos + n),
            Input::Text(s) => s.as_bytes().get(pos..pos + n),
            _ => None,
        }
    }

    /// Match a pattern against a sub-input (for nested value matching).
    fn match_value_pattern(
        &mut self,
        pattern: &Pattern,
        sub_input: &Input,
        pos: usize,
    ) -> Result<ParseResult> {
        // Temporarily swap input
        let original = std::mem::replace(&mut self.input, sub_input.clone());
        let result = self.match_pattern(pattern, pos);
        self.input = original;
        result
    }

    /// Evaluate a semantic predicate expression.
    fn evaluate_predicate(&mut self, expr: &crate::ast::Expr) -> Result<Value> {
        // If we have an action evaluator, use it for predicates too
        if let Some(ref mut evaluator) = self.action_evaluator {
            return evaluator(expr, &self.bindings);
        }

        // Fallback: always succeed (for testing without VM)
        Ok(Value::Bool(true))
    }

    /// Evaluate a semantic action expression.
    fn evaluate_action(&mut self, action: &crate::ast::Expr, matched: Value) -> Result<Value> {
        // If we have an action evaluator, use it
        if let Some(ref mut evaluator) = self.action_evaluator {
            return evaluator(action, &self.bindings);
        }

        // Fallback: return matched value (or last binding if available)
        if !self.bindings.is_empty()
            && let Some((_, v)) = self.bindings.iter().last()
        {
            return Ok(v.clone());
        }
        Ok(matched)
    }

    /// Get current bindings (for use in actions).
    pub fn bindings(&self) -> &HashMap<SmolStr, Value> {
        &self.bindings
    }
}

/// Convenience function to parse a string with a grammar rule.
pub fn parse(
    input: &str,
    registry: &GrammarRegistry,
    grammar_name: &str,
    rule_name: &str,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;

    let mut runtime = PegRuntime::new(input, registry, grammar);
    match runtime.parse(rule_name)? {
        ParseResult::Success(v, _) => Ok(Some(v)),
        ParseResult::Failure => Ok(None),
    }
}

/// Parse and ensure entire input is consumed.
pub fn parse_full(
    input: &str,
    registry: &GrammarRegistry,
    grammar_name: &str,
    rule_name: &str,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;

    let mut runtime = PegRuntime::new(input, registry, grammar);
    match runtime.parse(rule_name)? {
        ParseResult::Success(v, pos) if pos == input.len() => Ok(Some(v)),
        ParseResult::Success(_, _) => Ok(None), // Didn't consume all input
        ParseResult::Failure => Ok(None),
    }
}

/// Parse using a provided grammar value (for first-class grammars).
pub fn parse_full_with_grammar(
    input: &str,
    grammar: &Arc<Grammar>,
    registry: &GrammarRegistry,
    rule_name: &str,
) -> Result<Option<Value>> {
    let mut runtime = PegRuntime::new(input, registry, grammar.clone());
    match runtime.parse(rule_name)? {
        ParseResult::Success(v, pos) if pos == input.len() => Ok(Some(v)),
        ParseResult::Success(_, _) => Ok(None), // Didn't consume all input
        ParseResult::Failure => Ok(None),
    }
}

/// Apply a grammar rule to any value (polymorphic input).
/// Coerces the input based on type:
/// - String -> character stream (text parsing)
/// - List -> element stream (each element is one input)
/// - Other -> single-element stream (pattern matching)
pub fn apply_grammar_to_value(
    input: Value,
    grammar: &Arc<Grammar>,
    registry: &GrammarRegistry,
    rule_name: &str,
) -> Result<Option<Value>> {
    match input {
        Value::String(s) => {
            // Text parsing
            let mut runtime = PegRuntime::new(s.as_str(), registry, grammar.clone());
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == s.len() => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        Value::List(items) => {
            // List element stream
            let values: Vec<Value> = (*items).clone();
            let len = values.len();
            let mut runtime = PegRuntime::new_values(values, registry, grammar.clone());
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == len => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        other => {
            // Single value stream (pattern matching mode)
            let mut runtime = PegRuntime::new_values(vec![other], registry, grammar.clone());
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, 1) => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
    }
}

/// Apply a grammar rule to any value with an action evaluator.
pub fn apply_grammar_to_value_with_evaluator<'e>(
    input: Value,
    grammar: &Arc<Grammar>,
    registry: &GrammarRegistry,
    rule_name: &str,
    evaluator: ActionEvaluator<'e>,
) -> Result<Option<Value>> {
    match input {
        Value::String(s) => {
            let mut runtime = PegRuntime::new(s.as_str(), registry, grammar.clone())
                .with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == s.len() => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        Value::List(items) => {
            let values: Vec<Value> = (*items).clone();
            let len = values.len();
            let mut runtime = PegRuntime::new_values(values, registry, grammar.clone())
                .with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, pos) if pos == len => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
        other => {
            let mut runtime = PegRuntime::new_values(vec![other], registry, grammar.clone())
                .with_action_evaluator(evaluator);
            match runtime.parse(rule_name)? {
                ParseResult::Success(v, 1) => Ok(Some(v)),
                ParseResult::Success(_, _) => Ok(None),
                ParseResult::Failure => Ok(None),
            }
        }
    }
}

/// Apply a grammar rule by name with an action evaluator.
pub fn apply_grammar_by_name_with_evaluator<'e>(
    input: Value,
    grammar_name: &str,
    registry: &GrammarRegistry,
    rule_name: &str,
    evaluator: ActionEvaluator<'e>,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;
    apply_grammar_to_value_with_evaluator(input, &grammar, registry, rule_name, evaluator)
}

/// Apply a grammar rule by name to any value.
pub fn apply_grammar_by_name(
    input: Value,
    grammar_name: &str,
    registry: &GrammarRegistry,
    rule_name: &str,
) -> Result<Option<Value>> {
    let grammar = registry
        .get(grammar_name)
        .ok_or_else(|| Error::Runtime(format!("grammar not found: {}", grammar_name)))?;
    apply_grammar_to_value(input, &grammar, registry, rule_name)
}

#[cfg(test)]
mod tests {
    use super::super::CharRange;
    use super::*;

    #[test]
    fn test_parse_digit() {
        let registry = GrammarRegistry::new();
        let result = parse("5", &registry, "base::parser", "digit").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "5"));
    }

    #[test]
    fn test_parse_digits_fail() {
        let registry = GrammarRegistry::new();
        let result = parse("a", &registry, "base::parser", "digit").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_integer() {
        let registry = GrammarRegistry::new();
        let result = parse("12345", &registry, "base::parser", "integer").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "12345"));
    }

    #[test]
    fn test_parse_word() {
        let registry = GrammarRegistry::new();
        let result = parse("hello", &registry, "base::parser", "word").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "hello"));
    }

    #[test]
    fn test_parse_spaces() {
        let registry = GrammarRegistry::new();
        let result = parse("   ", &registry, "base::parser", "spaces").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "   "));
    }

    #[test]
    fn test_parse_eof() {
        let registry = GrammarRegistry::new();
        let result = parse("", &registry, "base::parser", "eof").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_custom_grammar() {
        let mut registry = GrammarRegistry::new();

        // Create a simple grammar
        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::simple"), SmolStr::new("base::parser"));

        // hex = [0-9a-fA-F]+
        grammar.add_rule(
            SmolStr::new("hex"),
            super::super::Rule::new(Pattern::Plus(Box::new(Pattern::CharClass(vec![
                CharRange::Range('0', '9'),
                CharRange::Range('a', 'f'),
                CharRange::Range('A', 'F'),
            ])))),
        );

        registry.register(grammar);

        let result = parse("deadBEEF", &registry, "test::simple", "hex").unwrap();
        assert!(matches!(result, Some(Value::String(s)) if s == "deadBEEF"));
    }

    #[test]
    fn test_sequence_pattern() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::seq"), SmolStr::new("base::parser"));

        // ident_colon = word ':' spaces word
        grammar.add_rule(
            SmolStr::new("pair"),
            super::super::Rule::new(Pattern::Seq(vec![
                Pattern::Rule(SmolStr::new("word")),
                Pattern::Char(':'),
                Pattern::Rule(SmolStr::new("spaces")),
                Pattern::Rule(SmolStr::new("word")),
            ])),
        );

        registry.register(grammar);

        let result = parse("foo: bar", &registry, "test::seq", "pair").unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_choice_pattern() {
        let mut registry = GrammarRegistry::new();

        let mut grammar = Grammar::new(SmolStr::new("test::choice"));

        // bool = "true" | "false"
        grammar.add_rule(
            SmolStr::new("bool"),
            super::super::Rule::new(Pattern::Choice(vec![
                Pattern::Literal(SmolStr::new("true")),
                Pattern::Literal(SmolStr::new("false")),
            ])),
        );

        registry.register(grammar);

        let result1 = parse("true", &registry, "test::choice", "bool").unwrap();
        assert!(matches!(result1, Some(Value::String(s)) if s == "true"));

        let result2 = parse("false", &registry, "test::choice", "bool").unwrap();
        assert!(matches!(result2, Some(Value::String(s)) if s == "false"));

        let result3 = parse("maybe", &registry, "test::choice", "bool").unwrap();
        assert!(result3.is_none());
    }

    #[test]
    fn test_lookahead() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::look"), SmolStr::new("base::parser"));

        // digit_followed_by_letter = &letter digit
        // Actually: letter_if_digit_ahead = &digit letter
        grammar.add_rule(
            SmolStr::new("check"),
            super::super::Rule::new(Pattern::Seq(vec![
                Pattern::Lookahead(Box::new(Pattern::Rule(SmolStr::new("digit")))),
                Pattern::Rule(SmolStr::new("digit")),
            ])),
        );

        registry.register(grammar);

        let result1 = parse("5", &registry, "test::look", "check").unwrap();
        assert!(result1.is_some());

        let result2 = parse("a", &registry, "test::look", "check").unwrap();
        assert!(result2.is_none());
    }

    #[test]
    fn test_not_pattern() {
        let mut registry = GrammarRegistry::new();

        let mut grammar = Grammar::new(SmolStr::new("test::not"));

        // not_a = ~'a' .
        grammar.add_rule(
            SmolStr::new("not_a"),
            super::super::Rule::new(Pattern::Seq(vec![
                Pattern::Not(Box::new(Pattern::Char('a'))),
                Pattern::Any,
            ])),
        );

        registry.register(grammar);

        let result1 = parse("b", &registry, "test::not", "not_a").unwrap();
        assert!(result1.is_some());

        let result2 = parse("a", &registry, "test::not", "not_a").unwrap();
        assert!(result2.is_none());
    }

    #[test]
    fn test_binding() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::bind"), SmolStr::new("base::parser"));

        // capture = word:w => w
        grammar.add_rule(
            SmolStr::new("capture"),
            super::super::Rule::new(Pattern::Bind(
                Box::new(Pattern::Rule(SmolStr::new("word"))),
                SmolStr::new("w"),
            )),
        );

        registry.register(grammar);

        let grammar = registry.get("test::bind").unwrap();
        let mut runtime = PegRuntime::new("hello", &registry, grammar);
        let result = runtime.parse("capture").unwrap();

        assert!(matches!(result, ParseResult::Success(Value::String(s), _) if s == "hello"));
        assert!(runtime.bindings().contains_key("w"));
    }

    #[test]
    fn test_parse_full() {
        let registry = GrammarRegistry::new();

        // Should succeed - consumes all input
        let result1 = parse_full("123", &registry, "base::parser", "integer").unwrap();
        assert!(result1.is_some());

        // Should fail - doesn't consume all input
        let result2 = parse_full("123abc", &registry, "base::parser", "integer").unwrap();
        assert!(result2.is_none());
    }

    // === Binary Parsing Tests ===

    #[test]
    fn test_binary_uint8() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0x42, 0x00], &registry, grammar);
        let result = runtime.parse("uint8").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(66), 1)));
    }

    #[test]
    fn test_binary_uint16be() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0x01, 0x02], &registry, grammar);
        let result = runtime.parse("uint16be").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(258), 2))); // 0x0102 = 258
    }

    #[test]
    fn test_binary_uint16le() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0x02, 0x01], &registry, grammar);
        let result = runtime.parse("uint16le").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(258), 2))); // 0x0102 = 258 in LE
    }

    #[test]
    fn test_binary_uint32be() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0x00, 0x00, 0x01, 0x00], &registry, grammar);
        let result = runtime.parse("uint32be").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(256), 4)));
    }

    #[test]
    fn test_binary_int8_negative() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0xFF], &registry, grammar);
        let result = runtime.parse("int8").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(-1), 1)));
    }

    #[test]
    fn test_binary_byte_pattern() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::binary"), SmolStr::new("base::binary"));

        // magic = byte(0x89) byte(0x50)
        grammar.add_rule(
            SmolStr::new("magic"),
            super::super::Rule::new(Pattern::Seq(vec![Pattern::Byte(0x89), Pattern::Byte(0x50)])),
        );

        registry.register(grammar);

        let grammar = registry.get("test::binary").unwrap();
        let mut runtime = PegRuntime::new_binary(vec![0x89, 0x50, 0x4E], &registry, grammar);
        let result = runtime.parse("magic").unwrap();
        assert!(result.is_success());

        // Should fail with wrong bytes
        let mut runtime2 = PegRuntime::new_binary(
            vec![0x00, 0x50],
            &registry,
            registry.get("test::binary").unwrap(),
        );
        let result2 = runtime2.parse("magic").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_binary_byte_range() {
        let mut registry = GrammarRegistry::new();

        let mut grammar = Grammar::new(SmolStr::new("test::byterange"));

        // printable = byte(0x20..0x7E)
        grammar.add_rule(
            SmolStr::new("printable"),
            super::super::Rule::new(Pattern::ByteRange(0x20, 0x7E)),
        );

        registry.register(grammar);

        let grammar = registry.get("test::byterange").unwrap();

        // 'A' = 0x41, should match
        let mut runtime = PegRuntime::new_binary(vec![0x41], &registry, grammar.clone());
        let result = runtime.parse("printable").unwrap();
        assert!(result.is_success());

        // 0x00 is not printable, should fail
        let mut runtime2 = PegRuntime::new_binary(vec![0x00], &registry, grammar);
        let result2 = runtime2.parse("printable").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_binary_bytes() {
        let mut registry = GrammarRegistry::new();

        let mut grammar = Grammar::new(SmolStr::new("test::bytes"));

        // four_bytes = bytes(4)
        grammar.add_rule(
            SmolStr::new("four_bytes"),
            super::super::Rule::new(Pattern::Bytes(4)),
        );

        registry.register(grammar);

        let grammar = registry.get("test::bytes").unwrap();
        let mut runtime =
            PegRuntime::new_binary(vec![0x01, 0x02, 0x03, 0x04, 0x05], &registry, grammar);
        let result = runtime.parse("four_bytes").unwrap();

        if let ParseResult::Success(Value::List(items), pos) = result {
            assert_eq!(pos, 4);
            assert_eq!(items.len(), 4);
            assert!(matches!(&items[0], Value::Int(1)));
            assert!(matches!(&items[3], Value::Int(4)));
        } else {
            panic!("expected list of bytes");
        }
    }

    // === Object/Value Parsing Tests ===

    #[test]
    fn test_tree_match_int() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();
        let mut runtime = PegRuntime::new_values(vec![Value::Int(42)], &registry, grammar);
        let result = runtime.parse("int").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Int(42), 1)));
    }

    #[test]
    fn test_tree_match_string() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();
        let mut runtime = PegRuntime::new_values(
            vec![Value::String(SmolStr::new("hello"))],
            &registry,
            grammar,
        );
        let result = runtime.parse("string").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::String(s), 1) if s == "hello"));
    }

    #[test]
    fn test_tree_match_symbol() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();
        let mut runtime =
            PegRuntime::new_values(vec![Value::Symbol(SmolStr::new("foo"))], &registry, grammar);
        let result = runtime.parse("symbol").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Symbol(s), 1) if s == "foo"));
    }

    #[test]
    fn test_tree_match_any() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();
        let mut runtime =
            PegRuntime::new_values(vec![Value::Bool(true), Value::Int(1)], &registry, grammar);
        let result = runtime.parse("any").unwrap();
        assert!(matches!(result, ParseResult::Success(Value::Bool(true), 1)));
    }

    #[test]
    fn test_tree_end_pattern() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();

        // Empty input - end should succeed
        let mut runtime = PegRuntime::new_values(vec![], &registry, grammar.clone());
        let result = runtime.parse("end").unwrap();
        assert!(result.is_success());

        // Non-empty input - end should fail at position 0
        let mut runtime2 = PegRuntime::new_values(vec![Value::Int(1)], &registry, grammar);
        let result2 = runtime2.parse("end").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_match_value() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::value"), SmolStr::new("base::tree"));

        // match_42 = 42
        grammar.add_rule(
            SmolStr::new("match_42"),
            super::super::Rule::new(Pattern::MatchValue(Value::Int(42))),
        );

        registry.register(grammar);

        let grammar = registry.get("test::value").unwrap();

        let mut runtime = PegRuntime::new_values(vec![Value::Int(42)], &registry, grammar.clone());
        let result = runtime.parse("match_42").unwrap();
        assert!(result.is_success());

        let mut runtime2 = PegRuntime::new_values(vec![Value::Int(43)], &registry, grammar);
        let result2 = runtime2.parse("match_42").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_symbol_match() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::sym"), SmolStr::new("base::tree"));

        // add_sym = :add
        grammar.add_rule(
            SmolStr::new("add_sym"),
            super::super::Rule::new(Pattern::SymbolMatch(SmolStr::new("add"))),
        );

        registry.register(grammar);

        let grammar = registry.get("test::sym").unwrap();

        let mut runtime = PegRuntime::new_values(
            vec![Value::Symbol(SmolStr::new("add"))],
            &registry,
            grammar.clone(),
        );
        let result = runtime.parse("add_sym").unwrap();
        assert!(result.is_success());

        let mut runtime2 =
            PegRuntime::new_values(vec![Value::Symbol(SmolStr::new("sub"))], &registry, grammar);
        let result2 = runtime2.parse("add_sym").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_list_match() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::list"), SmolStr::new("base::tree"));

        // pair = [int int]
        grammar.add_rule(
            SmolStr::new("pair"),
            super::super::Rule::new(Pattern::ListMatch(
                vec![
                    Pattern::MatchType(SmolStr::new("int")),
                    Pattern::MatchType(SmolStr::new("int")),
                ],
                None,
            )),
        );

        registry.register(grammar);

        let grammar = registry.get("test::list").unwrap();

        let mut runtime = PegRuntime::new_values(
            vec![Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]))],
            &registry,
            grammar.clone(),
        );
        let result = runtime.parse("pair").unwrap();
        assert!(result.is_success());

        // Wrong element type - should fail
        let mut runtime2 = PegRuntime::new_values(
            vec![Value::List(Arc::new(vec![
                Value::Int(1),
                Value::String(SmolStr::new("x")),
            ]))],
            &registry,
            grammar.clone(),
        );
        let result2 = runtime2.parse("pair").unwrap();
        assert!(result2.is_failure());

        // Wrong length - should fail
        let mut runtime3 = PegRuntime::new_values(
            vec![Value::List(Arc::new(vec![Value::Int(1)]))],
            &registry,
            grammar,
        );
        let result3 = runtime3.parse("pair").unwrap();
        assert!(result3.is_failure());
    }

    #[test]
    fn test_apply_pattern() {
        let mut registry = GrammarRegistry::new();

        let mut grammar =
            Grammar::with_parent(SmolStr::new("test::apply"), SmolStr::new("base::tree"));

        // number_in_list = ^int
        // This descends into the current value (expecting a list) and matches an int
        grammar.add_rule(
            SmolStr::new("single_int"),
            super::super::Rule::new(Pattern::Apply(Box::new(Pattern::MatchType(SmolStr::new(
                "int",
            ))))),
        );

        registry.register(grammar);

        let grammar = registry.get("test::apply").unwrap();

        // A list containing a single int
        let mut runtime = PegRuntime::new_values(
            vec![Value::List(Arc::new(vec![Value::Int(42)]))],
            &registry,
            grammar.clone(),
        );
        let result = runtime.parse("single_int").unwrap();
        assert!(result.is_success());

        // A list containing more than one element should fail (Apply consumes entire sub-input)
        let mut runtime2 = PegRuntime::new_values(
            vec![Value::List(Arc::new(vec![Value::Int(1), Value::Int(2)]))],
            &registry,
            grammar,
        );
        let result2 = runtime2.parse("single_int").unwrap();
        assert!(result2.is_failure());
    }

    #[test]
    fn test_arc_parent_rule_inheritance() {
        // Create parent grammar with a rule
        let mut parent = Grammar::new(SmolStr::new("parent"));
        parent.add_rule(
            SmolStr::new("greeting"),
            super::super::Rule::new(Pattern::Literal(SmolStr::new("hello"))),
        );
        let parent = Arc::new(parent);

        // Create child with Arc parent reference (no registry lookup needed)
        let child = Grammar::with_parent_grammar(SmolStr::new("child"), parent.clone());

        // Parse using child grammar - should inherit "greeting" rule from parent
        let registry = GrammarRegistry::new();
        let mut runtime = PegRuntime::new("hello", &registry, Arc::new(child));
        let result = runtime.parse("greeting").unwrap();

        assert!(result.is_success());
        if let ParseResult::Success(Value::String(s), _) = result {
            assert_eq!(s, "hello");
        } else {
            panic!("expected string result");
        }
    }
}
