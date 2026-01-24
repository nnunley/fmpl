//! Compiler: AST to Indexed RPN bytecode.
//!
//! This compiler emits Indexed RPN instructions where each instruction that
//! produces a value writes to `values[ip]`, and consuming instructions
//! explicitly reference operand indices.

use crate::ast::*;
use crate::error::{Error, Result};
use crate::grammar::Grammar;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashSet;
use std::sync::Arc;

/// Index into the instructions/values array.
///
/// Each instruction that produces a value writes to `values[InstrIndex]`.
/// Consuming instructions reference operands by their producing instruction's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrIndex(pub usize);

impl InstrIndex {
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl From<usize> for InstrIndex {
    fn from(val: usize) -> Self {
        InstrIndex(val)
    }
}

/// Bytecode instruction using Indexed RPN format.
///
/// Instructions that produce values store their result at `values[ip]`.
/// Instructions that consume values reference operands by their instruction index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    // Literals (produce values, no operand references)
    LoadNull,
    LoadBool(bool),
    LoadInt(i64),
    LoadFloat(f64),
    LoadString(SmolStr),
    LoadSymbol(SmolStr),

    // Variable access
    LoadVar(SmolStr),
    StoreVar {
        name: SmolStr,
        value: InstrIndex,
    },

    // Special references (produce values)
    LoadSelf,
    LoadParent,
    LoadCaller,
    LoadUser,
    LoadArgs,

    // Binary arithmetic (explicit operand indices)
    Add {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Sub {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Mul {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Div {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Mod {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },

    // Unary (explicit operand index)
    Neg {
        operand: InstrIndex,
    },
    Not {
        operand: InstrIndex,
    },

    // Comparison (explicit operand indices)
    Eq {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    NotEq {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Lt {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    Gt {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    LtEq {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },
    GtEq {
        lhs: InstrIndex,
        rhs: InstrIndex,
    },

    // Control flow
    Jump {
        target: InstrIndex,
    },
    JumpIfFalse {
        cond: InstrIndex,
        target: InstrIndex,
    },
    JumpIfTrue {
        cond: InstrIndex,
        target: InstrIndex,
    },

    // Functions and calls (explicit operand indices)
    Call {
        func: InstrIndex,
        args: Vec<InstrIndex>,
    },
    TailCall {
        func: InstrIndex,
        args: Vec<InstrIndex>,
    },
    MethodCall {
        receiver: InstrIndex,
        method: SmolStr,
        args: Vec<InstrIndex>,
    },
    Return {
        value: InstrIndex,
    },

    // Objects (explicit operand indices)
    GetProp {
        object: InstrIndex,
        name: SmolStr,
    },
    SetProp {
        object: InstrIndex,
        name: SmolStr,
        value: InstrIndex,
    },
    Spawn {
        object: InstrIndex,
        args: Vec<InstrIndex>,
    },
    GetFacet {
        object: InstrIndex,
        name: SmolStr,
    },

    // Sync/Async (explicit operand indices)
    SyncCall {
        target: InstrIndex,
    },
    AsyncCall {
        target: InstrIndex,
    },

    // Data structures (explicit operand indices)
    MakeList {
        elements: Vec<InstrIndex>,
    },
    MakeMap {
        pairs: Vec<(InstrIndex, InstrIndex)>,
    },
    Index {
        collection: InstrIndex,
        key: InstrIndex,
    },
    Slice {
        collection: InstrIndex,
        start: Option<InstrIndex>,
        end: Option<InstrIndex>,
    },

    // Binding & Scope (BlockStart/BlockEnd replace PushScope/PopScope for Indexed RPN)
    BlockStart, // Scope boundary: begin
    BlockEnd,   // Scope boundary: end
    Bind {
        name: SmolStr,
        value: InstrIndex,
    }, // Introducer: name → value index
    NameRef {
        bind: InstrIndex,
    }, // Reference to Bind instruction (resolved at compile time)

    // Legacy scope instructions (deprecated, use BlockStart/BlockEnd)
    PushScope,
    PopScope,

    // Lambda (explicit capture names)
    MakeLambda {
        params: Vec<SmolStr>,
        body: usize,
        captures: Vec<SmolStr>,
    },

    // Pipe (explicit operand indices)
    Pipe {
        arg: InstrIndex,
        func: InstrIndex,
    },

    // Streams (explicit operand indices)
    MakeStream {
        source: InstrIndex,
    },
    StreamMap {
        source: InstrIndex,
        func: InstrIndex,
    },
    StreamFilter {
        source: InstrIndex,
        pred: InstrIndex,
    },
    StreamFlatMap {
        source: InstrIndex,
        func: InstrIndex,
    },
    StreamReduce {
        source: InstrIndex,
        init: InstrIndex,
        func: InstrIndex,
    },
    StreamParse {
        source: InstrIndex,
        grammar: InstrIndex,
        rule: SmolStr,
    },

    // Pattern matching (explicit operand indices)
    MatchPattern {
        value: InstrIndex,
        fail_target: InstrIndex,
    },
    ExtractMapKey {
        source: InstrIndex,
        key: SmolStr,
    },
    ExtractListIndex {
        source: InstrIndex,
        index: usize,
    },

    // Object definition (creates object in DB)
    DefineObject(SmolStr),
    DefineMethod {
        object: InstrIndex,
        name: SmolStr,
        params: Vec<SmolStr>,
        body: usize,
    },
    DefineProp {
        object: InstrIndex,
        name: SmolStr,
        value: InstrIndex,
    },
    DefineFacet {
        object: InstrIndex,
        name: SmolStr,
        members: Vec<InstrIndex>,
        terminal: bool,
    },

    // Grammar application (explicit operand indices)
    GrammarApply {
        input: InstrIndex,
        grammar: InstrIndex,
        rule: SmolStr,
    },

    // Grammar values
    LoadGrammar(Arc<Grammar>),
    ExtendGrammar {
        base: InstrIndex,
        extension: Grammar,
    },

    // Exception handling
    PushHandler {
        catch_target: InstrIndex,
    },
    PopHandler,
    Throw {
        value: InstrIndex,
    },

    // Copy a value from one index to current (for control flow convergence)
    Copy {
        source: InstrIndex,
    },

    // Tuple space operations
    TupleSpaceNew,

    // No-op (placeholder, for eliminated Dup/Pop or control flow)
    Nop,
}

/// Compiled bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledCode {
    pub instructions: Vec<Instruction>,
    /// Nested code blocks (for lambdas, methods).
    pub nested: Vec<CompiledCode>,
    /// Original source code (for reflection/decompilation).
    pub source: Option<SmolStr>,
}

impl CompiledCode {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            nested: Vec::new(),
            source: None,
        }
    }

    pub fn with_source(source: SmolStr) -> Self {
        Self {
            instructions: Vec::new(),
            nested: Vec::new(),
            source: Some(source),
        }
    }

    /// Emit an instruction and return its index.
    fn emit(&mut self, instr: Instruction) -> InstrIndex {
        let idx = InstrIndex(self.instructions.len());
        self.instructions.push(instr);
        idx
    }

    /// Get the next instruction index (where the next emit will go).
    fn next_index(&self) -> InstrIndex {
        InstrIndex(self.instructions.len())
    }

    /// Patch a jump instruction's target.
    fn patch_jump_target(&mut self, idx: InstrIndex, target: InstrIndex) {
        match &mut self.instructions[idx.0] {
            Instruction::Jump { target: t } => *t = target,
            Instruction::JumpIfFalse { target: t, .. } => *t = target,
            Instruction::JumpIfTrue { target: t, .. } => *t = target,
            Instruction::MatchPattern { fail_target: t, .. } => *t = target,
            Instruction::PushHandler { catch_target: t } => *t = target,
            _ => panic!("not a jump instruction at index {}", idx.0),
        }
    }
}

impl Default for CompiledCode {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve all NameRef instructions to point to their Bind introducers.
///
/// This pass runs after initial compilation to wire all `NameRef` instructions
/// to their `Bind` introducers, eliminating runtime scope lookup.
///
/// # Algorithm
///
/// 1. Walk through instructions sequentially
/// 2. Track a scope stack where each scope maps names to Bind instruction indices
/// 3. On BlockStart: push new scope
/// 4. On BlockEnd: pop scope (inner bindings forgotten)
/// 5. On Bind: register name in current scope
/// 6. On LoadVar: if it references a bound name, convert to NameRef
///
/// # Key Properties
///
/// - Single pass: O(n) traversal of instruction array
/// - Lexical scoping: Inner scopes shadow outer bindings
/// - No runtime lookup: After resolution, NameRef directly references Bind index
pub fn resolve_names(code: &mut CompiledCode) {
    use std::collections::HashMap;

    // Stack of scopes: each scope maps name → Bind instruction index
    let mut scope_stack: Vec<HashMap<SmolStr, InstrIndex>> = vec![HashMap::new()];

    // First pass: identify bindings and their scopes
    let mut conversions: Vec<(usize, InstrIndex)> = Vec::new(); // (instruction index, bind index to use)

    for ip in 0..code.instructions.len() {
        match &code.instructions[ip] {
            Instruction::BlockStart | Instruction::PushScope => {
                // Push new scope
                scope_stack.push(HashMap::new());
            }

            Instruction::BlockEnd | Instruction::PopScope => {
                // Pop scope - inner bindings are forgotten
                if scope_stack.len() > 1 {
                    scope_stack.pop();
                }
            }

            Instruction::Bind { name, .. } => {
                // Register in current (top) scope
                if let Some(current) = scope_stack.last_mut() {
                    current.insert(name.clone(), InstrIndex(ip));
                }
            }

            Instruction::LoadVar(name) => {
                // Search from innermost to outermost scope for this name
                let target = scope_stack
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(name).copied());

                if let Some(bind_idx) = target {
                    // Mark for conversion to NameRef
                    conversions.push((ip, bind_idx));
                }
                // If not found, keep as LoadVar (could be a builtin, object name, etc.)
            }

            _ => {}
        }
    }

    // Second pass: apply conversions
    for (ip, bind_idx) in conversions {
        code.instructions[ip] = Instruction::NameRef { bind: bind_idx };
    }

    // Recursively resolve nested code blocks (lambdas, methods)
    for nested in &mut code.nested {
        resolve_names(nested);
    }
}

/// The compiler.
///
/// Compiles AST expressions to Indexed RPN bytecode where each instruction
/// that produces a value stores it at `values[ip]`, and consuming instructions
/// explicitly reference operand indices.
pub struct Compiler {
    code: CompiledCode,
    /// Counter for generating unique temporary variable names.
    temp_counter: usize,
    /// Track variables loaded via LoadVar (for capture analysis)
    loaded_vars: HashSet<SmolStr>,
    /// Track variables bound via Bind (for capture analysis)
    bound_vars: HashSet<SmolStr>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            code: CompiledCode::new(),
            temp_counter: 0,
            loaded_vars: std::collections::HashSet::new(),
            bound_vars: std::collections::HashSet::new(),
        }
    }

    /// Generate a unique temporary variable name.
    fn gen_temp_name(&mut self, prefix: &str) -> SmolStr {
        let name = SmolStr::new(format!("__{}{}", prefix, self.temp_counter));
        self.temp_counter += 1;
        name
    }

    /// Compile an expression.
    pub fn compile(mut self, expr: &Expr) -> Result<CompiledCode> {
        self.compile_expr(expr)?;
        Ok(self.code)
    }

    /// Compile an expression and return the index where its value is stored.
    fn compile_expr(&mut self, expr: &Expr) -> Result<InstrIndex> {
        match expr {
            Expr::Int(n) => Ok(self.code.emit(Instruction::LoadInt(*n))),
            Expr::Float(f) => Ok(self.code.emit(Instruction::LoadFloat(*f))),
            Expr::String(s) => Ok(self.code.emit(Instruction::LoadString(s.clone()))),
            Expr::Symbol(s) => Ok(self.code.emit(Instruction::LoadSymbol(s.clone()))),
            Expr::Bool(b) => Ok(self.code.emit(Instruction::LoadBool(*b))),
            Expr::Null => Ok(self.code.emit(Instruction::LoadNull)),
            Expr::Ident(name) => {
                self.loaded_vars.insert(name.clone());
                Ok(self.code.emit(Instruction::LoadVar(name.clone())))
            }
            Expr::Qualified(qn) => {
                // For now, treat as simple name lookup
                Ok(self
                    .code
                    .emit(Instruction::LoadVar(SmolStr::new(qn.to_string()))))
            }
            Expr::ObjTag(name) => Ok(self
                .code
                .emit(Instruction::LoadVar(SmolStr::new(format!("^{}", name))))),
            Expr::FnTag(name) => Ok(self
                .code
                .emit(Instruction::LoadVar(SmolStr::new(format!("@{}", name))))),
            Expr::Self_ => Ok(self.code.emit(Instruction::LoadSelf)),
            Expr::Parent => Ok(self.code.emit(Instruction::LoadParent)),
            Expr::Caller => Ok(self.code.emit(Instruction::LoadCaller)),
            Expr::User => Ok(self.code.emit(Instruction::LoadUser)),
            Expr::Args => Ok(self.code.emit(Instruction::LoadArgs)),
            Expr::Placeholder => Ok(self.code.emit(Instruction::LoadNull)),

            Expr::List(items) => {
                let mut element_indices = Vec::with_capacity(items.len());
                for item in items {
                    element_indices.push(self.compile_expr(item)?);
                }
                Ok(self.code.emit(Instruction::MakeList {
                    elements: element_indices,
                }))
            }
            Expr::ListCons(head, tail) => {
                let head_idx = self.compile_expr(head)?;
                let tail_idx = self.compile_expr(tail)?;
                // TODO: proper list cons instruction
                Ok(self.code.emit(Instruction::MakeList {
                    elements: vec![head_idx, tail_idx],
                }))
            }
            Expr::Map(entries) => {
                let mut pairs = Vec::with_capacity(entries.len());
                for entry in entries {
                    let (key_idx, val_idx) = match entry {
                        MapEntry::Symbol(key, val) => {
                            let k = self.code.emit(Instruction::LoadSymbol(key.clone()));
                            let v = self.compile_expr(val)?;
                            (k, v)
                        }
                        MapEntry::Computed(key, val) => {
                            let k = self.compile_expr(key)?;
                            let v = self.compile_expr(val)?;
                            (k, v)
                        }
                    };
                    pairs.push((key_idx, val_idx));
                }
                Ok(self.code.emit(Instruction::MakeMap { pairs }))
            }
            Expr::Binary(left, op, right) => self.compile_binary(left, op, right),
            Expr::Unary(op, expr) => {
                let operand = self.compile_expr(expr)?;
                Ok(match op {
                    UnaryOp::Neg => self.code.emit(Instruction::Neg { operand }),
                    UnaryOp::Not => self.code.emit(Instruction::Not { operand }),
                })
            }
            Expr::Index(expr, idx) => {
                let collection = self.compile_expr(expr)?;
                let key = self.compile_expr(idx)?;
                Ok(self.code.emit(Instruction::Index { collection, key }))
            }
            Expr::Slice(expr, start, end) => {
                let collection = self.compile_expr(expr)?;
                let start_idx = Some(self.compile_expr(start)?);
                let end_idx = Some(self.compile_expr(end)?);
                Ok(self.code.emit(Instruction::Slice {
                    collection,
                    start: start_idx,
                    end: end_idx,
                }))
            }
            Expr::Call(func, args) => self.compile_call(func, args),
            Expr::PropAccess(expr, name) => {
                let object = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::GetProp {
                    object,
                    name: name.clone(),
                }))
            }
            Expr::MethodCall(expr, name, args) => {
                let receiver = self.compile_expr(expr)?;
                let mut arg_indices = Vec::with_capacity(args.len());
                for arg in args {
                    match arg {
                        Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                        Arg::Placeholder => {
                            return Err(Error::Compiler(
                                "partial application not yet implemented".to_string(),
                            ));
                        }
                    }
                }
                Ok(self.code.emit(Instruction::MethodCall {
                    receiver,
                    method: name.clone(),
                    args: arg_indices,
                }))
            }
            Expr::If(cond, then_branch, else_branch) => {
                self.compile_if(cond, then_branch, else_branch.as_deref())
            }
            Expr::While(cond, body) => self.compile_while(cond, body),
            Expr::DoWhile(body, cond) => self.compile_do_while(body, cond),
            Expr::Return(expr) => {
                let value = if let Some(e) = expr {
                    self.compile_expr(e)?
                } else {
                    self.code.emit(Instruction::LoadNull)
                };
                Ok(self.code.emit(Instruction::Return { value }))
            }
            Expr::Lambda(params, body) => self.compile_lambda(params, body),
            Expr::ShortLambda(param, body) => self.compile_lambda(&[param.clone()], body),
            Expr::Let(bindings, body) => self.compile_let(bindings, body),
            Expr::LetStmt(name, expr) => self.compile_let_stmt(name, expr),
            Expr::Assignment(target, value) => self.compile_assignment(target, value),
            Expr::Sequence(exprs) => self.compile_sequence(exprs),
            Expr::ObjectDef(def) => self.compile_object_def(def),
            Expr::Match(scrutinee, cases) => self.compile_match(scrutinee, cases),
            Expr::Spawn(constructor, args) => {
                let object = self.compile_expr(constructor)?;
                let mut arg_indices = Vec::with_capacity(args.len());
                for arg in args {
                    match arg {
                        Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                        Arg::Placeholder => {
                            return Err(Error::Compiler(
                                "partial application not yet implemented".to_string(),
                            ));
                        }
                    }
                }
                Ok(self.code.emit(Instruction::Spawn {
                    object,
                    args: arg_indices,
                }))
            }
            Expr::SyncCall(expr) => {
                let target = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::SyncCall { target }))
            }
            Expr::AsyncCall(expr) => {
                let target = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::AsyncCall { target }))
            }
            Expr::FacetAccess(expr, facet) => {
                let object = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::GetFacet {
                    object,
                    name: facet.clone(),
                }))
            }
            Expr::GrammarApply {
                input,
                grammar,
                rule,
            } => {
                let input_idx = self.compile_expr(input)?;
                let grammar_idx = match grammar.as_ref() {
                    Expr::Qualified(qn) => self
                        .code
                        .emit(Instruction::LoadString(SmolStr::new(qn.to_string()))),
                    _ => self.compile_expr(grammar)?,
                };
                Ok(self.code.emit(Instruction::GrammarApply {
                    input: input_idx,
                    grammar: grammar_idx,
                    rule: rule.clone(),
                }))
            }
            Expr::GrammarLiteral(grammar) => Ok(self
                .code
                .emit(Instruction::LoadGrammar(Arc::new(grammar.clone())))),
            Expr::GrammarExtend { base, rules } => {
                let base_idx = self.compile_expr(base)?;
                Ok(self.code.emit(Instruction::ExtendGrammar {
                    base: base_idx,
                    extension: rules.clone(),
                }))
            }
            Expr::StreamLiteral(expr) => {
                let source = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::MakeStream { source }))
            }
            Expr::TryCatch {
                body,
                error_binding,
                catch_body,
            } => self.compile_try_catch(body, error_binding, catch_body),
            Expr::Throw(expr) => {
                let value = self.compile_expr(expr)?;
                Ok(self.code.emit(Instruction::Throw { value }))
            }
        }
    }

    /// Compile a binary operation.
    fn compile_binary(&mut self, left: &Expr, op: &BinOp, right: &Expr) -> Result<InstrIndex> {
        match op {
            BinOp::And => self.compile_short_circuit_and(left, right),
            BinOp::Or => self.compile_short_circuit_or(left, right),
            BinOp::Pipe => self.compile_pipe(left, right),
            _ => {
                let lhs = self.compile_expr(left)?;
                let rhs = self.compile_expr(right)?;
                Ok(match op {
                    BinOp::Add => self.code.emit(Instruction::Add { lhs, rhs }),
                    BinOp::Sub => self.code.emit(Instruction::Sub { lhs, rhs }),
                    BinOp::Mul => self.code.emit(Instruction::Mul { lhs, rhs }),
                    BinOp::Div => self.code.emit(Instruction::Div { lhs, rhs }),
                    BinOp::Mod => self.code.emit(Instruction::Mod { lhs, rhs }),
                    BinOp::Eq => self.code.emit(Instruction::Eq { lhs, rhs }),
                    BinOp::NotEq => self.code.emit(Instruction::NotEq { lhs, rhs }),
                    BinOp::Lt => self.code.emit(Instruction::Lt { lhs, rhs }),
                    BinOp::Gt => self.code.emit(Instruction::Gt { lhs, rhs }),
                    BinOp::LtEq => self.code.emit(Instruction::LtEq { lhs, rhs }),
                    BinOp::GtEq => self.code.emit(Instruction::GtEq { lhs, rhs }),
                    BinOp::And | BinOp::Or | BinOp::Pipe => unreachable!(),
                })
            }
        }
    }

    /// Compile short-circuit AND.
    /// Compiles as: if left is falsy, result is false; else evaluate right.
    /// Uses a temporary variable to properly converge both branches (like if expressions).
    fn compile_short_circuit_and(&mut self, left: &Expr, right: &Expr) -> Result<InstrIndex> {
        let result_var = self.gen_temp_name("and");
        let left_idx = self.compile_expr(left)?;

        // If left is falsy, skip to false result
        let jump_to_false = self.code.emit(Instruction::JumpIfFalse {
            cond: left_idx,
            target: InstrIndex(0), // placeholder
        });

        // Left was truthy, evaluate right and store in temp var
        let right_idx = self.compile_expr(right)?;
        self.code.emit(Instruction::StoreVar {
            name: result_var.clone(),
            value: right_idx,
        });

        // Jump over the false case
        let jump_to_end = self.code.emit(Instruction::Jump {
            target: InstrIndex(0), // placeholder
        });

        // False case: store false in temp var
        let false_target = self.code.next_index();
        self.code.patch_jump_target(jump_to_false, false_target);
        let false_idx = self.code.emit(Instruction::LoadBool(false));
        self.code.emit(Instruction::StoreVar {
            name: result_var.clone(),
            value: false_idx,
        });

        // End: load result from temp var
        let end_target = self.code.next_index();
        self.code.patch_jump_target(jump_to_end, end_target);

        Ok(self.code.emit(Instruction::LoadVar(result_var)))
    }

    /// Compile short-circuit OR.
    /// Compiles as: if left is truthy, result is true; else evaluate right.
    /// Uses a temporary variable to properly converge both branches (like if expressions).
    fn compile_short_circuit_or(&mut self, left: &Expr, right: &Expr) -> Result<InstrIndex> {
        let result_var = self.gen_temp_name("or");
        let left_idx = self.compile_expr(left)?;

        // If left is truthy, skip to true result
        let jump_to_true = self.code.emit(Instruction::JumpIfTrue {
            cond: left_idx,
            target: InstrIndex(0), // placeholder
        });

        // Left was falsy, evaluate right and store in temp var
        let right_idx = self.compile_expr(right)?;
        self.code.emit(Instruction::StoreVar {
            name: result_var.clone(),
            value: right_idx,
        });

        // Jump over the true case
        let jump_to_end = self.code.emit(Instruction::Jump {
            target: InstrIndex(0), // placeholder
        });

        // True case: store true in temp var
        let true_target = self.code.next_index();
        self.code.patch_jump_target(jump_to_true, true_target);
        let true_idx = self.code.emit(Instruction::LoadBool(true));
        self.code.emit(Instruction::StoreVar {
            name: result_var.clone(),
            value: true_idx,
        });

        // End: load result from temp var
        let end_target = self.code.next_index();
        self.code.patch_jump_target(jump_to_end, end_target);

        Ok(self.code.emit(Instruction::LoadVar(result_var)))
    }

    /// Compile pipe operator.
    fn compile_pipe(&mut self, left: &Expr, right: &Expr) -> Result<InstrIndex> {
        // Check for stream operators first
        if let Some(idx) = self.try_compile_stream_pipe(left, right)? {
            return Ok(idx);
        }

        // x |> f compiles to f(x)
        let arg = self.compile_expr(left)?;
        let func = self.compile_expr(right)?;
        Ok(self.code.emit(Instruction::Pipe { arg, func }))
    }

    /// Compile function call.
    fn compile_call(&mut self, func: &Expr, args: &[Arg]) -> Result<InstrIndex> {
        // Check for partial application
        let has_placeholder = args.iter().any(|a| matches!(a, Arg::Placeholder));
        if has_placeholder {
            return Err(Error::Compiler(
                "partial application not yet implemented".to_string(),
            ));
        }

        // Special handling for builtin qualified calls like json::parse(), json::stringify(), and sse::parse()
        if let Expr::Qualified(qn) = func {
            if qn.parts.len() == 2 {
                let module = &qn.parts[0];
                let method = &qn.parts[1];

                // Convert json::parse(args) and json::stringify(args) to __builtin_json.method(args)
                if module == "json" && (method == "parse" || method == "stringify") {
                    // Compile as if it were __builtin_json.method(args)
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_json")));
                    let mut arg_indices = Vec::with_capacity(args.len());
                    for arg in args {
                        match arg {
                            Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                            Arg::Placeholder => unreachable!(),
                        }
                    }
                    return Ok(self.code.emit(Instruction::MethodCall {
                        receiver: builtin_idx,
                        method: method.clone(),
                        args: arg_indices,
                    }));
                }

                // Convert sse::parse(args) to __builtin_sse.parse(args)
                if module == "sse" && method == "parse" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_sse")));
                    let mut arg_indices = Vec::with_capacity(args.len());
                    for arg in args {
                        match arg {
                            Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                            Arg::Placeholder => unreachable!(),
                        }
                    }
                    return Ok(self.code.emit(Instruction::MethodCall {
                        receiver: builtin_idx,
                        method: method.clone(),
                        args: arg_indices,
                    }));
                }

                // Convert time::sleep(args) to __builtin_time.sleep(args)
                if module == "time" && method == "sleep" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_time")));
                    let mut arg_indices = Vec::with_capacity(args.len());
                    for arg in args {
                        match arg {
                            Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                            Arg::Placeholder => unreachable!(),
                        }
                    }
                    return Ok(self.code.emit(Instruction::MethodCall {
                        receiver: builtin_idx,
                        method: method.clone(),
                        args: arg_indices,
                    }));
                }
            }
        }

        let mut arg_indices = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                Arg::Expr(e) => arg_indices.push(self.compile_expr(e)?),
                Arg::Placeholder => unreachable!(),
            }
        }
        let func_idx = self.compile_expr(func)?;
        Ok(self.code.emit(Instruction::Call {
            func: func_idx,
            args: arg_indices,
        }))
    }

    /// Compile if expression.
    fn compile_if(
        &mut self,
        cond: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<InstrIndex> {
        // Use a temporary variable to hold the result from whichever branch executes.
        // This avoids the phi-node problem in Indexed RPN where we need to know
        // at compile time which branch's result to read at convergence.
        let result_var = self.gen_temp_name("if");

        let cond_idx = self.compile_expr(cond)?;

        // Jump to else if condition is false
        let else_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: cond_idx,
            target: InstrIndex(0), // placeholder
        });

        // Then branch - store result in temp var
        let then_idx = self.compile_expr(then_branch)?;
        self.code.emit(Instruction::StoreVar {
            name: result_var.clone(),
            value: then_idx,
        });

        if let Some(else_expr) = else_branch {
            // Jump over else branch
            let end_jump = self.code.emit(Instruction::Jump {
                target: InstrIndex(0), // placeholder
            });

            // Else branch - store result in same temp var
            let else_start = self.code.next_index();
            self.code.patch_jump_target(else_jump, else_start);
            let else_idx = self.compile_expr(else_expr)?;
            self.code.emit(Instruction::StoreVar {
                name: result_var.clone(),
                value: else_idx,
            });

            // Patch end jump
            let end_target = self.code.next_index();
            self.code.patch_jump_target(end_jump, end_target);
        } else {
            // No else branch - store null in temp var
            let else_start = self.code.next_index();
            self.code.patch_jump_target(else_jump, else_start);
            let null_idx = self.code.emit(Instruction::LoadNull);
            self.code.emit(Instruction::StoreVar {
                name: result_var.clone(),
                value: null_idx,
            });
        }

        // Load the result from temp var - this is the if-expression's value
        Ok(self.code.emit(Instruction::LoadVar(result_var)))
    }

    /// Compile while loop.
    fn compile_while(&mut self, cond: &Expr, body: &Expr) -> Result<InstrIndex> {
        let loop_start = self.code.next_index();
        let cond_idx = self.compile_expr(cond)?;

        // Exit if condition is false
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: cond_idx,
            target: InstrIndex(0), // placeholder
        });

        // Body (result discarded)
        let _body_idx = self.compile_expr(body)?;

        // Jump back to start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // While returns null
        Ok(self.code.emit(Instruction::LoadNull))
    }

    /// Compile do-while loop.
    fn compile_do_while(&mut self, body: &Expr, cond: &Expr) -> Result<InstrIndex> {
        let loop_start = self.code.next_index();

        // Body (result discarded)
        let _body_idx = self.compile_expr(body)?;

        // Condition
        let cond_idx = self.compile_expr(cond)?;

        // Jump back if true
        self.code.emit(Instruction::JumpIfTrue {
            cond: cond_idx,
            target: loop_start,
        });

        // Do-while returns null
        Ok(self.code.emit(Instruction::LoadNull))
    }

    /// Compile lambda.
    fn compile_lambda(&mut self, params: &[SmolStr], body: &Expr) -> Result<InstrIndex> {
        // Compile lambda body to nested code
        let mut lambda_compiler = Compiler::new();

        // Don't emit Bind instructions for parameters
        // Parameters are bound to frame.locals at call time by call_value
        // Use LoadVar instructions which will check frame.locals

        let body_result = lambda_compiler.compile_expr(body)?;
        lambda_compiler
            .code
            .emit(Instruction::Return { value: body_result });
        lambda_compiler.code.source = Some(SmolStr::new(format!("{}", body)));

        let nested_idx = self.code.nested.len();
        self.code.nested.push(lambda_compiler.code);

        // Compute captures: variables loaded but not bound (excluding parameters)
        let mut captures: Vec<SmolStr> = lambda_compiler
            .loaded_vars
            .difference(&lambda_compiler.bound_vars)
            .filter(|name| !params.contains(name))
            .cloned()
            .collect();
        captures.sort(); // For deterministic ordering

        Ok(self.code.emit(Instruction::MakeLambda {
            params: params.to_vec(),
            body: nested_idx,
            captures,
        }))
    }

    /// Compile let expression.
    ///
    /// Emits: PushScope, bindings, body, PopScope, Copy(body_result)
    /// The final Copy ensures the result is at the last instruction position.
    fn compile_let(&mut self, bindings: &[LetBinding], body: &Expr) -> Result<InstrIndex> {
        self.code.emit(Instruction::PushScope);

        for binding in bindings {
            match binding {
                LetBinding::Simple(name, init) => {
                    let value = if let Some(expr) = init {
                        self.compile_expr(expr)?
                    } else {
                        self.code.emit(Instruction::LoadNull)
                    };
                    self.bound_vars.insert(name.clone());
                    self.code.emit(Instruction::Bind {
                        name: name.clone(),
                        value,
                    });
                }
                LetBinding::Destructure(pattern, expr) => {
                    let value = self.compile_expr(expr)?;
                    self.compile_pattern_binding(pattern, value)?;
                }
            }
        }

        let result = self.compile_expr(body)?;
        self.code.emit(Instruction::PopScope);
        // Copy the result to final position so it's the return value
        Ok(self.code.emit(Instruction::Copy { source: result }))
    }

    /// Compile let statement: let name = expr
    ///
    /// Binds to current scope (no PushScope/PopScope), returns the value.
    /// Emits: Bind(name, expr), Copy(bind_result)
    fn compile_let_stmt(&mut self, name: &SmolStr, expr: &Expr) -> Result<InstrIndex> {
        let value = self.compile_expr(expr)?;
        self.bound_vars.insert(name.clone());
        self.code.emit(Instruction::Bind {
            name: name.clone(),
            value,
        });
        // Return the bound value
        Ok(self.code.emit(Instruction::Copy { source: value }))
    }

    /// Compile assignment: target = value
    /// Supports:
    /// - Simple variable assignment: x = value
    /// - Property assignment: obj.prop = value
    /// Emits: StoreVar/SetProp, returns the assigned value
    fn compile_assignment(&mut self, target: &Expr, value: &Expr) -> Result<InstrIndex> {
        let value_idx = self.compile_expr(value)?;

        match target {
            // Simple variable assignment: x = value
            Expr::Ident(n) => {
                self.code.emit(Instruction::StoreVar {
                    name: n.clone(),
                    value: value_idx,
                });
                // Return the assigned value
                Ok(self.code.emit(Instruction::Copy { source: value_idx }))
            }
            // Property assignment: obj.prop = value
            Expr::PropAccess(obj, prop) => {
                let obj_idx = self.compile_expr(obj)?;
                self.code.emit(Instruction::SetProp {
                    object: obj_idx,
                    name: prop.clone(),
                    value: value_idx,
                });
                // SetProp returns the assigned value
                Ok(value_idx)
            }
            _ => Err(Error::Compiler(format!(
                "assignment target must be a simple variable or property access, got {:?}",
                target
            ))),
        }
    }

    /// Compile sequence of expressions.
    fn compile_sequence(&mut self, exprs: &[Expr]) -> Result<InstrIndex> {
        if exprs.is_empty() {
            return Ok(self.code.emit(Instruction::LoadNull));
        }

        let mut last_idx = InstrIndex(0);
        for expr in exprs {
            last_idx = self.compile_expr(expr)?;
            // Note: in Indexed RPN, we don't need Pop - unused values just sit in the values array
        }
        Ok(last_idx)
    }

    /// Compile match expression.
    fn compile_match(&mut self, scrutinee: &Expr, cases: &[MatchCase]) -> Result<InstrIndex> {
        let scrutinee_idx = self.compile_expr(scrutinee)?;

        let mut end_jumps = Vec::new();
        let mut last_body_idx = InstrIndex(0);

        for case in cases {
            // TODO: proper pattern compilation with failure jumps
            // For now, just support simple variable patterns
            match &case.pattern {
                Pattern::Var(name) => {
                    self.code.emit(Instruction::PushScope);
                    self.bound_vars.insert(name.clone());
                    self.code.emit(Instruction::Bind {
                        name: name.clone(),
                        value: scrutinee_idx,
                    });
                }
                Pattern::Wildcard => {
                    self.code.emit(Instruction::PushScope);
                }
                _ => {
                    return Err(Error::Compiler(
                        "complex patterns not yet implemented".to_string(),
                    ));
                }
            }

            // Guard
            if let Some(guard) = &case.guard {
                let guard_idx = self.compile_expr(guard)?;
                let skip = self.code.emit(Instruction::JumpIfFalse {
                    cond: guard_idx,
                    target: InstrIndex(0), // placeholder
                });

                let _ = last_body_idx;
                last_body_idx = self.compile_expr(&case.body)?;
                self.code.emit(Instruction::PopScope);
                end_jumps.push(self.code.emit(Instruction::Jump {
                    target: InstrIndex(0), // placeholder
                }));

                let skip_target = self.code.next_index();
                self.code.patch_jump_target(skip, skip_target);
                self.code.emit(Instruction::PopScope);
            } else {
                let _ = last_body_idx;
                last_body_idx = self.compile_expr(&case.body)?;
                self.code.emit(Instruction::PopScope);
                end_jumps.push(self.code.emit(Instruction::Jump {
                    target: InstrIndex(0), // placeholder
                }));
            }
        }

        // If no pattern matched, result is null
        let null_idx = self.code.emit(Instruction::LoadNull);

        let end_target = self.code.next_index();
        for jump in end_jumps {
            self.code.patch_jump_target(jump, end_target);
        }

        Ok(self.code.emit(Instruction::Copy { source: null_idx }))
    }

    /// Compile try-catch.
    fn compile_try_catch(
        &mut self,
        body: &Expr,
        error_binding: &SmolStr,
        catch_body: &Expr,
    ) -> Result<InstrIndex> {
        // PushHandler with placeholder
        let handler_idx = self.code.emit(Instruction::PushHandler {
            catch_target: InstrIndex(0),
        });

        // Try body
        let _try_result = self.compile_expr(body)?;

        self.code.emit(Instruction::PopHandler);

        // Jump over catch
        let jump_idx = self.code.emit(Instruction::Jump {
            target: InstrIndex(0), // placeholder
        });

        // Catch handler
        let catch_target = self.code.next_index();
        self.code.patch_jump_target(handler_idx, catch_target);

        // Bind error (the error value will be pushed by VM during exception handling)
        // For now, we load a placeholder that the VM will replace
        let error_val = self.code.emit(Instruction::LoadNull); // VM replaces this
        self.bound_vars.insert(error_binding.clone());
        self.code.emit(Instruction::Bind {
            name: error_binding.clone(),
            value: error_val,
        });

        let catch_result = self.compile_expr(catch_body)?;

        // Patch end jump
        let end_target = self.code.next_index();
        self.code.patch_jump_target(jump_idx, end_target);

        Ok(self.code.emit(Instruction::Copy {
            source: catch_result,
        }))
    }

    /// Try to compile stream pipe operators (map, filter, flatMap, reduce, parse).
    fn try_compile_stream_pipe(&mut self, left: &Expr, right: &Expr) -> Result<Option<InstrIndex>> {
        let Expr::Call(func, args) = right else {
            return Ok(None);
        };
        let Expr::Ident(name) = func.as_ref() else {
            return Ok(None);
        };

        match name.as_str() {
            "map" => {
                if args.len() != 1 {
                    return Err(Error::Compiler("map expects 1 argument".to_string()));
                }
                let source = self.compile_expr(left)?;
                let func = self.compile_arg(&args[0])?;
                Ok(Some(
                    self.code.emit(Instruction::StreamMap { source, func }),
                ))
            }
            "filter" => {
                if args.len() != 1 {
                    return Err(Error::Compiler("filter expects 1 argument".to_string()));
                }
                let source = self.compile_expr(left)?;
                let pred = self.compile_arg(&args[0])?;
                Ok(Some(
                    self.code.emit(Instruction::StreamFilter { source, pred }),
                ))
            }
            "flatMap" => {
                if args.len() != 1 {
                    return Err(Error::Compiler("flatMap expects 1 argument".to_string()));
                }
                let source = self.compile_expr(left)?;
                let func = self.compile_arg(&args[0])?;
                Ok(Some(
                    self.code.emit(Instruction::StreamFlatMap { source, func }),
                ))
            }
            "reduce" => {
                if args.len() != 2 {
                    return Err(Error::Compiler("reduce expects 2 arguments".to_string()));
                }
                let source = self.compile_expr(left)?;
                let init = self.compile_arg(&args[0])?;
                let func = self.compile_arg(&args[1])?;
                Ok(Some(self.code.emit(Instruction::StreamReduce {
                    source,
                    init,
                    func,
                })))
            }
            "parse" => {
                let (grammar_expr, rule) = self.extract_parse_target(args)?;
                let source = self.compile_expr(left)?;
                let grammar = self.compile_grammar_ref(&grammar_expr)?;
                Ok(Some(self.code.emit(Instruction::StreamParse {
                    source,
                    grammar,
                    rule,
                })))
            }
            _ => Ok(None),
        }
    }

    fn compile_arg(&mut self, arg: &Arg) -> Result<InstrIndex> {
        match arg {
            Arg::Expr(e) => self.compile_expr(e),
            Arg::Placeholder => Err(Error::Compiler(
                "partial application not yet implemented".to_string(),
            )),
        }
    }

    fn extract_parse_target(&self, args: &[Arg]) -> Result<(Expr, SmolStr)> {
        if args.len() != 1 {
            return Err(Error::Compiler(
                "stream operator parse expects 1 argument".to_string(),
            ));
        }
        let Arg::Expr(expr) = &args[0] else {
            return Err(Error::Compiler(
                "stream operator arguments must be expressions".to_string(),
            ));
        };

        match expr {
            Expr::PropAccess(base, rule) => Ok((*base.clone(), rule.clone())),
            Expr::Qualified(qn) if qn.parts.len() >= 2 => {
                let rule = qn.parts.last().unwrap().clone();
                let grammar_parts = qn.parts[..qn.parts.len() - 1].to_vec();
                Ok((
                    Expr::Qualified(QualifiedName {
                        parts: grammar_parts,
                    }),
                    rule,
                ))
            }
            _ => Err(Error::Compiler(
                "parse expects a grammar.rule argument".to_string(),
            )),
        }
    }

    fn compile_grammar_ref(&mut self, grammar: &Expr) -> Result<InstrIndex> {
        match grammar {
            Expr::Qualified(qn) => Ok(self
                .code
                .emit(Instruction::LoadString(SmolStr::new(qn.to_string())))),
            _ => self.compile_expr(grammar),
        }
    }

    /// Compile a pattern binding (destructuring).
    fn compile_pattern_binding(&mut self, pattern: &Pattern, source: InstrIndex) -> Result<()> {
        match pattern {
            Pattern::Wildcard => {
                // Discard - nothing to bind
            }
            Pattern::Var(name) => {
                self.bound_vars.insert(name.clone());
                self.code.emit(Instruction::Bind {
                    name: name.clone(),
                    value: source,
                });
            }
            Pattern::Map(entries) => {
                for (key, value_pattern) in entries {
                    let extracted = self.code.emit(Instruction::ExtractMapKey {
                        source,
                        key: key.clone(),
                    });
                    self.compile_pattern_binding(value_pattern, extracted)?;
                }
            }
            Pattern::List(patterns, None) => {
                for (i, pat) in patterns.iter().enumerate() {
                    let extracted = self
                        .code
                        .emit(Instruction::ExtractListIndex { source, index: i });
                    self.compile_pattern_binding(pat, extracted)?;
                }
            }
            _ => {
                return Err(Error::Compiler(format!(
                    "pattern type {:?} not supported in let binding",
                    pattern
                )));
            }
        }
        Ok(())
    }

    /// Compile object definition.
    fn compile_object_def(&mut self, def: &ObjectDef) -> Result<InstrIndex> {
        // Create the object
        let obj_idx = self.code.emit(Instruction::DefineObject(SmolStr::new(
            def.name.to_string(),
        )));

        // Compile and set properties/methods
        for binding in &def.bindings {
            if !binding.has_params {
                // Property
                let value = self.compile_expr(&binding.value)?;
                self.code.emit(Instruction::DefineProp {
                    object: obj_idx,
                    name: binding.name.clone(),
                    value,
                });
            } else {
                // Method
                let mut method_compiler = Compiler::new();
                let body_result = method_compiler.compile_expr(&binding.value)?;
                method_compiler
                    .code
                    .emit(Instruction::Return { value: body_result });
                method_compiler.code.source = Some(SmolStr::new(format!("{}", binding.value)));

                let nested_idx = self.code.nested.len();
                self.code.nested.push(method_compiler.code);

                self.code.emit(Instruction::DefineMethod {
                    object: obj_idx,
                    name: binding.name.clone(),
                    params: binding.params.clone(),
                    body: nested_idx,
                });
            }
        }

        // Define facets
        for facet in &def.facets {
            let mut member_indices = Vec::new();
            for member in &facet.members {
                member_indices.push(self.code.emit(Instruction::LoadSymbol(member.clone())));
            }
            self.code.emit(Instruction::DefineFacet {
                object: obj_idx,
                name: facet.name.clone(),
                members: member_indices,
                terminal: facet.terminal,
            });
        }

        Ok(obj_idx)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(source: &str) -> Result<CompiledCode> {
        let tokens = Lexer::new(source).tokenize()?;
        let ast = Parser::new(&tokens).parse()?;
        Compiler::new().compile(&ast)
    }

    #[test]
    fn test_compile_int() {
        let code = compile("42").unwrap();
        assert_eq!(code.instructions, vec![Instruction::LoadInt(42)]);
    }

    #[test]
    fn test_compile_add() {
        let code = compile("1 + 2").unwrap();
        assert_eq!(
            code.instructions,
            vec![
                Instruction::LoadInt(1),
                Instruction::LoadInt(2),
                Instruction::Add {
                    lhs: InstrIndex(0),
                    rhs: InstrIndex(1)
                }
            ]
        );
    }

    #[test]
    fn test_compile_if() {
        let code = compile("if true then 1 else 2").unwrap();
        // Check that there's a JumpIfFalse instruction
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfFalse { .. }))
        );
    }

    #[test]
    fn test_compile_lambda() {
        let code = compile("lambda (x) x + 1").unwrap();
        // Check that MakeLambda has the param name "x" and points to nested code 0
        assert!(matches!(
            &code.instructions[0],
            Instruction::MakeLambda { params, body: 0, .. } if params.len() == 1 && params[0] == "x"
        ));
        assert_eq!(code.nested.len(), 1);
    }
}
