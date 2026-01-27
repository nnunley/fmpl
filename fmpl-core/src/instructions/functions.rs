//! Function instruction handlers
//!
//! Implements execution for function calls and returns.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::vm::Vm;

pub fn execute_call(
    _vm: &mut Vm,
    _func: InstrIndex,
    _args: Vec<InstrIndex>,
) -> Result<ExecuteResult> {
    // TODO: Implement function call
    todo!("execute_call")
}

pub fn execute_return(vm: &mut Vm, value: InstrIndex) -> Result<ExecuteResult> {
    let frame = vm.current_frame();
    let result = frame.get(value);
    Ok(ExecuteResult::Return(result))
}
