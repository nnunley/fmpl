//! Parse state for grammar pattern matching
//!
//! Manages input position, grammar lookup, and memoization
//! for packrat parsing during pattern matching operations.

use crate::grammar::Grammar;
use crate::grammar::input::MemoEntry;
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Parse state for grammar pattern matching.
#[derive(Debug, Clone)]
pub struct ParseState {
    /// Input value being matched (if pattern matching is active).
    input_value: Option<Value>,
    /// Current position in the input (for simple index-based inputs).
    input_pos: Option<usize>,
    /// Grammar registry for rule lookup.
    grammar: Option<Arc<Grammar>>,
    /// Per-position memoization table for packrat parsing.
    /// Key: (position_index, rule_name), Value: memo entry
    memo: HashMap<(usize, SmolStr), MemoEntry>,
}

impl ParseState {
    pub fn new() -> Self {
        Self {
            input_value: None,
            input_pos: Some(0),
            grammar: None,
            memo: HashMap::new(),
        }
    }

    /// Set the input value for pattern matching.
    pub fn set_input(&mut self, value: Value) {
        self.input_value = Some(value);
        self.input_pos = Some(0);
    }

    /// Set the grammar for rule lookup.
    pub fn set_grammar(&mut self, grammar: Arc<Grammar>) {
        self.grammar = Some(grammar);
    }

    /// Get the current grammar (if set).
    pub fn grammar(&self) -> Option<&Arc<Grammar>> {
        self.grammar.as_ref()
    }

    /// Get the current input value.
    pub fn input(&self) -> Option<&Value> {
        self.input_value.as_ref()
    }

    /// Get the current input position.
    pub fn position(&self) -> usize {
        self.input_pos.unwrap_or(0)
    }

    /// Advance the input position by n.
    pub fn advance(&mut self, n: usize) {
        if let Some(pos) = self.input_pos.as_mut() {
            *pos += n;
        }
    }

    /// Check if at end of input.
    pub fn is_at_end(&self) -> bool {
        if let Some(ref value) = self.input_value {
            match value {
                Value::String(s) => self.input_pos.unwrap_or(0) >= s.len(),
                Value::List(items) => self.input_pos.unwrap_or(0) >= items.len(),
                _ => self.input_pos.unwrap_or(0) >= 1,
            }
        } else {
            true
        }
    }

    /// Get the current input item as a character (for text input).
    pub fn head_char(&self) -> Option<char> {
        if let Some(Value::String(s)) = &self.input_value {
            let pos = self.input_pos.unwrap_or(0);
            if pos < s.len() {
                s[pos..].chars().next()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get the current input item as a value (for value input).
    pub fn head_value(&self) -> Option<Value> {
        if let Some(Value::List(items)) = &self.input_value {
            let pos = self.input_pos.unwrap_or(0);
            if pos < items.len() {
                Some(items[pos].clone())
            } else {
                None
            }
        } else if self.input_value.is_some() && self.input_pos.unwrap_or(0) == 0 {
            // Single value input - return it once
            self.input_value.clone()
        } else {
            None
        }
    }

    /// Get the text slice starting at current position (for literal matching).
    pub fn text_from(&self) -> Option<&str> {
        if let Some(Value::String(s)) = &self.input_value {
            let pos = self.input_pos.unwrap_or(0);
            if pos <= s.len() {
                Some(s.get(pos..).unwrap_or(""))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if text at position starts with the given literal.
    pub fn starts_with(&self, literal: &str) -> bool {
        self.text_from()
            .is_some_and(|text| text.starts_with(literal))
    }

    /// Get memoization entry for a rule at current position.
    pub fn get_memo(&self, rule_name: &SmolStr) -> Option<MemoEntry> {
        let pos = self.input_pos.unwrap_or(0);
        self.memo.get(&(pos, rule_name.clone())).cloned()
    }

    /// Set memoization entry for a rule at current position.
    pub fn set_memo(&mut self, rule_name: SmolStr, entry: MemoEntry) {
        let pos = self.input_pos.unwrap_or(0);
        self.memo.insert((pos, rule_name), entry);
    }

    /// Create a checkpoint for backtracking.
    pub fn checkpoint(&self, _frame: &super::Frame) -> ParseCheckpoint {
        ParseCheckpoint {
            input_pos: self.input_pos,
        }
    }

    /// Restore state from a checkpoint.
    pub fn restore(&mut self, checkpoint: ParseCheckpoint) {
        self.input_pos = checkpoint.input_pos;
        // Note: The actual restoration of values and locals happens in the Frame
        // when it calls restore_from_checkpoint
    }
}

/// A checkpoint for backtracking during pattern matching.
///
/// Captures the state needed to restore to a previous position during
/// choice and lookahead operations.
#[derive(Debug, Clone)]
pub struct ParseCheckpoint {
    /// Saved input position.
    pub input_pos: Option<usize>,
}

// Additional internal accessors for vm.rs
impl ParseState {
    /// Get the input position (for direct access in vm.rs)
    pub fn input_pos(&self) -> Option<usize> {
        self.input_pos
    }

    /// Set the input position directly (for vm.rs)
    pub fn set_input_pos(&mut self, pos: Option<usize>) {
        self.input_pos = pos;
    }

    /// Get the input value (for direct access in vm.rs)
    pub fn input_value(&self) -> &Option<Value> {
        &self.input_value
    }
}
