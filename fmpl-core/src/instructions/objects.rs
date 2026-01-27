//! Object instruction handlers
//!
//! Implements execution for object property access and mutation.

use super::ExecuteResult;
use crate::compiler::InstrIndex;
use crate::error::Result;
use crate::vm::Vm;

pub fn execute_get_prop(
    _vm: &mut Vm,
    _object: InstrIndex,
    _name: smol_str::SmolStr,
) -> Result<ExecuteResult> {
    // TODO: Implement property get
    todo!("execute_get_prop")
}

pub fn execute_set_prop(
    _vm: &mut Vm,
    _object: InstrIndex,
    _name: smol_str::SmolStr,
    _value: InstrIndex,
) -> Result<ExecuteResult> {
    // TODO: Implement property set
    todo!("execute_set_prop")
}
