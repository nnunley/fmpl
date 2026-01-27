//! Frame module for Indexed RPN execution
//!
//! A call frame stores instruction results in an indexed array
//! where values[ip] holds the result of instruction ip.

use crate::compiler::{CompiledCode, InstrIndex};
use crate::object::ObjectId;
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

// ParseState will be defined in the same module (parse_state.rs)
use super::ParseState;

/// A call frame for Indexed RPN execution.
///
/// In this model, each frame has a `values` array where instruction results
/// are stored at their instruction index, and operands are read from the array
/// by index rather than from an operand stack.
#[derive(Debug)]
pub struct Frame {
    pub code: Arc<CompiledCode>,
    /// Instruction pointer (next instruction to execute).
    pub ip: usize,
    /// Values array: values[i] holds the result of instruction i.
    pub values: Vec<Value>,
    /// Local variable bindings (for parameters and let bindings).
    pub locals: HashMap<SmolStr, Value>,
    /// The `self` reference for method calls.
    pub this: Option<ObjectId>,
    /// The `caller` reference for method calls.
    pub caller: Option<ObjectId>,
    /// Parse state for grammar pattern matching.
    pub parse_state: ParseState,
}

impl Frame {
    pub fn new(code: Arc<CompiledCode>) -> Self {
        // Pre-allocate values array for all instructions
        let num_instructions = code.instructions.len();
        Self {
            code,
            ip: 0,
            values: vec![Value::Null; num_instructions],
            locals: HashMap::new(),
            this: None,
            caller: None,
            parse_state: ParseState::new(),
        }
    }

    /// Get the value at the given instruction index.
    #[inline]
    pub fn get(&self, idx: InstrIndex) -> Value {
        self.values[idx.0].clone()
    }

    /// Set the value at the current IP (before incrementing).
    #[inline]
    pub fn set_current(&mut self, value: Value) {
        if self.ip > 0 {
            self.values[self.ip - 1] = value;
        }
    }

    /// Get the result of the last executed instruction.
    pub fn result(&self) -> Value {
        if self.ip > 0 && self.ip <= self.values.len() {
            self.values[self.ip - 1].clone()
        } else if !self.values.is_empty() {
            self.values.last().cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        }
    }
}
