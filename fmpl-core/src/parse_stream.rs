//! ParseStream: unified stream type for grammar parsing.
//!
//! Wraps any iterable Value (String, List, Tagged) with position tracking
//! and checkpoint/restore for backtracking. Memoization will be added
//! when `apply()` is implemented (Task 4).

use crate::value::Value;
use smol_str::SmolStr;

/// A checkpoint for backtracking.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub position: usize,
}

/// Unified parse stream over any iterable Value.
#[derive(Debug, Clone)]
pub struct ParseStream {
    /// The source value being streamed over.
    source: Value,
    /// Current position in the source.
    position: usize,
}

impl ParseStream {
    /// Create a new ParseStream from any value.
    pub fn new(source: Value) -> Self {
        Self {
            source,
            position: 0,
        }
    }

    /// Get the current item without consuming it.
    /// Returns Null at end of input.
    pub fn head(&self) -> Value {
        match &self.source {
            Value::String(s) => {
                if let Some(ch) = s[self.position..].chars().next() {
                    Value::String(SmolStr::new(ch.to_string()))
                } else {
                    Value::Null
                }
            }
            Value::List(items) => {
                if self.position < items.len() {
                    items[self.position].clone()
                } else {
                    Value::Null
                }
            }
            Value::Tagged(_, _) => {
                if self.position == 0 {
                    self.source.clone()
                } else {
                    Value::Null
                }
            }
            // Single value: treat as one-element stream
            other => {
                if self.position == 0 {
                    other.clone()
                } else {
                    Value::Null
                }
            }
        }
    }

    /// Advance position by n items.
    pub fn advance(&mut self, n: usize) {
        match &self.source {
            Value::String(s) => {
                // Advance by n characters (not bytes)
                let mut chars = s[self.position..].chars();
                let mut bytes = 0;
                for _ in 0..n {
                    if let Some(ch) = chars.next() {
                        bytes += ch.len_utf8();
                    }
                }
                self.position += bytes;
            }
            _ => {
                self.position += n;
            }
        }
    }

    /// Save current position for backtracking.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            position: self.position,
        }
    }

    /// Restore to a previously saved checkpoint.
    pub fn restore(&mut self, cp: &Checkpoint) {
        self.position = cp.position;
    }

    /// Get current position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Check if at end of input.
    pub fn is_at_end(&self) -> bool {
        match &self.source {
            Value::String(s) => self.position >= s.len(),
            Value::List(items) => self.position >= items.len(),
            Value::Tagged(_, _) => self.position >= 1,
            _ => self.position >= 1,
        }
    }
}

/// Check if a character matches a class specification like "0-9", "a-zA-Z", etc.
///
/// Supports ranges (e.g., "a-z") and single characters (e.g., "abc").
/// Multiple ranges and characters can be combined: "a-zA-Z0-9_".
pub fn char_in_class(ch: char, class: &str) -> bool {
    let chars: Vec<char> = class.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' {
            // Range: a-z
            if ch >= chars[i] && ch <= chars[i + 2] {
                return true;
            }
            i += 3;
        } else {
            // Single char
            if ch == chars[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}
