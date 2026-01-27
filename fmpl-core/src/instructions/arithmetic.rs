//! Arithmetic instruction handlers
//!
//! Implements execution for arithmetic and comparison operations.
//! Uses macros to reduce boilerplate.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::vm::Vm;

// Import macros from crate root (where #[macro_export] puts them)
use crate::{binary_op, comparison_op, comparison_op_err, unary_op};

// Binary arithmetic operations
binary_op!(execute_sub, sub, "Execute subtraction instruction");
binary_op!(execute_mul, mul, "Execute multiplication instruction");
binary_op!(execute_mod, modulo, "Execute modulo instruction");

// Add has special null-handling behavior
binary_op!(execute_add, special, {
    let frame = vm.current_frame();
    let a = frame.get(lhs);
    let b = frame.get(rhs);
    if matches!(a, crate::value::Value::Null) || matches!(b, crate::value::Value::Null) {
        crate::value::Value::Null
    } else {
        match a.add(&b) {
            Ok(r) => r,
            Err(_) => crate::value::Value::Null, // Type error - return Null for pattern fail
        }
    }
});

// Division uses try_op wrapper for error handling
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
unary_op!(execute_not, special, {
    let frame = vm.current_frame();
    let a = frame.get(operand);
    a.not()
});

// Comparison operations (no error)
comparison_op!(execute_eq, eq, "Execute equality comparison");
comparison_op!(execute_ne, ne, "Execute inequality comparison");

// Comparison operations (can error)
comparison_op_err!(execute_lt, lt, "Execute less-than comparison");
comparison_op_err!(execute_gt, gt, "Execute greater-than comparison");
comparison_op_err!(execute_le, le, "Execute less-than-or-equal comparison");
comparison_op_err!(execute_ge, ge, "Execute greater-than-or-equal comparison");
