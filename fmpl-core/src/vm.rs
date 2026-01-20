//! Virtual Machine for FMPL bytecode execution.

use crate::compiler::{CompiledCode, Instruction};
use crate::error::{Error, Result};
use crate::grammar::{Grammar, GrammarRegistry};
use crate::object::{Facet, Method, ObjectDb, ObjectId};
use crate::value::{Lambda, Stream, StreamOp, Value};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// A call frame.
#[derive(Debug)]
struct Frame {
    code: Arc<CompiledCode>,
    ip: usize,
    #[allow(dead_code)]
    base: usize, // Stack base for this frame (used for future stack unwinding)
    locals: HashMap<SmolStr, Value>,
    this: Option<ObjectId>,
    caller: Option<ObjectId>,
    next_nested: usize,
}

impl Frame {
    fn new(code: Arc<CompiledCode>, base: usize) -> Self {
        Self {
            code,
            ip: 0,
            base,
            locals: HashMap::new(),
            this: None,
            caller: None,
            next_nested: 0,
        }
    }
}

/// Scope for let bindings.
#[derive(Debug, Default)]
struct Scope {
    bindings: HashMap<SmolStr, Value>,
}

/// The FMPL virtual machine.
pub struct Vm {
    pub objects: ObjectDb,
    pub grammars: GrammarRegistry,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    scopes: Vec<Scope>,
    /// The current user (for `user` builtin).
    pub current_user: Option<ObjectId>,
    /// Exception handler stack: (catch_ip, stack_depth, frame_depth)
    exception_handlers: Vec<(usize, usize, usize)>,
    /// Tokio runtime handle for async operations
    runtime: Option<tokio::runtime::Handle>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            objects: ObjectDb::new(),
            grammars: GrammarRegistry::new(),
            stack: Vec::new(),
            frames: Vec::new(),
            scopes: vec![Scope::default()],
            current_user: None,
            exception_handlers: Vec::new(),
            runtime: None,
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

    /// Get the runtime handle, if set.
    pub fn runtime(&self) -> Option<&tokio::runtime::Handle> {
        self.runtime.as_ref()
    }

    /// Run compiled code and return the result.
    pub fn run(&mut self, code: &CompiledCode) -> Result<Value> {
        let code = Arc::new(code.clone());
        let frame = Frame::new(code, self.stack.len());
        self.frames.push(frame);

        self.execute()?;

        self.stack.pop().ok_or(Error::StackUnderflow)
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
    ///
    /// If the input is an `AsyncStream`, uses `StreamingPegRuntime` which pulls
    /// values lazily from the stream with blocking recv.
    pub fn apply_grammar(
        &mut self,
        input: Value,
        grammar: Arc<Grammar>,
        rule_name: &str,
    ) -> Result<Option<Value>> {
        use crate::grammar::runtime::{
            apply_grammar_to_stream_with_evaluator, apply_grammar_to_value_with_evaluator,
        };

        // Clone the grammar registry so we can pass it to the runtime.
        // This is cheap since grammars are Arc-wrapped.
        let registry = self.grammars.clone();

        // Create an action evaluator that calls back into the VM.
        // This is safe now that ActionEvaluator<'e> supports non-'static lifetimes.
        let evaluator = Box::new(
            |expr: &crate::ast::Expr, bindings: &HashMap<SmolStr, Value>| {
                self.eval_with_bindings(expr, bindings)
            },
        );

        // If input is an AsyncStream, use streaming grammar application
        if let Value::AsyncStream(stream_arc) = input {
            // Extract the StreamHandle from the Mutex
            // We need to take ownership, which means the stream is consumed
            let mut guard = stream_arc
                .lock()
                .map_err(|_| Error::Runtime("failed to lock async stream".to_string()))?;

            // Create a new receiver by swapping with a closed one
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

    /// Main execution loop.
    fn execute(&mut self) -> Result<()> {
        while !self.frames.is_empty() {
            let frame = self.frames.last().unwrap();

            if frame.ip >= frame.code.instructions.len() {
                // End of code, pop frame
                self.frames.pop();
                continue;
            }

            let instr = frame.code.instructions[frame.ip].clone();
            let frame = self.frames.last_mut().unwrap();
            frame.ip += 1;

            match instr {
                Instruction::LoadNull => {
                    self.stack.push(Value::Null);
                }
                Instruction::LoadBool(b) => {
                    self.stack.push(Value::Bool(b));
                }
                Instruction::LoadInt(n) => {
                    self.stack.push(Value::Int(n));
                }
                Instruction::LoadFloat(f) => {
                    self.stack.push(Value::Float(f));
                }
                Instruction::LoadString(s) => {
                    self.stack.push(Value::String(s));
                }
                Instruction::LoadSymbol(s) => {
                    self.stack.push(Value::Symbol(s));
                }
                Instruction::LoadVar(name) => {
                    let val = self.lookup_var(&name)?;
                    self.stack.push(val);
                }
                Instruction::StoreVar(name) => {
                    let val = self.pop()?;
                    self.store_var(name, val);
                }
                Instruction::LoadSelf => {
                    let frame = self.frames.last().unwrap();
                    if let Some(id) = frame.this {
                        self.stack.push(Value::Object(id));
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instruction::LoadParent => {
                    let frame = self.frames.last().unwrap();
                    if let Some(id) = frame.this {
                        if let Some(obj) = self.objects.get(id) {
                            if let Some(parent) = obj.parent {
                                self.stack.push(Value::Object(parent));
                            } else {
                                self.stack.push(Value::Null);
                            }
                        } else {
                            self.stack.push(Value::Null);
                        }
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instruction::LoadCaller => {
                    let frame = self.frames.last().unwrap();
                    if let Some(id) = frame.caller {
                        self.stack.push(Value::Object(id));
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instruction::LoadUser => {
                    if let Some(id) = self.current_user {
                        self.stack.push(Value::Object(id));
                    } else {
                        self.stack.push(Value::Null);
                    }
                }
                Instruction::LoadArgs => {
                    // TODO: proper args handling
                    self.stack.push(Value::List(Arc::new(Vec::new())));
                }
                Instruction::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.add(&b)?);
                }
                Instruction::Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.sub(&b)?);
                }
                Instruction::Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.mul(&b)?);
                }
                Instruction::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.try_op(|_| a.div(&b))?;
                }
                Instruction::Mod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.modulo(&b)?);
                }
                Instruction::Neg => {
                    let a = self.pop()?;
                    self.stack.push(a.neg()?);
                }
                Instruction::Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.eq(&b));
                }
                Instruction::NotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.ne(&b));
                }
                Instruction::Lt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.lt(&b)?);
                }
                Instruction::Gt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.gt(&b)?);
                }
                Instruction::LtEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.le(&b)?);
                }
                Instruction::GtEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(a.ge(&b)?);
                }
                Instruction::Not => {
                    let a = self.pop()?;
                    self.stack.push(a.not());
                }
                Instruction::And | Instruction::Or => {
                    // These are handled with jumps, shouldn't reach here
                    unreachable!("And/Or handled with jumps");
                }
                Instruction::Jump(target) => {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target;
                }
                Instruction::JumpIfFalse(target) => {
                    let val = self.peek()?;
                    if val.is_falsy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }
                Instruction::JumpIfTrue(target) => {
                    let val = self.peek()?;
                    if val.is_truthy() {
                        let frame = self.frames.last_mut().unwrap();
                        frame.ip = target;
                    }
                }
                Instruction::Call(argc) => {
                    let func = self.pop()?;
                    self.call_value(func, argc)?;
                }
                Instruction::TailCall(argc) => {
                    let func = self.pop()?;
                    // Pop current frame first
                    self.frames.pop();
                    self.call_value(func, argc)?;
                }
                Instruction::MethodCall(name, argc) => {
                    // Get arguments and receiver
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let receiver = self.pop()?;
                    self.call_method(receiver, &name, args)?;
                }
                Instruction::Return => {
                    // Keep the return value on the stack
                    self.frames.pop();
                }
                Instruction::GetProp(name) => {
                    let obj = self.pop()?;
                    match obj {
                        Value::Object(id) => {
                            if let Some(val) = self.objects.get_property(id, &name) {
                                self.stack.push(val);
                            } else {
                                return Err(Error::UndefinedProperty(name.to_string()));
                            }
                        }
                        Value::Map(map) => {
                            if let Some(val) = map.get(&name) {
                                self.stack.push(val.clone());
                            } else {
                                return Err(Error::UndefinedProperty(name.to_string()));
                            }
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "object or map".to_string(),
                                got: obj.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::SetProp(name) => {
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    match obj {
                        Value::Object(id) => {
                            self.objects.set_property(id, name, val.clone())?;
                            self.stack.push(val);
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "object".to_string(),
                                got: obj.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::Spawn(argc) => {
                    // Get constructor and args
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let constructor = self.pop()?;
                    let obj_id = self.spawn_object(constructor, args)?;
                    self.stack.push(Value::Object(obj_id));
                }
                Instruction::GetFacet(facet_name) => {
                    let obj = self.pop()?;
                    match obj {
                        Value::Object(id) => {
                            if self.objects.get_facet(id, &facet_name).is_some() {
                                // TODO: create a faceted view wrapper
                                // For now, just return the object
                                self.stack.push(Value::Object(id));
                            } else {
                                return Err(Error::Runtime(format!(
                                    "undefined facet: {}",
                                    facet_name
                                )));
                            }
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "object".to_string(),
                                got: obj.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::SyncCall => {
                    // In Phase 1, sync call just evaluates the expression
                    // (no distinction from regular call)
                }
                Instruction::AsyncCall => {
                    let value = self.pop()?;

                    // For now, <- on a non-callable just wraps in a completed stream
                    // Real async calls will come with curl integration
                    if self.runtime.is_none() {
                        return Err(Error::Runtime(
                            "async call requires runtime handle - use Vm::with_runtime()"
                                .to_string(),
                        ));
                    }

                    // Create a stream that immediately completes with the value
                    use crate::stream::{StreamEvent, StreamHandle, next_id};
                    use tokio::sync::mpsc;

                    let (tx, rx) = mpsc::channel(1);
                    let _ = tx.try_send(StreamEvent::Ok(value));

                    let handle = StreamHandle::new(rx, next_id());
                    self.stack
                        .push(Value::AsyncStream(Arc::new(std::sync::Mutex::new(handle))));
                }
                Instruction::MakeList(count) => {
                    let mut items = Vec::new();
                    for _ in 0..count {
                        items.push(self.pop()?);
                    }
                    items.reverse();
                    self.stack.push(Value::List(Arc::new(items)));
                }
                Instruction::MakeMap(count) => {
                    let mut map = HashMap::new();
                    for _ in 0..count {
                        let val = self.pop()?;
                        let key = self.pop()?;
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
                    self.stack.push(Value::Map(Arc::new(map)));
                }
                Instruction::Index => {
                    let idx = self.pop()?;
                    let val = self.pop()?;
                    self.stack.push(val.index(&idx)?);
                }
                Instruction::Slice => {
                    let end = self.pop()?;
                    let start = self.pop()?;
                    let val = self.pop()?;
                    // TODO: proper slice implementation
                    let _ = (start, end);
                    self.stack.push(val);
                }
                Instruction::PushScope => {
                    self.scopes.push(Scope::default());
                }
                Instruction::PopScope => {
                    self.scopes.pop();
                }
                Instruction::Bind(name) => {
                    let val = self.pop()?;
                    if let Some(scope) = self.scopes.last_mut() {
                        scope.bindings.insert(name, val);
                    }
                }
                Instruction::MakeLambda(params, nested_idx) => {
                    let frame = self.frames.last().unwrap();
                    let nested_code = frame.code.nested.get(nested_idx).cloned();

                    if let Some(code) = nested_code {
                        // Capture current scope
                        let mut captures = HashMap::new();
                        for scope in &self.scopes {
                            for (k, v) in &scope.bindings {
                                captures.insert(k.clone(), v.clone());
                            }
                        }

                        let lambda = Lambda {
                            params: params.clone(),
                            code: Arc::new(code),
                            captures,
                        };
                        self.stack.push(Value::Lambda(Arc::new(lambda)));
                    } else {
                        return Err(Error::Runtime("invalid lambda code index".to_string()));
                    }
                }
                Instruction::Pop => {
                    self.pop()?;
                }
                Instruction::Dup => {
                    let val = self.peek()?;
                    self.stack.push(val);
                }
                Instruction::Pipe => {
                    // x |> f  =>  f(x)
                    let func = self.pop()?;
                    let arg = self.pop()?;
                    self.stack.push(arg);
                    self.call_value(func, 1)?;
                }
                Instruction::MakeStream => {
                    let source = self.pop()?;
                    let stream = Stream {
                        source,
                        ops: Vec::new(),
                    };
                    self.stack.push(Value::Stream(Arc::new(stream)));
                }
                Instruction::StreamMap => {
                    self.push_stream_op(StreamOp::Map)?;
                }
                Instruction::StreamFilter => {
                    self.push_stream_op(StreamOp::Filter)?;
                }
                Instruction::StreamFlatMap => {
                    self.push_stream_op(StreamOp::FlatMap)?;
                }
                Instruction::StreamReduce => {
                    self.push_stream_op(StreamOp::Reduce)?;
                }
                Instruction::StreamParse(rule) => {
                    self.push_stream_parse(rule)?;
                }
                Instruction::MatchPattern(_) => {
                    // Pattern matching handled differently
                    // This is a placeholder for more complex patterns
                }
                Instruction::ExtractMapKey(key) => {
                    let map = self.pop()?;
                    match map {
                        Value::Map(m) => {
                            let value = m.get(&key).cloned().ok_or_else(|| {
                                Error::Runtime(format!("key '{}' not found in map", key))
                            })?;
                            self.stack.push(value);
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "map".to_string(),
                                got: map.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::ExtractListIndex(idx) => {
                    let list = self.pop()?;
                    match list {
                        Value::List(l) => {
                            let value = l.get(idx).cloned().ok_or_else(|| {
                                Error::Runtime(format!("index {} out of bounds", idx))
                            })?;
                            self.stack.push(value);
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "list".to_string(),
                                got: list.type_name().to_string(),
                            });
                        }
                    }
                }
                Instruction::DefineObject(name) => {
                    let id = self.objects.create(None);
                    self.objects.register_name(name.clone(), id);
                    self.stack.push(Value::Object(id));
                }
                Instruction::DefineMethod(name, _param_count) => {
                    // Pop the object we're defining on
                    let obj = self.peek()?;
                    if let Value::Object(id) = obj {
                        let frame = self.frames.last_mut().unwrap();
                        let nested_idx = frame.next_nested;
                        frame.next_nested += 1;
                        if let Some(code) = frame.code.nested.get(nested_idx) {
                            let method = Method {
                                params: Vec::new(), // TODO: proper params
                                code: Arc::new(code.clone()),
                            };
                            self.objects.define_method(id, name, method)?;
                        }
                    }
                }
                Instruction::DefineProp(name) => {
                    let val = self.pop()?;
                    let obj = self.peek()?;
                    if let Value::Object(id) = obj {
                        self.objects.set_property(id, name, val)?;
                    }
                }
                Instruction::DefineFacet(name, member_count, terminal) => {
                    // Pop the member names
                    let mut members = Vec::new();
                    for _ in 0..member_count {
                        if let Value::Symbol(s) = self.pop()? {
                            members.push(s);
                        }
                    }
                    members.reverse();

                    let obj = self.peek()?;
                    if let Value::Object(id) = obj {
                        let facet = Facet { members, terminal };
                        self.objects.define_facet(id, name, facet)?;
                    }
                }
                Instruction::GrammarApply(rule_name) => {
                    let grammar_val = self.pop()?;
                    let input = self.pop()?;

                    // Resolve grammar to Arc<Grammar>
                    let grammar = match grammar_val {
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

                    // Apply the grammar with action evaluation
                    let result = self.apply_grammar(input, grammar, &rule_name)?;

                    match result {
                        Some(value) => self.stack.push(value),
                        None => {
                            return Err(Error::ParseFailed {
                                position: 0,
                                message: format!("failed to parse with rule {}", rule_name),
                            });
                        }
                    }
                }
                Instruction::LoadGrammar(grammar) => {
                    self.stack.push(Value::Grammar(grammar));
                }
                Instruction::ExtendGrammar(new_rules) => {
                    let base = self.pop()?;
                    match base {
                        Value::Grammar(base_grammar) => {
                            // Create a new grammar with base as parent
                            let mut extended = Grammar::with_parent_grammar(
                                SmolStr::new("<extended>"),
                                base_grammar,
                            );
                            // Add the new rules
                            for (name, rule) in &new_rules.rules {
                                extended.add_rule(name.clone(), rule.clone());
                            }
                            self.stack.push(Value::Grammar(Arc::new(extended)));
                        }
                        _ => {
                            return Err(Error::Type {
                                expected: "grammar".to_string(),
                                got: base.type_name().to_string(),
                            });
                        }
                    }
                }

                // Exception handling
                Instruction::PushHandler(catch_ip) => {
                    let stack_depth = self.stack.len();
                    let frame_depth = self.frames.len();
                    self.exception_handlers
                        .push((catch_ip, stack_depth, frame_depth));
                }
                Instruction::PopHandler => {
                    self.exception_handlers.pop();
                }
                Instruction::Throw => {
                    let error = self.pop()?;
                    self.throw_exception(error)?;
                }
            }
        }

        Ok(())
    }

    fn push_stream_op(&mut self, op: fn(Value) -> StreamOp) -> Result<()> {
        let func = self.pop()?;
        let stream = self.pop()?;
        let Value::Stream(stream) = stream else {
            return Err(Error::Type {
                expected: "stream".to_string(),
                got: stream.type_name().to_string(),
            });
        };

        let mut ops = stream.ops.clone();
        ops.push(op(func));
        let next = Stream {
            source: stream.source.clone(),
            ops,
        };
        self.stack.push(Value::Stream(Arc::new(next)));
        Ok(())
    }

    fn push_stream_parse(&mut self, rule: SmolStr) -> Result<()> {
        let grammar = self.pop()?;
        let stream = self.pop()?;
        let Value::Stream(stream) = stream else {
            return Err(Error::Type {
                expected: "stream".to_string(),
                got: stream.type_name().to_string(),
            });
        };

        let mut ops = stream.ops.clone();
        ops.push(StreamOp::Parse { grammar, rule });
        let next = Stream {
            source: stream.source.clone(),
            ops,
        };
        self.stack.push(Value::Stream(Arc::new(next)));
        Ok(())
    }

    fn pop(&mut self) -> Result<Value> {
        self.stack.pop().ok_or(Error::StackUnderflow)
    }

    fn peek(&self) -> Result<Value> {
        self.stack.last().cloned().ok_or(Error::StackUnderflow)
    }

    fn throw_exception(&mut self, error: Value) -> Result<()> {
        if let Some((catch_ip, stack_depth, frame_depth)) = self.exception_handlers.pop() {
            // Unwind frames to handler's frame
            while self.frames.len() > frame_depth {
                self.frames.pop();
            }
            // Unwind value stack to handler depth
            while self.stack.len() > stack_depth {
                self.stack.pop();
            }
            // Push error value for catch binding
            self.stack.push(error);
            // Jump to catch block in handler's frame
            if let Some(frame) = self.frames.last_mut() {
                frame.ip = catch_ip;
            }
            Ok(())
        } else {
            // No handler - propagate as Rust error
            Err(Error::Runtime(format!("uncaught exception: {}", error)))
        }
    }

    /// Execute an operation that may fail, routing errors to exception handlers if available.
    fn try_op<F>(&mut self, op: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<Value>,
    {
        match op(self) {
            Ok(val) => {
                self.stack.push(val);
                Ok(())
            }
            Err(e) if !self.exception_handlers.is_empty() => {
                let error = Value::String(SmolStr::new(e.to_string()));
                self.throw_exception(error)
            }
            Err(e) => Err(e),
        }
    }

    fn lookup_var(&self, name: &str) -> Result<Value> {
        // Check builtins first
        if name == "curl" {
            // Return a symbol that will be recognized during method calls
            return Ok(Value::Symbol(SmolStr::new("__builtin_curl")));
        }

        // Check scopes (innermost first)
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.bindings.get(name) {
                return Ok(val.clone());
            }
        }

        // Check frame locals
        if let Some(frame) = self.frames.last()
            && let Some(val) = frame.locals.get(name)
        {
            return Ok(val.clone());
        }

        // Check named objects
        if let Some(id) = self.objects.lookup_name(name) {
            return Ok(Value::Object(id));
        }

        // Check for constructor syntax (^name or @name)
        if name.starts_with('^')
            && let Some(id) = self.objects.lookup_name(&name[1..])
        {
            return Ok(Value::Object(id));
        }

        Err(Error::UndefinedVariable(name.to_string()))
    }

    fn store_var(&mut self, name: SmolStr, val: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, val);
        }
    }

    fn call_value(&mut self, func: Value, argc: usize) -> Result<()> {
        match func {
            Value::Lambda(lambda) => {
                // Pop arguments
                let mut args = Vec::new();
                for _ in 0..argc {
                    args.push(self.pop()?);
                }
                args.reverse();

                // Create new frame
                let mut frame = Frame::new(lambda.code.clone(), self.stack.len());

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
                // Calling an object as a constructor - look for 'call' method
                if self.objects.get_method(id, "call").is_some() {
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
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

    /// Call a built-in method.
    fn call_builtin(&mut self, object: &str, method: &str, args: Vec<Value>) -> Result<Value> {
        match (object, method) {
            ("__builtin_curl", "get") => {
                let url = match args.first() {
                    Some(Value::String(s)) => s.as_str(),
                    _ => return Err(Error::Runtime("curl.get requires string URL".to_string())),
                };
                let handle = self
                    .runtime
                    .as_ref()
                    .ok_or_else(|| Error::Runtime("curl requires runtime handle".to_string()))?;
                crate::builtins::CurlBuiltin::get(url, handle)
            }
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
                crate::builtins::CurlBuiltin::post(url, body, handle)
            }
            _ => Err(Error::Runtime(format!(
                "unknown builtin: {}.{}",
                object, method
            ))),
        }
    }

    fn call_method(&mut self, receiver: Value, name: &str, args: Vec<Value>) -> Result<()> {
        match receiver {
            Value::Symbol(ref s) if s.starts_with("__builtin_") => {
                let result = self.call_builtin(s.as_str(), name, args)?;
                self.stack.push(result);
                return Ok(());
            }
            Value::Object(id) => {
                if let Some(method) = self.objects.get_method(id, name).cloned() {
                    let mut frame = Frame::new(method.code, self.stack.len());
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
            Value::List(list) => {
                // Built-in list methods
                match name {
                    "len" => {
                        self.stack.push(Value::Int(list.len() as i64));
                    }
                    "first" => {
                        self.stack
                            .push(list.first().cloned().unwrap_or(Value::Null));
                    }
                    "last" => {
                        self.stack.push(list.last().cloned().unwrap_or(Value::Null));
                    }
                    "push" => {
                        let mut new_list = (*list).clone();
                        for arg in args {
                            new_list.push(arg);
                        }
                        self.stack.push(Value::List(Arc::new(new_list)));
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                }
            }
            Value::String(s) => {
                // Built-in string methods
                match name {
                    "len" => {
                        self.stack.push(Value::Int(s.len() as i64));
                    }
                    "upper" => {
                        self.stack
                            .push(Value::String(SmolStr::new(s.to_uppercase())));
                    }
                    "lower" => {
                        self.stack
                            .push(Value::String(SmolStr::new(s.to_lowercase())));
                    }
                    _ => return Err(Error::UndefinedMethod(name.to_string())),
                }
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
                // Create a new object with this parent
                let id = self.objects.create(Some(parent_id));

                // TODO: call constructor method if it exists
                let _ = args;

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
    use crate::eval;

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

    #[test]
    fn test_grammar_apply_digit() {
        let mut vm = Vm::new();
        // Parse a digit using the built-in base::parser grammar
        let result = eval(&mut vm, r#""5" @ base::parser.digit"#).unwrap();
        assert!(matches!(result, Value::String(s) if s == "5"));
    }

    #[test]
    fn test_grammar_apply_integer() {
        let mut vm = Vm::new();
        // Parse an integer using the built-in base::parser grammar
        let result = eval(&mut vm, r#""12345" @ base::parser.integer"#).unwrap();
        assert!(matches!(result, Value::String(s) if s == "12345"));
    }

    #[test]
    fn test_grammar_apply_word() {
        let mut vm = Vm::new();
        // Parse a word using the built-in base::parser grammar
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(matches!(result, Value::String(s) if s == "hello"));
    }

    #[test]
    fn test_grammar_apply_failure() {
        let mut vm = Vm::new();
        // Trying to parse a letter with digit rule should fail
        let result = eval(&mut vm, r#""a" @ base::parser.digit"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_grammar_literal() {
        let mut vm = Vm::new();
        // Create a grammar literal and verify it returns a grammar value
        let result = eval(&mut vm, r#"grammar { digit = [0-9] }"#).unwrap();
        match result {
            Value::Grammar(g) => {
                assert!(g.rules.contains_key("digit"));
            }
            _ => panic!("expected grammar value, got {:?}", result),
        }
    }

    #[test]
    fn test_grammar_extension() {
        let mut vm = Vm::new();
        // Create a base grammar and extend it
        let result = eval(
            &mut vm,
            r#"
            let (base = grammar { digit = [0-9] })
            base <: { hex = [0-9a-f] }
        "#,
        )
        .unwrap();
        match result {
            Value::Grammar(g) => {
                // Extended grammar should have the new rule
                assert!(g.rules.contains_key("hex"));
                // Extended grammar should have parent reference
                assert!(g.parent_grammar.is_some());
                // Parent should have digit
                assert!(
                    g.parent_grammar
                        .as_ref()
                        .unwrap()
                        .rules
                        .contains_key("digit")
                );
            }
            _ => panic!("expected grammar value, got {:?}", result),
        }
    }

    #[test]
    fn test_dynamic_grammar_apply() {
        let mut vm = Vm::new();
        // Use grammar literal directly with @ operator
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { digit = [0-9] })
            "5" @ g.digit
        "#,
        )
        .unwrap();
        assert!(matches!(result, Value::String(s) if s == "5"));
    }

    #[test]
    fn test_stream_map_filter_flatmap() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            r#"
            let (s = stream { [1, 2, 3] })
            s |> map(\x x + 1) |> filter(\x x > 2)
        "#,
        )
        .unwrap();
        assert!(matches!(result, Value::Stream(_)));
    }

    #[test]
    fn test_stream_parse_operator() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            r#"
            let (g = grammar { digit = [0-9] })
            let (s = stream { "5" })
            s |> parse(g.digit)
        "#,
        )
        .unwrap();
        assert!(matches!(result, Value::Stream(_)));
    }

    #[test]
    fn test_grammar_apply_to_int() {
        let mut vm = Vm::new();
        // Apply tree grammar to an integer value
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_grammar_apply_to_single_element_list() {
        let mut vm = Vm::new();
        // Apply tree grammar to a single-element list
        let result = eval(&mut vm, r#"[42] @ base::tree.int"#).unwrap();
        assert!(matches!(result, Value::Int(42)));
    }

    #[test]
    fn test_grammar_apply_to_bool() {
        let mut vm = Vm::new();
        // Apply tree grammar to a boolean
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(matches!(result, Value::Bool(true)));
    }

    #[test]
    fn test_anonymous_grammar_block_string() {
        let mut vm = Vm::new();
        // Anonymous grammar block with string input - semantic action evaluated
        let result = eval(&mut vm, r#""hello" @ { [a-z]+ => "word" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "word"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn test_anonymous_grammar_block_int() {
        let mut vm = Vm::new();
        // Anonymous grammar block matching an integer (using _ for any)
        let result = eval(&mut vm, r#"42 @ { _ => "matched" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "matched"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn test_anonymous_grammar_block_choice() {
        let mut vm = Vm::new();
        // Multiple cases - should match the first applicable one
        let result = eval(&mut vm, r#"42 @ { . => "any" }"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "any"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn test_try_catch_no_error() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "try { 42 } catch e { 0 }").unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_try_catch_with_error() {
        let mut vm = Vm::new();
        // Division by zero should be caught
        let result = eval(&mut vm, "try { 1 / 0 } catch e { 99 }").unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_async_call_without_runtime() {
        let mut vm = Vm::new();
        let result = eval(&mut vm, "<- 42");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("runtime"));
    }

    #[test]
    fn test_let_destructure_map() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            r#"
            let (%{x: a, y: b} = %{x: 1, y: 2}) a + b
        "#,
        )
        .unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_let_destructure_list() {
        let mut vm = Vm::new();
        let result = eval(
            &mut vm,
            r#"
            let ([first, second] = [10, 20]) first + second
        "#,
        )
        .unwrap();
        assert_eq!(result, Value::Int(30));
    }

    #[tokio::test]
    async fn test_curl_builtin_exists() {
        let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());
        // Just test that curl is accessible - actual HTTP test needs mock server
        let result = eval(&mut vm, "curl");
        assert!(result.is_ok());
        // Should return the builtin symbol
        assert!(matches!(result.unwrap(), Value::Symbol(s) if s == "__builtin_curl"));
    }
}
