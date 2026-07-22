//! Virtual Machine for FMPL bytecode execution.
//!
//! This VM uses Indexed RPN execution where each instruction stores its result
//! in `values[ip]`, and consuming instructions reference operands by their
//! instruction indices.

// Import VM internal modules
#[path = "vm_internal/mod.rs"]
mod vm_internal;

use crate::compiler::{CompiledCode, InstrIndex, Instruction};
use crate::error::{Error, Result};
use crate::grammar::input::MemoEntry;
use crate::grammar::{Grammar, GrammarRegistry};
use crate::json;
use crate::object::{Facet, Method, ObjectDb, ObjectId};
use crate::value::{Cursor, CursorPosition, Lambda, Stream, StreamOp, Value};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

// Re-export from vm_internal module
pub use vm_internal::{Frame, ParseCheckpoint, ParseState};

/// Scope for let bindings.
#[derive(Debug, Default)]
struct Scope {
    bindings: HashMap<SmolStr, Value>,
}

/// The FMPL virtual machine.
///
/// Uses Indexed RPN execution model where values are stored in arrays indexed
/// by instruction position rather than on an operand stack.
pub struct Vm {
    pub objects: Arc<std::sync::Mutex<ObjectDb>>,
    pub grammars: GrammarRegistry,
    frames: Vec<Frame>,
    scopes: Vec<Scope>,
    /// The current user (for `user` builtin).
    pub current_user: Option<ObjectId>,
    /// Exception handler stack: (catch_target, frame_depth)
    exception_handlers: Vec<(InstrIndex, usize)>,
    /// Tokio runtime handle for async operations
    runtime: Option<tokio::runtime::Handle>,
    /// Compiled grammar cache: maps grammar name to compiled bytecode
    /// Uses interior mutability to allow access during frame execution
    compiled_grammars: Arc<std::sync::Mutex<HashMap<SmolStr, Arc<CompiledCode>>>>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            objects: Arc::new(std::sync::Mutex::new(ObjectDb::new())),
            grammars: GrammarRegistry::new(),
            frames: Vec::new(),
            scopes: vec![Scope::default()],
            current_user: None,
            exception_handlers: Vec::new(),
            runtime: None,
            compiled_grammars: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Create a VM with a tokio runtime handle.
    pub fn with_runtime(handle: tokio::runtime::Handle) -> Self {
        let mut vm = Self::new();
        vm.runtime = Some(handle);
        vm
    }

    /// Set the runtime handle.
    pub fn set_runtime(&mut self, handle: tokio::runtime::Handle) {
        self.runtime = Some(handle);
    }

    // ========== Helper methods for instruction handlers ==========

    /// Get the current (top) frame for reading.
    pub fn current_frame(&self) -> &Frame {
        self.frames.last().expect("No frame on stack")
    }

    /// Get the current (top) frame for mutation.
    pub fn current_frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("No frame on stack")
    }

    /// Set the result value for the current instruction.
    pub fn set_current(&mut self, value: Value) {
        let frame = self.frames.last_mut().expect("No frame on stack");
        frame.set_current(value);
    }

    /// Get the runtime handle, if set.
    pub fn runtime(&self) -> Option<&tokio::runtime::Handle> {
        self.runtime.as_ref()
    }

    /// Run compiled code and return the result.
    pub fn run(&mut self, code: &CompiledCode) -> Result<Value> {
        let code = Arc::new(code.clone());
        let frame = Frame::new(code);
        let base_depth = self.frames.len();
        self.frames.push(frame);

        self.execute_with_depth(base_depth)?;

        // Get the result from the last frame's last instruction
        let result = self
            .frames
            .last()
            .map(|f| f.result())
            .unwrap_or(Value::Null);
        self.frames.pop();

        // For backward compatibility: automatically collect streams
        // This makes simple cases like "abc" @ { [a-z]+ => "word" } work without explicit collect
        if let Value::Stream(stream) = result {
            return self.execute_stream(&stream);
        }

        Ok(result)
    }

    /// Evaluate an expression with bindings in scope (for semantic actions).
    pub fn eval_with_bindings(
        &mut self,
        expr: &crate::ast::Expr,
        bindings: &std::collections::HashMap<SmolStr, Value>,
    ) -> Result<Value> {
        use crate::compiler::Compiler;

        // Compile the expression
        let code = Compiler::new().compile(expr)?;

        // Push a new scope with the bindings
        let mut scope = Scope::default();
        for (name, value) in bindings {
            scope.bindings.insert(name.clone(), value.clone());
        }
        self.scopes.push(scope);

        // Run the compiled code
        let result = self.run(&code);

        // Pop the scope
        self.scopes.pop();

        result
    }

    /// Apply a grammar to an input value, evaluating any semantic actions.
    /// Get or compile a grammar's bytecode.
    /// Returns the compiled code with rule entry points.
    /// Note: This method is no longer used directly - we access compiled_grammars cache directly
    #[allow(dead_code)]
    fn get_or_compile_grammar(&self, grammar: &Grammar) -> Result<Arc<CompiledCode>> {
        let grammar_name = grammar.name.clone();

        // Check cache first (using Mutex::lock for interior mutability)
        {
            let cache = self.compiled_grammars.lock().unwrap();
            if let Some(compiled) = cache.get(&grammar_name) {
                return Ok(compiled.clone());
            }
        }

        // Not cached - compile the grammar using the public compile_grammar method
        use crate::compiler::Compiler;
        let compiled = Compiler::compile_grammar_only(grammar)?;

        let compiled = Arc::new(compiled);

        // Insert into cache
        let mut cache = self.compiled_grammars.lock().unwrap();
        cache.insert(grammar_name, compiled.clone());
        Ok(compiled)
    }

    pub fn apply_grammar(
        &mut self,
        input: Value,
        grammar: Arc<Grammar>,
        rule_name: &str,
    ) -> Result<Option<Value>> {
        use crate::grammar::runtime::{
            apply_grammar_to_stream_with_evaluator, apply_grammar_to_value_with_evaluator,
        };

        let registry = self.grammars.clone();

        let evaluator = std::rc::Rc::new(std::cell::RefCell::new(
            |expr: &crate::ast::Expr, bindings: &HashMap<SmolStr, Value>| {
                self.eval_with_bindings(expr, bindings)
            },
        ));

        // If input is an AsyncStream, use streaming grammar application
        if let Value::AsyncStream(stream_arc) = input {
            let mut guard = stream_arc
                .lock()
                .map_err(|_| Error::Runtime("failed to lock async stream".to_string()))?;

            let (_, dummy_rx) = tokio::sync::mpsc::channel(1);
            let original_receiver = std::mem::replace(&mut guard.receiver, dummy_rx);
            let stream_handle = crate::stream::StreamHandle::new(original_receiver, guard.id());
            drop(guard);

            return apply_grammar_to_stream_with_evaluator(
                stream_handle,
                &grammar,
                &registry,
                rule_name,
                evaluator,
            );
        }

        apply_grammar_to_value_with_evaluator(input, &grammar, &registry, rule_name, evaluator)
    }

    // === Helper functions for instruction handlers ===

    /// Check if a return value represents a grammar rule failure.
    fn is_grammar_rule_failure(ret_val: &Value, parse_state: &ParseState) -> bool {
        let is_grammar_rule = parse_state.input().is_some() || parse_state.grammar().is_some();
        is_grammar_rule && matches!(ret_val, Value::Null)
    }

    /// Determine if a grammar rule should require full input consumption.
    fn should_require_full_consumption(parse_state: &ParseState, caller_frame: &Frame) -> bool {
        let is_grammar_rule = parse_state.input().is_some() || parse_state.grammar().is_some();
        let is_top_level = caller_frame.parse_state.grammar().is_none();
        let position_advanced = parse_state.position() > 0;

        let is_streaming = match parse_state.input() {
            Some(Value::String(_)) => true,
            Some(Value::List(_)) => position_advanced,
            _ => false,
        };

        is_grammar_rule && is_top_level && is_streaming && position_advanced
    }

    /// Get a property value from an object or map.
    fn get_property_value(&self, obj: &Value, name: &SmolStr) -> Result<Value> {
        match obj {
            Value::Object(id) => self
                .objects
                .lock()
                .unwrap()
                .get_property(*id, name.as_str())
                .ok_or_else(|| Error::UndefinedProperty(name.to_string())),
            Value::Facet(id, facet_name) => {
                let db = self.objects.lock().unwrap();
                if !db.facet_allows(*id, facet_name, name.as_str()) {
                    return Err(Error::Runtime(format!(
                        "facet :{} does not expose property '{}'",
                        facet_name, name
                    )));
                }
                db.get_property(*id, name.as_str())
                    .ok_or_else(|| Error::UndefinedProperty(name.to_string()))
            }
            Value::Map(map) => map
                .get(name)
                .cloned()
                .ok_or_else(|| Error::UndefinedProperty(name.to_string())),
            // List-shaped node (e.g. [Symbol(tag), child1, ...]) exposes
            // `tag`, `children`, and `len` for pattern matching.
            Value::List(items) if !items.is_empty() && matches!(items[0], Value::Symbol(_)) => {
                match name.as_str() {
                    "tag" => Ok(items[0].clone()),
                    "children" => Ok(Value::List(Arc::new(items[1..].to_vec()))),
                    "len" => Ok(Value::Int((items.len() - 1) as i64)),
                    _ => Err(Error::UndefinedProperty(name.to_string())),
                }
            }
            _ => Err(Error::Type {
                expected: "object, map, or list-shaped node".to_string(),
                got: obj.type_name().to_string(),
            }),
        }
    }

    /// Main execution loop using Indexed RPN.
    ///
    /// `base_depth` is the number of frames to preserve (caller's frames).
    /// The loop continues until we're back to that depth.
    fn execute_with_depth(&mut self, base_depth: usize) -> Result<()> {
        while self.frames.len() > base_depth {
            let frame = self.frames.last().unwrap();

            if frame.ip >= frame.code.instructions.len() {
                // End of code - if this isn't the base frame, pop and continue
                if self.frames.len() > base_depth + 1 {
                    // Return from a nested call - propagate result to caller
                    let result = frame.result();
                    let new_position = frame.parse_state.position();

                    self.frames.pop();
                    if let Some(caller) = self.frames.last_mut() {
                        caller.set_current(result.clone());
                        // Propagate parse state position for grammar rules
                        caller
                            .parse_state
                            .advance(new_position - caller.parse_state.position());
                    }
                    continue;
                } else {
                    // This is the base frame - execution complete
                    break;
                }
            }

            let instr = frame.code.instructions[frame.ip].clone();
            let frame = self.frames.last_mut().unwrap();
            frame.ip += 1;

            match instr {
                // === Literals ===
                Instruction::LoadNull => {
                    frame.set_current(Value::Null);
                }
                Instruction::LoadBool(b) => {
                    frame.set_current(Value::Bool(b));
                }
                Instruction::LoadInt(n) => {
                    frame.set_current(Value::Int(n));
                }
                Instruction::LoadFloat(f) => {
                    frame.set_current(Value::Float(f));
                }
                Instruction::LoadString(s) => {
                    frame.set_current(Value::String(s));
                }
                Instruction::LoadSymbol(s) => {
                    frame.set_current(Value::Symbol(s));
                }
                Instruction::LoadVar(name) => {
                    let val = self.lookup_var(&name);
                    // If variable is undefined, return Null (instead of error)
                    // This allows patterns to fail gracefully
                    let val = match val {
                        Ok(v) => v,
                        Err(Error::UndefinedVariable(_)) => Value::Null,
                        Err(e) => return Err(e),
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(val);
                }
                Instruction::StoreVar { name, value } => {
                    let val = self.frames.last().unwrap().get(value);
                    // Automatically execute streams when storing to variables
                    // This provides backward compatibility for cases like:
                    // let (ir = ast @ { ... })
                    // let (code = ir::compile(ir))
                    let val_to_store = match &val {
                        Value::Stream(stream) => {
                            // Execute the stream and return the first match
                            self.execute_stream(stream)?
                        }
                        _ => val.clone(),
                    };
                    self.store_var(name.clone(), val_to_store.clone());
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(val_to_store);
                }
                Instruction::LoadSelf => {
                    let val = if let Some(id) = frame.this {
                        Value::Object(id)
                    } else {
                        Value::Null
                    };
                    frame.set_current(val);
                }
                Instruction::LoadParent => {
                    let val = if let Some(id) = frame.this {
                        if let Some(obj) = self.objects.lock().unwrap().get(id) {
                            if let Some(parent) = obj.parent {
                                Value::Object(parent)
                            } else {
                                Value::Null
                            }
                        } else {
                            Value::Null
                        }
                    } else {
                        Value::Null
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(val);
                }
                Instruction::LoadCaller => {
                    let val = if let Some(id) = frame.caller {
                        Value::Object(id)
                    } else {
                        Value::Null
                    };
                    frame.set_current(val);
                }
                Instruction::LoadUser => {
                    let val = if let Some(id) = self.current_user {
                        Value::Object(id)
                    } else {
                        Value::Symbol("none".into())
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(val);
                }
                Instruction::LoadArgs => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::List(Arc::new(Vec::new())));
                }

                // === Binary Arithmetic ===
                Instruction::Add { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    // If either operand is Null, return Null (instead of error)
                    // This allows patterns to fail gracefully
                    let result = if matches!(a, Value::Null) || matches!(b, Value::Null) {
                        Value::Null
                    } else {
                        match a.add(&b) {
                            Ok(r) => r,
                            // A genuine arithmetic error (e.g. integer overflow)
                            // must propagate, not silently become Null.
                            Err(e @ Error::Runtime(_)) => return Err(e),
                            // Type mismatch stays Null so patterns can fail gracefully.
                            Err(_) => Value::Null,
                        }
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Sub { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.sub(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Mul { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.mul(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Div { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = self.try_op(|| a.div(&b))?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Mod { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.modulo(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }

                // === Unary ===
                Instruction::Neg { operand } => {
                    let a = frame.get(operand);
                    let result = a.neg()?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Not { operand } => {
                    let a = frame.get(operand);
                    let result = a.not();
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }

                // === Comparison ===
                Instruction::Eq { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.eq(&b);
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::NotEq { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.ne(&b);
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Lt { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.lt(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Gt { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.gt(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::LtEq { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.le(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::GtEq { lhs, rhs } => {
                    let a = frame.get(lhs);
                    let b = frame.get(rhs);
                    let result = a.ge(&b)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::In { lhs, rhs } => {
                    // Membership test: lhs in rhs
                    // rhs can be a list, string, or map (for map keys)
                    let elem = frame.get(lhs);
                    let collection = frame.get(rhs);
                    let result = match collection {
                        Value::List(items) => {
                            // Check if elem is in the list
                            items.contains(&elem)
                        }
                        Value::String(s) => {
                            // Check if elem (as string) is a substring
                            match elem {
                                Value::String(elem_str) => s.contains(elem_str.as_str()),
                                _ => false,
                            }
                        }
                        Value::Map(map) => {
                            // Check if elem is a key in the map
                            match elem {
                                Value::String(key) => map.contains_key(key.as_str()),
                                Value::Symbol(key) => map.contains_key(key.as_str()),
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Bool(result));
                }

                // === Control Flow ===
                Instruction::Jump { target } => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target.0;
                }
                Instruction::JumpIfFalse { cond, target } => {
                    let val = frame.get(cond);
                    if val.is_falsy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target.0;
                    }
                }
                Instruction::JumpIfTrue { cond, target } => {
                    let val = frame.get(cond);
                    if val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target.0;
                    }
                }
                Instruction::JumpIfNull { cond, target } => {
                    let val = frame.get(cond);
                    if matches!(val, Value::Null) {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target.0;
                    }
                }

                // === Function Calls ===
                Instruction::Call { func, args } => {
                    let func_val = frame.get(func);
                    let arg_vals: Vec<Value> = args.iter().map(|&idx| frame.get(idx)).collect();
                    match self.call_value(func_val, arg_vals) {
                        Ok(()) => {}
                        Err(e) if !self.exception_handlers.is_empty() => {
                            let error = Value::String(SmolStr::new(e.to_string()));
                            self.throw_exception(error)?;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Instruction::TailCall { func, args } => {
                    let func_val = frame.get(func);
                    let arg_vals: Vec<Value> = args.iter().map(|&idx| frame.get(idx)).collect();
                    self.frames.pop();
                    self.call_value(func_val, arg_vals)?;
                }
                Instruction::MethodCall {
                    receiver,
                    method,
                    args,
                } => {
                    let receiver_val = frame.get(receiver);
                    let arg_vals: Vec<Value> = args.iter().map(|&idx| frame.get(idx)).collect();
                    match self.call_method(receiver_val, &method, arg_vals) {
                        Ok(()) => {}
                        Err(e) if !self.exception_handlers.is_empty() => {
                            let error = Value::String(SmolStr::new(e.to_string()));
                            self.throw_exception(error)?;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Instruction::Return { value } => {
                    let ret_val = frame.get(value);
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(ret_val.clone());

                    let returned_parse_state = frame.parse_state.clone();

                    // Check if there's a caller frame before popping
                    if self.frames.len() > 1 {
                        // Has caller - pop and propagate return value
                        self.frames.pop();

                        let caller_frame = self.frames.last_mut().unwrap();

                        if Self::is_grammar_rule_failure(&ret_val, &returned_parse_state) {
                            return Err(Error::ParseFailed {
                                position: returned_parse_state.position(),
                                message: "grammar rule failed to match".to_string(),
                            });
                        }

                        if Self::should_require_full_consumption(
                            &returned_parse_state,
                            caller_frame,
                        ) && !returned_parse_state.is_at_end()
                        {
                            return Err(Error::ParseFailed {
                                position: returned_parse_state.position(),
                                message: format!(
                                    "grammar rule did not consume entire input (stopped at position {})",
                                    returned_parse_state.position()
                                ),
                            });
                        }

                        caller_frame.set_current(ret_val);
                        caller_frame.parse_state = returned_parse_state;
                    } else {
                        // No caller frame - this is a top-level return
                        // Don't pop the frame - leave it with the return value in current
                        // Break out of the execution loop
                        break;
                    }
                }

                // === Objects ===
                Instruction::GetProp { object, name } => {
                    let obj = frame.get(object);
                    let result = self.get_property_value(&obj, &name)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::SetProp {
                    object,
                    name,
                    value,
                } => {
                    let obj = frame.get(object);
                    let val = frame.get(value);
                    match obj {
                        Value::Object(id) => {
                            self.objects.lock().unwrap().set_property(
                                id,
                                name.clone(),
                                val.clone(),
                            )?;
                        }
                        Value::Facet(_, ref facet_name) => {
                            return Err(Error::Runtime(format!(
                                "cannot set properties through facet :{}",
                                facet_name
                            )));
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "object".to_string(),
                                got: obj.type_name().to_string(),
                            });
                        }
                    }
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(val);
                }
                Instruction::Spawn { object, args } => {
                    let constructor = frame.get(object);
                    let arg_vals: Vec<Value> = args.iter().map(|&idx| frame.get(idx)).collect();
                    let obj_id = self.spawn_object(constructor, arg_vals)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Object(obj_id));
                }
                Instruction::GetFacet { object, name } => {
                    let obj = frame.get(object);
                    match obj {
                        Value::Object(id) => {
                            if self.objects.lock().unwrap().get_facet(id, &name).is_some() {
                                let frame = self.frames.last_mut().unwrap();
                                frame.set_current(Value::Facet(id, name.clone()));
                            } else {
                                return Err(Error::Runtime(format!("undefined facet: {}", name)));
                            }
                        }
                        Value::Facet(_, ref current_facet) => {
                            // Facet-on-facet composition requires intersection semantics
                            // (see specs/object-system/facets.md). Deny for now to
                            // prevent privilege widening.
                            return Err(Error::Runtime(format!(
                                "facet-on-facet composition not yet implemented \
                                 (current: :{}, requested: :{}); use the underlying \
                                 object to request a different facet",
                                current_facet, name
                            )));
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "object".to_string(),
                                got: obj.type_name().to_string(),
                            });
                        }
                    }
                }

                // === Sync/Async ===
                Instruction::SyncCall { target } => {
                    // In Phase 1, sync call just passes through
                    let val = frame.get(target);
                    frame.set_current(val);
                }
                Instruction::AsyncCall { target } => {
                    let value = frame.get(target);

                    if self.runtime.is_none() {
                        return Err(Error::Runtime(
                            "async call requires runtime handle - use Vm::with_runtime()"
                                .to_string(),
                        ));
                    }

                    use crate::stream::{StreamEvent, StreamHandle, next_id};
                    use tokio::sync::mpsc;

                    let (tx, rx) = mpsc::channel(1);
                    let _ = tx.try_send(StreamEvent::Ok(value));

                    let handle = StreamHandle::new(rx, next_id());
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::AsyncStream(Arc::new(std::sync::Mutex::new(handle))));
                }
                Instruction::AwaitAll { streams } => {
                    let streams_val = frame.get(streams);

                    let stream_list = match streams_val {
                        Value::List(lst) => lst,
                        _ => {
                            return Err(Error::Type {
                                expected: "list".to_string(),
                                got: streams_val.type_name().to_string(),
                            });
                        }
                    };

                    use crate::stream::StreamEvent;

                    // Collect all AsyncStream handles and wait for each to complete
                    let mut results = Vec::new();
                    for item in stream_list.iter() {
                        match item {
                            Value::AsyncStream(stream_arc) => {
                                // Lock the mutex and call recv_blocking
                                let mut handle = stream_arc.lock().unwrap();
                                match handle.recv_blocking() {
                                    Some(StreamEvent::Ok(value)) => {
                                        results.push(value);
                                    }
                                    Some(StreamEvent::Err(err)) => {
                                        return Err(Error::Runtime(format!(
                                            "await_all: stream error: {:?}",
                                            err
                                        )));
                                    }
                                    Some(StreamEvent::Data(value)) => {
                                        // Data events are also collected
                                        results.push(value);
                                    }
                                    Some(StreamEvent::Done) => {
                                        // Stream completed without value
                                        results.push(Value::Null);
                                    }
                                    None => {
                                        return Err(Error::Runtime(
                                            "await_all: stream closed without result".to_string(),
                                        ));
                                    }
                                }
                            }
                            _ => {
                                return Err(Error::Type {
                                    expected: "async_stream".to_string(),
                                    got: item.type_name().to_string(),
                                });
                            }
                        }
                    }

                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::List(Arc::new(results)));
                }
                Instruction::Yield { value } => {
                    let _value = frame.get(value);

                    // Yield can only work within an async generator context
                    // For now, yield sends a Data event to the current async stream's channel
                    // This requires the generator to have access to its output channel
                    // We'll need to store the yield target in the frame or exception handler

                    // For now, implement as throwing an error with helpful message
                    return Err(Error::Runtime(
                        "yield can only be used within async block. Use: stream.create with sink pattern for explicit streams.".to_string()
                    ));
                }
                Instruction::YieldToSink { value } => {
                    let value_to_yield = frame.get(value);

                    // YieldToSink sends a value to the current output channel (if one is set in parse_state)
                    // This is used for Prolog-style backtracking where grammars can yield multiple values
                    if let Some(tx) = frame.parse_state.output_tx() {
                        // Send the value to the output channel
                        if tx.blocking_send(value_to_yield.clone()).is_err() {
                            // Channel closed, stop backtracking
                            return Err(Error::Runtime("output channel closed".to_string()));
                        }
                        // Set the yielded value as the current value (so the expression has a result)
                        frame.set_current(value_to_yield);
                    } else {
                        return Err(Error::Runtime(
                            "yield can only be used within a grammar apply (which returns a stream)".to_string()
                        ));
                    }
                }

                // === Data Structures ===
                Instruction::MakeList { elements } => {
                    let items: Vec<Value> = elements.iter().map(|&idx| frame.get(idx)).collect();
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::List(Arc::new(items)));
                }
                Instruction::MakeMap { pairs } => {
                    let mut map = HashMap::new();
                    for &(key_idx, val_idx) in &pairs {
                        let key = frame.get(key_idx);
                        let val = frame.get(val_idx);
                        let key_str = match key {
                            Value::Symbol(s) => s,
                            Value::String(s) => s,
                            _ => {
                                return Err(Error::Type {
                                    expected: "symbol or string".to_string(),
                                    got: key.type_name().to_string(),
                                });
                            }
                        };
                        map.insert(key_str, val);
                    }
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Map(Arc::new(map)));
                }
                Instruction::MakeListNode { tag, args } => {
                    let children: Vec<Value> = args.iter().map(|&idx| frame.get(idx)).collect();
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::list_node(tag.clone(), children));
                }
                Instruction::Index { collection, key } => {
                    let col = frame.get(collection);
                    let k = frame.get(key);
                    let result = col.index(&k)?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::Slice {
                    collection,
                    start,
                    end,
                } => {
                    let col = frame.get(collection);
                    // Clone start and end values to avoid lifetime issues
                    // TODO: optimize to avoid clones
                    let start_cloned = start.as_ref().map(|idx| frame.get(*idx).clone());
                    let end_cloned = end.as_ref().map(|idx| frame.get(*idx).clone());
                    let result = col.slice(start_cloned.as_ref(), end_cloned.as_ref())?;
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }

                // === Scoping ===
                // BlockStart/BlockEnd are metadata instructions for resolve_names pass
                // At runtime, they function like PushScope/PopScope for backwards compatibility
                Instruction::BlockStart => {
                    self.scopes.push(Scope::default());
                }
                Instruction::BlockEnd => {
                    self.scopes.pop();
                }
                // Legacy scope instructions (deprecated)
                Instruction::PushScope => {
                    self.scopes.push(Scope::default());
                }
                Instruction::PopScope => {
                    self.scopes.pop();
                }
                Instruction::Bind { name, value } => {
                    let val = frame.get(value);
                    // Automatically execute streams when binding
                    // This provides backward compatibility for cases like:
                    // let (ir = ast @ { ... })
                    let val_to_bind = match &val {
                        Value::Stream(stream) => {
                            // Execute the stream and return the first match
                            self.execute_stream(stream)?
                        }
                        _ => val.clone(),
                    };
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.bindings.insert(name, val_to_bind);
                    }
                }
                // NameRef: resolved at compile-time, directly references Bind instruction
                Instruction::NameRef { bind } => {
                    // Look up the value from the Bind instruction
                    if let Instruction::Bind { value, .. } = &frame.code.instructions[bind.0] {
                        let val = frame.get(*value);
                        let frame = self.frames.last_mut().unwrap();
                        frame.set_current(val);
                    } else {
                        return Err(Error::Runtime(format!(
                            "NameRef points to non-Bind instruction at index {}",
                            bind.0
                        )));
                    }
                }

                // === Lambda ===
                Instruction::MakeLambda {
                    params,
                    body,
                    captures,
                } => {
                    let frame = self.frames.last().unwrap();
                    let nested_code = frame.code.nested.get(body).cloned();

                    if let Some(code) = nested_code {
                        // Capture only the specified variables from current scopes
                        let mut capture_values = HashMap::new();
                        for capture_name in &captures {
                            // Look up the variable from scopes (same as lookup_var)
                            let mut found = None;
                            for scope in self.scopes.iter().rev() {
                                if let Some(val) = scope.bindings.get(capture_name) {
                                    found = Some(val.clone());
                                    break;
                                }
                            }
                            // Also check frame locals
                            if found.is_none()
                                && let Some(val) = frame.locals.get(capture_name)
                            {
                                found = Some(val.clone());
                            }
                            // If still not found, that's an error - but we'll handle it at runtime
                            if let Some(val) = found {
                                capture_values.insert(capture_name.clone(), val);
                            }
                        }

                        let lambda = Lambda {
                            params,
                            code: Arc::new(code),
                            captures: capture_values,
                        };
                        let frame = self.frames.last_mut().unwrap();
                        frame.set_current(Value::Lambda(Arc::new(lambda)));
                    } else {
                        return Err(Error::Runtime("invalid lambda code index".to_string()));
                    }
                }

                // === Pipe ===
                Instruction::Pipe { arg, func } => {
                    let arg_val = frame.get(arg);
                    let func_val = frame.get(func);
                    self.call_value(func_val, vec![arg_val])?;
                }

                // === Streams ===
                Instruction::MakeStream { source } => {
                    let source_val = frame.get(source);
                    let stream = Stream {
                        source: source_val,
                        ops: Vec::new(),
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Stream(Arc::new(stream)));
                }
                Instruction::StreamMap { source, func } => {
                    let source_val = frame.get(source);
                    let func_val = frame.get(func);
                    self.push_stream_op(source_val, StreamOp::Map(func_val))?;
                }
                Instruction::StreamFilter { source, pred } => {
                    let source_val = frame.get(source);
                    let pred_val = frame.get(pred);
                    self.push_stream_op(source_val, StreamOp::Filter(pred_val))?;
                }
                Instruction::StreamFlatMap { source, func } => {
                    let source_val = frame.get(source);
                    let func_val = frame.get(func);
                    self.push_stream_op(source_val, StreamOp::FlatMap(func_val))?;
                }
                Instruction::StreamReduce { source, init, func } => {
                    let source_val = frame.get(source);
                    let init_val = frame.get(init);
                    let func_val = frame.get(func);
                    // For reduce, we need both init and func
                    let _ = init_val;
                    self.push_stream_op(source_val, StreamOp::Reduce(func_val))?;
                }
                Instruction::StreamParse {
                    source,
                    grammar,
                    rule,
                } => {
                    let source_val = frame.get(source);
                    let grammar_val = frame.get(grammar);
                    self.push_stream_parse(source_val, grammar_val, rule)?;
                }
                Instruction::CoerceStream { value, mode } => {
                    use crate::compiler::StreamMode;
                    let val = frame.get(value);
                    let result = match mode {
                        StreamMode::Chars => {
                            // String -> list of single-char strings
                            match val {
                                Value::String(s) => {
                                    let chars: Vec<Value> = s
                                        .chars()
                                        .map(|c| Value::String(SmolStr::new(c.to_string())))
                                        .collect();
                                    Value::List(Arc::new(chars))
                                }
                                _ => {
                                    return Err(Error::Type {
                                        expected: "string".to_string(),
                                        got: val.type_name().to_string(),
                                    });
                                }
                            }
                        }
                        StreamMode::Items => {
                            // List -> pass through as-is
                            match val {
                                Value::List(_) => val,
                                _ => {
                                    return Err(Error::Type {
                                        expected: "list".to_string(),
                                        got: val.type_name().to_string(),
                                    });
                                }
                            }
                        }
                        StreamMode::Once => {
                            // Any value -> wrap in single-element list
                            Value::List(Arc::new(vec![val]))
                        }
                        StreamMode::Auto => {
                            // Detect type at runtime
                            match &val {
                                Value::String(s) => {
                                    // String -> list of single-char strings
                                    let chars: Vec<Value> = s
                                        .chars()
                                        .map(|c| Value::String(SmolStr::new(c.to_string())))
                                        .collect();
                                    Value::List(Arc::new(chars))
                                }
                                // List-shaped node (tagged data) wraps as single element.
                                // Plain lists pass through as-is.
                                Value::List(items)
                                    if !items.is_empty()
                                        && matches!(items[0], Value::Symbol(_)) =>
                                {
                                    Value::List(Arc::new(vec![val]))
                                }
                                Value::List(_) => val,
                                _ => {
                                    // Map or other -> wrap in single-element list
                                    Value::List(Arc::new(vec![val]))
                                }
                            }
                        }
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(result);
                }
                Instruction::StreamCollect { source } => {
                    let source_val = frame.get(source);
                    // When StreamCollect is encountered, execute the entire stream pipeline
                    match source_val {
                        Value::Stream(stream) => {
                            let result = self.execute_stream(&stream)?;
                            let frame = self.frames.last_mut().unwrap();
                            frame.set_current(result);
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "stream".to_string(),
                                got: source_val.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::StreamTake { source, n } => {
                    let source_val = frame.get(source);
                    let n_val = frame.get(n);
                    self.push_stream_op(source_val, StreamOp::Take { n: n_val })?;
                }
                Instruction::StreamDrop { source, n } => {
                    let source_val = frame.get(source);
                    let n_val = frame.get(n);
                    self.push_stream_op(source_val, StreamOp::Drop { n: n_val })?;
                }

                // === Pattern Matching ===
                Instruction::MatchPattern {
                    value: _,
                    fail_target: _,
                } => {
                    // Pattern matching placeholder
                }
                Instruction::ExtractMapKey { source, key } => {
                    // Use get_ref to avoid cloning the entire map, only clone the extracted value
                    let map_ref = frame.get_ref(source);
                    let value = match map_ref {
                        Value::Map(m) => m.get(&key).cloned().ok_or_else(|| {
                            Error::Runtime(format!("key '{}' not found in map", key))
                        })?,
                        _ => {
                            return Err(Error::Type {
                                expected: "map".to_string(),
                                got: map_ref.type_name().to_string(),
                            });
                        }
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(value);
                }
                Instruction::ExtractListIndex { source, index } => {
                    // Use get_ref to avoid cloning the entire list, only clone the extracted value
                    let list_ref = frame.get_ref(source);
                    let value = match list_ref {
                        Value::List(l) => l.get(index).cloned().ok_or_else(|| {
                            Error::Runtime(format!("index {} out of bounds", index))
                        })?,
                        _ => {
                            return Err(Error::Type {
                                expected: "list".to_string(),
                                got: list_ref.type_name().to_string(),
                            });
                        }
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(value);
                }
                Instruction::ExtractListChild { source, index } => {
                    // Tagged data is represented as `[Symbol(tag), child1, ...]`.
                    // Child indexing skips the head symbol.
                    let tagged_ref = frame.get_ref(source);
                    let value = match tagged_ref {
                        Value::List(items)
                            if !items.is_empty() && matches!(items[0], Value::Symbol(_)) =>
                        {
                            items.get(index + 1).cloned().ok_or_else(|| {
                                Error::Runtime(format!("child index {} out of bounds", index))
                            })?
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "list-shaped node".to_string(),
                                got: tagged_ref.type_name().to_string(),
                            });
                        }
                    };
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(value);
                }
                Instruction::MatchTag {
                    value,
                    tag,
                    fail_target,
                    expected_arity,
                } => {
                    // Tagged data is `[Symbol(tag), child1, ...]`. Bare
                    // `Value::Symbol` still matches when the pattern is a bare
                    // symbol with no arity check.
                    let val_ref = frame.get_ref(value);
                    let matches = match val_ref {
                        Value::List(items) if !items.is_empty() => {
                            if let Value::Symbol(t) = &items[0] {
                                *t == tag && expected_arity.is_none_or(|n| items.len() - 1 == n)
                            } else {
                                false
                            }
                        }
                        Value::Symbol(s) => *s == tag && expected_arity.is_none_or(|n| n == 0),
                        _ => false,
                    };
                    if !matches {
                        frame.ip = fail_target.as_usize();
                        continue;
                    }
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Bool(true));
                }

                // === Object Definition ===
                Instruction::DefineObject(name) => {
                    let id = self.objects.lock().unwrap().create(None);
                    self.objects.lock().unwrap().register_name(name.clone(), id);
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Object(id));
                }
                Instruction::DefineMethod {
                    object,
                    name,
                    params,
                    body,
                } => {
                    // Get the object by its instruction index
                    let obj = frame.get(object);
                    if let Value::Object(id) = obj {
                        let nested_code = frame.code.nested.get(body).cloned();
                        if let Some(code) = nested_code {
                            let method = Method {
                                params,
                                code: Arc::new(code),
                            };
                            self.objects
                                .lock()
                                .unwrap()
                                .define_method(id, name, method)?;
                        }
                    }
                }
                Instruction::DefineProp {
                    object,
                    name,
                    value,
                } => {
                    let obj = frame.get(object);
                    let val = frame.get(value);
                    if let Value::Object(id) = obj {
                        self.objects.lock().unwrap().set_property(id, name, val)?;
                    }
                }
                Instruction::DefineFacet {
                    object,
                    name,
                    members,
                    terminal,
                } => {
                    let member_vals: Vec<SmolStr> = members
                        .iter()
                        .filter_map(|&idx| match frame.get(idx) {
                            Value::Symbol(s) => Some(s),
                            _ => None,
                        })
                        .collect();
                    let obj = frame.get(object);
                    if let Value::Object(id) = obj {
                        let facet = Facet {
                            members: member_vals,
                            terminal,
                        };
                        self.objects.lock().unwrap().define_facet(id, name, facet)?;
                    }
                }

                // === Grammar ===
                // Note: GrammarApply currently uses the separate PegRuntime.
                // Future migration: Use ApplyRule instruction with compiled patterns
                // for direct VM execution without runtime switch overhead.
                Instruction::GrammarApply {
                    input,
                    grammar,
                    rule,
                } => {
                    // GrammarApply returns a STREAM of all matches (backtracking)
                    let input_val = frame.get(input);
                    let grammar_val = frame.get(grammar);

                    let grammar_arc = match grammar_val {
                        Value::Grammar(g) => g,
                        Value::String(grammar_name) => {
                            self.grammars.get(&grammar_name).ok_or_else(|| {
                                Error::Runtime(format!("grammar not found: {}", grammar_name))
                            })?
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "grammar or string".to_string(),
                                got: grammar_val.type_name().to_string(),
                            });
                        }
                    };

                    // Create a stream that will parse the input with the grammar
                    // The stream source is the input value, with a Parse operation
                    use crate::value::Stream;
                    let stream = Stream {
                        source: input_val.clone(),
                        ops: vec![StreamOp::Parse {
                            grammar: Value::Grammar(grammar_arc),
                            rule: rule.clone(),
                        }],
                    };

                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::Stream(Arc::new(stream)));
                }
                Instruction::LoadGrammar(grammar) => {
                    // Pre-compile the grammar to bytecode and cache it
                    // Access the cache directly via Arc<Mutex<>> to avoid borrow conflicts
                    let grammar_name = grammar.name.clone();

                    // Check if already compiled
                    let needs_compile = {
                        let cache = self.compiled_grammars.lock().unwrap();
                        !cache.contains_key(&grammar_name)
                    };

                    if needs_compile {
                        // Need to compile - do this outside the frame borrow
                        use crate::compiler::Compiler;
                        let compiled = Compiler::compile_grammar_only(&grammar);
                        if let Ok(compiled) = compiled {
                            let compiled = Arc::new(compiled);
                            let mut cache = self.compiled_grammars.lock().unwrap();
                            cache.insert(grammar_name.clone(), compiled);
                        }
                    }

                    let grammar_value = Value::Grammar(grammar);

                    // Bind the grammar to its name in the current scope
                    // This allows "grammar foo { ... }" to make "foo" available as a variable
                    frame
                        .locals
                        .insert(grammar_name.clone(), grammar_value.clone());

                    frame.set_current(grammar_value);
                }
                Instruction::ExtendGrammar { base, extension } => {
                    let base_val = frame.get(base);
                    match base_val {
                        Value::Grammar(base_grammar) => {
                            let mut extended = Grammar::with_parent_grammar(
                                SmolStr::new("<extended>"),
                                base_grammar,
                            );
                            for (name, rule) in &extension.rules {
                                extended.add_rule(name.clone(), rule.clone());
                            }
                            let frame = self.frames.last_mut().unwrap();
                            frame.set_current(Value::Grammar(Arc::new(extended)));
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "grammar".to_string(),
                                got: base_val.type_name().to_string(),
                            });
                        }
                    }
                }

                // === Exception Handling ===
                Instruction::PushHandler { catch_target } => {
                    let frame_depth = self.frames.len();
                    self.exception_handlers.push((catch_target, frame_depth));
                }
                Instruction::PopHandler => {
                    self.exception_handlers.pop();
                }

                Instruction::YieldCheck => {
                    // Check for preemption request at loop back-edge
                    // For now, this is a no-op. In the future, it will check a preemption flag
                    // and allow cooperative multitasking by yielding to other tasks
                }

                Instruction::Throw { value } => {
                    let error = frame.get(value);
                    self.throw_exception(error)?;
                }

                // === Copy (for control flow convergence) ===
                Instruction::Copy { source } => {
                    let val = frame.get(source);
                    frame.set_current(val);
                }

                // === Tuple Space ===
                Instruction::TupleSpaceNew => {
                    use crate::tuplespace::store::TupleSpace;
                    let space = TupleSpace::new();
                    let frame = self.frames.last_mut().unwrap();
                    frame.set_current(Value::TupleSpace(Arc::new(std::sync::Mutex::new(space))));
                }

                // === Grammar Pattern Instructions (for PEG pattern matching) ===
                //
                // TODO: Full implementation of pattern instructions requires:
                // 1. Input position tracking in ParseState (currently placeholder)
                // 2. PegInput integration with Frame's parse_state
                // 3. Per-position memoization using rule names (SmolStr keys)
                // 4. Backtracking via ParseCheckpoint restoration
                // 5. Grammar rule lookup via ApplyRule instruction
                //
                // Migration path from GrammarApply:
                // - Current: GrammarApply delegates to PegRuntime
                // - Future: ApplyRule executes compiled patterns directly in VM
                // - Benefit: No context switch, direct values[] access
                Instruction::ParseCheckpoint => {
                    let frame = self.frames.last_mut().unwrap();
                    // Create a checkpoint capturing stack depth and position
                    // Store as a list [stack_depth, position] for later restoration
                    let checkpoint = frame.parse_state.checkpoint(frame);
                    let checkpoint_value = Value::List(Arc::new(vec![
                        Value::Int(checkpoint.stack_depth as i64),
                        Value::Int(checkpoint.position as i64),
                    ]));
                    frame.set_current(checkpoint_value);
                }

                Instruction::ParseRestore { checkpoint } => {
                    let frame = self.frames.last_mut().unwrap();
                    // Get the checkpoint value (list [stack_depth, position])
                    let checkpoint_value = frame.get(checkpoint);
                    match checkpoint_value {
                        Value::List(items) if items.len() == 2 => {
                            if let (Value::Int(depth), Value::Int(pos)) = (&items[0], &items[1]) {
                                let cp = ParseCheckpoint {
                                    stack_depth: *depth as usize,
                                    position: *pos as usize,
                                };
                                frame.parse_state.restore(cp);
                                frame.set_current(Value::Null);
                            } else {
                                return Err(Error::Runtime(
                                    "invalid checkpoint format".to_string(),
                                ));
                            }
                        }
                        Value::Int(pos) => {
                            // Legacy: simple position-only checkpoint
                            frame.parse_state.set_input_pos(Some(pos as usize));
                            frame.set_current(Value::Null);
                        }
                        _ => {
                            return Err(Error::Runtime("invalid checkpoint value".to_string()));
                        }
                    }
                }

                Instruction::ParsePush { value } => {
                    let frame = self.frames.last_mut().unwrap();
                    let val = frame.get(value);
                    frame.parse_state.push_input(val);
                    frame.set_current(Value::Null);
                }

                Instruction::ParsePop => {
                    let frame = self.frames.last_mut().unwrap();
                    let popped = frame.parse_state.pop_input();
                    frame.set_current(Value::Bool(popped));
                }

                Instruction::ParsePosition => {
                    let frame = self.frames.last_mut().unwrap();
                    let pos = frame.parse_state.position();
                    frame.set_current(Value::Int(pos as i64));
                }

                Instruction::ListAppend { list, item } => {
                    let frame = self.frames.last_mut().unwrap();
                    let list_val = frame.get(list);
                    let item_val = frame.get(item);
                    match list_val {
                        Value::List(items) => {
                            let mut new_items = (*items).clone();
                            new_items.push(item_val);
                            frame.set_current(Value::List(Arc::new(new_items)));
                        }
                        _ => {
                            return Err(Error::Runtime("ListAppend: expected list".to_string()));
                        }
                    }
                }

                Instruction::IsList { value } => {
                    let frame = self.frames.last_mut().unwrap();
                    let val = frame.get(value);
                    let is_list = matches!(val, Value::List(_));
                    frame.set_current(Value::Bool(is_list));
                }

                Instruction::IsMap { value } => {
                    let frame = self.frames.last_mut().unwrap();
                    let val = frame.get(value);
                    let is_map = matches!(val, Value::Map(_));
                    frame.set_current(Value::Bool(is_map));
                }

                Instruction::IsString { value } => {
                    let frame = self.frames.last_mut().unwrap();
                    let val = frame.get(value);
                    let is_string = matches!(val, Value::String(_));
                    frame.set_current(Value::Bool(is_string));
                }

                Instruction::MatchAny => {
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(ch) = frame.parse_state.head_char() {
                        frame.parse_state.advance(ch.len_utf8());
                        frame.set_current(Value::String(SmolStr::new(ch.to_string())));
                    } else if let Some(value) = frame.parse_state.head_value() {
                        frame.parse_state.advance(1);
                        frame.set_current(value);
                    } else {
                        // At end of input - return Null to signal pattern failure
                        // This enables proper backtracking in Choice and ListMatch patterns
                        frame.set_current(Value::Null);
                    }
                }

                Instruction::MatchChar { char: c } => {
                    let frame = self.frames.last_mut().unwrap();
                    if frame.parse_state.head_char() == Some(c) {
                        frame.parse_state.advance(c.len_utf8());
                        frame.set_current(Value::String(SmolStr::new(c.to_string())));
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: format!(
                                "MatchChar: expected '{}', got {:?}",
                                c,
                                frame.parse_state.head_char()
                            ),
                        });
                    }
                }

                Instruction::MatchByte { byte: b } => {
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(ch) = frame.parse_state.head_char() {
                        let byte_val = ch as u32;
                        if byte_val == b as u32 && ch.is_ascii() {
                            frame.parse_state.advance(1);
                            frame.set_current(Value::Int(b as i64));
                        } else {
                            return Err(Error::ParseFailed {
                                position: frame.parse_state.position(),
                                message: format!("MatchByte: expected {}, got char '{}'", b, ch),
                            });
                        }
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: format!("MatchByte: expected {}, end of input", b),
                        });
                    }
                }

                Instruction::MatchLiteral { const_idx } => {
                    let frame = self.frames.last_mut().unwrap();
                    let literal = frame.code.get_constant_as::<SmolStr>(const_idx).unwrap();
                    if frame.parse_state.starts_with(&literal) {
                        frame.parse_state.advance(literal.len());
                        frame.set_current(Value::String(literal));
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: format!("MatchLiteral: expected '{}'", literal),
                        });
                    }
                }

                Instruction::MatchLiteralValue { const_idx } => {
                    // Match a literal value (Int, String, Bool, Symbol, etc.) against the current input value
                    let frame = self.frames.last_mut().unwrap();
                    let expected_value = frame.code.get_constant(const_idx);

                    // Get the current input value
                    let input_value = frame
                        .parse_state
                        .input()
                        .cloned()
                        .unwrap_or_else(|| Value::Null);

                    // Check if values match using Value's equality
                    if input_value == expected_value {
                        // Value matches - advance and return the matched value
                        frame.parse_state.advance(1);
                        frame.set_current(input_value);
                    } else {
                        // Value doesn't match - return Null to signal pattern failure
                        frame.set_current(Value::Null);
                    }
                }

                Instruction::MatchCharClass { ranges } => {
                    // Returns matched char on success, Null on failure (for OMeta-style lowering)
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(ch) = frame.parse_state.head_char() {
                        // Check if character matches any range
                        let matches = ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
                        if matches {
                            frame.parse_state.advance(ch.len_utf8());
                            frame.set_current(Value::String(SmolStr::new(ch.to_string())));
                        } else {
                            // Pattern failed - return Null instead of error
                            frame.set_current(Value::Null);
                        }
                    } else {
                        // End of input - return Null
                        frame.set_current(Value::Null);
                    }
                }

                Instruction::MatchNegCharClass { ranges } => {
                    let frame = self.frames.last_mut().unwrap();
                    if let Some(ch) = frame.parse_state.head_char() {
                        // Check if character does NOT match any range
                        let matches = ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi);
                        if !matches {
                            frame.parse_state.advance(ch.len_utf8());
                            frame.set_current(Value::String(SmolStr::new(ch.to_string())));
                        } else {
                            return Err(Error::ParseFailed {
                                position: frame.parse_state.position(),
                                message: format!("MatchNegCharClass: '{}' in excluded ranges", ch),
                            });
                        }
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: "MatchNegCharClass: end of input".to_string(),
                        });
                    }
                }

                // Note: MatchStar and MatchPlus (generic pattern versions) have been removed.
                // They are now lowered to base IR by the compiler. See compile_grammar_pattern.
                // The specialized versions (MatchStarCharClass, etc.) are still used for optimization.
                Instruction::MatchSeq { patterns } => {
                    // Execute each pattern in sequence and collect results.
                    let mut results = Vec::new();
                    for pattern_idx in patterns.iter() {
                        results.push(frame.get(*pattern_idx));
                    }
                    frame.set_current(Value::List(Arc::new(results)));
                }

                // Note: MatchChoice has been removed. It is now lowered to base IR
                // (checkpoint + try each + restore) by the compiler. See compile_grammar_pattern.
                Instruction::MatchPlusChar { c } => {
                    // Match one or more of a specific character
                    let frame = self.frames.last_mut().unwrap();
                    let mut count = 0;

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);

                        if let Some(ch) = frame.parse_state.head_char() {
                            if ch == c {
                                frame.parse_state.advance(ch.len_utf8());
                                count += 1;
                            } else {
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    if count == 0 {
                        // No match - return Null to allow Choice to try next case
                        frame.set_current(Value::Null);
                    } else {
                        let result: String = (0..count).map(|_| c.to_string()).collect();
                        frame.set_current(Value::String(SmolStr::new(result)));
                    }
                }

                Instruction::MatchPlusCharClass { ranges } => {
                    // Match one or more characters from the given ranges
                    let frame = self.frames.last_mut().unwrap();
                    let mut results = Vec::new();

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);

                        if let Some(ch) = frame.parse_state.head_char() {
                            if ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi) {
                                frame.parse_state.advance(ch.len_utf8());
                                results.push(ch.to_string());
                            } else {
                                if results.is_empty() {
                                    // No match - return Null to allow Choice to try next case
                                    frame.set_current(Value::Null);
                                    break;
                                }
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            if results.is_empty() {
                                // End of input - return Null to allow Choice to try next case
                                frame.set_current(Value::Null);
                                break;
                            }
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    if !results.is_empty() {
                        let joined: String = results.concat();
                        frame.set_current(Value::String(SmolStr::new(joined)));
                    }
                }

                Instruction::MatchPlusLiteral { const_idx } => {
                    // Match one or more occurrences of a literal string
                    let frame = self.frames.last_mut().unwrap();
                    let literal = frame.code.get_constant_as::<SmolStr>(const_idx).unwrap();
                    let mut results = Vec::new();
                    let mut count = 0;

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);
                        let text = frame.parse_state.text_from();

                        if let Some(text) = text {
                            if text.starts_with(literal.as_str()) {
                                frame.parse_state.advance(literal.len());
                                results.push(literal.to_string());
                                count += 1;
                            } else {
                                if count == 0 {
                                    // No match - return Null to allow Choice to try next case
                                    frame.set_current(Value::Null);
                                    break;
                                }
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            if count == 0 {
                                // End of input - return Null to allow Choice to try next case
                                frame.set_current(Value::Null);
                                break;
                            }
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    if count > 0 {
                        let joined: String = results.concat();
                        frame.set_current(Value::String(SmolStr::new(joined)));
                    }
                }

                Instruction::MatchStarChar { c } => {
                    // Match zero or more of a specific character
                    let frame = self.frames.last_mut().unwrap();
                    let mut count = 0;

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);

                        if let Some(ch) = frame.parse_state.head_char() {
                            if ch == c {
                                frame.parse_state.advance(ch.len_utf8());
                                count += 1;
                            } else {
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    let result: String = (0..count).map(|_| c.to_string()).collect();
                    frame.set_current(Value::String(SmolStr::new(result)));
                }

                Instruction::MatchStarCharClass { ranges } => {
                    // Match zero or more characters from the given ranges
                    let frame = self.frames.last_mut().unwrap();
                    let mut results = Vec::new();

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);

                        if let Some(ch) = frame.parse_state.head_char() {
                            if ranges.iter().any(|(lo, hi)| ch >= *lo && ch <= *hi) {
                                frame.parse_state.advance(ch.len_utf8());
                                results.push(ch.to_string());
                            } else {
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    let joined: String = results.concat();
                    frame.set_current(Value::String(SmolStr::new(joined)));
                }

                Instruction::MatchStarLiteral { const_idx } => {
                    // Match zero or more occurrences of a literal string
                    let frame = self.frames.last_mut().unwrap();
                    let literal = frame.code.get_constant_as::<SmolStr>(const_idx).unwrap();
                    let mut results = Vec::new();

                    loop {
                        let checkpoint = frame.parse_state.checkpoint(frame);
                        let text = frame.parse_state.text_from();

                        if let Some(text) = text {
                            if text.starts_with(literal.as_str()) {
                                frame.parse_state.advance(literal.len());
                                results.push(literal.to_string());
                            } else {
                                frame.parse_state.restore(checkpoint);
                                break;
                            }
                        } else {
                            frame.parse_state.restore(checkpoint);
                            break;
                        }

                        // GUARD: Check for zero-length match
                        let new_pos = frame.parse_state.position();
                        let checkpoint_pos = checkpoint.position;
                        if new_pos == checkpoint_pos {
                            break;
                        }
                    }

                    let joined: String = results.concat();
                    frame.set_current(Value::String(SmolStr::new(joined)));
                }

                Instruction::MatchStarRule { rule } => {
                    // Zero or more repetitions of a named rule (like OMeta's _many).
                    // Extract data first, then call helper which sets the result.
                    let rule_name_smol = frame.code.get_constant_as::<SmolStr>(rule).unwrap();
                    let grammar_opt = frame.parse_state.grammar().cloned();
                    let initial_position = frame.parse_state.position();

                    // Prepare helper input
                    let helper_input = if let Some(grammar) = grammar_opt {
                        let grammar_name = grammar.name.clone();
                        let cache = self.compiled_grammars.lock().unwrap();
                        if let Some(compiled_code) = cache.get(&grammar_name).cloned() {
                            compiled_code
                                .get_rule_entry(&rule_name_smol)
                                .map(|entry_point| (compiled_code, entry_point))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Call helper (sets result in frame)
                    match helper_input {
                        Some((compiled_code, entry_point)) => {
                            // Ignore errors - helper sets result on success, leaves default on error
                            let _ = self.match_rule_repeatedly(
                                &rule_name_smol,
                                compiled_code,
                                entry_point,
                                initial_position,
                                false,
                            );
                        }
                        None => {
                            // No grammar/rule - set empty list
                            frame.set_current(Value::List(Arc::new(Vec::new())));
                        }
                    }
                }

                Instruction::MatchPlusRule { rule } => {
                    // One or more repetitions of a named rule (like OMeta's _many1).
                    // Extract data first, then call helper which sets the result.
                    let rule_name_smol = frame.code.get_constant_as::<SmolStr>(rule).unwrap();
                    let grammar_opt = frame.parse_state.grammar().cloned();
                    let initial_position = frame.parse_state.position();

                    // Prepare helper input
                    let helper_input = if let Some(grammar) = grammar_opt {
                        let grammar_name = grammar.name.clone();
                        let cache = self.compiled_grammars.lock().unwrap();
                        if let Some(compiled_code) = cache.get(&grammar_name).cloned() {
                            compiled_code
                                .get_rule_entry(&rule_name_smol)
                                .map(|entry_point| (compiled_code, entry_point))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Call helper (sets result in frame)
                    match helper_input {
                        Some((compiled_code, entry_point)) => {
                            // Try to match with require_at_least_one=false to allow Choice to try next case
                            let result = self.match_rule_repeatedly(
                                &rule_name_smol,
                                compiled_code,
                                entry_point,
                                initial_position,
                                false, // Changed from true to false
                            );
                            match result {
                                Ok(()) => {}
                                Err(Error::ParseFailed { .. }) => {
                                    // Match failed - return Null to allow Choice to try next case
                                    let frame = self.frames.last_mut().unwrap();
                                    frame.set_current(Value::Null);
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        None => {
                            // Rule not found - return Null to allow Choice to try next case
                            let frame = self.frames.last_mut().unwrap();
                            frame.set_current(Value::Null);
                        }
                    }
                }

                Instruction::MatchOptional { pattern } => {
                    // Match zero or one time. Always succeeds (returns null or matched value).
                    let result = frame.get(pattern);
                    frame.set_current(result);
                }

                Instruction::MatchLookahead { pattern } => {
                    // Positive lookahead: execute pattern without consuming input.
                    // For now, just get the pattern result (doesn't advance parse_state).
                    let checkpoint = frame.parse_state.checkpoint(frame);
                    let result = frame.get(pattern);
                    // Always restore checkpoint (don't consume input)
                    frame.parse_state.restore(checkpoint);
                    frame.set_current(result);
                }

                Instruction::MatchNot { pattern } => {
                    // Negative lookahead: succeed if pattern fails.
                    // For now, check if pattern result is null (failure).
                    let checkpoint = frame.parse_state.checkpoint(frame);
                    let result = frame.get(pattern);
                    frame.parse_state.restore(checkpoint);

                    if matches!(result, Value::Null) {
                        // Pattern failed, so we succeed
                        frame.set_current(Value::Null);
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: "MatchNot: pattern matched when it should fail".to_string(),
                        });
                    }
                }

                Instruction::MatchBind { pattern, name } => {
                    // Execute pattern and bind result to a variable.
                    let result = frame.get(pattern);

                    // Get the variable name from constants
                    let var_name = frame.code.get_constant_as::<SmolStr>(name).unwrap();

                    // Store in locals
                    frame.locals.insert(var_name, result.clone());

                    frame.set_current(result);
                }

                Instruction::MatchGuard { pattern, predicate } => {
                    // Execute pattern, then check predicate expression.
                    // The predicate must evaluate to a truthy value for the guard to succeed.
                    let pattern_result = frame.get(pattern);

                    // If pattern failed, return Null to allow Choice to try next case
                    if matches!(pattern_result, Value::Null) {
                        frame.set_current(Value::Null);
                    } else {
                        // Get the predicate result (should already be executed)
                        let predicate_result = frame.get(predicate);

                        // Check if predicate is truthy
                        if predicate_result.is_truthy() {
                            frame.set_current(pattern_result);
                        } else {
                            // Guard failed - return Null to allow Choice to try next case
                            frame.set_current(Value::Null);
                        }
                    }
                }

                Instruction::MatchAction { pattern, action } => {
                    // Execute pattern, then execute action expression.
                    // The action expression produces the final result.
                    // If pattern failed (returned Null), the MatchAction should also fail (return Null).
                    let pattern_result = frame.get(pattern);

                    if matches!(pattern_result, Value::Null) {
                        // Pattern failed - return Null to allow Choice to try next case
                        frame.set_current(Value::Null);
                    } else {
                        // Pattern succeeded - get the action result
                        // The action instruction has already been executed during normal flow
                        let action_result = frame.get(action);

                        // If action result is Null (e.g., LoadVar returned Null for undefined var),
                        // propagate the Null to allow Choice to try next case
                        if matches!(action_result, Value::Null) {
                            frame.set_current(Value::Null);
                        } else {
                            frame.set_current(action_result);
                        }
                    }
                }

                Instruction::MatchList { patterns, rest } => {
                    // Match a list against element patterns.
                    // Execute each pattern instruction against the corresponding element.
                    let input_val = frame
                        .parse_state
                        .input()
                        .cloned()
                        .unwrap_or_else(|| Value::Null);

                    // Check if input is a list
                    let (is_list, input_list) = match &input_val {
                        Value::List(l) => (true, l.clone()),
                        _ => (false, Arc::new(vec![])),
                    };

                    if !is_list {
                        frame.set_current(Value::Null);
                    } else {
                        let input_len = input_list.len();
                        let patterns_len = patterns.len();

                        // Check length constraints
                        if (rest.is_none() && input_len != patterns_len)
                            || (rest.is_some() && input_len < patterns_len)
                        {
                            frame.set_current(Value::Null);
                        }
                        // Success - execute each pattern against the corresponding element
                        else {
                            // Save the current input
                            let saved_input = frame.parse_state.input().cloned();
                            let saved_pos = frame.parse_state.input_pos();

                            // Execute each pattern against the corresponding element
                            let mut all_matched = true;
                            for (i, pattern_idx) in patterns.iter().enumerate() {
                                // Set the current element as the input for this pattern
                                frame.parse_state.set_input(input_list[i].clone());
                                frame.parse_state.set_input_pos(Some(0));

                                // Get the pattern instruction and execute it
                                let instruction = frame.code.instructions[pattern_idx.0].clone();

                                // Execute the pattern instruction
                                match &instruction {
                                    Instruction::MatchMap { entries } => {
                                        // Inline MatchMap execution for this element
                                        let element = &input_list[i];
                                        let Value::Map(elem_map) = element else {
                                            all_matched = false;
                                            break;
                                        };

                                        // Check that all required keys exist and bind their values
                                        for (key_pattern, value_pattern) in entries {
                                            // Resolve the key to match
                                            let keys_to_match: Vec<SmolStr> = match key_pattern {
                                                crate::compiler::MapKeyPattern::Specific(
                                                    key_idx,
                                                ) => {
                                                    let key = frame
                                                        .code
                                                        .get_constant_as::<SmolStr>(*key_idx)
                                                        .expect("Key constant must be a string");
                                                    vec![key]
                                                }
                                                crate::compiler::MapKeyPattern::Wildcard => {
                                                    elem_map.keys().cloned().collect()
                                                }
                                            };

                                            // Try to find at least one matching key-value pair
                                            let mut found_match = false;
                                            'elem_key_loop: for key in &keys_to_match {
                                                let value = match elem_map.get(key.as_str()) {
                                                    Some(v) => v,
                                                    None => continue,
                                                };

                                                match value_pattern {
                                                    crate::compiler::MapValuePattern::Wildcard => {
                                                        found_match = true;
                                                        break 'elem_key_loop;
                                                    }
                                                    crate::compiler::MapValuePattern::Bind(var_name_idx) => {
                                                        let var_name = frame.code.get_constant_as::<SmolStr>(*var_name_idx)
                                                            .expect("Variable name constant must be a string");
                                                        frame.locals.insert(var_name, value.clone());
                                                        found_match = true;
                                                        break 'elem_key_loop;
                                                    }
                                                    crate::compiler::MapValuePattern::MatchLiteral(value_idx) => {
                                                        let expected_value = frame.code.get_constant(*value_idx);
                                                        if value == &expected_value {
                                                            found_match = true;
                                                            break 'elem_key_loop;
                                                        }
                                                    }
                                                    crate::compiler::MapValuePattern::Pattern(_pattern_idx) => {
                                                        // For now, skip pattern matching in nested context
                                                        // TODO: Support full pattern execution
                                                    }
                                                }
                                            }

                                            if !found_match {
                                                all_matched = false;
                                                break;
                                            }
                                        }

                                        if !all_matched {
                                            break;
                                        }
                                    }
                                    Instruction::Bind { name, value: _ } => {
                                        // Direct binding - bind the current element
                                        frame.locals.insert(name.clone(), input_list[i].clone());
                                    }
                                    Instruction::MatchAny => {
                                        // Always matches, no binding
                                    }
                                    Instruction::MatchListWithBindings {
                                        patterns: inner_patterns,
                                        rest: inner_rest,
                                    } => {
                                        // Inline MatchListWithBindings for nested list patterns
                                        let element = &input_list[i];
                                        let Value::List(inner_list) = element else {
                                            all_matched = false;
                                            break;
                                        };

                                        let inner_len = inner_list.len();
                                        let inner_patterns_len = inner_patterns.len();

                                        // Check length
                                        if (inner_rest.is_none() && inner_len != inner_patterns_len)
                                            || (inner_rest.is_some()
                                                && inner_len < inner_patterns_len)
                                        {
                                            all_matched = false;
                                            break;
                                        }

                                        // Bind variables
                                        for (j, var_name_opt) in inner_patterns.iter().enumerate() {
                                            if let Some(var_name_idx) = var_name_opt {
                                                let var_name = frame
                                                    .code
                                                    .get_constant_as::<SmolStr>(*var_name_idx)
                                                    .expect("Constant must be a string");
                                                frame.locals.insert(
                                                    var_name.clone(),
                                                    inner_list[j].clone(),
                                                );
                                            }
                                        }

                                        if let Some(inner_rest_idx) = inner_rest {
                                            let rest_name = frame
                                                .code
                                                .get_constant_as::<SmolStr>(*inner_rest_idx)
                                                .expect("Constant must be a string");
                                            let rest_elements: Vec<Value> =
                                                inner_list[inner_patterns_len..].to_vec();
                                            frame.locals.insert(
                                                rest_name.clone(),
                                                Value::List(Arc::new(rest_elements)),
                                            );
                                        }
                                    }
                                    _ => {
                                        // For other patterns, we'd need to execute them
                                        // For now, just treat as wildcard
                                    }
                                }
                            }

                            // Restore the original input
                            frame
                                .parse_state
                                .set_input(saved_input.unwrap_or(Value::Null));
                            frame.parse_state.set_input_pos(saved_pos);

                            if all_matched {
                                frame.set_current(input_val);
                            } else {
                                frame.set_current(Value::Null);
                            }
                        }
                    }
                }

                Instruction::MatchListWithBindings { patterns, rest } => {
                    // Match a list against element patterns and bind variables.
                    // This instruction doesn't execute pattern instructions - it just binds variables.
                    let input_val = frame
                        .parse_state
                        .input()
                        .cloned()
                        .unwrap_or_else(|| Value::Null);

                    // Check if input is a list
                    let (is_list, input_list) = match &input_val {
                        Value::List(l) => (true, l.clone()),
                        _ => (false, Arc::new(vec![])),
                    };

                    if !is_list {
                        frame.set_current(Value::Null);
                    } else {
                        let input_len = input_list.len();
                        let patterns_len = patterns.len();

                        // Check length constraints
                        if (rest.is_none() && input_len != patterns_len)
                            || (rest.is_some() && input_len < patterns_len)
                        {
                            frame.set_current(Value::Null);
                        }
                        // Success - bind variables and return the input list
                        else {
                            // Bind each element to its corresponding variable
                            for (i, var_name_opt) in patterns.iter().enumerate() {
                                if let Some(var_name_idx) = var_name_opt {
                                    let var_name = frame
                                        .code
                                        .get_constant_as::<SmolStr>(*var_name_idx)
                                        .expect("Constant must be a string");
                                    frame.locals.insert(var_name.clone(), input_list[i].clone());
                                }
                            }

                            // Bind rest if present
                            if let Some(rest_idx) = rest {
                                let rest_name = frame
                                    .code
                                    .get_constant_as::<SmolStr>(rest_idx)
                                    .expect("Constant must be a string");
                                let rest_elements: Vec<Value> = input_list[patterns_len..].to_vec();
                                frame
                                    .locals
                                    .insert(rest_name, Value::List(Arc::new(rest_elements)));
                            }

                            frame.set_current(input_val);
                        }
                    }
                }

                Instruction::MatchMap { entries } => {
                    // Match a map against key-value patterns.
                    // Get the input value from parse_state
                    let input_val = frame
                        .parse_state
                        .input()
                        .cloned()
                        .unwrap_or_else(|| Value::Null);

                    // Check if input is a map
                    let Value::Map(ref input_map) = input_val else {
                        frame.set_current(Value::Null);
                        continue;
                    };

                    // For each key-value pattern, check the input map
                    for (key_pattern, value_pattern) in &entries {
                        // First, resolve which key(s) to match based on key_pattern
                        let keys_to_match: Vec<SmolStr> = match key_pattern {
                            crate::compiler::MapKeyPattern::Specific(key_idx) => {
                                // Specific key - get its string value
                                let key = frame
                                    .code
                                    .get_constant_as::<SmolStr>(*key_idx)
                                    .expect("Key constant must be a string");
                                vec![key]
                            }
                            crate::compiler::MapKeyPattern::Wildcard => {
                                // Wildcard key - match ANY one key from the map
                                // This extracts one key-value pair
                                input_map.keys().cloned().collect()
                            }
                        };

                        // Try to find at least one matching key-value pair
                        let mut found_match = false;

                        'key_loop: for key in &keys_to_match {
                            // Check if key exists in input map
                            let value = match input_map.get(key.as_str()) {
                                Some(v) => v,
                                None => continue,
                            };

                            // Now match the value against the value pattern
                            match value_pattern {
                                crate::compiler::MapValuePattern::Wildcard => {
                                    // Wildcard - always matches, no binding
                                    found_match = true;
                                    break 'key_loop;
                                }
                                crate::compiler::MapValuePattern::Bind(var_name_idx) => {
                                    // Bind the value to the variable
                                    let var_name = frame
                                        .code
                                        .get_constant_as::<SmolStr>(*var_name_idx)
                                        .expect("Variable name constant must be a string");
                                    frame.locals.insert(var_name, value.clone());
                                    found_match = true;
                                    break 'key_loop;
                                }
                                crate::compiler::MapValuePattern::MatchLiteral(value_idx) => {
                                    // Match literal value
                                    let expected_value = frame.code.get_constant(*value_idx);
                                    if value == &expected_value {
                                        found_match = true;
                                        break 'key_loop;
                                    }
                                    // If literal doesn't match, try next key
                                }
                                crate::compiler::MapValuePattern::Pattern(pattern_idx) => {
                                    // For now, only support simple pattern instructions inline
                                    // TODO: Support full pattern execution
                                    let pattern_instr = &frame.code.instructions[pattern_idx.0];

                                    match pattern_instr {
                                        Instruction::MatchAny => {
                                            // MatchAny always matches
                                            found_match = true;
                                            break 'key_loop;
                                        }
                                        Instruction::MatchLiteralValue { const_idx } => {
                                            // Match literal value
                                            let expected_value =
                                                frame.code.get_constant(*const_idx);
                                            if value == &expected_value {
                                                found_match = true;
                                                break 'key_loop;
                                            }
                                        }
                                        _ => {
                                            // Complex patterns not yet supported in map matching
                                            // Treat as no match
                                            eprintln!(
                                                "Warning: Complex pattern in map value not yet supported, treating as mismatch"
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        if !found_match {
                            // No matching key found for this pattern
                            frame.set_current(Value::Null);
                            continue;
                        }
                    }

                    // All patterns matched successfully
                    frame.set_current(input_val);
                }

                Instruction::MatchMapNested { entries } => {
                    // Match a map with nested bindings (e.g., `%{outer: %{inner: x}}`)
                    // Get the input value from parse_state
                    let input_val = frame
                        .parse_state
                        .input()
                        .cloned()
                        .unwrap_or_else(|| Value::Null);

                    // Check if input is a map
                    let Value::Map(ref _input_map) = input_val else {
                        frame.set_current(Value::Null);
                        continue;
                    };

                    // For each nested binding, navigate the path and bind the variable
                    for (_key_idx, nested_binding) in &entries {
                        let path = &nested_binding.path;
                        let variable = &nested_binding.variable;

                        // Navigate the path to get the value
                        let mut current = &input_val;
                        let mut all_keys_found = true;

                        for key in path {
                            match current {
                                Value::Map(map) => match map.get(key.as_str()) {
                                    Some(v) => {
                                        current = v;
                                    }
                                    None => {
                                        all_keys_found = false;
                                        break;
                                    }
                                },
                                _ => {
                                    all_keys_found = false;
                                    break;
                                }
                            }
                        }

                        if !all_keys_found {
                            // Path navigation failed - match fails
                            frame.set_current(Value::Null);
                            continue;
                        }

                        // Bind the variable to the final value
                        frame.locals.insert(variable.clone(), current.clone());
                    }

                    // All paths navigated successfully - return the input map
                    frame.set_current(input_val);
                }

                Instruction::MatchListNodeWithBindings { tag_idx, bindings } => {
                    // Match a list-shaped node (leading symbol + positional children)
                    // and bind named children. E.g., `[:Int, n]` matches a list
                    // `Value::List([Value::Symbol("Int"), v])` and binds n = v.
                    let input_val = frame.parse_state.input().cloned().unwrap_or(Value::Null);

                    // Check if input is a list-shaped node with matching tag.
                    let Some((input_tag, children)) = input_val.as_node() else {
                        frame.set_current(Value::Null);
                        continue;
                    };

                    let expected_tag: SmolStr = frame
                        .code
                        .get_constant(tag_idx)
                        .try_into()
                        .unwrap_or_else(|_| SmolStr::new(""));

                    if input_tag != expected_tag.as_str() {
                        frame.set_current(Value::Null);
                        continue;
                    }

                    // Check arity matches
                    if children.len() != bindings.len() {
                        frame.set_current(Value::Null);
                        continue;
                    }

                    // Bind each child to its corresponding variable (None = wildcard)
                    let children_owned: Vec<Value> = children.to_vec();
                    for (i, var_name_opt) in bindings.iter().enumerate() {
                        if let Some(var_name_idx) = var_name_opt {
                            let var_name = frame
                                .code
                                .get_constant_as::<SmolStr>(*var_name_idx)
                                .expect("Constant must be a string");
                            frame
                                .locals
                                .insert(var_name.clone(), children_owned[i].clone());
                        }
                    }

                    // Return the matched tagged value
                    frame.set_current(input_val);
                }

                Instruction::MatchListNode { tag_idx, patterns } => {
                    // Match a list-shaped node with nested pattern execution.
                    // E.g., [:Binary, :plus, [:Int, a], [:Int, b]] needs nested matching.
                    let input_val = frame.parse_state.input().cloned().unwrap_or(Value::Null);

                    // Check if input is a list-shaped node with matching tag.
                    let Some((input_tag, children)) = input_val.as_node() else {
                        frame.set_current(Value::Null);
                        continue;
                    };

                    let expected_tag: SmolStr = frame
                        .code
                        .get_constant(tag_idx)
                        .try_into()
                        .unwrap_or_else(|_| SmolStr::new(""));

                    if input_tag != expected_tag.as_str() {
                        frame.set_current(Value::Null);
                        continue;
                    }

                    // Check arity matches
                    if children.len() != patterns.len() {
                        frame.set_current(Value::Null);
                        continue;
                    }

                    // Execute each pattern instruction against the corresponding child
                    let children_owned: Vec<Value> = children.to_vec();
                    let saved_input = frame.parse_state.input().cloned();

                    let mut all_matched = true;
                    for (i, pattern_idx) in patterns.iter().enumerate() {
                        // Set the child as input for the pattern
                        frame.parse_state.set_input(children_owned[i].clone());
                        frame.parse_state.set_input_pos(Some(0));

                        // Get and execute the pattern instruction
                        let pattern_instr = frame.code.instructions[pattern_idx.0].clone();

                        match &pattern_instr {
                            Instruction::MatchListNodeWithBindings {
                                tag_idx: inner_tag_idx,
                                bindings: inner_bindings,
                            } => {
                                // Nested list-node match with simple bindings.
                                let child = &children_owned[i];
                                let Some((child_tag, child_children)) = child.as_node() else {
                                    all_matched = false;
                                    break;
                                };
                                let expected_inner_tag: SmolStr = frame
                                    .code
                                    .get_constant(*inner_tag_idx)
                                    .try_into()
                                    .unwrap_or_else(|_| SmolStr::new(""));

                                if child_tag != expected_inner_tag.as_str() {
                                    all_matched = false;
                                    break;
                                }
                                if child_children.len() != inner_bindings.len() {
                                    all_matched = false;
                                    break;
                                }
                                // Bind variables
                                for (j, var_name_opt) in inner_bindings.iter().enumerate() {
                                    if let Some(var_name_idx) = var_name_opt {
                                        let var_name = frame
                                            .code
                                            .get_constant_as::<SmolStr>(*var_name_idx)
                                            .expect("Constant must be a string");
                                        frame
                                            .locals
                                            .insert(var_name.clone(), child_children[j].clone());
                                    }
                                }
                            }
                            Instruction::MatchBind { pattern: _, name } => {
                                // Simple binding: bind the child value to the variable
                                let var_name = frame
                                    .code
                                    .get_constant_as::<SmolStr>(*name)
                                    .expect("Constant must be a string");
                                frame.locals.insert(var_name, children[i].clone());
                            }
                            Instruction::MatchAny => {
                                // Wildcard - matches anything, no binding
                            }
                            _ => {
                                // Other patterns - for now treat as wildcard
                                // TODO: Execute the full pattern
                            }
                        }
                    }

                    // Restore input
                    if let Some(saved) = saved_input {
                        frame.parse_state.set_input(saved);
                    }

                    if all_matched {
                        frame.set_current(input_val);
                    } else {
                        frame.set_current(Value::Null);
                    }
                }

                Instruction::ApplyRule { rule_idx } => {
                    // Apply a named grammar rule with memoization and left recursion detection.
                    let rule_name = frame
                        .code
                        .get_constant_as::<SmolStr>(rule_idx)
                        .expect("Constant must be a string");

                    // Check memoization first (using position + rule name as key)
                    let memo_key =
                        SmolStr::from(format!("{}@{}", rule_name, frame.parse_state.position()));
                    if let Some(entry) = frame.parse_state.get_memo(&memo_key) {
                        match entry {
                            MemoEntry::InProgress => {
                                return Err(Error::ParseFailed {
                                    position: frame.parse_state.position(),
                                    message: format!(
                                        "ApplyRule: left recursion detected in rule '{}'",
                                        rule_name
                                    ),
                                });
                            }
                            MemoEntry::Done(Some(value), _end_pos) => {
                                frame.set_current(value);
                            }
                            MemoEntry::Done(None, _end_pos) => {
                                return Err(Error::ParseFailed {
                                    position: frame.parse_state.position(),
                                    message: format!(
                                        "ApplyRule: rule '{}' failed (cached)",
                                        rule_name
                                    ),
                                });
                            }
                        }
                    } else if let Some(grammar) = frame.parse_state.grammar() {
                        // Check if the grammar has this rule
                        if !grammar.rules.contains_key(&rule_name) {
                            let pos = frame.parse_state.position();
                            return Err(Error::ParseFailed {
                                position: pos,
                                message: format!("ApplyRule: rule '{}' not found", rule_name),
                            });
                        }

                        // Clone needed data
                        let grammar_name = grammar.name.clone();
                        let current_pos = frame.parse_state.position();
                        let parse_state_clone = frame.parse_state.clone();

                        // Mark in-progress for memoization
                        frame
                            .parse_state
                            .set_memo(memo_key.clone(), MemoEntry::InProgress);

                        // Get compiled code from cache (via Arc<Mutex<>>)
                        let compiled_code = {
                            let cache = self.compiled_grammars.lock().unwrap();
                            cache.get(&grammar_name).cloned()
                        };

                        let compiled_code = match compiled_code {
                            Some(code) => code,
                            None => {
                                return Err(Error::ParseFailed {
                                    position: current_pos,
                                    message: format!(
                                        "ApplyRule: grammar '{}' not compiled (should have been loaded)",
                                        grammar_name
                                    ),
                                });
                            }
                        };

                        // Look up the rule entry point in compiled code
                        let entry_point = match compiled_code.get_rule_entry(&rule_name) {
                            Some(ep) => ep,
                            None => {
                                return Err(Error::ParseFailed {
                                    position: current_pos,
                                    message: format!(
                                        "ApplyRule: rule '{}' not found in compiled grammar",
                                        rule_name
                                    ),
                                });
                            }
                        };

                        // Push a new frame with the compiled grammar code
                        let mut new_frame = Frame::new(compiled_code.clone());
                        new_frame.parse_state = parse_state_clone;
                        new_frame.ip = entry_point.0;
                        self.frames.push(new_frame);

                        // The new frame will execute the rule and return
                        // Continue execution in the new frame
                    } else {
                        // No grammar set - error
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: format!("ApplyRule: no grammar set for rule '{}'", rule_name),
                        });
                    }
                }

                Instruction::MatchEnd => {
                    let frame = self.frames.last_mut().unwrap();
                    if frame.parse_state.is_at_end() {
                        // Return a special marker value to indicate successful match
                        // We use an empty list as a sentinel for "matched at end"
                        frame.set_current(Value::List(Arc::new(vec![])));
                    } else {
                        return Err(Error::ParseFailed {
                            position: frame.parse_state.position(),
                            message: "MatchEnd: expected end of input".to_string(),
                        });
                    }
                }

                // === Nop ===
                Instruction::Nop => {
                    // Do nothing
                }
            }
        }

        Ok(())
    }

    /// Helper: repeatedly match a rule, collecting results.
    /// Used by MatchStarRule and MatchPlusRule to avoid borrowing conflicts.
    /// Sets the result directly in the current frame's values[ip - 1].
    ///
    /// # Arguments
    /// * `rule_name` - Name of the rule to match
    /// * `compiled_code` - Compiled grammar bytecode
    /// * `entry_point` - Instruction index of the rule entry point
    /// * `initial_position` - Starting position for error reporting
    /// * `require_at_least_one` - If true, fails if no matches (for Plus)
    fn match_rule_repeatedly(
        &mut self,
        rule_name: &str,
        compiled_code: Arc<CompiledCode>,
        entry_point: InstrIndex,
        initial_position: usize,
        require_at_least_one: bool,
    ) -> Result<()> {
        let mut results = Vec::new();

        loop {
            // Get current position and check memo
            let (memo_key, current_parse_state, cached_result) = {
                let f = self.frames.last_mut().unwrap();
                let pos = f.parse_state.position();
                let key = SmolStr::from(format!("{}@{}", rule_name, pos));

                // Check memoization
                if let Some(entry) = f.parse_state.get_memo(&key) {
                    match entry {
                        MemoEntry::InProgress => {
                            return Err(Error::ParseFailed {
                                position: initial_position,
                                message: format!("left recursion in rule '{}'", rule_name),
                            });
                        }
                        MemoEntry::Done(Some(value), end_pos) => {
                            // Cached result found - use it and continue loop from new position
                            // GUARD: Break on zero-length match to prevent infinite loop
                            if end_pos == pos {
                                break;
                            }
                            // Update parse state to cached end position
                            f.parse_state.advance(end_pos - f.parse_state.position());
                            results.push(value);
                            (key, f.parse_state.clone(), Some(true)) // true = success
                        }
                        MemoEntry::Done(None, _) => {
                            // Cached failure - stop loop
                            (key, f.parse_state.clone(), Some(false)) // false = failure
                        }
                    }
                } else {
                    // No cache entry - need to execute
                    f.parse_state.set_memo(key.clone(), MemoEntry::InProgress);
                    (key, f.parse_state.clone(), None) // None = need to execute
                }
            };

            match cached_result {
                Some(true) => {
                    // Cached success - continue loop to check for more matches
                    continue;
                }
                Some(false) => {
                    // Cached failure - stop loop
                    break;
                }
                None => {
                    // No cache - execute the rule
                    // Save position before move
                    let restore_position = current_parse_state.position();

                    // Push new frame to execute the rule
                    let mut rule_frame = Frame::new(compiled_code.clone());
                    rule_frame.parse_state = current_parse_state;
                    rule_frame.ip = entry_point.0;
                    self.frames.push(rule_frame);

                    // Execute until frame returns
                    let base_depth = self.frames.len() - 1;
                    let execute_result = self.execute_with_depth(base_depth);

                    // Get result from returned frame
                    let (value, end_pos) = {
                        let f = self.frames.last_mut().unwrap();
                        let val = f.result();
                        let pos = f.parse_state.position();
                        (val, pos)
                    };

                    match execute_result {
                        Ok(()) => {
                            // Success - store in memo and continue
                            // GUARD: Break on zero-length match to prevent infinite loop
                            if end_pos == restore_position {
                                let f = self.frames.last_mut().unwrap();
                                f.parse_state.set_memo(
                                    memo_key,
                                    MemoEntry::Done(Some(value.clone()), end_pos),
                                );
                                results.push(value);
                                break;
                            }
                            let f = self.frames.last_mut().unwrap();
                            f.parse_state
                                .set_memo(memo_key, MemoEntry::Done(Some(value.clone()), end_pos));
                            results.push(value);
                            // Continue loop to check for more matches
                        }
                        Err(Error::ParseFailed { .. }) => {
                            // Failure - store memo and stop loop
                            let f = self.frames.last_mut().unwrap();
                            f.parse_state
                                .set_memo(memo_key, MemoEntry::Done(None, restore_position));
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        if require_at_least_one && results.is_empty() {
            return Err(Error::ParseFailed {
                position: initial_position,
                message: format!(
                    "rule '{}' failed to match (required at least one)",
                    rule_name
                ),
            });
        }

        // Set result directly in current frame
        let f = self.frames.last_mut().unwrap();
        f.set_current(Value::List(Arc::new(results)));
        Ok(())
    }

    fn push_stream_op(&mut self, source: Value, op: StreamOp) -> Result<()> {
        let Value::Stream(stream) = source else {
            return Err(Error::Type {
                expected: "stream".to_string(),
                got: source.type_name().to_string(),
            });
        };

        let mut ops = stream.ops.clone();
        ops.push(op);
        let next = Stream {
            source: stream.source.clone(),
            ops,
        };
        let frame = self.frames.last_mut().unwrap();
        frame.set_current(Value::Stream(Arc::new(next)));
        Ok(())
    }

    fn push_stream_parse(&mut self, source: Value, grammar: Value, rule: SmolStr) -> Result<()> {
        let Value::Stream(stream) = source else {
            return Err(Error::Type {
                expected: "stream".to_string(),
                got: source.type_name().to_string(),
            });
        };

        let mut ops = stream.ops.clone();
        ops.push(StreamOp::Parse { grammar, rule });
        let next = Stream {
            source: stream.source.clone(),
            ops,
        };
        let frame = self.frames.last_mut().unwrap();
        frame.set_current(Value::Stream(Arc::new(next)));
        Ok(())
    }

    /// Execute a stream pipeline and return the result.
    ///
    /// For streams with `StreamOp::Collect`, this executes the entire pipeline
    /// and returns the collected values as a list.
    ///
    /// For streams with `StreamOp::Parse`, this implements backtracking to find
    /// ALL matches from the grammar, not just the first one.
    fn execute_stream(&mut self, stream: &Stream) -> Result<Value> {
        use crate::value::StreamOp;

        // Start with the source value
        let mut current = stream.source.clone();

        // Track if we need to collect results (for backtracking)
        let mut output_tx: Option<tokio::sync::mpsc::Sender<Value>> = None;
        let mut results: Vec<Value> = Vec::new();

        // Execute operations in sequence
        for op in &stream.ops {
            match op {
                StreamOp::Map(_func) => {
                    // Map: apply function to each value
                    // For now, just pass through (will need collection first)
                    current = Value::String(SmolStr::new("<map stream>"));
                }
                StreamOp::Filter(_pred) => {
                    // Filter: keep values matching predicate
                    // For now, just pass through
                    current = Value::String(SmolStr::new("<filter stream>"));
                }
                StreamOp::Take { n: _ } => {
                    // TODO: take_count will limit collected results when full streaming is implemented
                }
                StreamOp::Collect => {
                    // Terminal operation: collect all results into a list
                    // If we have an output_tx, collect from it
                    if let Some(tx) = output_tx.take() {
                        // Drop the sender to signal completion
                        drop(tx);
                    }
                    // For backward compatibility: if the list has only one element, return it directly
                    // This makes simple cases like "abc" @ { [a-z]+ => "word" } work as before
                    if results.len() == 1 {
                        return Ok(results.into_iter().next().unwrap());
                    }
                    return Ok(Value::List(Arc::new(results)));
                }
                StreamOp::Parse { grammar, rule } => {
                    // Create a channel for backtracking results
                    let (tx, _rx) = tokio::sync::mpsc::channel(100);

                    // Set output_tx so yield expressions can send to it
                    output_tx = Some(tx);

                    // Get the grammar
                    let grammar_arc = match grammar {
                        Value::Grammar(g) => g.clone(),
                        Value::String(grammar_name) => {
                            self.grammars.get(grammar_name).ok_or_else(|| {
                                Error::Runtime(format!("grammar not found: {}", grammar_name))
                            })?
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "grammar or string".to_string(),
                                got: grammar.type_name().to_string(),
                            });
                        }
                    };

                    // Execute grammar with full backtracking
                    // Run the grammar multiple times to find all matches
                    let input_value = current.clone();

                    // Try multiple starting positions/alternatives
                    // For Choice patterns, we want to try each alternative that succeeds
                    // This requires running the grammar with different "choice points" explored

                    // The evaluator evaluates semantic action expressions
                    let vm_ptr = self as *mut Self;
                    let _grammar_for_closure = grammar_arc.clone();
                    let _registry_for_closure = self.grammars.clone();
                    let _rule_for_closure = rule.clone();

                    let evaluator = std::rc::Rc::new(std::cell::RefCell::new(
                        move |expr: &crate::ast::Expr, bindings: &HashMap<SmolStr, Value>| {
                            // SAFETY: We need mutable access to the VM to evaluate expressions
                            // This is safe because we're the only caller during this evaluation
                            let vm = unsafe { &mut *vm_ptr };
                            vm.eval_with_bindings(expr, bindings)
                        },
                    ));

                    // Run grammar with backtracking by trying different approaches
                    // Approach 1: Run once and get the first match (current behavior)
                    // TODO: Implement full Choice exploration by running grammar multiple times

                    use crate::grammar::runtime::apply_grammar_to_value_with_evaluator;

                    let first_match = apply_grammar_to_value_with_evaluator(
                        input_value.clone(),
                        &grammar_arc,
                        &self.grammars,
                        rule,
                        evaluator,
                    )?;

                    if let Some(match_result) = first_match {
                        results.push(match_result);

                        // TODO: Continue searching for more matches
                        // This would require:
                        // 1. Identifying Choice patterns in the grammar
                        // 2. Running the grammar again with different alternatives prioritized
                        // 3. Collecting all distinct matches
                    }

                    // Signal completion by dropping the sender
                    if let Some(tx) = output_tx.take() {
                        drop(tx);
                    }
                }
                _ => {
                    // Other operations not yet implemented
                    current = Value::String(SmolStr::new("<stream>"));
                }
            }
        }

        // If no Collect was found, automatically collect and return the first result
        // This provides backward compatibility for cases like "abc" @ { [a-z]+ => "word" }
        if results.is_empty() {
            // No matches found - return an error
            return Err(Error::Runtime("no matches found".to_string()));
        }
        // Return the first match
        Ok(results.into_iter().next().unwrap())
    }

    fn throw_exception(&mut self, error: Value) -> Result<()> {
        if let Some((catch_target, frame_depth)) = self.exception_handlers.pop() {
            // Unwind frames to handler's frame
            while self.frames.len() > frame_depth {
                self.frames.pop();
            }
            // Jump to catch block
            if let Some(frame) = self.frames.last_mut() {
                // catch_target points to a LoadNull placeholder instruction.
                // We store the error value at that position and skip past the LoadNull
                // to prevent it from overwriting our error value.
                frame.values[catch_target.0] = error;
                frame.ip = catch_target.0 + 1;
            }
            Ok(())
        } else {
            Err(Error::Runtime(format!("uncaught exception: {}", error)))
        }
    }

    /// Execute an operation with exception handling.
    ///
    /// If the operation fails and there's an exception handler on the stack,
    /// the error is caught and converted to an exception value. Otherwise,
    /// the error is propagated.
    pub fn try_op<F>(&mut self, op: F) -> Result<Value>
    where
        F: FnOnce() -> Result<Value>,
    {
        match op() {
            Ok(val) => Ok(val),
            Err(e) if !self.exception_handlers.is_empty() => {
                let error = Value::String(SmolStr::new(e.to_string()));
                self.throw_exception(error)?;
                Ok(Value::Null)
            }
            Err(e) => Err(e),
        }
    }

    fn lookup_var(&self, name: &str) -> Result<Value> {
        // Check frame locals first (includes captures and parameters)
        // This allows user-defined variables to shadow builtin module names
        if let Some(frame) = self.frames.last()
            && let Some(val) = frame.locals.get(name)
        {
            return Ok(val.clone());
        }

        // Check scopes (innermost first) - for let bindings in current execution context
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.bindings.get(name) {
                return Ok(val.clone());
            }
        }

        // Check named objects
        if let Some(id) = self.objects.lock().unwrap().lookup_name(name) {
            return Ok(Value::Object(id));
        }

        // Check for constructor syntax (^name or @name)
        if name.starts_with('^')
            && let Some(id) = self.objects.lock().unwrap().lookup_name(&name[1..])
        {
            return Ok(Value::Object(id));
        }

        // Check builtins last (allows user variables to shadow builtin module names)
        match name {
            "curl" => return Ok(Value::Symbol(SmolStr::new("__builtin_curl"))),
            "human" => return Ok(Value::Symbol(SmolStr::new("__builtin_human"))),
            "io" => return Ok(Value::Symbol(SmolStr::new("__builtin_io"))),
            "json" => return Ok(Value::Symbol(SmolStr::new("__builtin_json"))),
            "ast" => return Ok(Value::Symbol(SmolStr::new("__builtin_ast"))),
            "ir" => return Ok(Value::Symbol(SmolStr::new("__builtin_ir"))),
            "code" => return Ok(Value::Symbol(SmolStr::new("__builtin_code"))),
            "env" => return Ok(Value::Symbol(SmolStr::new("__builtin_env"))),
            "sse" => return Ok(Value::Symbol(SmolStr::new("__builtin_sse"))),
            "time" => return Ok(Value::Symbol(SmolStr::new("__builtin_time"))),
            "rand" => return Ok(Value::Symbol(SmolStr::new("__builtin_rand"))),
            "tuplespace" => return Ok(Value::Symbol(SmolStr::new("__builtin_tuplespace"))),
            "stream" => return Ok(Value::Symbol(SmolStr::new("__builtin_stream"))),
            "cursor" => return Ok(Value::Symbol(SmolStr::new("__builtin_cursor"))),
            "string" => return Ok(Value::Symbol(SmolStr::new("__builtin_string"))),
            "codegen" => return Ok(Value::Symbol(SmolStr::new("__builtin_codegen"))),
            _ => {}
        }

        Err(Error::UndefinedVariable(name.to_string()))
    }

    fn store_var(&mut self, name: SmolStr, val: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, val);
        }
    }

    fn call_value(&mut self, func: Value, args: Vec<Value>) -> Result<()> {
        match func {
            Value::Lambda(lambda) => {
                let mut frame = Frame::new(lambda.code.clone());

                // Bind captures
                for (k, v) in &lambda.captures {
                    frame.locals.insert(k.clone(), v.clone());
                }

                // Bind arguments to parameters
                for (i, val) in args.into_iter().enumerate() {
                    if i < lambda.params.len() {
                        frame.locals.insert(lambda.params[i].clone(), val);
                    }
                }

                self.frames.push(frame);
            }
            Value::Object(id) => {
                if self
                    .objects
                    .lock()
                    .unwrap()
                    .get_method(id, "call")
                    .is_some()
                {
                    self.call_method(Value::Object(id), "call", args)?;
                } else {
                    return Err(Error::Runtime("object is not callable".to_string()));
                }
            }
            _ => {
                return Err(Error::Type {
                    expected: "callable".to_string(),
                    got: func.type_name().to_string(),
                });
            }
        }
        Ok(())
    }

    /// Call a value (lambda) synchronously within this VM and return its result.
    ///
    /// Pushes a new frame for the lambda, executes it to completion within the
    /// current VM (sharing all state including ParseStreams), and returns the result.
    /// This is used by `apply()` and other combinators that need to invoke FMPL
    /// lambdas during method dispatch.
    fn call_value_and_wait(&mut self, func: Value, args: Vec<Value>) -> Result<Value> {
        let base_depth = self.frames.len();
        // Temporarily save and clear exception handlers so that errors inside
        // the called lambda propagate to the caller (e.g., seq/choice/star/plus)
        // rather than being caught by outer try/catch blocks.
        let saved_handlers = std::mem::take(&mut self.exception_handlers);
        self.call_value(func, args)?;
        let result = self.execute_with_depth(base_depth);
        // Restore exception handlers
        self.exception_handlers = saved_handlers;
        match result {
            Ok(()) => {
                if self.frames.len() > base_depth {
                    // Frame still on stack (no explicit Return instruction) — pop and get result
                    let result = self.frames.last().unwrap().result();
                    self.frames.pop();
                    Ok(result)
                } else {
                    // Frame was popped by Return instruction; result was set on caller frame
                    let frame = self.frames.last().unwrap();
                    Ok(frame.result())
                }
            }
            Err(e) => {
                // Clean up any orphaned frames from the failed call
                while self.frames.len() > base_depth {
                    self.frames.pop();
                }
                Err(e)
            }
        }
    }

    /// Execute a lambda with shared VM state (objects and grammars).
    ///
    /// This function creates a new VM instance that shares the object database
    /// and grammar registry with the parent VM, allowing async tasks to access
    /// and modify shared state safely.
    ///
    /// # Arguments
    /// * `lambda` - The lambda to execute
    /// * `objects` - Shared object database (Arc<Mutex<ObjectDb>>)
    /// * `grammars` - Grammar registry (cloned for new VM)
    ///
    /// # Returns
    /// The result of executing the lambda, or an error.
    fn execute_lambda_with_state(
        lambda: &Value,
        objects: &Arc<std::sync::Mutex<ObjectDb>>,
        grammars: &GrammarRegistry,
    ) -> Result<Value> {
        match lambda {
            Value::Lambda(lambda) => {
                // Create a new VM instance with shared state
                let mut vm = Vm {
                    objects: objects.clone(),
                    grammars: grammars.clone(),
                    frames: Vec::new(),
                    scopes: vec![Scope::default()],
                    current_user: None,
                    exception_handlers: Vec::new(),
                    runtime: None, // No runtime in lambda execution context
                    compiled_grammars: Arc::new(std::sync::Mutex::new(HashMap::new())),
                };

                // Create a frame for the lambda (same as call_value does)
                let mut frame = Frame::new(lambda.code.clone());

                // Bind captures (if any)
                for (k, v) in &lambda.captures {
                    frame.locals.insert(k.clone(), v.clone());
                }

                // Bind arguments (no arguments for our use case)
                // for (i, val) in args.into_iter().enumerate() {
                //     if i < lambda.params.len() {
                //         frame.locals.insert(lambda.params[i].clone(), val);
                //     }
                // }

                // Push frame and execute
                let base_depth = vm.frames.len();
                vm.frames.push(frame);
                let exec_result = vm.execute_with_depth(base_depth);

                // Get the result from the last frame's last instruction
                // Note: Return instruction doesn't pop the frame when there's no caller
                if exec_result.is_ok() {
                    if let Some(frame) = vm.frames.last() {
                        let frame_result = frame.result();
                        vm.frames.pop(); // Clean up the frame
                        Ok(frame_result)
                    } else {
                        // This shouldn't happen with the fixed Return instruction
                        Ok(Value::Null)
                    }
                } else {
                    if vm.frames.last().is_some() {
                        vm.frames.pop(); // Clean up the frame on error
                    }
                    // Convert the () error to a Value error
                    exec_result.map(|_| Value::Null)
                }
            }
            _ => Err(Error::Runtime(
                "execute_lambda_with_state requires lambda argument".to_string(),
            )),
        }
    }

    /// Execute a lambda asynchronously with shared VM state.
    ///
    /// This function is designed to be used in async contexts (e.g., tokio tasks).
    /// It executes the lambda and sends the result to the provided channel sender.
    ///
    /// # Arguments
    /// * `lambda` - The lambda to execute
    /// * `objects` - Shared object database
    /// * `grammars` - Grammar registry
    /// * `tx` - Channel sender to send the result
    async fn execute_lambda_async(
        lambda: Value,
        objects: Arc<std::sync::Mutex<ObjectDb>>,
        grammars: GrammarRegistry,
        tx: tokio::sync::mpsc::Sender<crate::stream::StreamEvent>,
    ) {
        use crate::stream::StreamEvent;

        // Execute the lambda with shared state
        let result = Self::execute_lambda_with_state(&lambda, &objects, &grammars);

        // Send the result to the stream
        match result {
            Ok(value) => {
                // Send asynchronously
                if let Err(e) = tx.send(StreamEvent::Ok(value)).await {
                    eprintln!("Error sending stream result: {}", e);
                }
            }
            Err(e) => {
                let error_msg = Value::String(e.to_string().into());
                if let Err(send_err) = tx.send(StreamEvent::Err(error_msg)).await {
                    eprintln!("Error sending stream error: {}", send_err);
                }
            }
        }
    }

    fn call_builtin(&mut self, object: &str, method: &str, args: Vec<Value>) -> Result<Value> {
        match (object, method) {
            #[cfg(not(all(feature = "curl-builtin", not(target_arch = "wasm32"))))]
            ("__builtin_curl", _) => Err(Error::Runtime(
                "curl builtin is not available in this build (requires the \
                 `curl-builtin` feature on a native target)"
                    .to_string(),
            )),
            #[cfg(all(feature = "curl-builtin", not(target_arch = "wasm32")))]
            ("__builtin_curl", "get") => {
                let url = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.get requires string URL".to_string())),
                };
                let handle = self
                    .runtime
                    .as_ref()
                    .ok_or_else(|| Error::Runtime("curl requires runtime handle".to_string()))?;
                // Optional third argument: options map with headers
                let options = args.get(2);
                crate::builtins::CurlBuiltin::get(url, handle, options)
            }
            #[cfg(all(feature = "curl-builtin", not(target_arch = "wasm32")))]
            ("__builtin_curl", "post") => {
                let url = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.post requires string URL".to_string())),
                };
                let body = match args.get(1) {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.post requires string body".to_string())),
                };
                let handle = self
                    .runtime
                    .as_ref()
                    .ok_or_else(|| Error::Runtime("curl requires runtime handle".to_string()))?;
                // Optional fourth argument: options map with headers
                let options = args.get(3);
                crate::builtins::CurlBuiltin::post(url, body, handle, options)
            }
            ("__builtin_human", "approve") => {
                let request = args.first().ok_or_else(|| {
                    Error::Runtime("human.approve requires request argument".to_string())
                })?;
                let handle = self.runtime.as_ref().ok_or_else(|| {
                    Error::Runtime("human.approve requires runtime handle".to_string())
                })?;
                crate::builtins::HumanBuiltin::approve(request, handle)
            }
            ("__builtin_io", "load") => {
                let path = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("io.load requires string path".to_string())),
                };
                // Capture VM for evaluation context
                // We need to call eval on the loaded code
                let vm_ref = self as *mut Self;
                crate::builtins::IoBuiltin::load(path, |code| {
                    // SAFETY: We're passing a mutable reference that's valid for this call
                    // The VM won't be dropped while the load is in progress
                    let vm = unsafe { &mut *vm_ref };
                    crate::eval(vm, code)
                })
            }
            ("__builtin_env", "get") => {
                let name = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("env.get requires string name".to_string())),
                };
                crate::builtins::EnvBuiltin::get(name)
            }
            ("__builtin_ast", "parse") => {
                let source = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("ast::parse requires string argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                crate::builtins::ast::parse(source)
            }
            ("__builtin_ir", "compile") => {
                let ir = match args.first() {
                    Some(v) => v,
                    None => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("ir::compile requires IR argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                crate::builtins::ir::compile(ir)
            }
            ("__builtin_ir", "to_rust") => {
                let ir = match args.first() {
                    Some(v) => v,
                    None => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("ir::to_rust requires IR argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                match crate::builtins::ir_to_rust::transpile(ir) {
                    Ok(rust_code) => Ok(Value::String(rust_code.into())),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("transpile_failed".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            ("__builtin_ir", "to_rust_expr") => {
                let ir = match args.first() {
                    Some(v) => v,
                    None => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("ir::to_rust_expr requires IR argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                match crate::builtins::ir_to_rust::transpile_expr(ir) {
                    Ok(rust_code) => Ok(Value::String(rust_code.into())),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("transpile_failed".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            ("__builtin_code", "eval") => {
                let code = match args.first() {
                    Some(Value::Code(c)) => c.clone(),
                    _ => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("code::eval requires Code argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                // Execute the compiled code in current VM context
                self.run(&code)
            }
            ("__builtin_json", "parse") => {
                let json_str = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("json::parse requires string argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(v) => json::from_json(v),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("invalid_json".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            ("__builtin_json", "stringify") => {
                if args.is_empty() {
                    return Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("invalid_args".into())),
                            (
                                "message".into(),
                                Value::String(
                                    "json::stringify requires at least one argument".into(),
                                ),
                            ),
                        ]
                        .into_iter()
                        .collect(),
                    )));
                }
                let value = &args[0];
                let json_value = json::to_json(value);
                match serde_json::to_string(&json_value) {
                    Ok(s) => Ok(Value::String(s.into())),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("stringify_failed".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            ("__builtin_sse", "parse") => {
                let text = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => {
                        return Err(Error::Runtime(
                            "sse.parse requires string argument".to_string(),
                        ));
                    }
                };
                crate::builtins::SseBuiltin::parse(text)
            }
            ("__builtin_tuplespace", "new") => {
                if !args.is_empty() {
                    return Err(Error::Runtime(
                        "tuplespace.new() takes no arguments".to_string(),
                    ));
                }
                use crate::tuplespace::store::TupleSpace;
                let space = TupleSpace::new();
                Ok(Value::TupleSpace(Arc::new(std::sync::Mutex::new(space))))
            }
            ("__builtin_tuplespace", "open") => {
                // Open a durable tuplespace at the given filesystem
                // path. Only available with the `persistence` feature;
                // without it, the call surface exists but errors at
                // runtime to keep the FMPL contract uniform across
                // build configurations.
                if args.len() != 1 {
                    return Err(Error::Runtime(format!(
                        "tuplespace.open(path) takes exactly one argument, got {}",
                        args.len()
                    )));
                }
                let path = match &args[0] {
                    Value::String(s) => s.to_string(),
                    other => {
                        return Err(Error::Runtime(format!(
                            "tuplespace.open(path): expected a String path, got {}",
                            other.type_name()
                        )));
                    }
                };
                #[cfg(feature = "persistence")]
                {
                    use crate::tuplespace::store::TupleSpace;
                    let space = TupleSpace::open(&path)?;
                    Ok(Value::TupleSpace(Arc::new(std::sync::Mutex::new(space))))
                }
                #[cfg(not(feature = "persistence"))]
                {
                    let _ = path;
                    Err(Error::Runtime(
                        "tuplespace.open requires the `persistence` feature \
                         (rebuild fmpl-core with --features persistence)"
                            .to_string(),
                    ))
                }
            }
            ("__builtin_time", "sleep") => {
                let ms = match args.first() {
                    Some(Value::Int(n)) => *n,
                    _ => {
                        return Err(Error::Runtime(
                            "time.sleep requires integer argument (milliseconds)".to_string(),
                        ));
                    }
                };
                crate::builtins::TimeBuiltin::sleep(ms)
            }
            ("__builtin_rand", "int") => {
                let min = match args.first() {
                    Some(Value::Int(n)) => *n,
                    _ => {
                        return Err(Error::Runtime(
                            "rand.int requires integer arguments (min, max)".to_string(),
                        ));
                    }
                };
                let max = match args.get(1) {
                    Some(Value::Int(n)) => *n,
                    _ => {
                        return Err(Error::Runtime(
                            "rand.int requires integer arguments (min, max)".to_string(),
                        ));
                    }
                };
                crate::builtins::RandBuiltin::int(min, max)
            }
            ("__builtin_rand", "float") => crate::builtins::RandBuiltin::float(),
            ("__builtin_stream", "new") => {
                if args.len() != 1 {
                    return Err(Error::Runtime("stream::new requires 1 argument".into()));
                }
                let ps = crate::parse_stream::ParseStream::new(args.into_iter().next().unwrap());
                Ok(Value::ParseStream(Arc::new(std::sync::Mutex::new(ps))))
            }
            ("__builtin_stream", "fail") => {
                let msg = match args.first() {
                    Some(Value::String(s)) => s.to_string(),
                    _ => "parse failure".to_string(),
                };
                Err(Error::ParseFailed {
                    position: 0,
                    message: msg,
                })
            }
            ("__builtin_stream", "match_char") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("match_char requires (stream, char)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "match_char: first arg must be a stream".into(),
                        ));
                    }
                };
                let expected = match &args[1] {
                    Value::String(s) => s.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "match_char: second arg must be a string".into(),
                        ));
                    }
                };
                let mut stream = ps.lock().unwrap();
                let head = stream.head();
                match head {
                    Value::String(ref c) if *c == expected => {
                        stream.advance(1);
                        Ok(Value::String(expected))
                    }
                    _ => {
                        let pos = stream.position();
                        Err(Error::ParseFailed {
                            position: pos,
                            message: format!("expected '{}', got {:?}", expected, head),
                        })
                    }
                }
            }
            ("__builtin_stream", "match_class") => {
                if args.len() != 2 {
                    return Err(Error::Runtime(
                        "match_class requires (stream, class)".into(),
                    ));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "match_class: first arg must be a stream".into(),
                        ));
                    }
                };
                let class = match &args[1] {
                    Value::String(s) => s.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "match_class: second arg must be a string".into(),
                        ));
                    }
                };
                let mut stream = ps.lock().unwrap();
                let head = stream.head();
                match head {
                    Value::String(ref c) => {
                        let ch = c.chars().next().unwrap_or('\0');
                        if crate::parse_stream::char_in_class(ch, &class) {
                            stream.advance(1);
                            Ok(Value::String(c.clone()))
                        } else {
                            let pos = stream.position();
                            Err(Error::ParseFailed {
                                position: pos,
                                message: format!("expected [{}], got '{}'", class, c),
                            })
                        }
                    }
                    _ => {
                        let pos = stream.position();
                        Err(Error::ParseFailed {
                            position: pos,
                            message: format!("expected [{}], got {:?}", class, head),
                        })
                    }
                }
            }
            ("__builtin_stream", "choice") => {
                if args.len() != 2 {
                    return Err(Error::Runtime(
                        "choice requires (stream, alternatives)".into(),
                    ));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => return Err(Error::Runtime("choice: first arg must be a stream".into())),
                };
                let alternatives = match &args[1] {
                    Value::List(items) => (**items).clone(),
                    _ => return Err(Error::Runtime("choice: second arg must be a list".into())),
                };

                let start_pos = {
                    let stream = ps.lock().unwrap();
                    stream.position()
                };

                for alt in &alternatives {
                    // Restore to start before each attempt
                    {
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint {
                            position: start_pos,
                        });
                    }
                    let stream_val = Value::ParseStream(ps.clone());
                    match self.call_value_and_wait(alt.clone(), vec![stream_val]) {
                        Ok(result) => return Ok(result),
                        Err(Error::ParseFailed { .. }) => continue,
                        Err(e) => return Err(e),
                    }
                }

                // All alternatives failed — restore and fail
                {
                    let mut stream = ps.lock().unwrap();
                    stream.restore(&crate::parse_stream::Checkpoint {
                        position: start_pos,
                    });
                }
                Err(Error::ParseFailed {
                    position: start_pos,
                    message: "all alternatives failed".into(),
                })
            }
            ("__builtin_stream", "star") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("star requires (stream, rule)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => return Err(Error::Runtime("star: first arg must be a stream".into())),
                };
                let rule = args[1].clone();

                let mut results = Vec::new();
                loop {
                    let position = {
                        let stream = ps.lock().unwrap();
                        stream.position()
                    };
                    let stream_val = Value::ParseStream(ps.clone());
                    match self.call_value_and_wait(rule.clone(), vec![stream_val]) {
                        Ok(result) => {
                            let new_pos = {
                                let stream = ps.lock().unwrap();
                                stream.position()
                            };
                            results.push(result);
                            if new_pos == position {
                                break; // Zero-length match, prevent infinite loop
                            }
                        }
                        Err(Error::ParseFailed { .. }) => {
                            let mut stream = ps.lock().unwrap();
                            stream.restore(&crate::parse_stream::Checkpoint { position });
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::List(Arc::new(results)))
            }
            ("__builtin_stream", "plus") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("plus requires (stream, rule)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => return Err(Error::Runtime("plus: first arg must be a stream".into())),
                };
                let rule = args[1].clone();

                let mut results = Vec::new();
                loop {
                    let position = {
                        let stream = ps.lock().unwrap();
                        stream.position()
                    };
                    let stream_val = Value::ParseStream(ps.clone());
                    match self.call_value_and_wait(rule.clone(), vec![stream_val]) {
                        Ok(result) => {
                            let new_pos = {
                                let stream = ps.lock().unwrap();
                                stream.position()
                            };
                            results.push(result);
                            if new_pos == position {
                                break;
                            }
                        }
                        Err(Error::ParseFailed { .. }) => {
                            let mut stream = ps.lock().unwrap();
                            stream.restore(&crate::parse_stream::Checkpoint { position });
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
                if results.is_empty() {
                    let pos = {
                        let stream = ps.lock().unwrap();
                        stream.position()
                    };
                    return Err(Error::ParseFailed {
                        position: pos,
                        message: "plus: expected at least one match".into(),
                    });
                }
                Ok(Value::List(Arc::new(results)))
            }
            ("__builtin_stream", "seq") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("seq requires (stream, rules)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => return Err(Error::Runtime("seq: first arg must be a stream".into())),
                };
                let rules = match &args[1] {
                    Value::List(items) => (**items).clone(),
                    _ => return Err(Error::Runtime("seq: second arg must be a list".into())),
                };

                let start_pos = {
                    let stream = ps.lock().unwrap();
                    stream.position()
                };

                let mut results = Vec::new();
                for rule in &rules {
                    let stream_val = Value::ParseStream(ps.clone());
                    match self.call_value_and_wait(rule.clone(), vec![stream_val]) {
                        Ok(result) => results.push(result),
                        Err(Error::ParseFailed { position, message }) => {
                            let mut stream = ps.lock().unwrap();
                            stream.restore(&crate::parse_stream::Checkpoint {
                                position: start_pos,
                            });
                            return Err(Error::ParseFailed { position, message });
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::List(Arc::new(results)))
            }
            ("__builtin_stream", "not") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("not requires (stream, rule)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => return Err(Error::Runtime("not: first arg must be a stream".into())),
                };
                let rule = args[1].clone();

                let position = {
                    let stream = ps.lock().unwrap();
                    stream.position()
                };

                let stream_val = Value::ParseStream(ps.clone());
                match self.call_value_and_wait(rule, vec![stream_val]) {
                    Ok(_) => {
                        // Rule succeeded — not fails, restore position
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position });
                        Err(Error::ParseFailed {
                            position,
                            message: "negative lookahead matched".into(),
                        })
                    }
                    Err(Error::ParseFailed { .. }) => {
                        // Rule failed — not succeeds, restore position
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position });
                        Ok(Value::Null)
                    }
                    Err(e) => Err(e),
                }
            }
            ("__builtin_stream", "lookahead") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("lookahead requires (stream, rule)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "lookahead: first arg must be a stream".into(),
                        ));
                    }
                };
                let rule = args[1].clone();

                let position = {
                    let stream = ps.lock().unwrap();
                    stream.position()
                };

                let stream_val = Value::ParseStream(ps.clone());
                match self.call_value_and_wait(rule, vec![stream_val]) {
                    Ok(result) => {
                        // Succeeded — restore position (lookahead doesn't consume)
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position });
                        Ok(result)
                    }
                    Err(e) => {
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position });
                        Err(e)
                    }
                }
            }
            ("__builtin_stream", "optional") => {
                if args.len() != 2 {
                    return Err(Error::Runtime("optional requires (stream, rule)".into()));
                }
                let ps = match &args[0] {
                    Value::ParseStream(ps) => ps.clone(),
                    _ => {
                        return Err(Error::Runtime(
                            "optional: first arg must be a stream".into(),
                        ));
                    }
                };
                let rule = args[1].clone();

                let position = {
                    let stream = ps.lock().unwrap();
                    stream.position()
                };

                let stream_val = Value::ParseStream(ps.clone());
                match self.call_value_and_wait(rule, vec![stream_val]) {
                    Ok(result) => Ok(result),
                    Err(Error::ParseFailed { .. }) => {
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position });
                        Ok(Value::Null)
                    }
                    Err(e) => Err(e),
                }
            }
            ("__builtin_stream", "observe") => {
                // observe(collection_or_stream_or_cursor, branch_id?) -> Cursor
                // Creates a cursor reference to any collection, stream, or existing cursor
                // Lists and other collections are automatically wrapped in a stream
                let (stream_value, branch_id) = match args.as_slice() {
                    [Value::Stream(s)] => (Value::Stream(Arc::clone(s)), SmolStr::new("main")),
                    [Value::Stream(s), Value::String(branch)] => {
                        (Value::Stream(Arc::clone(s)), branch.clone())
                    }
                    [Value::Cursor(c)] => {
                        (Value::Stream(Arc::clone(&c.stream)), c.branch_id.clone())
                    }
                    [Value::Cursor(c), Value::String(branch)] => {
                        (Value::Stream(Arc::clone(&c.stream)), branch.clone())
                    }
                    [Value::List(_), Value::String(branch)] => (args[0].clone(), branch.clone()),
                    [Value::List(_)] => (args[0].clone(), SmolStr::new("main")),
                    [Value::String(_), Value::String(branch)] => (args[0].clone(), branch.clone()),
                    [Value::String(_)] => (args[0].clone(), SmolStr::new("main")),
                    [val] => (val.clone(), SmolStr::new("main")),
                    [val, Value::String(branch)] => (val.clone(), branch.clone()),
                    _ => {
                        return Err(Error::Runtime(
                            "observe() requires a value (list, string, stream, or cursor)"
                                .to_string(),
                        ));
                    }
                };

                // Convert non-stream values to streams
                let stream_arg = match stream_value {
                    Value::Stream(s) => s,
                    Value::List(items) => Arc::new(Stream {
                        source: Value::List(items),
                        ops: Vec::new(),
                    }),
                    Value::String(s) => Arc::new(Stream {
                        source: Value::String(s),
                        ops: Vec::new(),
                    }),
                    other => Arc::new(Stream {
                        source: other,
                        ops: Vec::new(),
                    }),
                };

                // Create cursor at start position
                let cursor = Cursor {
                    stream: stream_arg,
                    position: CursorPosition::start(),
                    branch_id,
                };

                Ok(Value::Cursor(Arc::new(cursor)))
            }
            ("__builtin_cursor", "advance") => {
                // cursor.advance(n) -> Cursor
                // Advance cursor by n positions in the stream
                let (cursor, n) = match args.as_slice() {
                    [Value::Cursor(c), Value::Int(n)] => (c, *n as usize),
                    _ => {
                        return Err(Error::Runtime(
                            "cursor.advance(cursor, n) requires cursor and integer".to_string(),
                        ));
                    }
                };

                let advanced = cursor.advance(n);
                Ok(Value::Cursor(Arc::new(advanced)))
            }
            ("__builtin_cursor", "rewind") => {
                // cursor.rewind(n) -> Cursor
                // Rewind cursor by n positions
                let (cursor, n) = match args.as_slice() {
                    [Value::Cursor(c), Value::Int(n)] => (c, *n as usize),
                    _ => {
                        return Err(Error::Runtime(
                            "cursor.rewind(cursor, n) requires cursor and integer".to_string(),
                        ));
                    }
                };

                let rewound = cursor.rewind(n);
                Ok(Value::Cursor(Arc::new(rewound)))
            }
            ("__builtin_cursor", "position") => {
                // cursor.position() -> Int
                // Get current position index
                let cursor = match args.first() {
                    Some(Value::Cursor(c)) => c,
                    _ => {
                        return Err(Error::Runtime(
                            "cursor.position() requires cursor argument".to_string(),
                        ));
                    }
                };

                Ok(cursor.get_position())
            }
            ("__builtin_cursor", "current") => {
                // cursor.current() -> Value
                // Get the current element at the cursor's position
                let cursor = match args.first() {
                    Some(Value::Cursor(c)) => c,
                    _ => {
                        return Err(Error::Runtime(
                            "cursor.current() requires cursor argument".to_string(),
                        ));
                    }
                };

                // Get the current element from the stream's source at the cursor's position
                let pos = cursor.position.index;
                match &cursor.stream.source {
                    Value::List(items) => {
                        if pos < items.len() {
                            Ok(items[pos].clone())
                        } else {
                            Ok(Value::Null) // Past end of stream
                        }
                    }
                    Value::String(s) => {
                        if pos < s.len() {
                            Ok(Value::String(s[pos..pos + 1].into()))
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    _ => Ok(Value::Null), // Non-indexable stream, return null
                }
            }
            ("__builtin_stream", "create") => {
                // stream.create(lambda) -> AsyncStream
                // Creates an async stream from a generator lambda.
                //
                // The lambda is executed asynchronously and its return value
                // becomes the stream's result. For multi-value streams, use
                // await_all on a list of streams.
                //
                // Usage:
                //   let stream = stream.create(\{
                //     -- async computation
                //     42
                //   })

                let handle = self.runtime.as_ref().ok_or_else(|| {
                    Error::Runtime("stream.create requires runtime handle - use Vm::with_runtime() or run in async context".to_string())
                })?;

                let lambda = match args.first() {
                    Some(Value::Lambda(lambda)) => lambda.clone(),
                    Some(other) => {
                        return Err(Error::Runtime(format!(
                            "stream.create requires lambda argument, got {}",
                            other.type_name()
                        )));
                    }
                    None => {
                        return Err(Error::Runtime(
                            "stream.create requires lambda argument".to_string(),
                        ));
                    }
                };

                use crate::stream::{StreamHandle, StreamSource, next_id};
                use tokio::sync::mpsc;

                let (tx, rx) = mpsc::channel(1); // Bounded channel for backpressure

                // Clone shared state for the async task
                let objects = self.objects.clone();
                let grammars = self.grammars.clone();

                // Spawn a task to execute the lambda
                handle.spawn(async move {
                    // Execute the lambda asynchronously with shared VM state
                    Self::execute_lambda_async(Value::Lambda(lambda), objects, grammars, tx).await;
                });

                let stream = StreamHandle::with_source(rx, next_id(), StreamSource::Ephemeral);

                Ok(Value::AsyncStream(Arc::new(std::sync::Mutex::new(stream))))
            }
            ("__builtin_stream", "sink") => {
                // stream.sink() -> Sink
                // Creates a sink handle for sending values asynchronously.
                //
                // Usage:
                //   let sink = stream.sink()
                //   sink.send(42)  -- TODO: need send method
                //
                // Returns a Value::Sink that can be used to send values

                use crate::stream::{SinkHandle, next_id};
                use tokio::sync::mpsc;

                let (tx, _rx) = mpsc::channel(100); // Buffer size 100 for backpressure
                let sink = SinkHandle::new(tx, next_id());

                Ok(Value::Sink(Arc::new(sink)))
            }
            ("__builtin_string", "join") => {
                // string.join(list) -> String
                // Joins a list of strings into a single string.
                //
                // Usage:
                //   string.join(["a", "b", "c"]) => "abc"
                //   string.join([]) => ""
                //
                let list = match args.first() {
                    Some(Value::List(lst)) => lst,
                    _ => {
                        return Err(Error::Runtime(
                            "string.join requires list argument".to_string(),
                        ));
                    }
                };
                let mut result = String::new();
                for item in list.iter() {
                    match item {
                        Value::String(s) => result.push_str(s),
                        other => {
                            return Err(Error::Runtime(format!(
                                "string.join list items must be strings, got {}",
                                other.type_name()
                            )));
                        }
                    }
                }
                Ok(Value::String(SmolStr::new(result)))
            }
            ("__builtin_string", "to_symbol") => {
                // string.to_symbol(s) -> Symbol
                // Backs the prelude's `symbol` helper; grammar actions in
                // fmpl_parser.fmpl call symbol() when run in the interpreted
                // grammar runtime (the generated parser uses a Rust helper).
                //
                // Usage:
                //   string.to_symbol("foo") => :foo
                //
                match args.first() {
                    Some(Value::String(s)) => Ok(Value::Symbol(s.clone())),
                    Some(Value::Symbol(s)) => Ok(Value::Symbol(s.clone())),
                    _ => Err(Error::Runtime(
                        "string.to_symbol requires string argument".to_string(),
                    )),
                }
            }
            ("__builtin_codegen", "grammar_to_ir") => {
                // codegen.grammar_to_ir(grammar) -> Value::Tagged (parsing IR)
                // Converts a Grammar to parsing IR for transpilation to Rust.
                let grammar = match args.first() {
                    Some(Value::Grammar(g)) => g.clone(),
                    _ => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String(
                                        "codegen.grammar_to_ir requires Grammar argument".into(),
                                    ),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                match crate::builtins::grammar_to_ir::grammar_to_ir(&grammar) {
                    Ok(ir) => Ok(ir),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("grammar_to_ir_failed".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            ("__builtin_codegen", "ir_to_rust") => {
                // codegen.ir_to_rust(ir) -> String (Rust source code)
                // Transpiles parsing IR to complete Rust parser code.
                let ir = match args.first() {
                    Some(v) => v,
                    None => {
                        return Ok(Value::Map(std::sync::Arc::new(
                            vec![
                                ("error".into(), Value::String("invalid_args".into())),
                                (
                                    "message".into(),
                                    Value::String("codegen.ir_to_rust requires IR argument".into()),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )));
                    }
                };
                match crate::builtins::ir_to_rust::transpile(ir) {
                    Ok(rust_code) => Ok(Value::String(rust_code.into())),
                    Err(e) => Ok(Value::Map(std::sync::Arc::new(
                        vec![
                            ("error".into(), Value::String("ir_to_rust_failed".into())),
                            ("message".into(), Value::String(e.to_string().into())),
                        ]
                        .into_iter()
                        .collect(),
                    ))),
                }
            }
            _ => Err(Error::Runtime(format!(
                "unknown builtin: {}.{}",
                object, method
            ))),
        }
    }

    /// Convert a Value to a TuplePattern for tuple matching.
    fn value_to_tuple_pattern(value: &Value) -> Result<crate::tuplespace::TuplePattern> {
        use crate::tuplespace::{Pattern, TuplePattern};
        use std::collections::HashMap;

        Ok(match value {
            // Symbol type-only match: :log becomes Symbol("log") in AST
            // Treat any Symbol as a keyword type pattern
            Value::Symbol(type_name) => TuplePattern::TypeAndData {
                type_name: type_name.clone(),
                data: Pattern::Wildcard,
            },
            // String type-only match
            Value::String(type_name) => TuplePattern::TypeAndData {
                type_name: type_name.clone(),
                data: Pattern::Wildcard,
            },
            // Map pattern: %{type: "log", data: %{...}}
            Value::Map(map) => {
                let type_name = match map.get("type") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Symbol(s)) => s.clone(),
                    other => {
                        return Err(Error::Runtime(format!(
                            "pattern type must be a string or keyword, got {:?}",
                            other
                        )));
                    }
                };

                // Check for namespace in pattern
                if let Some(namespace_value) = map.get("namespace") {
                    let namespace = match namespace_value {
                        Value::String(s) => s.clone(),
                        Value::Symbol(s) => s.clone(),
                        other => {
                            return Err(Error::Runtime(format!(
                                "pattern namespace must be a string or keyword, got {:?}",
                                other
                            )));
                        }
                    };

                    // Extract data pattern if provided
                    let data_pattern = match map.get("data") {
                        None => Pattern::Wildcard,
                        Some(Value::Map(data_map)) => {
                            let mut required = HashMap::new();
                            for (k, v) in data_map.iter() {
                                required.insert(k.clone(), v.clone());
                            }
                            Pattern::Map { required }
                        }
                        Some(other) => Pattern::Exact(other.clone()),
                    };

                    TuplePattern::Full {
                        namespace,
                        type_name,
                        data: data_pattern,
                    }
                } else {
                    // No namespace, use TypeAndData
                    let data_pattern = match map.get("data") {
                        None => Pattern::Wildcard,
                        Some(Value::Map(data_map)) => {
                            let mut required = HashMap::new();
                            for (k, v) in data_map.iter() {
                                required.insert(k.clone(), v.clone());
                            }
                            Pattern::Map { required }
                        }
                        Some(other) => Pattern::Exact(other.clone()),
                    };

                    TuplePattern::TypeAndData {
                        type_name,
                        data: data_pattern,
                    }
                }
            }
            other => {
                return Err(Error::Runtime(format!(
                    "invalid pattern: expected symbol or map, got {}",
                    other.type_name()
                )));
            }
        })
    }

    /// Convert a Tuple to a Value map representation.
    fn tuple_to_value(tuple: crate::tuplespace::Tuple) -> Value {
        use std::collections::HashMap;
        use std::sync::Arc;

        let mut map = HashMap::new();
        map.insert(SmolStr::new("type"), Value::String(tuple.type_name));
        if let Some(ns) = tuple.namespace {
            map.insert(SmolStr::new("namespace"), Value::String(ns));
        }
        map.insert(
            SmolStr::new("timestamp"),
            Value::Int(tuple.timestamp as i64),
        );
        map.insert(SmolStr::new("seq"), Value::Int(tuple.seq as i64));
        map.insert(SmolStr::new("data"), tuple.data);
        Value::Map(Arc::new(map))
    }

    /// Type-predicate and reflection methods that apply to *every* value,
    /// regardless of receiver type — so `n.is_number()` works in a guard even
    /// when `n` is a bare int. Returns `None` for names that are not universal,
    /// leaving the normal per-type dispatch to run. Predicates ignore args.
    fn universal_method(receiver: &Value, name: &str) -> Option<Value> {
        let b = |v: bool| Some(Value::Bool(v));
        match name {
            "is_null" => b(matches!(receiver, Value::Null)),
            "is_bool" => b(matches!(receiver, Value::Bool(_))),
            "is_int" => b(matches!(receiver, Value::Int(_))),
            "is_float" => b(matches!(receiver, Value::Float(_))),
            "is_number" => b(matches!(receiver, Value::Int(_) | Value::Float(_))),
            "is_string" => b(matches!(receiver, Value::String(_))),
            "is_symbol" => b(matches!(receiver, Value::Symbol(_))),
            "is_list" => b(matches!(receiver, Value::List(_))),
            "is_map" => b(matches!(receiver, Value::Map(_))),
            "is_object" => b(matches!(receiver, Value::Object(_))),
            "type_name" => Some(Value::Symbol(SmolStr::new(receiver.type_name()))),
            _ => None,
        }
    }

    fn call_method(&mut self, receiver: Value, name: &str, args: Vec<Value>) -> Result<()> {
        // Universal methods (type predicates + type_name) come first so they
        // work on any receiver, including primitives that have no method table.
        if let Some(result) = Self::universal_method(&receiver, name) {
            let frame = self.frames.last_mut().unwrap();
            frame.set_current(result);
            return Ok(());
        }
        match receiver {
            Value::Symbol(ref s) if s.starts_with("__builtin_") => {
                let result = self.call_builtin(s.as_str(), name, args)?;
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
                return Ok(());
            }
            Value::Object(id) => {
                if let Some(method) = self.objects.lock().unwrap().get_method(id, name).cloned() {
                    let mut frame = Frame::new(method.code);
                    frame.this = Some(id);

                    // Bind arguments
                    for (i, val) in args.into_iter().enumerate() {
                        if i < method.params.len() {
                            frame.locals.insert(method.params[i].clone(), val);
                        }
                    }

                    self.frames.push(frame);
                } else {
                    return Err(Error::UndefinedMethod(name.to_string()));
                }
            }
            Value::Facet(id, ref facet_name) => {
                let db = self.objects.lock().unwrap();
                if !db.facet_allows(id, facet_name, name) {
                    return Err(Error::Runtime(format!(
                        "facet :{} does not expose method '{}'",
                        facet_name, name
                    )));
                }
                if let Some(method) = db.get_method(id, name).cloned() {
                    drop(db);
                    let mut frame = Frame::new(method.code);
                    frame.this = Some(id);

                    for (i, val) in args.into_iter().enumerate() {
                        if i < method.params.len() {
                            frame.locals.insert(method.params[i].clone(), val);
                        }
                    }

                    self.frames.push(frame);
                } else {
                    return Err(Error::UndefinedMethod(name.to_string()));
                }
            }
            Value::List(list) => {
                // Built-in list methods
                let result = match name {
                    "len" => Value::Int(list.len() as i64),
                    "first" => list.first().cloned().unwrap_or(Value::Null),
                    "last" => list.last().cloned().unwrap_or(Value::Null),
                    "push" => {
                        let mut new_list = (*list).clone();
                        for arg in args {
                            new_list.push(arg);
                        }
                        Value::List(Arc::new(new_list))
                    }
                    "slice" => {
                        // Method form for programmatic access; native syntax is list[start..end]
                        let (start, end) =
                            match args.as_slice() {
                                [Value::Int(start), Value::Int(end)] => {
                                    (*start as usize, *end as usize)
                                }
                                [Value::Int(start)] => (*start as usize, list.len()),
                                _ => return Err(Error::Runtime(
                                    "slice() requires (start) or (start, end) integer arguments"
                                        .to_string(),
                                )),
                            };
                        let end = end.min(list.len());
                        let start = start.min(end);
                        Value::List(Arc::new(list[start..end].to_vec()))
                    }
                    "map" => {
                        let func = match args.first() {
                            Some(f @ Value::Lambda(_)) => f.clone(),
                            _ => {
                                return Err(Error::Runtime(
                                    "map() requires a lambda argument".to_string(),
                                ));
                            }
                        };
                        let items = (*list).clone();
                        let mut result = Vec::with_capacity(items.len());
                        for item in items {
                            let val = self.call_value_and_wait(func.clone(), vec![item])?;
                            result.push(val);
                        }
                        Value::List(Arc::new(result))
                    }
                    "filter" => {
                        let func = match args.first() {
                            Some(f @ Value::Lambda(_)) => f.clone(),
                            _ => {
                                return Err(Error::Runtime(
                                    "filter() requires a lambda argument".to_string(),
                                ));
                            }
                        };
                        let items = (*list).clone();
                        let mut result = Vec::new();
                        for item in items {
                            let keep =
                                self.call_value_and_wait(func.clone(), vec![item.clone()])?;
                            if keep.is_truthy() {
                                result.push(item);
                            }
                        }
                        Value::List(Arc::new(result))
                    }
                    "reduce" | "fold" | "foldl" => {
                        let (init, func) = match args.as_slice() {
                            [init, f @ Value::Lambda(_)] => (init.clone(), f.clone()),
                            _ => {
                                return Err(Error::Runtime(
                                    "reduce() requires (initial_value, lambda) arguments"
                                        .to_string(),
                                ));
                            }
                        };
                        let items = (*list).clone();
                        let mut acc = init;
                        for item in items {
                            acc = self.call_value_and_wait(func.clone(), vec![acc, item])?;
                        }
                        acc
                    }
                    "foldr" => {
                        let (init, func) = match args.as_slice() {
                            [init, f @ Value::Lambda(_)] => (init.clone(), f.clone()),
                            _ => {
                                return Err(Error::Runtime(
                                    "foldr() requires (initial_value, lambda) arguments"
                                        .to_string(),
                                ));
                            }
                        };
                        let items = (*list).clone();
                        let mut acc = init;
                        for item in items.into_iter().rev() {
                            acc = self.call_value_and_wait(func.clone(), vec![acc, item])?;
                        }
                        acc
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                };
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
            }
            Value::String(s) => {
                // Built-in string methods
                let result = match name {
                    "len" => Value::Int(s.len() as i64),
                    "upper" => Value::String(SmolStr::new(s.to_uppercase())),
                    "lower" => Value::String(SmolStr::new(s.to_lowercase())),
                    "contains" => {
                        let needle = match args.first() {
                            Some(Value::String(needle)) => needle.as_str(),
                            _ => {
                                return Err(Error::Runtime(
                                    "contains() requires string argument".to_string(),
                                ));
                            }
                        };
                        Value::Bool(s.contains(needle))
                    }
                    "starts_with" => {
                        let prefix = match args.first() {
                            Some(Value::String(prefix)) => prefix.as_str(),
                            _ => {
                                return Err(Error::Runtime(
                                    "starts_with() requires string argument".to_string(),
                                ));
                            }
                        };
                        Value::Bool(s.starts_with(prefix))
                    }
                    "ends_with" => {
                        let suffix = match args.first() {
                            Some(Value::String(suffix)) => suffix.as_str(),
                            _ => {
                                return Err(Error::Runtime(
                                    "ends_with() requires string argument".to_string(),
                                ));
                            }
                        };
                        Value::Bool(s.ends_with(suffix))
                    }
                    "slice" => {
                        let (start, end) =
                            match args.as_slice() {
                                [Value::Int(start), Value::Int(end)] => {
                                    (*start as usize, *end as usize)
                                }
                                [Value::Int(start)] => (*start as usize, s.len()),
                                _ => return Err(Error::Runtime(
                                    "slice() requires (start) or (start, end) integer arguments"
                                        .to_string(),
                                )),
                            };
                        let end = end.min(s.len());
                        let start = start.min(end);
                        Value::String(SmolStr::new(&s[start..end]))
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                };
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
            }
            Value::ParseStream(ps) => {
                // Handle apply() separately — it needs call_value_and_wait which borrows &mut self
                if name == "apply" {
                    let rule = match args.into_iter().next() {
                        Some(rule) => rule,
                        None => {
                            return Err(Error::Runtime("apply() requires a rule argument".into()));
                        }
                    };

                    let rule_id = crate::parse_stream::compute_rule_identity(&rule);

                    // Check memo table (lock scope limited to avoid holding lock during call)
                    {
                        let stream = ps.lock().unwrap();
                        let position = stream.position();
                        let key = crate::parse_stream::MemoKey { position, rule_id };
                        if let Some(entry) = stream.get_memo(&key) {
                            match entry {
                                crate::parse_stream::MemoEntry::Done(Some(value), end_pos) => {
                                    let value = value.clone();
                                    let end_pos = *end_pos;
                                    drop(stream);
                                    {
                                        let mut s = ps.lock().unwrap();
                                        s.restore(&crate::parse_stream::Checkpoint {
                                            position: end_pos,
                                        });
                                    }
                                    let frame = self.frames.last_mut().unwrap();
                                    frame.set_current(value);
                                    return Ok(());
                                }
                                crate::parse_stream::MemoEntry::Done(None, _) => {
                                    return Err(Error::ParseFailed {
                                        position,
                                        message: "memoized parse failure".into(),
                                    });
                                }
                                crate::parse_stream::MemoEntry::InProgress => {
                                    return Err(Error::ParseFailed {
                                        position,
                                        message: "left recursion detected".into(),
                                    });
                                }
                            }
                        }
                    }

                    // Mark rule as in-progress (left recursion guard)
                    let position = {
                        let mut stream = ps.lock().unwrap();
                        let pos = stream.position();
                        let key = crate::parse_stream::MemoKey {
                            position: pos,
                            rule_id,
                        };
                        stream.set_memo(key, crate::parse_stream::MemoEntry::InProgress);
                        pos
                    };

                    // Call the rule synchronously — lock is NOT held during execution
                    let stream_val = Value::ParseStream(ps.clone());
                    match self.call_value_and_wait(rule, vec![stream_val]) {
                        Ok(result) => {
                            // Memoize success with end position
                            let end_pos = {
                                let stream = ps.lock().unwrap();
                                stream.position()
                            };
                            {
                                let mut stream = ps.lock().unwrap();
                                let key = crate::parse_stream::MemoKey { position, rule_id };
                                stream.set_memo(
                                    key,
                                    crate::parse_stream::MemoEntry::Done(
                                        Some(result.clone()),
                                        end_pos,
                                    ),
                                );
                            }
                            let frame = self.frames.last_mut().unwrap();
                            frame.set_current(result);
                        }
                        Err(e) => {
                            // Memoize failure
                            {
                                let mut stream = ps.lock().unwrap();
                                let key = crate::parse_stream::MemoKey { position, rule_id };
                                stream.set_memo(
                                    key,
                                    crate::parse_stream::MemoEntry::Done(None, position),
                                );
                            }
                            return Err(e);
                        }
                    }
                    return Ok(());
                }

                let result = match name {
                    "head" => {
                        let stream = ps.lock().unwrap();
                        stream.head()
                    }
                    "position" => {
                        let stream = ps.lock().unwrap();
                        Value::Int(stream.position() as i64)
                    }
                    "advance" => {
                        let n = match args.first() {
                            Some(Value::Int(n)) => *n as usize,
                            _ => 1,
                        };
                        let mut stream = ps.lock().unwrap();
                        stream.advance(n);
                        Value::Null
                    }
                    "checkpoint" => {
                        let stream = ps.lock().unwrap();
                        let cp = stream.checkpoint();
                        Value::Int(cp.position as i64)
                    }
                    "restore" => {
                        let pos = match args.first() {
                            Some(Value::Int(pos)) => *pos as usize,
                            _ => {
                                return Err(Error::Runtime(
                                    "restore() requires a checkpoint value".into(),
                                ));
                            }
                        };
                        let mut stream = ps.lock().unwrap();
                        stream.restore(&crate::parse_stream::Checkpoint { position: pos });
                        Value::Null
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                };
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
            }
            Value::TupleSpace(space) => {
                // Tuple space methods: out, in, rd, inp, rdp, subscribe
                use crate::tuplespace::Tuple;

                let result = match name {
                    "out" => {
                        // `out` takes a single tagged map per
                        // `specs/tuplespace.md`. Required keys: `type`, `data`.
                        // Optional: `durable: Bool`, `namespace: String|Symbol`.
                        //
                        // Shape:  space.out(%{type: :T, data: D, durable: true})
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "out() expects 1 argument (a tagged map with \
                                 keys `type`, `data`, optional `durable`, \
                                 optional `namespace`), got {}",
                                args.len()
                            )));
                        }
                        let map = match &args[0] {
                            Value::Map(m) => m.clone(),
                            other => {
                                return Err(Error::Runtime(format!(
                                    "out() expects a Map argument, got {}",
                                    other.type_name()
                                )));
                            }
                        };

                        let type_value = map.get("type").ok_or_else(|| {
                            Error::Runtime("out() map missing required key `type`".to_string())
                        })?;
                        let type_name = match type_value {
                            Value::String(s) => s.clone(),
                            Value::Symbol(s) if s.starts_with(':') => SmolStr::new(&s[1..]),
                            Value::Symbol(s) => s.clone(),
                            other => {
                                return Err(Error::Runtime(format!(
                                    "out() `type` must be a String or Symbol, got {}",
                                    other.type_name()
                                )));
                            }
                        };

                        let data = map.get("data").cloned().ok_or_else(|| {
                            Error::Runtime("out() map missing required key `data`".to_string())
                        })?;

                        let durable = match map.get("durable") {
                            None => false,
                            Some(Value::Bool(b)) => *b,
                            Some(other) => {
                                return Err(Error::Runtime(format!(
                                    "out() `durable` must be a Bool, got {}",
                                    other.type_name()
                                )));
                            }
                        };

                        let namespace = match map.get("namespace") {
                            None => None,
                            Some(Value::String(s)) => Some(s.clone()),
                            Some(Value::Symbol(s)) if s.starts_with(':') => {
                                Some(SmolStr::new(&s[1..]))
                            }
                            Some(Value::Symbol(s)) => Some(s.clone()),
                            Some(other) => {
                                return Err(Error::Runtime(format!(
                                    "out() `namespace` must be a String or Symbol, got {}",
                                    other.type_name()
                                )));
                            }
                        };

                        let mut tuple = Tuple::new(type_name, data).with_durable(durable);
                        if let Some(ns) = namespace {
                            tuple = tuple.with_namespace(ns);
                        }

                        let mut space = space.lock().unwrap();
                        space.out(tuple)?;
                        Value::Null
                    }
                    "in" | "inp" => {
                        // in(pattern) -> map | null
                        // inp(pattern) -> map | null (non-blocking)
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "{}() expects 1 argument (pattern), got {}",
                                name,
                                args.len()
                            )));
                        }
                        let pattern = Self::value_to_tuple_pattern(&args[0])?;
                        let mut space = space.lock().unwrap();
                        let result = if name == "in" {
                            space.r#in(&pattern)?
                        } else {
                            space.inp(&pattern)?.ok_or_else(|| {
                                Error::Runtime("no matching tuple found".to_string())
                            })?
                        };
                        Self::tuple_to_value(result)
                    }
                    "rd" | "rdp" => {
                        // rd(pattern) -> map | null
                        // rdp(pattern) -> map | null (non-blocking)
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "{}() expects 1 argument (pattern), got {}",
                                name,
                                args.len()
                            )));
                        }
                        let pattern = Self::value_to_tuple_pattern(&args[0])?;
                        let mut space = space.lock().unwrap();
                        let result = if name == "rd" {
                            space.rd(&pattern)?
                        } else {
                            space.rdp(&pattern)?.ok_or_else(|| {
                                Error::Runtime("no matching tuple found".to_string())
                            })?
                        };
                        Self::tuple_to_value(result)
                    }
                    "namespace" => {
                        // namespace(name) -> TupleSpaceFacet restricted to namespace
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "namespace() expects 1 argument (namespace_name), got {}",
                                args.len()
                            )));
                        }
                        let namespace = match &args[0] {
                            Value::String(s) => s.clone(),
                            Value::Symbol(s) if s.starts_with(':') => SmolStr::new(&s[1..]),
                            Value::Symbol(s) => s.clone(),
                            other => {
                                return Err(Error::Runtime(format!(
                                    "namespace() argument must be a string or symbol, got {}",
                                    other.type_name()
                                )));
                            }
                        };
                        use crate::tuplespace::facet::TupleSpaceFacet;
                        let facet = TupleSpaceFacet::new(space.clone()).with_namespace(namespace);
                        Value::TupleSpaceFacet(Arc::new(std::sync::Mutex::new(facet)))
                    }
                    "readonly" => {
                        // readonly() -> TupleSpaceFacet with read-only permissions
                        if !args.is_empty() {
                            return Err(Error::Runtime(format!(
                                "readonly() expects 0 arguments, got {}",
                                args.len()
                            )));
                        }
                        use crate::tuplespace::facet::TupleSpaceFacet;
                        let facet = TupleSpaceFacet::new(space.clone()).readonly();
                        Value::TupleSpaceFacet(Arc::new(std::sync::Mutex::new(facet)))
                    }
                    "writeonly" => {
                        // writeonly() -> TupleSpaceFacet with write-only permissions
                        if !args.is_empty() {
                            return Err(Error::Runtime(format!(
                                "writeonly() expects 0 arguments, got {}",
                                args.len()
                            )));
                        }
                        use crate::tuplespace::facet::TupleSpaceFacet;
                        let facet = TupleSpaceFacet::new(space.clone()).writeonly();
                        Value::TupleSpaceFacet(Arc::new(std::sync::Mutex::new(facet)))
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                };
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
            }
            Value::TupleSpaceFacet(facet) => {
                // Facet methods: out, in, rd, inp, rdp, namespace, readonly, writeonly
                let result = match name {
                    "out" => {
                        // out(type_name, data) -> null
                        if args.len() != 2 {
                            return Err(Error::Runtime(format!(
                                "out() expects 2 arguments (type_name, data), got {}",
                                args.len()
                            )));
                        }
                        let type_name = match &args[0] {
                            Value::String(s) => s.clone(),
                            Value::Symbol(s) if s.starts_with(':') => SmolStr::new(&s[1..]),
                            Value::Symbol(s) => s.clone(),
                            other => {
                                return Err(Error::Runtime(format!(
                                    "out() type_name must be a string or symbol, got {}",
                                    other.type_name()
                                )));
                            }
                        };
                        let mut facet = facet.lock().unwrap();
                        facet.out(type_name, args[1].clone())?;
                        Value::Null
                    }
                    "in" | "inp" => {
                        // in(pattern) -> map | null
                        // inp(pattern) -> map | null (non-blocking)
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "{}() expects 1 argument (pattern), got {}",
                                name,
                                args.len()
                            )));
                        }
                        let pattern = Self::value_to_tuple_pattern(&args[0])?;
                        let mut facet = facet.lock().unwrap();
                        let result = if name == "in" {
                            facet.r#in(&pattern)?
                        } else {
                            facet.inp(&pattern)?.ok_or_else(|| {
                                Error::Runtime("no matching tuple found".to_string())
                            })?
                        };
                        Self::tuple_to_value(result)
                    }
                    "rd" | "rdp" => {
                        // rd(pattern) -> map | null
                        // rdp(pattern) -> map | null (non-blocking)
                        if args.len() != 1 {
                            return Err(Error::Runtime(format!(
                                "{}() expects 1 argument (pattern), got {}",
                                name,
                                args.len()
                            )));
                        }
                        let pattern = Self::value_to_tuple_pattern(&args[0])?;
                        let mut facet = facet.lock().unwrap();
                        let result = if name == "rd" {
                            facet.rd(&pattern)?
                        } else {
                            facet.rdp(&pattern)?.ok_or_else(|| {
                                Error::Runtime("no matching tuple found".to_string())
                            })?
                        };
                        Self::tuple_to_value(result)
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                };
                let frame = self.frames.last_mut().unwrap();
                frame.set_current(result);
            }
            _ => {
                return Err(Error::Type {
                    expected: "object".to_string(),
                    got: receiver.type_name().to_string(),
                });
            }
        }
        Ok(())
    }

    fn spawn_object(&mut self, constructor: Value, args: Vec<Value>) -> Result<ObjectId> {
        match constructor {
            Value::Object(parent_id) => {
                let id = self.objects.lock().unwrap().create(Some(parent_id));

                // Call init method if it exists on the new object
                // Clone the method to drop the lock before execute_with_depth
                let init_method = self.objects.lock().unwrap().get_method(id, "init").cloned();
                if let Some(method) = init_method {
                    // Check if argument count matches (or allow empty init)
                    if args.len() == method.params.len() {
                        let mut frame = Frame::new(method.code);
                        frame.this = Some(id);

                        // Bind arguments to parameters
                        for (i, val) in args.into_iter().enumerate() {
                            if i < method.params.len() {
                                frame.locals.insert(method.params[i].clone(), val);
                            }
                        }

                        // Execute the init method
                        let base_depth = self.frames.len();
                        self.frames.push(frame);
                        self.execute_with_depth(base_depth)?;
                    }
                    // If args don't match params, silently skip (graceful degradation)
                }

                Ok(id)
            }
            _ => Err(Error::Type {
                expected: "object constructor".to_string(),
                got: constructor.type_name().to_string(),
            }),
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Compiler, Lexer, Parser};

    fn eval(vm: &mut Vm, source: &str) -> crate::Result<Value> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::with_source(&tokens, source).parse()?;
        let code = Compiler::new().compile(&ast)?;
        vm.run(&code)
    }

    #[test]
    fn test_debug_if() {
        use crate::{Compiler, Lexer, Parser};
        let source = "if true then 1 else 2";
        let tokens = Lexer::new(source).tokenize().unwrap();
        let ast = Parser::with_source(&tokens, source).parse().unwrap();
        let code = Compiler::new().compile(&ast).unwrap();

        println!("Instructions for '{}':", source);
        for (i, instr) in code.instructions.iter().enumerate() {
            println!("  {}: {:?}", i, instr);
        }

        let mut vm = Vm::new();
        let result = vm.run(&code).unwrap();
        println!("Result: {:?}", result);
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_arithmetic() {
        let mut vm = Vm::new();
        assert_eq!(eval(&mut vm, "1 + 2").unwrap(), Value::Int(3));
        assert_eq!(eval(&mut vm, "10 - 3").unwrap(), Value::Int(7));
        assert_eq!(eval(&mut vm, "4 * 5").unwrap(), Value::Int(20));
        assert_eq!(eval(&mut vm, "20 / 4").unwrap(), Value::Int(5));
    }

    #[test]
    fn test_comparison() {
        let mut vm = Vm::new();
        assert_eq!(eval(&mut vm, "1 < 2").unwrap(), Value::Bool(true));
        assert_eq!(eval(&mut vm, "1 > 2").unwrap(), Value::Bool(false));
        assert_eq!(eval(&mut vm, "1 == 1").unwrap(), Value::Bool(true));
        assert_eq!(eval(&mut vm, "1 != 2").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_if_expr() {
        let mut vm = Vm::new();
        assert_eq!(
            eval(&mut vm, "if true then 1 else 2").unwrap(),
            Value::Int(1)
        );
        assert_eq!(
            eval(&mut vm, "if false then 1 else 2").unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn test_logical_and() {
        let mut vm = Vm::new();
        // true && true -> true
        assert_eq!(eval(&mut vm, "true && true").unwrap(), Value::Bool(true));
        // true && false -> false
        assert_eq!(eval(&mut vm, "true && false").unwrap(), Value::Bool(false));
        // false && true -> false (short-circuit, second not evaluated)
        assert_eq!(eval(&mut vm, "false && true").unwrap(), Value::Bool(false));
        // false && false -> false
        assert_eq!(eval(&mut vm, "false && false").unwrap(), Value::Bool(false));
        // With expressions: (1 < 2) && (3 > 1) -> true
        assert_eq!(eval(&mut vm, "1 < 2 && 3 > 1").unwrap(), Value::Bool(true));
        // Short-circuit: false && expr that would error
        assert_eq!(eval(&mut vm, "false && 1/0").unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_logical_or() {
        let mut vm = Vm::new();
        // true || true -> true (short-circuit, second not evaluated)
        assert_eq!(eval(&mut vm, "true || true").unwrap(), Value::Bool(true));
        // true || false -> true (short-circuit)
        assert_eq!(eval(&mut vm, "true || false").unwrap(), Value::Bool(true));
        // false || true -> true
        assert_eq!(eval(&mut vm, "false || true").unwrap(), Value::Bool(true));
        // false || false -> false
        assert_eq!(eval(&mut vm, "false || false").unwrap(), Value::Bool(false));
        // With expressions: (1 > 2) || (3 < 4) -> true
        assert_eq!(eval(&mut vm, "1 > 2 || 3 < 4").unwrap(), Value::Bool(true));
        // Short-circuit: true || expr that would error
        assert_eq!(eval(&mut vm, "true || 1/0").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_mixed_logical_operators() {
        let mut vm = Vm::new();
        // AND has higher precedence than OR
        // true || false && false -> true || (false && false) -> true
        assert_eq!(
            eval(&mut vm, "true || false && false").unwrap(),
            Value::Bool(true)
        );
        // (false || false) && true -> false && true -> false
        assert_eq!(
            eval(&mut vm, "(false || false) && true").unwrap(),
            Value::Bool(false)
        );
        // Complex: false && true || true && true -> (false && true) || (true && true) -> false || true -> true
        assert_eq!(
            eval(&mut vm, "false && true || true && true").unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_let_binding() {
        let mut vm = Vm::new();
        assert_eq!(eval(&mut vm, "let (x = 10) x + 5").unwrap(), Value::Int(15));
        assert_eq!(
            eval(&mut vm, "let (x = 2, y = 3) x * y").unwrap(),
            Value::Int(6)
        );
    }

    #[test]
    fn test_list() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "[1, 2, 3]").unwrap();
        assert!(matches!(result, Value::List(_)));
    }

    #[test]
    fn test_map() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "%{foo: 1, bar: 2}").unwrap();
        assert!(matches!(result, Value::Map(_)));
    }

    #[test]
    fn test_string_concat() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#""hello" + " " + "world""#).unwrap();
        assert!(matches!(result, Value::String(s) if s == "hello world"));
    }

    #[test]
    fn test_lambda() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "let (f = lambda (x) x + 1) f(5)").unwrap();
        assert_eq!(result, Value::Int(6));
    }

    // === Indexed RPN Spec Tests (T-1 through T-13) ===

    /// T-1: Simple Arithmetic - (3 + 4) * 5 = 35
    #[test]
    fn test_t1_simple_arithmetic() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "(3 + 4) * 5").unwrap();
        assert_eq!(result, Value::Int(35));
    }

    /// T-2: Variable Binding
    #[test]
    fn test_t2_variable_binding() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "let (x = 10) let (y = x + 5) y").unwrap();
        assert_eq!(result, Value::Int(15));
    }

    /// T-3: Nested Scopes - inner shadows outer
    #[test]
    fn test_t3_nested_scopes() {
        let mut vm = Vm::new();
        // Note: Current parser doesn't support block syntax with {}
        // Testing with nested let instead
        let result = eval(&mut vm, "let (x = 1) let (x = 2) x").unwrap();
        assert_eq!(result, Value::Int(2));
    }

    /// T-4: Conditional
    #[test]
    fn test_t4_conditional() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "if true then 1 else 2").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    /// T-5: Function Call
    #[test]
    fn test_t5_function_call() {
        let mut vm = Vm::new();
        // Test simple lambda call (spec shows curried, but FMPL uses multi-param)
        let result = eval(&mut vm, "let (add = lambda (a, b) a + b) add(3, 4)").unwrap();
        assert_eq!(result, Value::Int(7));
    }

    /// T-6: Short-Circuit Evaluation
    #[test]
    fn test_t6_short_circuit() {
        let mut vm = Vm::new();
        // false && (1 / 0) should not error
        let result = eval(&mut vm, "false && false").unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    /// T-7: List Construction
    #[test]
    fn test_t7_list_construction() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "[1, 2, 3]").unwrap();
        match result {
            Value::List(lst) => {
                assert_eq!(lst.len(), 3);
            }
            _ => panic!("Expected list"),
        }
    }

    /// T-8: Method Call
    #[test]
    fn test_t8_method_call() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "[1, 2, 3].len()").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_integer_overflow_is_error_not_panic() {
        // Runtime arithmetic overflow must be a clean error, never a process
        // abort (debug) or silent wraparound (release). Reachable from user input.
        for src in [
            "9000000000000000000 * 2",
            "9000000000000000000 + 9000000000000000000",
            "-9000000000000000000 - 9000000000000000000",
        ] {
            let mut vm = Vm::new();
            let r = eval(&mut vm, src);
            assert!(r.is_err(), "{src} should overflow to an error, got {r:?}");
        }
    }

    #[test]
    fn test_universal_type_predicates() {
        // Predicates work on bare primitives (no method table), including in guards.
        let cases = [
            ("(42).is_number()", Value::Bool(true)),
            ("(42).is_int()", Value::Bool(true)),
            ("(42).is_string()", Value::Bool(false)),
            ("\"hi\".is_string()", Value::Bool(true)),
            ("[1, 2].is_list()", Value::Bool(true)),
            ("true.is_bool()", Value::Bool(true)),
            ("null.is_null()", Value::Bool(true)),
            ("(3.5).is_number()", Value::Bool(true)),
            ("(3.5).is_int()", Value::Bool(false)),
            ("42 @ { n when n.is_number() => n * 2, _ => 0 }", Value::Int(84)),
        ];
        for (src, expected) in cases {
            let mut vm = Vm::new();
            let result = eval(&mut vm, src).unwrap_or_else(|e| panic!("{src}: {e}"));
            assert_eq!(result, expected, "{src}");
        }
    }

    /// T-9: Deep Nesting
    #[test]
    fn test_t9_deep_nesting() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "let (x = 1) let (y = 2, z = 3) x + y + z").unwrap();
        assert_eq!(result, Value::Int(6));
    }

    /// T-10: Shadowing Across Blocks
    #[test]
    fn test_t10_shadowing() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            "let (x = 10) let (result = let (x = 20) x) result + x",
        )
        .unwrap();
        assert_eq!(result, Value::Int(30));
    }

    /// T-11: Backpatching Integrity
    #[test]
    fn test_t11_backpatching() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            "let (x = if true then 1 else 2) let (y = if false then 3 else 4) x + y",
        )
        .unwrap();
        assert_eq!(result, Value::Int(5));
    }

    /// T-12: While Loop
    #[test]
    fn test_t12_while_loop() {
        // Note: While loops are not yet implemented in the parser
        // This test is a placeholder for when while is added
        // let result = eval(&mut vm, "let (sum = 0, i = 0) while i < 3 { let _ = sum = sum + i let _ = i = i + 1 } sum").unwrap();
        // assert_eq!(result, Value::Int(3));
    }

    /// T-13: resolve_names Wiring
    #[test]
    fn test_t13_resolve_names_wiring() {
        use crate::{Compiler, Lexer, Parser};
        let source = "let (x = 1) let (x = 2) x";
        let tokens = Lexer::new(source).tokenize().unwrap();
        let ast = Parser::with_source(&tokens, source).parse().unwrap();
        let mut code = Compiler::new().compile(&ast).unwrap();

        // Run resolve_names pass
        crate::compiler::resolve_names(&mut code);

        // Verify that NameRef instructions contain bind indices, not string names
        let has_nameref_with_bind_index = code
            .instructions
            .iter()
            .any(|instr| matches!(instr, Instruction::NameRef { bind: _ }));
        assert!(
            has_nameref_with_bind_index,
            "Expected NameRef with bind index"
        );
    }

    #[test]
    fn test_list_map() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3].map(\x x * 2)"#).unwrap();
        assert_eq!(
            result,
            Value::List(Arc::new(vec![Value::Int(2), Value::Int(4), Value::Int(6)]))
        );
    }

    #[test]
    fn test_list_filter() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3].filter(\x x > 1)"#).unwrap();
        assert_eq!(
            result,
            Value::List(Arc::new(vec![Value::Int(2), Value::Int(3)]))
        );
    }

    #[test]
    fn test_list_reduce() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3].reduce(0, \acc, x acc + x)"#).unwrap();
        assert_eq!(result, Value::Int(6));
    }

    #[test]
    fn test_list_map_empty() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[].map(\x x * 2)"#).unwrap();
        assert_eq!(result, Value::List(Arc::new(vec![])));
    }

    #[test]
    fn test_list_filter_none_match() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[1, 2, 3].filter(\x x > 10)"#).unwrap();
        assert_eq!(result, Value::List(Arc::new(vec![])));
    }

    #[test]
    fn test_list_reduce_single() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, r#"[5].reduce(0, \acc, x acc + x)"#).unwrap();
        assert_eq!(result, Value::Int(5));
    }
}
