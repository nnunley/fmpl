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
        // Accept both shapes: list-shape `[Symbol(tag), child1, ...]` (the
        // canonical post-ITER-0004b form) and `Value::Tagged(tag, [...])` (the
        // legacy shape, retained until Phase C deletes it).
        if let Some((tag, children)) = ir.as_node() {
            return self.compile_tagged(tag.as_str(), children);
        }
        match ir {
            Value::Tagged(tag, children) => self.compile_tagged(tag.as_str(), children),
            _ => Err(Error::Runtime(format!(
                "IR compile expected tagged or list-shaped value, got {}",
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
            "LoadSymbol" => {
                let s = self.expect_symbol(&children[0])?;
                Ok(self.emit(Instruction::LoadSymbol(s)))
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
            "And" => {
                // Short-circuit AND: if left is falsy, result is false; else evaluate right
                let result_var =
                    SmolStr::new(format!("__and_result_{}", self.code.instructions.len()));
                let left_idx = self.compile_ir(&children[0])?;

                // If left is falsy, skip to false result
                let jump_to_false_idx = self.code.instructions.len();
                self.emit(Instruction::JumpIfFalse {
                    cond: left_idx,
                    target: InstrIndex(0), // placeholder
                });

                // Left was truthy, evaluate right and store
                let right_idx = self.compile_ir(&children[1])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: right_idx,
                });

                // Jump over false case
                let jump_to_end_idx = self.code.instructions.len();
                self.emit(Instruction::Jump {
                    target: InstrIndex(0), // placeholder
                });

                // False case
                let false_target = InstrIndex(self.code.instructions.len());
                if let Instruction::JumpIfFalse { target, .. } =
                    &mut self.code.instructions[jump_to_false_idx]
                {
                    *target = false_target;
                }
                let false_idx = self.emit(Instruction::LoadBool(false));
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: false_idx,
                });

                // End
                let end_target = InstrIndex(self.code.instructions.len());
                if let Instruction::Jump { target } = &mut self.code.instructions[jump_to_end_idx] {
                    *target = end_target;
                }

                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "Or" => {
                // Short-circuit OR: if left is truthy, result is true; else evaluate right
                let result_var =
                    SmolStr::new(format!("__or_result_{}", self.code.instructions.len()));
                let left_idx = self.compile_ir(&children[0])?;

                // If left is truthy, skip to true result
                let jump_to_true_idx = self.code.instructions.len();
                self.emit(Instruction::JumpIfTrue {
                    cond: left_idx,
                    target: InstrIndex(0), // placeholder
                });

                // Left was falsy, evaluate right and store
                let right_idx = self.compile_ir(&children[1])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: right_idx,
                });

                // Jump over true case
                let jump_to_end_idx = self.code.instructions.len();
                self.emit(Instruction::Jump {
                    target: InstrIndex(0), // placeholder
                });

                // True case
                let true_target = InstrIndex(self.code.instructions.len());
                if let Instruction::JumpIfTrue { target, .. } =
                    &mut self.code.instructions[jump_to_true_idx]
                {
                    *target = true_target;
                }
                let true_idx = self.emit(Instruction::LoadBool(true));
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: true_idx,
                });

                // End
                let end_target = InstrIndex(self.code.instructions.len());
                if let Instruction::Jump { target } = &mut self.code.instructions[jump_to_end_idx] {
                    *target = end_target;
                }

                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "Let" => {
                // :Let(:name, :value_ir, :body_ir)
                let name = self.expect_symbol(&children[0])?;
                let value_idx = self.compile_ir(&children[1])?;
                self.bindings.insert(name, value_idx);
                self.compile_ir(&children[2])
            }
            "Seq" => {
                // Two forms:
                // :Seq([ir1, ir2, ...]) - list form
                // :Seq(ir_first, ir_rest) - two-child form from ast_to_ir.fmpl
                if children.len() == 1 {
                    // List form
                    let items = self.expect_list(&children[0])?;
                    let mut last_idx = self.emit(Instruction::LoadNull);
                    for item in items {
                        last_idx = self.compile_ir(&item)?;
                    }
                    Ok(last_idx)
                } else if children.len() == 2 {
                    // Two-child form: evaluate first, then rest
                    self.compile_ir(&children[0])?;
                    self.compile_ir(&children[1])
                } else {
                    Err(Error::Runtime(format!(
                        "Seq expects 1 or 2 children, got {}",
                        children.len()
                    )))
                }
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
            "MakeMap" => {
                // :MakeMap([[key_ir1, val_ir1], [key_ir2, val_ir2], ...])
                let pair_list = self.expect_list(&children[0])?;
                let mut pairs = Vec::new();
                for pair in pair_list {
                    let pair_items = self.expect_list(&pair)?;
                    if pair_items.len() != 2 {
                        return Err(Error::Runtime(format!(
                            "MakeMap pair must have 2 elements, got {}",
                            pair_items.len()
                        )));
                    }
                    let key = self.compile_ir(&pair_items[0])?;
                    let val = self.compile_ir(&pair_items[1])?;
                    pairs.push((key, val));
                }
                Ok(self.emit(Instruction::MakeMap { pairs }))
            }
            "Index" => {
                // :Index(collection_ir, key_ir)
                let collection = self.compile_ir(&children[0])?;
                let key = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Index { collection, key }))
            }
            "Call" => {
                // :Call(func_ir, [arg_ir1, arg_ir2, ...])
                let func = self.compile_ir(&children[0])?;
                let arg_list = self.expect_list(&children[1])?;
                let mut args = Vec::new();
                for arg in arg_list {
                    args.push(self.compile_ir(&arg)?);
                }
                Ok(self.emit(Instruction::Call { func, args }))
            }
            "MethodCall" => {
                // :MethodCall(receiver_ir, :method_name, [arg_ir1, arg_ir2, ...])
                let receiver = self.compile_ir(&children[0])?;
                let method = self.expect_symbol(&children[1])?;
                let arg_list = self.expect_list(&children[2])?;
                let mut args = Vec::new();
                for arg in arg_list {
                    args.push(self.compile_ir(&arg)?);
                }
                Ok(self.emit(Instruction::MethodCall {
                    receiver,
                    method,
                    args,
                }))
            }
            "GetProp" => {
                // :GetProp(object_ir, :prop_name)
                let object = self.compile_ir(&children[0])?;
                let name = self.expect_symbol(&children[1])?;
                Ok(self.emit(Instruction::GetProp { object, name }))
            }
            "SetProp" => {
                // :SetProp(object_ir, :prop_name, value_ir)
                let object = self.compile_ir(&children[0])?;
                let name = self.expect_symbol(&children[1])?;
                let value = self.compile_ir(&children[2])?;
                Ok(self.emit(Instruction::SetProp {
                    object,
                    name,
                    value,
                }))
            }
            "SyncCall" => {
                // :SyncCall(target_ir)
                let target = self.compile_ir(&children[0])?;
                Ok(self.emit(Instruction::SyncCall { target }))
            }
            "AsyncCall" => {
                // :AsyncCall(target_ir)
                let target = self.compile_ir(&children[0])?;
                Ok(self.emit(Instruction::AsyncCall { target }))
            }
            "Spawn" => {
                // :Spawn(constructor_ir, [arg_ir1, arg_ir2, ...])
                let object = self.compile_ir(&children[0])?;
                let arg_list = self.expect_list(&children[1])?;
                let mut args = Vec::new();
                for arg in arg_list {
                    args.push(self.compile_ir(&arg)?);
                }
                Ok(self.emit(Instruction::Spawn { object, args }))
            }
            "GetFacet" => {
                // :GetFacet(object_ir, :facet_name)
                let object = self.compile_ir(&children[0])?;
                let name = self.expect_symbol(&children[1])?;
                Ok(self.emit(Instruction::GetFacet { object, name }))
            }
            "Lambda" => {
                // :Lambda([:param1, :param2, ...], body_ir)
                // Compiles to MakeLambda which creates a closure
                let param_list = self.expect_list(&children[0])?;
                let mut params = Vec::new();
                for p in param_list {
                    params.push(self.expect_symbol(&p)?);
                }

                // Collect free variables from the body
                let mut free_vars = std::collections::HashSet::new();
                let mut bound_vars = std::collections::HashSet::new();
                // Params are bound in the lambda body
                for p in &params {
                    bound_vars.insert(p.clone());
                }
                Self::collect_free_vars(&children[1], &bound_vars, &mut free_vars);

                // All free variables need to be captured - the VM will resolve them at runtime
                let captures: Vec<SmolStr> = free_vars.into_iter().collect();

                // Save current bindings
                let saved_bindings = self.bindings.clone();

                // Body is compiled separately - we need to track where nested code starts
                let nested_idx = self.code.nested.len();

                // Create a new compiler for the lambda body
                let mut body_compiler = IrCompiler::new();
                // Params are available in the lambda body scope
                // (handled by VM at runtime, not tracked here)
                let body_idx = body_compiler.compile_ir(&children[1])?;
                // Add return instruction if body doesn't end with one
                body_compiler.emit(Instruction::Return { value: body_idx });

                // Store the nested code
                self.code.nested.push(body_compiler.finish());

                // Restore bindings
                self.bindings = saved_bindings;

                Ok(self.emit(Instruction::MakeLambda {
                    params,
                    body: nested_idx,
                    captures,
                }))
            }
            "While" => {
                // :While(cond_ir, body_ir)
                let result_var =
                    SmolStr::new(format!("__while_result_{}", self.code.instructions.len()));
                // Initialize result to null
                let null_idx = self.emit(Instruction::LoadNull);
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: null_idx,
                });

                let loop_start = InstrIndex(self.code.instructions.len());
                let cond = self.compile_ir(&children[0])?;
                let jump_if_false_idx = self.code.instructions.len();
                self.emit(Instruction::JumpIfFalse {
                    cond,
                    target: InstrIndex(0), // placeholder
                });

                let body = self.compile_ir(&children[1])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: body,
                });
                self.emit(Instruction::Jump { target: loop_start });

                let end = InstrIndex(self.code.instructions.len());
                if let Instruction::JumpIfFalse { target, .. } =
                    &mut self.code.instructions[jump_if_false_idx]
                {
                    *target = end;
                }

                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "DoWhile" => {
                // :DoWhile(body_ir, cond_ir)
                let result_var =
                    SmolStr::new(format!("__dowhile_result_{}", self.code.instructions.len()));
                let loop_start = InstrIndex(self.code.instructions.len());
                let body = self.compile_ir(&children[0])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: body,
                });
                let cond = self.compile_ir(&children[1])?;
                self.emit(Instruction::JumpIfTrue {
                    cond,
                    target: loop_start,
                });
                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "For" => {
                // :For(pat, iter_ir, body_ir) — pat may be :PatVar(:x) or bare :x
                let pat_name = match &children[0] {
                    Value::Tagged(tag, inner) if tag.as_str() == "PatVar" => {
                        self.expect_symbol(&inner[0])?
                    }
                    other => self.expect_symbol(other)?,
                };
                let iter_idx = self.compile_ir(&children[1])?;

                // Create index counter and result
                let idx_var = SmolStr::new(format!("__for_idx_{}", self.code.instructions.len()));
                let result_var =
                    SmolStr::new(format!("__for_result_{}", self.code.instructions.len()));
                let len_var = SmolStr::new(format!("__for_len_{}", self.code.instructions.len()));

                let zero = self.emit(Instruction::LoadInt(0));
                self.emit(Instruction::StoreVar {
                    name: idx_var.clone(),
                    value: zero,
                });
                let null_idx = self.emit(Instruction::LoadNull);
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: null_idx,
                });

                // Store iter for access in loop, then get length
                self.emit(Instruction::StoreVar {
                    name: SmolStr::new("__for_iter"),
                    value: iter_idx,
                });
                let iter_ref = self.emit(Instruction::LoadVar(SmolStr::new("__for_iter")));
                let len_call = self.emit(Instruction::MethodCall {
                    receiver: iter_ref,
                    method: SmolStr::new("len"),
                    args: vec![],
                });
                self.emit(Instruction::StoreVar {
                    name: len_var.clone(),
                    value: len_call,
                });

                // Loop start
                let loop_start = InstrIndex(self.code.instructions.len());
                let cur_idx = self.emit(Instruction::LoadVar(idx_var.clone()));
                let cur_len = self.emit(Instruction::LoadVar(len_var.clone()));
                let cond = self.emit(Instruction::Lt {
                    lhs: cur_idx,
                    rhs: cur_len,
                });
                let jump_if_false_idx = self.code.instructions.len();
                self.emit(Instruction::JumpIfFalse {
                    cond,
                    target: InstrIndex(0),
                });

                // Get current element
                let iter_ref2 = self.emit(Instruction::LoadVar(SmolStr::new("__for_iter")));
                let cur_idx2 = self.emit(Instruction::LoadVar(idx_var.clone()));
                let elem = self.emit(Instruction::Index {
                    collection: iter_ref2,
                    key: cur_idx2,
                });
                self.emit(Instruction::StoreVar {
                    name: pat_name,
                    value: elem,
                });

                // Execute body
                let body = self.compile_ir(&children[2])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: body,
                });

                // Increment counter
                let cur_idx3 = self.emit(Instruction::LoadVar(idx_var.clone()));
                let one = self.emit(Instruction::LoadInt(1));
                let new_idx = self.emit(Instruction::Add {
                    lhs: cur_idx3,
                    rhs: one,
                });
                self.emit(Instruction::StoreVar {
                    name: idx_var,
                    value: new_idx,
                });
                self.emit(Instruction::Jump { target: loop_start });

                let end = InstrIndex(self.code.instructions.len());
                if let Instruction::JumpIfFalse { target, .. } =
                    &mut self.code.instructions[jump_if_false_idx]
                {
                    *target = end;
                }

                // For loops return null for parity with Rust compiler
                Ok(self.emit(Instruction::LoadNull))
            }
            "Block" => {
                // :Block([stmt_ir1, stmt_ir2, ...])
                let stmts = self.expect_list(&children[0])?;
                if stmts.is_empty() {
                    return Ok(self.emit(Instruction::LoadNull));
                }
                let mut last_idx = self.emit(Instruction::LoadNull);
                for stmt in stmts {
                    last_idx = self.compile_ir(&stmt)?;
                }
                Ok(last_idx)
            }
            "Pipe" => {
                // :Pipe(arg_ir, func_ir)
                let arg = self.compile_ir(&children[0])?;
                let func = self.compile_ir(&children[1])?;
                Ok(self.emit(Instruction::Pipe { arg, func }))
            }
            "Match" => {
                // :Match(expr_ir, [case_ir1, case_ir2, ...])
                // Cases may be :Case(pat, guard_or_null, body_ir) with 3 children
                // or :Case(pat, body_ir) with 2 children
                let expr_idx = self.compile_ir(&children[0])?;
                let cases = self.expect_list(&children[1])?;
                let result_var =
                    SmolStr::new(format!("__match_result_{}", self.code.instructions.len()));
                let match_val_var =
                    SmolStr::new(format!("__match_val_{}", self.code.instructions.len()));

                // Store the match expression value for pattern testing
                self.emit(Instruction::StoreVar {
                    name: match_val_var.clone(),
                    value: expr_idx,
                });

                let mut jump_to_end_indices = Vec::new();

                for case in &cases {
                    if let Value::Tagged(case_tag, case_children) = case
                        && (case_tag.as_str() == "Case" || case_tag.as_str() == "CaseGuard")
                    {
                        let (pat, body_ir) = if case_children.len() == 3 {
                            // 3-child: :Case(pat, guard_or_null, body_ir)
                            (&case_children[0], &case_children[2])
                        } else if case_children.len() == 2 {
                            // 2-child: :Case(pat, body_ir)
                            (&case_children[0], &case_children[1])
                        } else {
                            continue;
                        };

                        // Check if this is a wildcard pattern
                        let is_wildcard = matches!(
                            pat,
                            Value::Tagged(t, _) if t.as_str() == "PatWildcard"
                        );

                        if is_wildcard {
                            // Wildcard: always matches, compile body
                            let body = self.compile_ir(body_ir)?;
                            self.emit(Instruction::StoreVar {
                                name: result_var.clone(),
                                value: body,
                            });
                            let jmp_idx = self.code.instructions.len();
                            self.emit(Instruction::Jump {
                                target: InstrIndex(0),
                            });
                            jump_to_end_indices.push(jmp_idx);
                        } else if let Some((pat_tag, pat_children)) = pat.as_node() {
                            match pat_tag.as_str() {
                                "PatVar" => {
                                    // :PatVar(:name) — bind match value to name, always matches
                                    let var_name = self.expect_symbol(&pat_children[0])?;
                                    let val_ref =
                                        self.emit(Instruction::LoadVar(match_val_var.clone()));
                                    self.emit(Instruction::StoreVar {
                                        name: var_name,
                                        value: val_ref,
                                    });
                                    let body = self.compile_ir(body_ir)?;
                                    self.emit(Instruction::StoreVar {
                                        name: result_var.clone(),
                                        value: body,
                                    });
                                    let jmp_idx = self.code.instructions.len();
                                    self.emit(Instruction::Jump {
                                        target: InstrIndex(0),
                                    });
                                    jump_to_end_indices.push(jmp_idx);
                                }
                                "PatConstructor" if pat_children.len() >= 2 => {
                                    // :PatConstructor(:Tag, [:PatVar(:x), :PatVar(:y)])
                                    // Check tag matches, then bind children
                                    let expected_tag = self.expect_symbol(&pat_children[0])?;
                                    let sub_patterns = self.expect_list(&pat_children[1])?;

                                    // Load the match value and get its tag
                                    let val_ref =
                                        self.emit(Instruction::LoadVar(match_val_var.clone()));
                                    let tag_ref = self.emit(Instruction::GetProp {
                                        object: val_ref,
                                        name: SmolStr::new("tag"),
                                    });
                                    let expected_tag_idx =
                                        self.emit(Instruction::LoadSymbol(expected_tag));
                                    let tag_matches = self.emit(Instruction::Eq {
                                        lhs: tag_ref,
                                        rhs: expected_tag_idx,
                                    });

                                    // If tag doesn't match, skip to next case
                                    let skip_idx = self.code.instructions.len();
                                    self.emit(Instruction::JumpIfFalse {
                                        cond: tag_matches,
                                        target: InstrIndex(0), // placeholder
                                    });

                                    // Tag matches — bind children by index
                                    let val_ref2 =
                                        self.emit(Instruction::LoadVar(match_val_var.clone()));
                                    for (idx, sub_pat) in sub_patterns.iter().enumerate() {
                                        if let Value::Tagged(sp_tag, sp_children) = sub_pat
                                            && sp_tag.as_str() == "PatVar"
                                            && !sp_children.is_empty()
                                        {
                                            let var_name = self.expect_symbol(&sp_children[0])?;
                                            let idx_val =
                                                self.emit(Instruction::LoadInt(idx as i64));
                                            let elem = self.emit(Instruction::Index {
                                                collection: val_ref2,
                                                key: idx_val,
                                            });
                                            self.emit(Instruction::StoreVar {
                                                name: var_name,
                                                value: elem,
                                            });
                                        }
                                    }

                                    let body_result = self.compile_ir(body_ir)?;
                                    self.emit(Instruction::StoreVar {
                                        name: result_var.clone(),
                                        value: body_result,
                                    });
                                    let jmp_idx = self.code.instructions.len();
                                    self.emit(Instruction::Jump {
                                        target: InstrIndex(0),
                                    });
                                    jump_to_end_indices.push(jmp_idx);

                                    // Patch skip target
                                    let next_case = InstrIndex(self.code.instructions.len());
                                    if let Instruction::JumpIfFalse { target, .. } =
                                        &mut self.code.instructions[skip_idx]
                                    {
                                        *target = next_case;
                                    }
                                }
                                _ => {
                                    // Other patterns: treat as wildcard for now
                                    let body = self.compile_ir(body_ir)?;
                                    self.emit(Instruction::StoreVar {
                                        name: result_var.clone(),
                                        value: body,
                                    });
                                    let jmp_idx = self.code.instructions.len();
                                    self.emit(Instruction::Jump {
                                        target: InstrIndex(0),
                                    });
                                    jump_to_end_indices.push(jmp_idx);
                                }
                            }
                        }
                    }
                }

                let end = InstrIndex(self.code.instructions.len());
                for jmp_idx in jump_to_end_indices {
                    if let Instruction::Jump { target } = &mut self.code.instructions[jmp_idx] {
                        *target = end;
                    }
                }

                Ok(self.emit(Instruction::LoadVar(result_var)))
            }
            "TryCatch" => {
                // :TryCatch(body_ir, catch_var, catch_body_ir)
                // Matches Rust compiler behavior: PushHandler, try body, PopHandler,
                // jump over catch, catch handler, result
                let catch_var = self.expect_symbol(&children[1])?;
                let result_var =
                    SmolStr::new(format!("__try_result_{}", self.code.instructions.len()));

                // PushHandler with placeholder catch target
                let handler_idx = self.code.instructions.len();
                self.emit(Instruction::PushHandler {
                    catch_target: InstrIndex(0),
                });

                // Try body
                let body = self.compile_ir(&children[0])?;
                self.emit(Instruction::PopHandler);
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: body,
                });

                // Jump over catch
                let jump_idx = self.code.instructions.len();
                self.emit(Instruction::Jump {
                    target: InstrIndex(0),
                });

                // Catch handler target
                let catch_target = InstrIndex(self.code.instructions.len());
                if let Instruction::PushHandler {
                    catch_target: target,
                } = &mut self.code.instructions[handler_idx]
                {
                    *target = catch_target;
                }

                // Bind error value (VM pushes error on catch)
                let error_val = self.emit(Instruction::LoadNull);
                self.emit(Instruction::StoreVar {
                    name: catch_var,
                    value: error_val,
                });

                // Catch body
                let catch_body = self.compile_ir(&children[2])?;
                self.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: catch_body,
                });

                // Patch jump over catch
                let end = InstrIndex(self.code.instructions.len());
                if let Instruction::Jump { target } = &mut self.code.instructions[jump_idx] {
                    *target = end;
                }

                // TryCatch returns null for parity with Rust compiler
                Ok(self.emit(Instruction::LoadNull))
            }
            "Slice" => {
                // :Slice(collection_ir, start_or_null, end_or_null)
                // Start/end may be raw AST values (passed through from ast_to_ir)
                let collection = self.compile_ir(&children[0])?;
                let start = match &children[1] {
                    Value::Null => None,
                    other => Some(self.compile_ir(other)?),
                };
                let end = match &children[2] {
                    Value::Null => None,
                    other => Some(self.compile_ir(other)?),
                };
                Ok(self.emit(Instruction::Slice {
                    collection,
                    start,
                    end,
                }))
            }
            "Assign" => {
                // :Assign(target_name, value_ir)
                let name = self.expect_symbol(&children[0])?;
                let value = self.compile_ir(&children[1])?;
                self.emit(Instruction::StoreVar { name, value });
                Ok(value)
            }
            "QualifiedName" => {
                // :QualifiedName([part1, part2, ...]) — module::function references
                let parts = self.expect_list(&children[0])?;
                let mut name_parts = Vec::new();
                for p in &parts {
                    name_parts.push(self.expect_symbol(p)?);
                }
                let qualified = name_parts.join("::");
                Ok(self.emit(Instruction::LoadVar(SmolStr::new(qualified))))
            }
            // Raw AST nodes that may pass through ast_to_ir unchanged
            // (e.g., in Slice bounds where null prevents expr application)
            "Int" => {
                let n = self.expect_int(&children[0])?;
                Ok(self.emit(Instruction::LoadInt(n)))
            }
            "Float" => {
                let n = self.expect_float(&children[0])?;
                Ok(self.emit(Instruction::LoadFloat(n)))
            }
            "String" if !children.is_empty() => {
                let s = self.expect_string(&children[0])?;
                Ok(self.emit(Instruction::LoadString(s)))
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
            Value::Symbol(s) => Ok(s.clone()),
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

    /// Collect free variables from an IR value.
    /// `bound` contains variables that are bound in the current scope (params, let bindings).
    /// `free` accumulates the free variables found.
    fn collect_free_vars(
        ir: &Value,
        bound: &std::collections::HashSet<SmolStr>,
        free: &mut std::collections::HashSet<SmolStr>,
    ) {
        match ir {
            Value::Tagged(tag, children) => {
                match tag.as_str() {
                    "Var" => {
                        // Variable reference - check if it's free
                        if let Some(Value::Symbol(name)) = children.first()
                            && !bound.contains(name)
                        {
                            free.insert(name.clone());
                        }
                    }
                    "Let" => {
                        // :Let(:name, value_ir, body_ir)
                        // The name is bound in the body but not in the value
                        if children.len() >= 3 {
                            // Collect from value (name not yet bound)
                            Self::collect_free_vars(&children[1], bound, free);
                            // Collect from body (name is bound)
                            if let Value::Symbol(name) = &children[0] {
                                let mut new_bound = bound.clone();
                                new_bound.insert(name.clone());
                                Self::collect_free_vars(&children[2], &new_bound, free);
                            }
                        }
                    }
                    "Lambda" => {
                        // :Lambda([params], body_ir)
                        // Params are bound in the body
                        if children.len() >= 2
                            && let Value::List(params) = &children[0]
                        {
                            let mut new_bound = bound.clone();
                            for p in params.iter() {
                                if let Value::Symbol(name) = p {
                                    new_bound.insert(name.clone());
                                }
                            }
                            Self::collect_free_vars(&children[1], &new_bound, free);
                        }
                    }
                    // For all other IR nodes, recurse into children
                    _ => {
                        for child in children.iter() {
                            Self::collect_free_vars(child, bound, free);
                        }
                    }
                }
            }
            Value::List(items) => {
                for item in items.iter() {
                    Self::collect_free_vars(item, bound, free);
                }
            }
            // Other values (Int, Bool, String, etc.) have no free variables
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_load_int() {
        let ir = Value::list_node("LoadInt", vec![Value::Int(42)]);
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }

    #[test]
    fn test_compile_add() {
        let ir = Value::list_node(
            "Add",
            vec![
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(1)])),
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(2)])),
            ],
        );
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }

    #[test]
    fn test_compile_let() {
        let ir = Value::list_node(
            "Let",
            vec![
                Value::Symbol(SmolStr::new("x")),
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(42)])),
                Value::Tagged(
                    SmolStr::new("Var"),
                    Arc::new(vec![Value::Symbol(SmolStr::new("x"))]),
                ),
            ],
        );
        let result = compile(&ir).unwrap();
        assert!(matches!(result, Value::Code(_)));
    }
}
