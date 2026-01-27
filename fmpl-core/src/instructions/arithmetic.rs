//! Arithmetic instruction handlers
//!
//! Implements execution for arithmetic and comparison operations.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::value::Value;
use crate::vm::Vm;

// Import macros from crate root (where #[macro_export] puts them)
use crate::{binary_op, comparison_op, comparison_op_err, unary_op};

// Binary arithmetic operations
binary_op!(execute_sub, sub, "Execute subtraction instruction");
binary_op!(execute_mul, mul, "Execute multiplication instruction");
binary_op!(execute_mod, modulo, "Execute modulo instruction");

// Add has special null-handling behavior - implement directly
pub fn execute_add(vm: &mut Vm, lhs: InstrIndex, rhs: InstrIndex) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let a = frame.get(lhs);
    let b = frame.get(rhs);

    // If either operand is Null, return Null (instead of error)
    // This allows patterns to fail gracefully
    let result = if matches!(a, Value::Null) || matches!(b, Value::Null) {
        Value::Null
    } else {
        match a.add(&b) {
            Ok(r) => r,
            Err(_) => Value::Null, // Type error - return Null to allow pattern to fail
        }
    };

    vm.set_current(result);
    Ok(ExecuteResult::Advance)
}

// Division uses try_op wrapper for error handling - implement directly
pub fn execute_div(vm: &mut Vm, lhs: InstrIndex, rhs: InstrIndex) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let a = frame.get(lhs);
    let b = frame.get(rhs);
    let result = vm.try_op(|| a.div(&b))?;
    vm.set_current(result);
    Ok(ExecuteResult::Advance)
}

// Unary operations
unary_op!(execute_neg, neg, "Execute negation instruction");

// Not doesn't error - implement directly
pub fn execute_not(vm: &mut Vm, operand: InstrIndex) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let a = frame.get(operand);
    let result = a.not();
    vm.set_current(result);
    Ok(ExecuteResult::Advance)
}

// Comparison operations (no error)
comparison_op!(execute_eq, eq, "Execute equality comparison");
comparison_op!(execute_ne, ne, "Execute inequality comparison");

// Comparison operations (can error)
comparison_op_err!(execute_lt, lt, "Execute less-than comparison");
comparison_op_err!(execute_gt, gt, "Execute greater-than comparison");
comparison_op_err!(execute_le, le, "Execute less-than-or-equal comparison");
comparison_op_err!(execute_ge, ge, "Execute greater-than-or-equal comparison");
