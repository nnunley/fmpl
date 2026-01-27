//! Trait-based instruction dispatch
//!
//! Defines the InstructionHandler trait and provides implementations
//! for each instruction category.

use crate::compiler::Instruction;
use crate::error::Result;
use crate::value::Value;
use crate::vm::Vm;

// Import sibling handler modules
use super::{arithmetic, control_flow, functions, objects};

/// Trait for instruction execution.
///
/// Each instruction implementation handles execution logic and returns
/// whether the instruction pointer should be advanced (true for normal
/// execution, false for jumps/returns that manage IP themselves).
pub trait InstructionHandler {
    /// Execute the instruction.
    ///
    /// Returns Ok(true) if IP should advance normally.
    /// Returns Ok(false) if IP was modified (jump, call, return).
    /// Returns Err on runtime error.
    fn execute(&self, vm: &mut Vm) -> Result<ExecuteResult>;
}

/// Result of instruction execution.
pub enum ExecuteResult {
    /// Normal execution - advance IP to next instruction
    Advance,
    /// IP was modified (jump, call, return) - don't advance
    Jump,
    /// Function should return - pop frame
    Return(Value),
}

// Implement the handler for each instruction type
impl InstructionHandler for Instruction {
    fn execute(&self, vm: &mut Vm) -> Result<ExecuteResult> {
        match self {
            // Arithmetic operations
            Instruction::Add { lhs, rhs } => arithmetic::execute_add(vm, *lhs, *rhs),
            Instruction::Sub { lhs, rhs } => arithmetic::execute_sub(vm, *lhs, *rhs),
            Instruction::Mul { lhs, rhs } => arithmetic::execute_mul(vm, *lhs, *rhs),
            Instruction::Div { lhs, rhs } => arithmetic::execute_div(vm, *lhs, *rhs),
            Instruction::Mod { lhs, rhs } => arithmetic::execute_mod(vm, *lhs, *rhs),

            // Unary operations
            Instruction::Neg { operand } => arithmetic::execute_neg(vm, *operand),
            Instruction::Not { operand } => arithmetic::execute_not(vm, *operand),

            // Comparison operations
            Instruction::Eq { lhs, rhs } => arithmetic::execute_eq(vm, *lhs, *rhs),
            Instruction::NotEq { lhs, rhs } => arithmetic::execute_ne(vm, *lhs, *rhs),
            Instruction::Lt { lhs, rhs } => arithmetic::execute_lt(vm, *lhs, *rhs),
            Instruction::Gt { lhs, rhs } => arithmetic::execute_gt(vm, *lhs, *rhs),
            Instruction::LtEq { lhs, rhs } => arithmetic::execute_le(vm, *lhs, *rhs),
            Instruction::GtEq { lhs, rhs } => arithmetic::execute_ge(vm, *lhs, *rhs),

            // Control flow
            Instruction::Jump { target } => control_flow::execute_jump(vm, *target),
            Instruction::JumpIfFalse { cond, target } => {
                control_flow::execute_jump_if_false(vm, *cond, *target)
            }
            Instruction::JumpIfTrue { cond, target } => {
                control_flow::execute_jump_if_true(vm, *cond, *target)
            }
            Instruction::JumpIfNull { cond, target } => {
                control_flow::execute_jump_if_null(vm, *cond, *target)
            }

            // Function calls
            Instruction::Call { func, args } => functions::execute_call(vm, *func, args.clone()),
            Instruction::Return { value } => functions::execute_return(vm, *value),

            // Object operations
            Instruction::GetProp { object, name } => {
                objects::execute_get_prop(vm, *object, name.clone())
            }
            Instruction::SetProp {
                object,
                name,
                value,
            } => objects::execute_set_prop(vm, *object, name.clone(), *value),

            // More instructions...
            _ => todo!("Instruction handler not yet implemented: {:?}", self),
        }
    }
}
