//! Macros for instruction handler generation
//!
//! Reduces boilerplate when implementing instruction handlers.

/// Macro to generate binary instruction handlers.
///
/// Generates a handler function that:
/// 1. Gets operand values from the current frame
/// 2. Applies the operation
/// 3. Sets the result
/// 4. Returns Advance
#[macro_export]
macro_rules! binary_op {
    // For operations that can error (most Value methods)
    (
        $func_name:ident,
        $op_method:ident,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            lhs: $crate::compiler::InstrIndex,
            rhs: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(lhs);
            let b = frame.get(rhs);
            let result = a.$op_method(&b)?;
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };

    // For operations that need special handling (like Add with null behavior)
    (
        $func_name:ident,
        special,
        $body:expr
    ) => {
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            lhs: $crate::compiler::InstrIndex,
            rhs: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(lhs);
            let b = frame.get(rhs);
            let result = $body;
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };
}

/// Macro to generate unary instruction handlers.
#[macro_export]
macro_rules! unary_op {
    (
        $func_name:ident,
        $op_method:ident,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            operand: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(operand);
            let result = a.$op_method()?;
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };

    // For operations that don't error (like Not)
    (
        $func_name:ident,
        special,
        $body:expr
    ) => {
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            operand: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(operand);
            let result = $body;
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };
}

/// Macro to generate comparison instruction handlers.
#[macro_export]
macro_rules! comparison_op {
    (
        $func_name:ident,
        $op_method:ident,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            lhs: $crate::compiler::InstrIndex,
            rhs: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(lhs);
            let b = frame.get(rhs);
            let result = a.$op_method(&b);
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };
}

/// Macro to generate comparison instruction handlers that can error.
#[macro_export]
macro_rules! comparison_op_err {
    (
        $func_name:ident,
        $op_method:ident,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub fn $func_name(
            vm: &mut $crate::vm::Vm,
            lhs: $crate::compiler::InstrIndex,
            rhs: $crate::compiler::InstrIndex,
        ) -> Result<$crate::instructions::ExecuteResult> {
            let frame = vm.current_frame();
            let a = frame.get(lhs);
            let b = frame.get(rhs);
            let result = a.$op_method(&b)?;
            vm.set_current(result);
            Ok($crate::instructions::ExecuteResult::Advance)
        }
    };
}
