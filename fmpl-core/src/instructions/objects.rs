//! Object instruction handlers
//!
//! Implements execution for object property access and mutation.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::value::Value;
use crate::vm::Vm;

pub fn execute_get_prop(
    vm: &mut Vm,
    object: InstrIndex,
    name: smol_str::SmolStr,
) -> Result<ExecuteResult> {
    // TODO: Implement property get
    todo!("execute_get_prop")
}

pub fn execute_set_prop(
    vm: &mut Vm,
    object: InstrIndex,
    name: smol_str::SmolStr,
    value: InstrIndex,
) -> Result<ExecuteResult> {
    // TODO: Implement property set
    todo!("execute_set_prop")
}
