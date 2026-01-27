//! Control flow instruction handlers
//!
//! Implements execution for jumps and returns.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::value::Value;
use crate::vm::Vm;

/// Execute unconditional jump
pub fn execute_jump(vm: &mut Vm, target: InstrIndex) -> Result<ExecuteResult> {
    let frame = vm.current_frame_mut();
    frame.ip = target.0;
    Ok(ExecuteResult::Jump)
}

/// Execute conditional jump (false)
pub fn execute_jump_if_false(
    vm: &mut Vm,
    cond: InstrIndex,
    target: InstrIndex,
) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let cond_val = frame.get(cond);

    let should_jump = match cond_val {
        Value::Bool(b) => !b,
        Value::Null => true, // Null is falsy
        _ => false,
    };

    if should_jump {
        let frame = vm.current_frame_mut();
        frame.ip = target.0;
        Ok(ExecuteResult::Jump)
    } else {
        Ok(ExecuteResult::Advance)
    }
}

/// Execute conditional jump (true)
pub fn execute_jump_if_true(
    vm: &mut Vm,
    cond: InstrIndex,
    target: InstrIndex,
) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let cond_val = frame.get(cond);

    let should_jump = match cond_val {
        Value::Bool(b) => b,
        Value::Null => false,
        _ => true, // Non-null, non-bool values are truthy
    };

    if should_jump {
        let frame = vm.current_frame_mut();
        frame.ip = target.0;
        Ok(ExecuteResult::Jump)
    } else {
        Ok(ExecuteResult::Advance)
    }
}

/// Execute conditional jump (null)
pub fn execute_jump_if_null(
    vm: &mut Vm,
    cond: InstrIndex,
    target: InstrIndex,
) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let cond_val = frame.get(cond);

    let should_jump = matches!(cond_val, Value::Null);

    if should_jump {
        let frame = vm.current_frame_mut();
        frame.ip = target.0;
        Ok(ExecuteResult::Jump)
    } else {
        Ok(ExecuteResult::Advance)
    }
}
