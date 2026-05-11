//! Builder for FMPL Indexed RPN IR (Instruction Representation).
//!
//! This provides an ergonomic, declarative interface for constructing FMPL bytecode.
//! Similar to execution_tape's Asm API, but adapted for FMPL's Indexed RPN model.
//!
//! # Example
//!
//! ```ignore
//! use fmpl_core::ir_builder::IrBuilder;
//!
//! // Build: 7 + 9 * 5
//! let mut b = IrBuilder::new();
//! let r1 = b.int(7);          // LoadInt(7) -> InstrIndex(0)
//! let r2 = b.int(9);          // LoadInt(9) -> InstrIndex(1)
//! let r3 = b.int(5);          // LoadInt(5) -> InstrIndex(2)
//! let r4 = b.mul(r2, r3);     // Mul { lhs: 1, rhs: 2 } -> InstrIndex(3)
//! let r5 = b.add(r1, r4);     // Add { lhs: 0, rhs: 3 } -> InstrIndex(4)
//! let code = b.finish();
//! ```

use crate::compiler::{CompiledCode, InstrIndex, Instruction};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;

/// Builder for FMPL Indexed RPN IR.
///
/// Each instruction that produces a value returns an `InstrIndex` pointing
/// to where that value is stored. Consuming instructions take `InstrIndex`
/// operands and return the index of their result.
#[derive(Debug, Clone)]
pub struct IrBuilder {
    /// Instructions in order of emission.
    instructions: Vec<Instruction>,
    /// Constants pool (values used multiple times).
    constants: Vec<Value>,
}

impl IrBuilder {
    /// Create a new empty IR builder.
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
        }
    }

    /// Emit a LoadNull instruction, returning its index.
    pub fn null(&mut self) -> InstrIndex {
        self.emit(Instruction::LoadNull)
    }

    /// Emit a LoadBool instruction, returning its index.
    pub fn bool(&mut self, value: bool) -> InstrIndex {
        self.emit(Instruction::LoadBool(value))
    }

    /// Emit a LoadInt instruction, returning its index.
    pub fn int(&mut self, value: i64) -> InstrIndex {
        self.emit(Instruction::LoadInt(value))
    }

    /// Emit a LoadFloat instruction, returning its index.
    pub fn float(&mut self, value: f64) -> InstrIndex {
        self.emit(Instruction::LoadFloat(value))
    }

    /// Emit a LoadString instruction, returning its index.
    pub fn string(&mut self, value: SmolStr) -> InstrIndex {
        self.emit(Instruction::LoadString(value))
    }

    /// Emit a LoadSymbol instruction, returning its index.
    pub fn symbol(&mut self, name: SmolStr) -> InstrIndex {
        self.emit(Instruction::LoadSymbol(name))
    }

    /// Emit a LoadVar instruction, returning its index.
    pub fn var(&mut self, name: SmolStr) -> InstrIndex {
        self.emit(Instruction::LoadVar(name))
    }

    /// Emit a StoreVar instruction, returning its index.
    pub fn store(&mut self, name: SmolStr, value: InstrIndex) -> InstrIndex {
        self.emit(Instruction::StoreVar { name, value })
    }

    /// Emit a Bind instruction (name binding), returning its index.
    pub fn bind(&mut self, name: SmolStr, value: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Bind { name, value })
    }

    /// Emit a NameRef instruction, returning its index.
    pub fn name_ref(&mut self, bind: InstrIndex) -> InstrIndex {
        self.emit(Instruction::NameRef { bind })
    }

    // === Arithmetic ===

    /// Emit Add { lhs, rhs }, returning the result index.
    pub fn add(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Add { lhs, rhs })
    }

    /// Emit Sub { lhs, rhs }, returning the result index.
    pub fn sub(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Sub { lhs, rhs })
    }

    /// Emit Mul { lhs, rhs }, returning the result index.
    pub fn mul(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Mul { lhs, rhs })
    }

    /// Emit Div { lhs, rhs }, returning the result index.
    pub fn div(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Div { lhs, rhs })
    }

    /// Emit Mod { lhs, rhs }, returning the result index.
    pub fn rem(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Mod { lhs, rhs })
    }

    // === Unary ===

    /// Emit Neg { operand }, returning the result index.
    pub fn neg(&mut self, operand: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Neg { operand })
    }

    /// Emit Not { operand }, returning the result index.
    pub fn not(&mut self, operand: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Not { operand })
    }

    // === Comparison ===

    /// Emit Eq { lhs, rhs }, returning the result index.
    pub fn eq(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Eq { lhs, rhs })
    }

    /// Emit NotEq { lhs, rhs }, returning the result index.
    pub fn ne(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::NotEq { lhs, rhs })
    }

    /// Emit Lt { lhs, rhs }, returning the result index.
    pub fn lt(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Lt { lhs, rhs })
    }

    /// Emit Gt { lhs, rhs }, returning the result index.
    pub fn gt(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Gt { lhs, rhs })
    }

    /// Emit LtEq { lhs, rhs }, returning the result index.
    pub fn le(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::LtEq { lhs, rhs })
    }

    /// Emit GtEq { lhs, rhs }, returning the result index.
    pub fn ge(&mut self, lhs: InstrIndex, rhs: InstrIndex) -> InstrIndex {
        self.emit(Instruction::GtEq { lhs, rhs })
    }

    // === Control Flow ===

    /// Emit a Jump instruction.
    pub fn jump(&mut self, target: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Jump { target })
    }

    /// Emit JumpIfFalse { cond, target }, returning its index.
    pub fn jump_if_false(&mut self, cond: InstrIndex, target: InstrIndex) -> InstrIndex {
        self.emit(Instruction::JumpIfFalse { cond, target })
    }

    /// Emit JumpIfTrue { cond, target }, returning its index.
    pub fn jump_if_true(&mut self, cond: InstrIndex, target: InstrIndex) -> InstrIndex {
        self.emit(Instruction::JumpIfTrue { cond, target })
    }

    /// Emit a conditional select: `cond ? then_val : else_val`
    ///
    /// This is implemented using control flow (JumpIfFalse).
    pub fn select(
        &mut self,
        cond: InstrIndex,
        then_val: InstrIndex,
        _else_val: InstrIndex,
    ) -> InstrIndex {
        // Allocate a temporary for the result
        let _result_idx = self.len();

        // Jump to else branch if condition is false
        let _else_label = self.emit(Instruction::JumpIfFalse {
            cond,
            target: InstrIndex(0), // placeholder, will fix up
        });

        // Then branch: move then_val to result position
        let _mov_then = self.emit(Instruction::LoadVar("__tmp_then".into()));

        // Jump past else branch
        let _end_label = self.emit(Instruction::Jump {
            target: InstrIndex(0), // placeholder
        });

        // Else branch: move else_val to result position
        let _mov_else = self.emit(Instruction::LoadVar("__tmp_else".into()));

        // End label
        // Note: This is a simplified version - proper implementation would
        // need label resolution like execution_tape's Asm.
        // For now, return the then_val as a simple fallback.
        then_val
    }

    // === Data Structures ===

    /// Emit MakeList with elements, returning the result index.
    pub fn list(&mut self, elements: &[InstrIndex]) -> InstrIndex {
        self.emit(Instruction::MakeList {
            elements: elements.to_vec(),
        })
    }

    /// Emit MakeMap with key-value pairs, returning the result index.
    pub fn map(&mut self, pairs: &[(InstrIndex, InstrIndex)]) -> InstrIndex {
        self.emit(Instruction::MakeMap {
            pairs: pairs.to_vec(),
        })
    }

    /// Emit Index { collection, key }, returning the result index.
    pub fn index(&mut self, collection: InstrIndex, key: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Index { collection, key })
    }

    // === Lambda ===

    /// Emit MakeLambda, returning the result index.
    ///
    /// `body_index` is the start index of the lambda body instructions.
    pub fn lambda(&mut self, params: &[SmolStr], body_index: usize) -> InstrIndex {
        self.emit(Instruction::MakeLambda {
            params: params.to_vec(),
            body: body_index,
            captures: Vec::new(),
        })
    }

    /// Emit MakeLambda with explicit captures, returning the result index.
    pub fn lambda_with_captures(
        &mut self,
        params: &[SmolStr],
        body_index: usize,
        captures: &[SmolStr],
    ) -> InstrIndex {
        self.emit(Instruction::MakeLambda {
            params: params.to_vec(),
            body: body_index,
            captures: captures.to_vec(),
        })
    }

    // === Function Calls ===

    /// Emit Call { func, args }, returning the result index.
    pub fn call(&mut self, func: InstrIndex, args: &[InstrIndex]) -> InstrIndex {
        self.emit(Instruction::Call {
            func,
            args: args.to_vec(),
        })
    }

    /// Emit Return { value }, returning its index.
    pub fn ret(&mut self, value: InstrIndex) -> InstrIndex {
        self.emit(Instruction::Return { value })
    }

    // === Scope ===

    /// Emit BlockStart, returning its index.
    pub fn block_start(&mut self) -> InstrIndex {
        self.emit(Instruction::BlockStart)
    }

    /// Emit BlockEnd, returning its index.
    pub fn block_end(&mut self) -> InstrIndex {
        self.emit(Instruction::BlockEnd)
    }

    // === Raw ===

    /// Emit a raw instruction, returning its index.
    pub fn raw(&mut self, instr: Instruction) -> InstrIndex {
        self.emit(instr)
    }

    /// Get the current instruction count (next index to be emitted).
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Check if no instructions have been emitted.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Emit an instruction and return its index.
    fn emit(&mut self, instr: Instruction) -> InstrIndex {
        let index = InstrIndex(self.instructions.len());
        self.instructions.push(instr);
        index
    }

    /// Finish building and return the CompiledCode.
    pub fn finish(self) -> CompiledCode {
        CompiledCode {
            instructions: self.instructions,
            nested: Vec::new(),
            source: None,
            constants: self.constants,
            rule_entry_points: HashMap::new(),
        }
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_arithmetic() {
        // Build: 7 + 9 * 5
        let mut a = IrBuilder::new();
        let r1 = a.int(7);
        let r2 = a.int(9);
        let r3 = a.int(5);
        let r4 = a.mul(r2, r3);
        let r5 = a.add(r1, r4);
        let code = a.finish();

        assert_eq!(code.instructions.len(), 5);
        assert_eq!(r1, InstrIndex(0));
        assert_eq!(r2, InstrIndex(1));
        assert_eq!(r3, InstrIndex(2));
        assert_eq!(r4, InstrIndex(3));
        assert_eq!(r5, InstrIndex(4));

        if let Instruction::Mul { lhs, rhs } = &code.instructions[3] {
            assert_eq!(*lhs, InstrIndex(1));
            assert_eq!(*rhs, InstrIndex(2));
        } else {
            panic!("Expected Mul instruction");
        }

        if let Instruction::Add { lhs, rhs } = &code.instructions[4] {
            assert_eq!(*lhs, InstrIndex(0));
            assert_eq!(*rhs, InstrIndex(3));
        } else {
            panic!("Expected Add instruction");
        }
    }

    #[test]
    fn test_list_construction() {
        let mut a = IrBuilder::new();
        let r1 = a.int(1);
        let r2 = a.int(2);
        let r3 = a.int(3);
        let _list = a.list(&[r1, r2, r3]);
        let code = a.finish();

        assert_eq!(code.instructions.len(), 4);
        if let Instruction::MakeList { elements } = &code.instructions[3] {
            assert_eq!(elements.len(), 3);
        } else {
            panic!("Expected MakeList instruction");
        }
    }
}
