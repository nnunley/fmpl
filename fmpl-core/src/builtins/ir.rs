//! IR compilation builtins.
//!
//! Provides functions to compile tagged value IR representation to executable bytecode.

use crate::compiler::{CompiledCode, InstrIndex, Instruction};
use crate::error::{Error, Result};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Compile IR (tagged values) to executable bytecode.
pub fn compile(ir: &Value) -> Result<Value> {
    let mut compiler = IrCompiler::new();
    compiler.compile_ir(ir)?;
    Ok(Value::Code(Arc::new(compiler.finish())))
}

struct IrCompiler {
    code: CompiledCode,
    bindings: HashMap<SmolStr, InstrIndex>,
}

impl IrCompiler {
    fn new() -> Self {
        Self {
            code: CompiledCode::new(),
            bindings: HashMap::new(),
        }
    }

    fn emit(&mut self, instr: Instruction) -> InstrIndex {
        let idx = InstrIndex(self.code.instructions.len());
        self.code.instructions.push(instr);
        idx
    }

    fn compile_ir(&mut self, ir: &Value) -> Result<InstrIndex> {
        match ir {
            Value::Tagged(tag, children) => self.compile_tagged(tag.as_str(), children),
            _ => Err(Error::Runtime(format!(
                "IR compile expected tagged value, got {}",
                ir.type_name()
            ))),
        }
    }

    fn compile_tagged(&mut self, tag: &str, children: &[Value]) -> Result<InstrIndex> {
        match tag {
            "LoadNull" => Ok(self.emit(Instruction::LoadNull)),
            "LoadBool" => {
                let b = self.expect_bool(&children[0])?;
                Ok(self.emit(Instruction::LoadBool(b)))
            }
            "LoadInt" => {
                let n = self.expect_int(&children[0])?;
                Ok(self.emit(Instruction::LoadInt(n)))
            }
            "LoadFloat" => {
                let n = self.expect_float(&children[0])?;
                Ok(self.emit(Instruction::LoadFloat(n)))
            }
            "LoadString" => {
                let s = self.expect_string(&children[0])?;
                Ok(self.emit(Instruction::LoadString(s)))
            }
            "LoadVar" => {
                let name = self.expect_symbol(&children[0])?;
                Ok(self.emit(Instruction::LoadVar(name)))
            }
            "Var" => {
                // Reference to a Let-bound variable
                let name = self.expect_symbol(&children[0])?;
                if let Some(idx) = self.bindings.get(&name) {
                    // Just reference the existing value
                    Ok(*idx)
                } else {
                    // Fall back to LoadVar
                    Ok(self.emit(Instruction::LoadVar(name)))
                }
            }
            "Add" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Add { lhs, rhs }))
            }
            "Sub" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Sub { lhs, rhs }))
            }
            "Mul" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Mul { lhs, rhs }))
            }
            "Div" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Div { lhs, rhs }))
            }
            "Mod" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Mod { lhs, rhs }))
            }
            "Neg" => {
                let operand = self.compile_ir(&children[0])?;
                Ok(self.emit(Instruction::Neg { operand }))
            }
            "Not" => {
                let operand = self.compile_ir(&children[0])?;
                Ok(self.emit(Instruction::Not { operand }))
            }
            "Eq" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Eq { lhs, rhs }))
            }
            "NotEq" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::NotEq { lhs, rhs }))
            }
            "Lt" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Lt { lhs, rhs }))
            }
            "Gt" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Gt { lhs, rhs }))
            }
            "LtEq" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::LtEq { lhs, rhs }))
            }
            "GtEq" => {
                let lhs = self.compile_ir(&children[0])?;
                let rhs = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::GtEq { lhs, rhs }))
            }
            "Let" => {
                // :Let(:name, :value_ir, :body_ir)
                let name = self.expect_symbol(&children[0])?;
                let value_idx = self.compile_ir(&children[1])?;
                self.bindings.insert(name, value_idx);
                self.compile_ir(&children[2])
            }
            "Seq" => {
                // :Seq([ir1, ir2, ...])
                let items = self.expect_list(&children[0])?;
                let mut last_idx = self.emit(Instruction::LoadNull);
                for item in items {
                    last_idx = self.compile_ir(&item)?;
                }
                Ok(last_idx)
            }
            "If" => {
                // :If(:cond, :then, :else)
                // Use a temp variable to hold the result (same technique as main compiler)
                let result_var =
                    SmolStr::new(format!("__if_result_{}", self.code.instructions.len()));

                let cond = self.compile_ir(&children[0])?;
                // Placeholder for jump
                let jump_if_false_idx = self.code.instructions.len();
                self.emit(Instruction::JumpIfFalse {
                    cond,
                    target: InstrIndex(0),
                });

                // Then branch - store result
                let then_idx = self.compile_ir(&children[1])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: then_idx,
                });

                let jump_to_end_idx = self.code.instructions.len();
                self.emit(Instruction::Jump {
                    target: InstrIndex(0),
                });

                // Else branch - store result
                let else_start = InstrIndex(self.code.instructions.len());
                let else_idx = self.compile_ir(&children[2])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: else_idx,
                });

                let end = InstrIndex(self.code.instructions.len());

                // Patch jumps
                if let Instruction::JumpIfFalse { target, .. } =
                    &mut self.code.instructions[jump_if_false_idx]
                {
                    *target = else_start;
                }
                if let Instruction::Jump { target } = &mut self.code.instructions[jump_to_end_idx] {
                    *target = end;
                }

                // Load the result from temp var
                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "Return" => {
                let value = self.compile_ir(&children[0])?;
                Ok(self.emit(Instruction::Return { value }))
            }
            "MakeList" => {
                // :MakeList([ir1, ir2, ...])
                let items = self.expect_list(&children[0])?;
                let mut elements = Vec::new();
                for item in items {
                    elements.push(self.compile_ir(&item)?);
                }
                Ok(self.emit(Instruction::MakeList { elements }))
            }
            "MakeTagged" => {
                // :MakeTagged(:tag, [arg_ir1, arg_ir2, ...])
                let tag = self.expect_symbol(&children[0])?;
                let arg_irs = self.expect_list(&children[1])?;
                let mut args = Vec::new();
                for arg_ir in arg_irs {
                    args.push(self.compile_ir(&arg_ir)?);
                }
                Ok(self.emit(Instruction::MakeTagged { tag, args }))
            }
            _ => Err(Error::Runtime(format!("Unknown IR node: {}", tag))),
        }
    }

    fn expect_bool(&self, val: &Value) -> Result<bool> {
        match val {
            Value::Bool(b) => Ok(*b),
            _ => Err(Error::Runtime(format!(
                "Expected bool, got {}",
                val.type_name()
            ))),
        }
    }

    fn expect_int(&self, val: &Value) -> Result<i64> {
        match val {
            Value::Int(n) => Ok(*n),
            _ => Err(Error::Runtime(format!(
                "Expected int, got {}",
                val.type_name()
            ))),
        }
    }

    fn expect_float(&self, val: &Value) -> Result<f64> {
        match val {
            Value::Float(n) => Ok(*n),
            _ => Err(Error::Runtime(format!(
                "Expected float, got {}",
                val.type_name()
            ))),
        }
    }

    fn expect_string(&self, val: &Value) -> Result<SmolStr> {
        match val {
            Value::String(s) => Ok(s.clone()),
            _ => Err(Error::Runtime(format!(
                "Expected string, got {}",
                val.type_name()
            ))),
        }
    }

    fn expect_symbol(&self, val: &Value) -> Result<SmolStr> {
        match val {
            Value::Symbol(s) => Ok(s.clone()),
            _ => Err(Error::Runtime(format!(
                "Expected symbol, got {}",
                val.type_name()
            ))),
        }
    }

    fn expect_list(&self, val: &Value) -> Result<Vec<Value>> {
        match val {
            Value::List(items) => Ok(items.as_ref().clone()),
            _ => Err(Error::Runtime(format!(
                "Expected list, got {}",
                val.type_name()
            ))),
        }
    }

    fn finish(self) -> CompiledCode {
        self.code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_load_int() {
        let ir = Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(42)]));
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }

    #[test]
    fn test_compile_add() {
        let ir = Value::Tagged(
            SmolStr::new("Add"),
            Arc::new(vec![
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(1)])),
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(2)])),
            ]),
        );
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }

    #[test]
    fn test_compile_let() {
        let ir = Value::Tagged(
            SmolStr::new("Let"),
            Arc::new(vec![
                Value::Symbol(SmolStr::new("x")),
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(42)])),
                Value::Tagged(
                    SmolStr::new("Var"),
                    Arc::new(vec![Value::Symbol(SmolStr::new("x"))]),
                ),
            ]),
        );
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }
}
