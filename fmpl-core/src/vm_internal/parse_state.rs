//! Parse state for grammar pattern matching
//!
//! Manages input position, grammar lookup, and memoization
//! for packrat parsing during pattern matching operations.
//!
//! The input stack model enables OMeta-style tree matching:
//! - Text parsing: `input_stack = [(\"hello world\", 5)]`
//! - Tree parsing: `input_stack = [(outer_list, 2), (inner_list, 0)]`
//!
//! Descending into a list pushes a new frame. Ascending pops back.

use crate::grammar::Grammar;
use crate::grammar::input::MemoEntry;
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A single frame in the input stack for tree descent.
#[derive(Debug, Clone)]
pub struct InputFrame {
    /// The value being parsed (string, list, map, or single value).
    pub value: Value,
    /// Current position within this value.
    pub position: usize,
    /// Identity hash of the value (for memoization keying).
    pub identity: u64,
}

impl InputFrame {
    /// Create a new input frame for the given value.
    pub fn new(value: Value) -> Self {
        // Use pointer-based identity for memoization
        let identity = compute_value_identity(&value);
        Self {
            value,
            position: 0,
            identity,
        }
    }
}

/// Compute an identity hash for a value (for memoization keying).
/// Uses pointer address for heap-allocated types, value hash for primitives.
fn compute_value_identity(value: &Value) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    match value {
        // For Arc-wrapped types, use the pointer address
        Value::List(items) => Arc::as_ptr(items) as u64,
        Value::Map(map) => Arc::as_ptr(map) as u64,
        Value::String(s) => {
            // SmolStr may be inlined, so hash the content
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            hasher.finish()
        }
        // For other types, hash the value
        _ => {
            let mut hasher = DefaultHasher::new();
            // Use debug representation as a simple hash
            format!("{:?}", value).hash(&mut hasher);
            hasher.finish()
        }
    }
}

/// Parse state for grammar pattern matching.
#[derive(Debug, Clone)]
pub struct ParseState {
    /// Stack of input frames for tree descent.
    /// Top of stack is the current input being parsed.
    input_stack: Vec<InputFrame>,
    /// Grammar registry for rule lookup.
    grammar: Option<Arc<Grammar>>,
    /// Per-position memoization table for packrat parsing.
    /// Key: (identity, position, rule_name), Value: memo entry
    memo: HashMap<MemoKey, MemoEntry>,
    /// Output channel for stream-based grammar apply.
    /// When set, each match sends its result to this channel.
    /// Used for Prolog-style backtracking where grammar apply returns a stream of all matches.
    output_tx: Option<mpsc::Sender<Value>>,
    /// Abort flag for early termination (e.g., when take(n) is satisfied)
    abort: Arc<std::sync::atomic::AtomicBool>,
}

/// Key for memoization table.
/// Uses identity hash to distinguish different tree instances at same position.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoKey {
    /// Identity hash of the input value.
    pub identity: u64,
    /// Position within the input.
    pub position: usize,
    /// Rule name being applied.
    pub rule: SmolStr,
}

impl ParseState {
    pub fn new() -> Self {
        Self {
            input_stack: Vec::new(),
            grammar: None,
            memo: HashMap::new(),
            output_tx: None,
            abort: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Set the output channel for stream-based grammar apply.
    pub fn set_output_tx(&mut self, tx: mpsc::Sender<Value>) {
        self.output_tx = Some(tx);
    }

    /// Get the current output channel (if set).
    pub fn output_tx(&self) -> Option<&mpsc::Sender<Value>> {
        self.output_tx.as_ref()
    }

    /// Clear the output channel.
    pub fn clear_output_tx(&mut self) {
        self.output_tx = None;
    }

    /// Get the abort flag (for early termination).
    pub fn abort_flag(&self) -> &Arc<std::sync::atomic::AtomicBool> {
        &self.abort
    }

    /// Check if aborted.
    pub fn is_aborted(&self) -> bool {
        self.abort.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Set abort flag (stops backtracking).
    pub fn abort(&self) {
        self.abort.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set the input value for pattern matching.
    /// Clears existing stack and pushes the value as the sole input frame.
    pub fn set_input(&mut self, value: Value) {
        self.input_stack.clear();
        self.input_stack.push(InputFrame::new(value));
    }

    /// Push a new value onto the input stack (for tree descent).
    /// This is used when descending into a nested structure.
    pub fn push_input(&mut self, value: Value) {
        self.input_stack.push(InputFrame::new(value));
    }

    /// Pop the top input frame from the stack (for tree ascent).
    /// Returns true if a frame was popped, false if stack was empty.
    pub fn pop_input(&mut self) -> bool {
        if self.input_stack.len() > 1 {
            self.input_stack.pop();
            true
        } else {
            false
        }
    }

    /// Get the current stack depth (for checkpoint/restore).
    pub fn stack_depth(&self) -> usize {
        self.input_stack.len()
    }

    /// Set the grammar for rule lookup.
    pub fn set_grammar(&mut self, grammar: Arc<Grammar>) {
        self.grammar = Some(grammar);
    }

    /// Get the current grammar (if set).
    pub fn grammar(&self) -> Option<&Arc<Grammar>> {
        self.grammar.as_ref()
    }

    /// Get the current input value (top of stack).
    pub fn input(&self) -> Option<&Value> {
        self.input_stack.last().map(|f| &f.value)
    }

    /// Get the current input position (top of stack).
    pub fn position(&self) -> usize {
        self.input_stack.last().map(|f| f.position).unwrap_or(0)
    }

    /// Advance the input position by n (top of stack).
    pub fn advance(&mut self, n: usize) {
        if let Some(frame) = self.input_stack.last_mut() {
            frame.position += n;
        }
    }

    /// Check if at end of input (top of stack).
    pub fn is_at_end(&self) -> bool {
        if let Some(frame) = self.input_stack.last() {
            match &frame.value {
                Value::String(s) => frame.position >= s.len(),
                Value::List(items) => frame.position >= items.len(),
                _ => frame.position >= 1,
            }
        } else {
            true
        }
    }

    /// Get the current input item as a character (for text input).
    pub fn head_char(&self) -> Option<char> {
        if let Some(frame) = self.input_stack.last() {
            if let Value::String(s) = &frame.value {
                if frame.position < s.len() {
                    return s[frame.position..].chars().next();
                }
            }
        }
        None
    }

    /// Get the current input item as a value (for value input).
    pub fn head_value(&self) -> Option<Value> {
        if let Some(frame) = self.input_stack.last() {
            match &frame.value {
                Value::List(items) => {
                    if frame.position < items.len() {
                        Some(items[frame.position].clone())
                    } else {
                        None
                    }
                }
                _ if frame.position == 0 => {
                    // Single value input - return it once
                    Some(frame.value.clone())
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Get the text slice starting at current position (for literal matching).
    pub fn text_from(&self) -> Option<&str> {
        if let Some(frame) = self.input_stack.last() {
            if let Value::String(s) = &frame.value {
                if frame.position <= s.len() {
                    return Some(s.get(frame.position..).unwrap_or(""));
                }
            }
        }
        None
    }

    /// Check if text at position starts with the given literal.
    pub fn starts_with(&self, literal: &str) -> bool {
        self.text_from()
            .is_some_and(|text| text.starts_with(literal))
    }

    /// Get memoization entry for a rule at current position.
    pub fn get_memo(&self, rule_name: &SmolStr) -> Option<MemoEntry> {
        if let Some(frame) = self.input_stack.last() {
            let key = MemoKey {
                identity: frame.identity,
                position: frame.position,
                rule: rule_name.clone(),
            };
            self.memo.get(&key).cloned()
        } else {
            None
        }
    }

    /// Set memoization entry for a rule at current position.
    pub fn set_memo(&mut self, rule_name: SmolStr, entry: MemoEntry) {
        if let Some(frame) = self.input_stack.last() {
            let key = MemoKey {
                identity: frame.identity,
                position: frame.position,
                rule: rule_name,
            };
            self.memo.insert(key, entry);
        }
    }

    /// Create a checkpoint for backtracking.
    /// Captures stack depth and position for restoration.
    pub fn checkpoint(&self, _frame: &super::Frame) -> ParseCheckpoint {
        ParseCheckpoint {
            stack_depth: self.input_stack.len(),
            position: self.position(),
        }
    }

    /// Restore state from a checkpoint.
    /// Truncates input stack and restores position.
    pub fn restore(&mut self, checkpoint: ParseCheckpoint) {
        // Truncate stack to checkpoint depth
        self.input_stack.truncate(checkpoint.stack_depth);
        // Restore position in current frame
        if let Some(frame) = self.input_stack.last_mut() {
            frame.position = checkpoint.position;
        }
    }
}

/// A checkpoint for backtracking during pattern matching.
///
/// Captures the state needed to restore to a previous position during
/// choice and lookahead operations. Includes stack depth for tree descent.
#[derive(Debug, Clone)]
pub struct ParseCheckpoint {
    /// Depth of input stack at checkpoint time.
    pub stack_depth: usize,
    /// Position within the current input frame.
    pub position: usize,
}

// Additional internal accessors for vm.rs
impl ParseState {
    /// Get the input position (for direct access in vm.rs)
    /// Returns Some(position) if there's an input frame, None otherwise.
    pub fn input_pos(&self) -> Option<usize> {
        self.input_stack.last().map(|f| f.position)
    }

    /// Set the input position directly (for vm.rs)
    pub fn set_input_pos(&mut self, pos: Option<usize>) {
        if let Some(frame) = self.input_stack.last_mut() {
            if let Some(p) = pos {
                frame.position = p;
            }
        }
    }

    /// Get the input value (for direct access in vm.rs)
    pub fn input_value(&self) -> Option<&Value> {
        self.input_stack.last().map(|f| &f.value)
    }
}
