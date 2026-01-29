//! Compiler: AST to Indexed RPN bytecode.
//!
//! This compiler emits Indexed RPN instructions where each instruction that
//! produces a value writes to `values[ip]`, and consuming instructions
//! explicitly reference operand indices.

use crate::ast::*;
use crate::error::{Error, Result};
use crate::grammar::Grammar;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
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

/// Index into the constants array for string/symbol literals.
///
/// Used by pattern matching instructions to reference constant strings/symbols
/// that have been stored in the constants pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConstIndex(pub usize);

impl ConstIndex {
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl From<usize> for ConstIndex {
    fn from(val: usize) -> Self {
        ConstIndex(val)
    }
}

/// Bytecode instruction using Indexed RPN format.
///
/// Instructions that produce values store their result at `values[ip]`.
/// Instructions that consume values reference operands by their instruction index.
/// Pattern for matching map keys in MatchMap instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapKeyPattern {
    /// Match a specific key (string or symbol).
    Specific(ConstIndex),
    /// Match any key (wildcard).
    Wildcard,
}

/// Pattern for matching map values in MatchMap instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MapValuePattern {
    /// Match any value, no binding (wildcard).
    Wildcard,
    /// Match any value and bind it to a variable.
    Bind(ConstIndex),
    /// Match a specific literal value (int, string, bool, etc.).
    MatchLiteral(ConstIndex),
    /// Execute a full pattern instruction.
    Pattern(InstrIndex),
}

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
    In {
        lhs: InstrIndex,
        rhs: InstrIndex,
    }, // Membership test: lhs in rhs

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
    JumpIfNull {
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
    AwaitAll {
        streams: InstrIndex,
    }, // Wait for all async streams in a list
    Yield {
        value: InstrIndex,
    }, // Yield value from async generator
    YieldToSink {
        value: InstrIndex,
    }, // Yield value to current sink (for grammar backtracking)

    // Data structures (explicit operand indices)
    MakeList {
        elements: Vec<InstrIndex>,
    },
    MakeMap {
        pairs: Vec<(InstrIndex, InstrIndex)>,
    },
    MakeTagged {
        tag: SmolStr,
        args: Vec<InstrIndex>,
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
    StreamCollect {
        source: InstrIndex,
    },
    StreamTake {
        source: InstrIndex,
        n: InstrIndex,
    },
    StreamDrop {
        source: InstrIndex,
        n: InstrIndex,
    },
    StreamParse {
        source: InstrIndex,
        grammar: InstrIndex,
        rule: SmolStr,
    },

    // Pattern matching (explicit operand indices) - For destructuring in let/match
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
    ExtractTaggedChild {
        source: InstrIndex,
        index: usize,
    },
    /// Check if value is a Tagged with expected tag, jump to fail_target if not
    MatchTag {
        value: InstrIndex,
        tag: SmolStr,
        fail_target: InstrIndex,
    },

    // === Grammar Pattern Instructions (for PEG pattern matching) ===
    // These instructions implement PEG pattern matching directly in the VM

    // Backtracking support (for lowering Star/Plus/Choice to base IR)
    ParseCheckpoint, // Create checkpoint for backtracking, store in values[ip]
    ParseRestore {
        checkpoint: InstrIndex,
    }, // Restore from checkpoint value

    // Input stack management (for OMeta-style tree descent)
    ParsePush {
        value: InstrIndex,
    }, // Push value as new input stream (for tree descent)
    ParsePop,      // Pop to previous input stream (for tree ascent)
    ParsePosition, // Get current position as Int (for zero-length guard)

    // List building (for collecting Star/Plus results)
    ListAppend {
        list: InstrIndex,
        item: InstrIndex,
    }, // Append item to list, return new list

    // Type checking (for OMeta-style tree matching)
    IsList {
        value: InstrIndex,
    }, // Check if value is a list, return Bool
    IsMap {
        value: InstrIndex,
    }, // Check if value is a map, return Bool
    IsString {
        value: InstrIndex,
    }, // Check if value is a string, return Bool

    // Leaf patterns (write value to values[ip])
    MatchAny, // Match any item from input, write to values[ip]
    MatchChar {
        char: char,
    }, // Match specific character
    MatchByte {
        byte: u8,
    }, // Match specific byte
    MatchLiteral {
        const_idx: ConstIndex,
    }, // Match string constant from pool
    MatchLiteralValue {
        const_idx: ConstIndex,
    }, // Match literal value (Int, String, Bool, etc.) from constant pool
    MatchCharClass {
        ranges: Vec<(char, char)>,
    }, // Match character in range [a-z]
    MatchNegCharClass {
        ranges: Vec<(char, char)>,
    }, // Match character NOT in range [^a-z]

    // Repeated patterns store pattern data directly (not instruction index)
    // This avoids double-execution of the inner pattern
    MatchPlusChar {
        c: char,
    }, // Match one or more of specific char
    MatchPlusCharClass {
        ranges: Vec<(char, char)>,
    }, // Match one or more chars in range
    MatchPlusLiteral {
        const_idx: ConstIndex,
    }, // Match one or more of literal
    MatchStarChar {
        c: char,
    }, // Match zero or more of specific char
    MatchStarCharClass {
        ranges: Vec<(char, char)>,
    }, // Match zero or more chars in range
    MatchStarLiteral {
        const_idx: ConstIndex,
    }, // Match zero or more of literal

    // Combinators (reference pattern instructions by index)
    // Note: MatchStar, MatchPlus, MatchChoice have been removed - they are now
    // lowered to base IR (loops + jumps) by the compiler. See compile_grammar_pattern.
    MatchSeq {
        patterns: Vec<InstrIndex>,
    }, // Sequence: match all in order
    MatchStarRule {
        rule: ConstIndex,
    }, // Zero or more of a rule (by symbol name)
    MatchPlusRule {
        rule: ConstIndex,
    }, // One or more of a rule (by symbol name)
    MatchOptional {
        pattern: InstrIndex,
    }, // Zero or one

    // Lookahead (don't consume input)
    MatchLookahead {
        pattern: InstrIndex,
    }, // Positive lookahead
    MatchNot {
        pattern: InstrIndex,
    }, // Negative lookahead

    // Binding and actions
    MatchBind {
        pattern: InstrIndex,
        name: ConstIndex,
    }, // Bind result to variable
    MatchGuard {
        pattern: InstrIndex,
        predicate: InstrIndex,
    }, // Match if predicate true
    MatchAction {
        pattern: InstrIndex,
        action: InstrIndex,
    }, // Match + evaluate expr
    MatchList {
        patterns: Vec<InstrIndex>,
        rest: Option<InstrIndex>,
    }, // Match list with element patterns
    MatchListWithBindings {
        patterns: Vec<Option<ConstIndex>>,
        rest: Option<ConstIndex>,
    }, // Match list with bindings (no pattern execution)
    MatchMap {
        entries: Vec<(MapKeyPattern, MapValuePattern)>,
    }, // Match map with key and value patterns
    MatchMapNested {
        entries: Vec<(ConstIndex, NestedBinding)>,
    }, // Match map with nested bindings
    /// Match a tagged/constructor value: :Tag(child_patterns...)
    /// tag_idx is constant index of expected tag name
    /// patterns are child pattern instruction indices (can be nested TagMatch, Bind, Any, etc.)
    MatchTagged {
        tag_idx: ConstIndex,
        patterns: Vec<InstrIndex>,
    },
    /// Match a tagged value with simple bindings (no nested patterns execution)
    MatchTaggedWithBindings {
        tag_idx: ConstIndex,
        bindings: Vec<Option<ConstIndex>>,
    },

    // Rule application
    ApplyRule {
        rule_idx: ConstIndex,
    }, // Apply named grammar rule

    // End of input
    MatchEnd, // Match end of stream

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

/// A nested binding for pattern matching.
/// For example, `%{outer: %{inner: x}}` would have path ["outer", "inner"] and variable "x".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NestedBinding {
    /// The path of keys to navigate (e.g., ["outer", "inner"])
    pub path: Vec<SmolStr>,
    /// The variable name to bind (e.g., "x")
    pub variable: SmolStr,
}

/// Compiled bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledCode {
    pub instructions: Vec<Instruction>,
    /// Nested code blocks (for lambdas, methods).
    pub nested: Vec<CompiledCode>,
    /// Original source code (for reflection/decompilation).
    pub source: Option<SmolStr>,
    /// Constants pool for values (Int, Bool, String, Symbol, etc.) referenced by ConstIndex.
    /// Used by pattern matching instructions and other constant value needs.
    pub constants: Vec<Value>,
    /// Rule entry points for grammar compilation.
    /// Maps rule names to their instruction indices for ApplyRule/MatchStarRule/MatchPlusRule.
    pub rule_entry_points: HashMap<SmolStr, InstrIndex>,
}

impl CompiledCode {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            nested: Vec::new(),
            source: None,
            constants: Vec::new(),
            rule_entry_points: HashMap::new(),
        }
    }

    pub fn with_source(source: SmolStr) -> Self {
        Self {
            instructions: Vec::new(),
            nested: Vec::new(),
            source: Some(source),
            constants: Vec::new(),
            rule_entry_points: HashMap::new(),
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
            Instruction::JumpIfNull { target: t, .. } => *t = target,
            Instruction::MatchPattern { fail_target: t, .. } => *t = target,
            Instruction::PushHandler { catch_target: t } => *t = target,
            _ => panic!("not a jump instruction at index {}", idx.0),
        }
    }

    /// Add a value to the constants pool and return its index.
    /// Accepts any type that implements Into<Value>, including primitives.
    fn add_constant<T: Into<Value>>(&mut self, value: T) -> ConstIndex {
        let value = value.into();
        // Check if already exists (using Value's PartialEq)
        if let Some(idx) = self.constants.iter().position(|v| v == &value) {
            return ConstIndex(idx);
        }
        // Add new constant
        let idx = ConstIndex(self.constants.len());
        self.constants.push(value);
        idx
    }

    /// Get a constant value by index.
    pub fn get_constant(&self, idx: ConstIndex) -> Value {
        self.constants[idx.0].clone()
    }

    /// Get a constant and convert it to the target type using TryInto.
    pub fn get_constant_as<T>(&self, idx: ConstIndex) -> crate::error::Result<T>
    where
        T: std::convert::TryFrom<Value>,
        <T as std::convert::TryFrom<Value>>::Error: Into<crate::error::Error>,
    {
        self.constants[idx.0]
            .clone()
            .try_into()
            .map_err(|e| Into::<crate::error::Error>::into(e))
    }

    /// Add a rule entry point for grammar compilation.
    pub fn add_rule_entry(&mut self, rule_name: SmolStr, entry_point: InstrIndex) {
        self.rule_entry_points.insert(rule_name, entry_point);
    }

    /// Get a rule entry point by name.
    pub fn get_rule_entry(&self, rule_name: &str) -> Option<InstrIndex> {
        self.rule_entry_points.get(rule_name).copied()
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

    /// Compile a Grammar to bytecode with rule entry points (static method).
    /// This is a standalone compilation that creates a new CompiledCode.
    pub fn compile_grammar_only(grammar: &Grammar) -> Result<CompiledCode> {
        let mut compiler = Self::new();
        compiler.compile_grammar(grammar)?;
        Ok(compiler.code)
    }

    /// Compile an expression and return the index where its value is stored.
    fn compile_expr(&mut self, expr: &Expr) -> Result<InstrIndex> {
        match expr {
            Expr::Int(n) => Ok(self.code.emit(Instruction::LoadInt(*n))),
            Expr::Float(f) => Ok(self.code.emit(Instruction::LoadFloat(*f))),
            Expr::String(s) => Ok(self.code.emit(Instruction::LoadString(s.clone()))),
            Expr::Symbol(s) => Ok(self.code.emit(Instruction::LoadSymbol(s.clone()))),
            Expr::Tagged(tag, args) => {
                let mut arg_indices = Vec::with_capacity(args.len());
                for arg in args {
                    arg_indices.push(self.compile_expr(arg)?);
                }
                Ok(self.code.emit(Instruction::MakeTagged {
                    tag: tag.clone(),
                    args: arg_indices,
                }))
            }
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
            Expr::For(pattern, iterable, body) => self.compile_for(pattern, iterable, body),
            Expr::Fold {
                initial,
                acc_var,
                iterable,
                body,
            } => self.compile_fold(initial, acc_var, iterable, body),
            Expr::Foldr {
                initial,
                acc_var,
                iterable,
                body,
            } => self.compile_foldr(initial, acc_var, iterable, body),
            Expr::MapEach {
                elem_var,
                iterable,
                body,
            } => self.compile_map_each(elem_var, iterable, body),
            Expr::Filter {
                elem_var,
                iterable,
                body,
            } => self.compile_filter(elem_var, iterable, body),
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
                // GrammarApply now always returns a stream
                Ok(self.code.emit(Instruction::GrammarApply {
                    input: input_idx,
                    grammar: grammar_idx,
                    rule: rule.clone(),
                }))
            }
            Expr::GrammarLiteral(grammar) => {
                // For now, just load the grammar AST (not compiled to bytecode yet)
                // Grammar compilation to bytecode happens on-demand when rules are applied
                Ok(self
                    .code
                    .emit(Instruction::LoadGrammar(Arc::new(grammar.clone()))))
            }
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
            Expr::Yield(expr) => {
                let value = self.compile_expr(expr)?;
                // YieldToSink sends the value to the current sink and continues execution
                Ok(self.code.emit(Instruction::YieldToSink { value }))
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
                    BinOp::In => self.code.emit(Instruction::In { lhs, rhs }),
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

                // Convert ast::parse(args) to __builtin_ast.parse(args)
                if module == "ast" && method == "parse" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_ast")));
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

                // Convert ir::compile(args) to __builtin_ir.compile(args)
                if module == "ir" && method == "compile" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_ir")));
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

                // Convert code::eval(args) to __builtin_code.eval(args)
                if module == "code" && method == "eval" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_code")));
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

                // Convert rand::int(args) and rand::float(args) to __builtin_rand.method(args)
                if module == "rand" && (method == "int" || method == "float") {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_rand")));
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

                // Convert stream::observe(args) to __builtin_stream.observe(args)
                if module == "stream" && method == "observe" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_stream")));
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

                // Convert cursor::advance, cursor::rewind, cursor::position, cursor::current to __builtin_cursor.method(args)
                if module == "cursor"
                    && (method == "advance"
                        || method == "rewind"
                        || method == "position"
                        || method == "current")
                {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
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

                // Convert io::load(args) to __builtin_io.load(args)
                if module == "io" && method == "load" {
                    let builtin_idx = self
                        .code
                        .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_io")));
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

        // Check for await_all(list) - special RLM primitive
        if let Expr::Ident(name) = func {
            if name == "await_all" {
                if args.len() != 1 {
                    return Err(Error::Compiler(format!(
                        "await_all expects 1 argument, got {}",
                        args.len()
                    )));
                }
                match &args[0] {
                    Arg::Expr(e) => {
                        let streams_idx = self.compile_expr(e)?;
                        return Ok(self.code.emit(Instruction::AwaitAll {
                            streams: streams_idx,
                        }));
                    }
                    Arg::Placeholder => {
                        return Err(Error::Compiler(
                            "await_all does not support partial application".to_string(),
                        ));
                    }
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

    /// Compile for loop: for pattern in iterable { body }.
    ///
    /// This desugars to cursor-based iteration:
    /// - Convert iterable to stream if needed (via observe)
    /// - Create cursor from stream
    /// - Loop while cursor has data
    /// - Bind pattern to current element in body
    /// - Advance cursor each iteration
    ///
    /// Returns null (for loops are for side effects).
    fn compile_for(
        &mut self,
        pattern: &Pattern,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<InstrIndex> {
        // Compile the iterable expression
        let iterable_idx = self.compile_expr(iterable)?;

        // Convert to cursor via observe: __builtin_stream.observe(iterable)
        let builtin_stream = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_stream")));
        let cursor_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_stream,
            method: SmolStr::new("observe"),
            args: vec![iterable_idx],
        });

        // Store cursor in temp variable for access in loop
        let cursor_var = self.gen_temp_name("cursor");
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: cursor_idx,
        });

        // Loop start
        let loop_start = self.code.next_index();

        // Load cursor and check if we have data
        // Call __builtin_cursor.current(cursor) and check if it's null
        let cursor_load = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let builtin_cursor = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let current_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor,
            method: SmolStr::new("current"),
            args: vec![cursor_load],
        });

        // Check if current element is not null
        let null_val = self.code.emit(Instruction::LoadNull);
        let has_more = self.code.emit(Instruction::NotEq {
            lhs: current_idx,
            rhs: null_val,
        });

        // Exit if no more data
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: has_more,
            target: InstrIndex(0), // placeholder
        });

        // Push scope for pattern binding
        self.code.emit(Instruction::PushScope);

        // Bind pattern to current element (reuse current_idx from above)
        self.compile_pattern_binding(pattern, current_idx)?;

        // Compile body (result discarded)
        let _body_idx = self.compile_expr(body)?;

        // Pop scope (unbind pattern variables)
        self.code.emit(Instruction::PopScope);

        // Advance cursor for next iteration
        // Call __builtin_cursor.advance(cursor, 1)
        let cursor_for_advance = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let advance_arg = self.code.emit(Instruction::LoadInt(1));
        let builtin_cursor3 = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let advanced_cursor = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor3,
            method: SmolStr::new("advance"),
            args: vec![cursor_for_advance, advance_arg],
        });

        // Update cursor variable
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: advanced_cursor,
        });

        // Jump back to start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // For returns null
        Ok(self.code.emit(Instruction::LoadNull))
    }

    /// Compile fold left: fold initial, acc in iterable { body }
    /// Returns the accumulated value
    fn compile_fold(
        &mut self,
        initial: &Expr,
        acc_var: &SmolStr,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<InstrIndex> {
        // Compile initial value
        let initial_idx = self.compile_expr(initial)?;

        // Store accumulator in variable
        self.code.emit(Instruction::StoreVar {
            name: acc_var.clone(),
            value: initial_idx,
        });

        // Compile iterable to cursor
        let iterable_idx = self.compile_expr(iterable)?;
        let builtin_stream = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_stream")));
        let cursor_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_stream,
            method: SmolStr::new("observe"),
            args: vec![iterable_idx],
        });

        // Store cursor in temp variable
        let cursor_var = self.gen_temp_name("cursor");
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: cursor_idx,
        });

        // Loop start
        let loop_start = self.code.next_index();

        // Check if cursor has more data
        let cursor_load = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let builtin_cursor = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let current_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor,
            method: SmolStr::new("current"),
            args: vec![cursor_load],
        });

        // Exit if null
        let null_val = self.code.emit(Instruction::LoadNull);
        let has_more = self.code.emit(Instruction::NotEq {
            lhs: current_idx,
            rhs: null_val,
        });
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: has_more,
            target: InstrIndex(0),
        });

        // Store current element in standard variable for function call
        let elem_var = SmolStr::new("_elem");
        self.code.emit(Instruction::StoreVar {
            name: elem_var.clone(),
            value: current_idx,
        });

        // Compile body (function) and call it with curried application
        // For curried lambdas: (\acc \elem acc + elem)(acc_val)(elem_val)
        // We need to call sequentially, not all at once
        let lambda_idx = self.compile_expr(body)?;

        // Load accumulator
        let acc_idx = self.code.emit(Instruction::LoadVar(acc_var.clone()));

        // First call: apply lambda to acc - returns inner function
        let inner_func_idx = self.code.emit(Instruction::Call {
            func: lambda_idx,
            args: vec![acc_idx],
        });

        // Load element
        let elem_idx = self.code.emit(Instruction::LoadVar(elem_var.clone()));

        // Second call: apply inner function to elem - returns result
        let body_idx = self.code.emit(Instruction::Call {
            func: inner_func_idx,
            args: vec![elem_idx],
        });

        // Update accumulator with body result
        self.code.emit(Instruction::StoreVar {
            name: acc_var.clone(),
            value: body_idx,
        });

        // Advance cursor
        let cursor_for_advance = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let advance_arg = self.code.emit(Instruction::LoadInt(1));
        let builtin_cursor2 = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let advanced_cursor = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor2,
            method: SmolStr::new("advance"),
            args: vec![cursor_for_advance, advance_arg],
        });
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: advanced_cursor,
        });

        // Jump back to loop start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // Return accumulator value
        Ok(self.code.emit(Instruction::LoadVar(acc_var.clone())))
    }

    /// Compile fold right: foldr func initial iterable
    /// Returns the accumulated value
    fn compile_foldr(
        &mut self,
        initial: &Expr,
        acc_var: &SmolStr,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<InstrIndex> {
        // For foldr, we need to process elements in reverse order
        // We'll collect elements into a list first, then iterate backwards

        // Compile initial value
        let initial_idx = self.compile_expr(initial)?;

        // Store accumulator in variable
        self.code.emit(Instruction::StoreVar {
            name: acc_var.clone(),
            value: initial_idx,
        });

        // Compile iterable to get the list
        let iterable_idx = self.compile_expr(iterable)?;

        // Store list in temp variable
        let list_var = self.gen_temp_name("list");
        self.code.emit(Instruction::StoreVar {
            name: list_var.clone(),
            value: iterable_idx,
        });

        // Get list length
        let list_load = self.code.emit(Instruction::LoadVar(list_var.clone()));
        let len_idx = self.code.emit(Instruction::MethodCall {
            receiver: list_load,
            method: SmolStr::new("len"),
            args: vec![],
        });

        // Store length and create index variable
        let len_var = self.gen_temp_name("len");
        self.code.emit(Instruction::StoreVar {
            name: len_var.clone(),
            value: len_idx,
        });

        // Initialize index to len - 1 (start from last element)
        let len_loaded = self.code.emit(Instruction::LoadVar(len_var.clone()));
        let one = self.code.emit(Instruction::LoadInt(1));
        let idx_idx = self.code.emit(Instruction::Sub {
            lhs: len_loaded,
            rhs: one,
        });
        let idx_var = self.gen_temp_name("idx");
        self.code.emit(Instruction::StoreVar {
            name: idx_var.clone(),
            value: idx_idx,
        });

        // Loop start
        let loop_start = self.code.next_index();

        // Check if index >= 0 (equivalent to 0 <= index)
        let zero = self.code.emit(Instruction::LoadInt(0));
        let idx_load = self.code.emit(Instruction::LoadVar(idx_var.clone()));
        let has_more = self.code.emit(Instruction::LtEq {
            lhs: zero,
            rhs: idx_load,
        });
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: has_more,
            target: InstrIndex(0),
        });

        // Get element at index
        let list_load2 = self.code.emit(Instruction::LoadVar(list_var.clone()));
        let idx_load2 = self.code.emit(Instruction::LoadVar(idx_var.clone()));
        let current_idx = self.code.emit(Instruction::Index {
            collection: list_load2,
            key: idx_load2,
        });

        // Store current element in standard variable for function call
        let elem_var = SmolStr::new("_elem");
        self.code.emit(Instruction::StoreVar {
            name: elem_var.clone(),
            value: current_idx,
        });

        // Compile body (function) and call it with curried application
        // For curried lambdas: (\acc \elem elem * acc)(acc_val)(elem_val)
        let lambda_idx = self.compile_expr(body)?;

        // Load accumulator
        let acc_idx = self.code.emit(Instruction::LoadVar(acc_var.clone()));

        // First call: apply lambda to acc - returns inner function
        let inner_func_idx = self.code.emit(Instruction::Call {
            func: lambda_idx,
            args: vec![acc_idx],
        });

        // Load element
        let elem_idx = self.code.emit(Instruction::LoadVar(elem_var.clone()));

        // Second call: apply inner function to elem - returns result
        let body_idx = self.code.emit(Instruction::Call {
            func: inner_func_idx,
            args: vec![elem_idx],
        });

        // Update accumulator with body result
        self.code.emit(Instruction::StoreVar {
            name: acc_var.clone(),
            value: body_idx,
        });

        // Decrement index
        let idx_load3 = self.code.emit(Instruction::LoadVar(idx_var.clone()));
        let one2 = self.code.emit(Instruction::LoadInt(1));
        let new_idx = self.code.emit(Instruction::Sub {
            lhs: idx_load3,
            rhs: one2,
        });
        self.code.emit(Instruction::StoreVar {
            name: idx_var.clone(),
            value: new_idx,
        });

        // Jump back to loop start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // Return accumulator value
        Ok(self.code.emit(Instruction::LoadVar(acc_var.clone())))
    }

    /// Compile map: map x in iterable { body }
    /// Returns a new list with transformed elements
    fn compile_map_each(
        &mut self,
        elem_var: &SmolStr,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<InstrIndex> {
        // Create empty result list
        let result_list_var = self.gen_temp_name("result");
        let empty_list = self.code.emit(Instruction::MakeList { elements: vec![] });
        self.code.emit(Instruction::StoreVar {
            name: result_list_var.clone(),
            value: empty_list,
        });

        // Compile iterable to cursor
        let iterable_idx = self.compile_expr(iterable)?;
        let builtin_stream = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_stream")));
        let cursor_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_stream,
            method: SmolStr::new("observe"),
            args: vec![iterable_idx],
        });

        // Store cursor in temp variable
        let cursor_var = self.gen_temp_name("cursor");
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: cursor_idx,
        });

        // Loop start
        let loop_start = self.code.next_index();

        // Check if cursor has more data
        let cursor_load = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let builtin_cursor = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let current_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor,
            method: SmolStr::new("current"),
            args: vec![cursor_load],
        });

        // Exit if null
        let null_val = self.code.emit(Instruction::LoadNull);
        let has_more = self.code.emit(Instruction::NotEq {
            lhs: current_idx,
            rhs: null_val,
        });
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: has_more,
            target: InstrIndex(0),
        });

        // Store current element for body access
        self.code.emit(Instruction::StoreVar {
            name: elem_var.clone(),
            value: current_idx,
        });

        // Compile body to get transformed value
        // If body is a lambda/short lambda, we need to call it with the element
        let body_idx = match body {
            // For short lambda \x expr, call it with the element
            Expr::ShortLambda(_, _) | Expr::Lambda(_, _) => {
                // Compile the lambda to get a callable value
                let lambda_idx = self.compile_expr(body)?;

                // Load the element as argument
                let elem_idx = self.code.emit(Instruction::LoadVar(elem_var.clone()));

                // Call the lambda with the element
                self.code.emit(Instruction::Call {
                    func: lambda_idx,
                    args: vec![elem_idx],
                })
            }
            // For direct expression, compile it directly
            _ => self.compile_expr(body)?,
        };

        // Append to result list: list.push(body_result)
        let result_list_load = self
            .code
            .emit(Instruction::LoadVar(result_list_var.clone()));
        let _push_result = self.code.emit(Instruction::MethodCall {
            receiver: result_list_load,
            method: SmolStr::new("push"),
            args: vec![body_idx],
        });
        // Note: push returns new list, but we're not storing it back
        // This is a bug in the current approach - we need to store the result

        // For now, let's store the push result back
        self.code.emit(Instruction::StoreVar {
            name: result_list_var.clone(),
            value: _push_result,
        });

        // Advance cursor
        let cursor_for_advance = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let advance_arg = self.code.emit(Instruction::LoadInt(1));
        let builtin_cursor2 = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let advanced_cursor = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor2,
            method: SmolStr::new("advance"),
            args: vec![cursor_for_advance, advance_arg],
        });
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: advanced_cursor,
        });

        // Jump back to loop start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // Return result list
        Ok(self
            .code
            .emit(Instruction::LoadVar(result_list_var.clone())))
    }

    /// Compile filter: filter x in iterable { body }
    /// Returns a new list with elements where body is truthy
    fn compile_filter(
        &mut self,
        elem_var: &SmolStr,
        iterable: &Expr,
        body: &Expr,
    ) -> Result<InstrIndex> {
        // Create empty result list
        let result_list_var = self.gen_temp_name("result");
        let empty_list = self.code.emit(Instruction::MakeList { elements: vec![] });
        self.code.emit(Instruction::StoreVar {
            name: result_list_var.clone(),
            value: empty_list,
        });

        // Compile iterable to cursor
        let iterable_idx = self.compile_expr(iterable)?;
        let builtin_stream = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_stream")));
        let cursor_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_stream,
            method: SmolStr::new("observe"),
            args: vec![iterable_idx],
        });

        // Store cursor in temp variable
        let cursor_var = self.gen_temp_name("cursor");
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: cursor_idx,
        });

        // Loop start
        let loop_start = self.code.next_index();

        // Check if cursor has more data
        let cursor_load = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let builtin_cursor = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let current_idx = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor,
            method: SmolStr::new("current"),
            args: vec![cursor_load],
        });

        // Exit if null
        let null_val = self.code.emit(Instruction::LoadNull);
        let has_more = self.code.emit(Instruction::NotEq {
            lhs: current_idx,
            rhs: null_val,
        });
        let exit_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: has_more,
            target: InstrIndex(0),
        });

        // Store current element for predicate access
        let elem_var_local = elem_var.clone();
        self.code.emit(Instruction::StoreVar {
            name: elem_var_local.clone(),
            value: current_idx,
        });

        // Compile body (predicate)
        // If body is a lambda/short lambda, we need to call it with the element
        let pred_idx = match body {
            // For short lambda \x expr, call it with the element
            Expr::ShortLambda(_, _) | Expr::Lambda(_, _) => {
                // Compile the lambda to get a callable value
                let lambda_idx = self.compile_expr(body)?;

                // Load the element as argument
                let elem_idx = self.code.emit(Instruction::LoadVar(elem_var_local.clone()));

                // Call the lambda with the element
                self.code.emit(Instruction::Call {
                    func: lambda_idx,
                    args: vec![elem_idx],
                })
            }
            // For direct expression, compile it directly
            _ => self.compile_expr(body)?,
        };

        // Check if predicate is truthy (not false and not null)
        // Filter keeps elements where body evaluates to true
        let false_val = self.code.emit(Instruction::LoadBool(false));
        let is_not_false = self.code.emit(Instruction::NotEq {
            lhs: pred_idx,
            rhs: false_val,
        });
        let skip_jump = self.code.emit(Instruction::JumpIfFalse {
            cond: is_not_false,
            target: InstrIndex(0),
        });

        // If truthy, append element to result
        let result_list_load = self
            .code
            .emit(Instruction::LoadVar(result_list_var.clone()));
        let elem_load = self.code.emit(Instruction::LoadVar(elem_var.clone()));
        let _push_result = self.code.emit(Instruction::MethodCall {
            receiver: result_list_load,
            method: SmolStr::new("push"),
            args: vec![elem_load],
        });
        self.code.emit(Instruction::StoreVar {
            name: result_list_var.clone(),
            value: _push_result,
        });

        // Patch skip jump
        let skip_target = self.code.next_index();
        self.code.patch_jump_target(skip_jump, skip_target);

        // Advance cursor
        let cursor_for_advance = self.code.emit(Instruction::LoadVar(cursor_var.clone()));
        let advance_arg = self.code.emit(Instruction::LoadInt(1));
        let builtin_cursor2 = self
            .code
            .emit(Instruction::LoadSymbol(SmolStr::new("__builtin_cursor")));
        let advanced_cursor = self.code.emit(Instruction::MethodCall {
            receiver: builtin_cursor2,
            method: SmolStr::new("advance"),
            args: vec![cursor_for_advance, advance_arg],
        });
        self.code.emit(Instruction::StoreVar {
            name: cursor_var.clone(),
            value: advanced_cursor,
        });

        // Jump back to loop start
        self.code.emit(Instruction::Jump { target: loop_start });

        // Exit point
        let end_target = self.code.next_index();
        self.code.patch_jump_target(exit_jump, end_target);

        // Return result list
        Ok(self
            .code
            .emit(Instruction::LoadVar(result_list_var.clone())))
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

            // Compile guard condition if present
            let skip_jump = if let Some(guard) = &case.guard {
                let guard_idx = self.compile_expr(guard)?;
                Some(self.code.emit(Instruction::JumpIfFalse {
                    cond: guard_idx,
                    target: InstrIndex(0), // placeholder
                }))
            } else {
                None
            };

            // Compile case body (shared between guarded and unguarded cases)
            let _ = last_body_idx;
            last_body_idx = self.compile_expr(&case.body)?;
            self.code.emit(Instruction::PopScope);
            end_jumps.push(self.code.emit(Instruction::Jump {
                target: InstrIndex(0), // placeholder
            }));

            // Patch guard jump to skip to after PopScope
            if let Some(skip) = skip_jump {
                let skip_target = self.code.next_index();
                self.code.patch_jump_target(skip, skip_target);
                self.code.emit(Instruction::PopScope);
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
            "collect" => {
                if !args.is_empty() {
                    return Err(Error::Compiler("collect expects 0 arguments".to_string()));
                }
                let source = self.compile_expr(left)?;
                Ok(Some(self.code.emit(Instruction::StreamCollect { source })))
            }
            "take" => {
                if args.len() != 1 {
                    return Err(Error::Compiler("take expects 1 argument".to_string()));
                }
                let source = self.compile_expr(left)?;
                let n = self.compile_arg(&args[0])?;
                Ok(Some(self.code.emit(Instruction::StreamTake { source, n })))
            }
            "drop" => {
                if args.len() != 1 {
                    return Err(Error::Compiler("drop expects 1 argument".to_string()));
                }
                let source = self.compile_expr(left)?;
                let n = self.compile_arg(&args[0])?;
                Ok(Some(self.code.emit(Instruction::StreamDrop { source, n })))
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

    /// Collect all rules from a grammar, including parent grammars.
    /// Returns a HashMap of rule_name -> rule (child rules shadow parent rules).
    fn collect_all_rules(grammar: &Grammar) -> HashMap<SmolStr, crate::grammar::Rule> {
        let mut all_rules = HashMap::new();

        // First add parent rules (if any)
        if let Some(parent) = &grammar.parent_grammar {
            all_rules.extend(Self::collect_all_rules(parent));
        }

        // Then add this grammar's rules (shadowing parent rules)
        all_rules.extend(grammar.rules.clone());

        all_rules
    }

    /// Compile a Grammar to bytecode with rule entry points.
    ///
    /// This method takes a Grammar AST and compiles all rules to bytecode,
    /// storing entry points for each rule. The resulting bytecode can be used
    /// by ApplyRule, MatchStarRule, and MatchPlusRule instructions.
    pub fn compile_grammar(&mut self, grammar: &Grammar) -> Result<()> {
        // Collect all rules including parent rules (child rules shadow parent rules)
        let all_rules = Self::collect_all_rules(grammar);

        for (rule_name, rule) in all_rules {
            // Record the entry point for this rule
            let entry_point = self.code.next_index();
            self.code.add_rule_entry(rule_name.clone(), entry_point);

            // Compile the rule's pattern
            let pattern_idx = self.compile_grammar_pattern(&rule.pattern)?;

            // If there's a semantic action, compile it as MatchAction
            let result_idx = if let Some(action) = &rule.action {
                // Compile the action expression
                let action_idx = self.compile_expr(action)?;
                // Emit MatchAction that transforms the pattern result
                self.code.emit(Instruction::MatchAction {
                    pattern: pattern_idx,
                    action: action_idx,
                })
            } else {
                // No action - just use the pattern result directly
                pattern_idx
            };

            // Each rule should end with a Return instruction to return to the caller
            self.code.emit(Instruction::Return { value: result_idx });
        }

        Ok(())
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
            Pattern::Constructor(_tag, patterns) => {
                // Extract children positionally from Tagged value
                // Note: this doesn't do runtime tag checking in let bindings
                // Full matching with tag check happens in @ blocks via MatchTag
                for (i, pat) in patterns.iter().enumerate() {
                    let extracted = self
                        .code
                        .emit(Instruction::ExtractTaggedChild { source, index: i });
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

    /// Extract the nested binding path and variable name from a pattern.
    /// For example, for key="outer" and pattern=`%{inner: x}`, returns `(vec!["outer", "inner"], "x")`.
    fn extract_nested_binding_path(
        pattern: &crate::grammar::Pattern,
        key: &SmolStr,
    ) -> Option<(Vec<SmolStr>, SmolStr)> {
        use crate::grammar::Pattern as GP;

        match pattern {
            GP::Bind(_, name, _) => {
                // Direct binding - path is just [key]
                Some((vec![key.clone()], name.clone()))
            }
            GP::MapMatch(entries) => {
                // Nested map pattern - extract bindings from nested entries
                for (nested_key, nested_pattern) in entries {
                    if let Some((mut path, variable)) =
                        Self::extract_nested_binding_path(nested_pattern, nested_key)
                    {
                        // Prepend the current key to the path
                        path.insert(0, key.clone());
                        return Some((path, variable));
                    }
                }
                None
            }
            GP::Any => None, // Wildcard - no binding
            _ => None,       // Other patterns not supported for nested binding extraction
        }
    }

    /// Compile a grammar pattern into pattern matching instructions.
    /// This compiles grammar::Pattern AST into the new pattern instruction set.
    pub fn compile_grammar_pattern(
        &mut self,
        pattern: &crate::grammar::Pattern,
    ) -> Result<InstrIndex> {
        use crate::grammar::Pattern as GP;

        Ok(match pattern {
            GP::Empty => {
                // Empty pattern matches nothing and succeeds
                let const_idx = self.code.add_constant(SmolStr::new(""));
                self.code.emit(Instruction::MatchLiteral { const_idx })
            }

            GP::Any => self.code.emit(Instruction::MatchAny),

            GP::Char(c) => self.code.emit(Instruction::MatchChar { char: *c }),

            GP::Literal(s) => {
                let const_idx = self.code.add_constant(s.clone());
                self.code.emit(Instruction::MatchLiteral { const_idx })
            }

            GP::Seq(patterns) => {
                // Check if the last pattern is an Action - if so, handle specially
                let len = patterns.len();
                if len > 0 {
                    let last = &patterns[len - 1];
                    if let GP::Action(_pattern, action) = last {
                        // Clone the action expression to avoid borrow issues
                        let action_clone = action.clone();

                        // Compile the prefix patterns (without the action)
                        let compiled: Vec<InstrIndex> = patterns[..len - 1]
                            .iter()
                            .map(|p| self.compile_grammar_pattern(p))
                            .collect::<Result<Vec<_>>>()?;

                        // Emit MatchSeq for the prefix, then execute the action
                        let seq_idx = if compiled.is_empty() {
                            // Empty sequence - use special action instruction
                            let const_idx = self.code.add_constant(SmolStr::new(""));
                            self.code.emit(Instruction::MatchLiteral { const_idx })
                        } else if compiled.len() == 1 {
                            // Single pattern - just use it directly
                            compiled[0]
                        } else {
                            self.code.emit(Instruction::MatchSeq { patterns: compiled })
                        };

                        // Now emit the action instruction
                        let action_idx = self.compile_expr(&action_clone)?;
                        self.code.emit(Instruction::MatchAction {
                            pattern: seq_idx,
                            action: action_idx,
                        })
                    } else {
                        // No action - compile normally
                        let compiled: Vec<InstrIndex> = patterns
                            .iter()
                            .map(|p| self.compile_grammar_pattern(p))
                            .collect::<Result<Vec<_>>>()?;
                        self.code.emit(Instruction::MatchSeq { patterns: compiled })
                    }
                } else {
                    // Empty sequence
                    let const_idx = self.code.add_constant(SmolStr::new(""));
                    self.code.emit(Instruction::MatchLiteral { const_idx })
                }
            }

            GP::Choice(patterns) => {
                // Choice with optional backtracking marker per alternative.
                // Each pattern is (Pattern, uses_backtracking).
                //
                // Traditional PEG semantics (no ? markers):
                //   First match wins, stop immediately
                //
                // Backtracking semantics (with ? markers):
                //   Collect all marked alternatives for backtracking search
                //   Unmarked alternatives still use traditional semantics (return first match)

                if patterns.is_empty() {
                    // Empty choice - always fail (return Null)
                    return Ok(self.code.emit(Instruction::LoadNull));
                }

                if patterns.len() == 1 {
                    // Single pattern - no choice needed, ignore backtracking flag
                    return self.compile_grammar_pattern(&patterns[0].0);
                }

                // Check if any alternative uses backtracking
                let any_backtracking = patterns.iter().any(|(_, uses_bt)| *uses_bt);

                if any_backtracking {
                    // TODO: Full backtracking implementation
                    // For now, emit the same IR as traditional PEG
                    // Future: Create BacktrackEntry::Choice with all successful alternatives
                }

                // Compile choice to IR with checkpoint/restore:
                //   result_var = null
                //   checkpoint = ParseCheckpoint
                //   for each pattern:
                //     case_result = <compile pattern>
                //     if not last:
                //       JumpIfNull case_result, next_case
                //       StoreVar result_var, case_result
                //       Jump done
                //       next_case: ParseRestore checkpoint
                //     else:
                //       StoreVar result_var, case_result
                //   done: LoadVar result_var

                // Generate unique var name for result
                let result_var =
                    SmolStr::new(format!("__choice_result_{}", self.code.next_index().0));

                // Initialize result to null
                let null_idx = self.code.emit(Instruction::LoadNull);
                self.code.emit(Instruction::StoreVar {
                    name: result_var.clone(),
                    value: null_idx,
                });

                // Create checkpoint for backtracking
                let checkpoint_idx = self.code.emit(Instruction::ParseCheckpoint);

                let patterns_len = patterns.len();
                let mut jump_to_done: Vec<InstrIndex> = Vec::new();

                for (i, (pattern, _uses_bt)) in patterns.iter().enumerate() {
                    let is_last = i == patterns_len - 1;

                    // Compile this case
                    let case_result_idx = self.compile_grammar_pattern(pattern)?;

                    if !is_last {
                        // Not the last case - check for failure and jump to next
                        let jump_to_next = self.code.emit(Instruction::JumpIfNull {
                            cond: case_result_idx,
                            target: InstrIndex(0), // placeholder
                        });

                        // Store successful result
                        self.code.emit(Instruction::StoreVar {
                            name: result_var.clone(),
                            value: case_result_idx,
                        });

                        // Jump to done
                        let jump_done = self.code.emit(Instruction::Jump {
                            target: InstrIndex(0), // placeholder
                        });
                        jump_to_done.push(jump_done);

                        // Next case label - restore checkpoint first
                        let next_case_label = self.code.next_index();
                        self.code.patch_jump_target(jump_to_next, next_case_label);

                        // Restore position before trying next case
                        self.code.emit(Instruction::ParseRestore {
                            checkpoint: checkpoint_idx,
                        });
                    } else {
                        // Last case - just store result (may be null if failed)
                        self.code.emit(Instruction::StoreVar {
                            name: result_var.clone(),
                            value: case_result_idx,
                        });
                    }
                }

                // Done label - patch all jumps
                let done_label = self.code.next_index();
                for jump_idx in jump_to_done {
                    self.code.patch_jump_target(jump_idx, done_label);
                }

                // Return the result
                self.code.emit(Instruction::LoadVar(result_var))
            }

            GP::Star(p) => {
                // Star(p) compiles differently based on pattern type:
                // - Char-based patterns (Char, CharClass, Literal): use specialized instructions
                //   that join results into a string (for backward compatibility)
                // - Rule patterns: use MatchStarRule
                // - Other patterns: lower to IR loop

                match p.as_ref() {
                    GP::Rule(name) => {
                        // Rule reference - use rule-based repetition
                        let const_idx = self.code.add_constant(name.clone());
                        self.code
                            .emit(Instruction::MatchStarRule { rule: const_idx })
                    }
                    GP::Char(c) => self.code.emit(Instruction::MatchStarChar { c: *c }),
                    GP::CharClass(ranges) => {
                        use crate::grammar::CharRange;
                        let range_tuples: Vec<(char, char)> = ranges
                            .iter()
                            .map(|r| match r {
                                CharRange::Char(c) => (*c, *c),
                                CharRange::Range(lo, hi) => (*lo, *hi),
                            })
                            .collect();
                        self.code.emit(Instruction::MatchStarCharClass {
                            ranges: range_tuples,
                        })
                    }
                    GP::Literal(s) => {
                        let const_idx = self.code.add_constant(s.clone());
                        self.code.emit(Instruction::MatchStarLiteral { const_idx })
                    }
                    _ => {
                        // Other patterns - lower to IR loop
                        // Uses a local variable to accumulate results across loop iterations.
                        let var_name =
                            SmolStr::new(format!("__star_results_{}", self.code.next_index().0));

                        // Create empty results list and store in variable
                        let empty_list_idx =
                            self.code.emit(Instruction::MakeList { elements: vec![] });
                        self.code.emit(Instruction::StoreVar {
                            name: var_name.clone(),
                            value: empty_list_idx,
                        });

                        // Loop start label
                        let loop_start = self.code.next_index();

                        // Record start position for zero-length guard
                        let start_pos_idx = self.code.emit(Instruction::ParsePosition);

                        // Compile the inner pattern
                        let result_idx = self.compile_grammar_pattern(p)?;

                        // If pattern failed (null), exit loop
                        let jump_if_null = self.code.emit(Instruction::JumpIfNull {
                            cond: result_idx,
                            target: InstrIndex(0), // placeholder, will patch
                        });

                        // Load current results, append, and store back
                        let current_results_idx =
                            self.code.emit(Instruction::LoadVar(var_name.clone()));
                        let new_results_idx = self.code.emit(Instruction::ListAppend {
                            list: current_results_idx,
                            item: result_idx,
                        });
                        self.code.emit(Instruction::StoreVar {
                            name: var_name.clone(),
                            value: new_results_idx,
                        });

                        // Record end position for zero-length guard
                        let end_pos_idx = self.code.emit(Instruction::ParsePosition);

                        // Zero-length guard
                        let cmp_idx = self.code.emit(Instruction::Eq {
                            lhs: start_pos_idx,
                            rhs: end_pos_idx,
                        });
                        let jump_if_zero_length = self.code.emit(Instruction::JumpIfTrue {
                            cond: cmp_idx,
                            target: InstrIndex(0), // placeholder, will patch
                        });

                        // Jump back to loop start
                        self.code.emit(Instruction::Jump { target: loop_start });

                        // Loop end - patch forward jumps
                        let loop_end = self.code.next_index();
                        self.code.patch_jump_target(jump_if_null, loop_end);
                        self.code.patch_jump_target(jump_if_zero_length, loop_end);

                        // Return the accumulated results
                        self.code.emit(Instruction::LoadVar(var_name))
                    }
                }
            }

            GP::Plus(p) => {
                // Plus(p) compiles differently based on pattern type:
                // - Char-based patterns: use specialized instructions that join into string
                // - Rule patterns: use MatchPlusRule
                // - Other patterns: lower to IR loop

                match p.as_ref() {
                    GP::Rule(name) => {
                        let const_idx = self.code.add_constant(name.clone());
                        self.code
                            .emit(Instruction::MatchPlusRule { rule: const_idx })
                    }
                    GP::Char(c) => self.code.emit(Instruction::MatchPlusChar { c: *c }),
                    GP::CharClass(ranges) => {
                        use crate::grammar::CharRange;
                        let range_tuples: Vec<(char, char)> = ranges
                            .iter()
                            .map(|r| match r {
                                CharRange::Char(c) => (*c, *c),
                                CharRange::Range(lo, hi) => (*lo, *hi),
                            })
                            .collect();
                        self.code.emit(Instruction::MatchPlusCharClass {
                            ranges: range_tuples,
                        })
                    }
                    GP::Literal(s) => {
                        let const_idx = self.code.add_constant(s.clone());
                        self.code.emit(Instruction::MatchPlusLiteral { const_idx })
                    }
                    _ => {
                        // Other patterns - lower to IR loop
                        // Same as Star but requires at least one match.
                        let var_name =
                            SmolStr::new(format!("__plus_results_{}", self.code.next_index().0));

                        // First match - must succeed
                        let first_result_idx = self.compile_grammar_pattern(p)?;
                        let jump_if_first_null = self.code.emit(Instruction::JumpIfNull {
                            cond: first_result_idx,
                            target: InstrIndex(0), // placeholder: will patch to fail label
                        });

                        // Create results list with first match and store in variable
                        let initial_list_idx = self.code.emit(Instruction::MakeList {
                            elements: vec![first_result_idx],
                        });
                        self.code.emit(Instruction::StoreVar {
                            name: var_name.clone(),
                            value: initial_list_idx,
                        });

                        // Loop start label
                        let loop_start = self.code.next_index();

                        // Record start position for zero-length guard
                        let start_pos_idx = self.code.emit(Instruction::ParsePosition);

                        // Compile the inner pattern again (for subsequent matches)
                        let result_idx = self.compile_grammar_pattern(p)?;

                        // If pattern failed (null), exit loop
                        let jump_if_null = self.code.emit(Instruction::JumpIfNull {
                            cond: result_idx,
                            target: InstrIndex(0), // placeholder, will patch
                        });

                        // Load current results, append, and store back
                        let current_results_idx =
                            self.code.emit(Instruction::LoadVar(var_name.clone()));
                        let new_results_idx = self.code.emit(Instruction::ListAppend {
                            list: current_results_idx,
                            item: result_idx,
                        });
                        self.code.emit(Instruction::StoreVar {
                            name: var_name.clone(),
                            value: new_results_idx,
                        });

                        // Record end position for zero-length guard
                        let end_pos_idx = self.code.emit(Instruction::ParsePosition);

                        // Zero-length guard
                        let cmp_idx = self.code.emit(Instruction::Eq {
                            lhs: start_pos_idx,
                            rhs: end_pos_idx,
                        });
                        let jump_if_zero_length = self.code.emit(Instruction::JumpIfTrue {
                            cond: cmp_idx,
                            target: InstrIndex(0), // placeholder, will patch
                        });

                        // Jump back to loop start
                        self.code.emit(Instruction::Jump { target: loop_start });

                        // Fail label - no matches (should return Null)
                        let fail_label = self.code.next_index();
                        self.code.patch_jump_target(jump_if_first_null, fail_label);
                        let _null_idx = self.code.emit(Instruction::LoadNull);
                        let jump_to_done = self.code.emit(Instruction::Jump {
                            target: InstrIndex(0),
                        }); // placeholder

                        // Loop end - patch forward jumps
                        let loop_end = self.code.next_index();
                        self.code.patch_jump_target(jump_if_null, loop_end);
                        self.code.patch_jump_target(jump_if_zero_length, loop_end);
                        let final_result_idx = self.code.emit(Instruction::LoadVar(var_name));

                        // Done label - patch jump from fail path
                        let done_label = self.code.next_index();
                        self.code.patch_jump_target(jump_to_done, done_label);

                        // Copy the appropriate result
                        self.code.emit(Instruction::Copy {
                            source: final_result_idx,
                        })
                    }
                }
            }

            GP::Optional(p) => {
                let inner = self.compile_grammar_pattern(p)?;
                self.code
                    .emit(Instruction::MatchOptional { pattern: inner })
            }

            GP::Lookahead(p) => {
                let inner = self.compile_grammar_pattern(p)?;
                self.code
                    .emit(Instruction::MatchLookahead { pattern: inner })
            }

            GP::Not(p) => {
                let inner = self.compile_grammar_pattern(p)?;
                self.code.emit(Instruction::MatchNot { pattern: inner })
            }

            GP::Bind(p, name, _) => {
                let pattern_idx = self.compile_grammar_pattern(p)?;
                let const_idx = self.code.add_constant(name.clone());
                self.code.emit(Instruction::MatchBind {
                    pattern: pattern_idx,
                    name: const_idx,
                })
            }

            GP::Action(p, action) => {
                eprintln!(
                    "DEBUG compile_grammar_pattern: compiling Action pattern={:?}",
                    p
                );
                let pattern_idx = self.compile_grammar_pattern(p)?;
                eprintln!(
                    "DEBUG compile_grammar_pattern: Action pattern_idx={:?}",
                    pattern_idx
                );
                let action_idx = self.compile_expr(action)?;
                eprintln!(
                    "DEBUG compile_grammar_pattern: Action action_idx={:?}",
                    action_idx
                );
                let match_action_idx = self.code.emit(Instruction::MatchAction {
                    pattern: pattern_idx,
                    action: action_idx,
                });
                // Return the match_action_idx, not the pattern_idx
                // This is important for Choice compilation - we want to Return the MatchAction result
                match_action_idx
            }

            GP::Guard(p, guard) => {
                let pattern_idx = self.compile_grammar_pattern(p)?;
                let guard_idx = self.compile_expr(guard)?;
                self.code.emit(Instruction::MatchGuard {
                    pattern: pattern_idx,
                    predicate: guard_idx,
                })
            }

            GP::Rule(name) => {
                let const_idx = self.code.add_constant(name.clone());
                self.code.emit(Instruction::ApplyRule {
                    rule_idx: const_idx,
                })
            }

            GP::Byte(b) => self.code.emit(Instruction::MatchByte { byte: *b }),

            GP::End => self.code.emit(Instruction::MatchEnd),

            // Character classes
            GP::CharClass(ranges) => {
                use crate::grammar::CharRange;
                let range_tuples: Vec<(char, char)> = ranges
                    .iter()
                    .map(|r| match r {
                        CharRange::Char(c) => (*c, *c),
                        CharRange::Range(lo, hi) => (*lo, *hi),
                    })
                    .collect();
                self.code.emit(Instruction::MatchCharClass {
                    ranges: range_tuples,
                })
            }

            GP::NegCharClass(ranges) => {
                use crate::grammar::CharRange;
                let range_tuples: Vec<(char, char)> = ranges
                    .iter()
                    .map(|r| match r {
                        CharRange::Char(c) => (*c, *c),
                        CharRange::Range(lo, hi) => (*lo, *hi),
                    })
                    .collect();
                self.code.emit(Instruction::MatchNegCharClass {
                    ranges: range_tuples,
                })
            }

            GP::Super(_) => {
                // For now, compile as MatchAny - TODO: implement proper grammar inheritance
                self.code.emit(Instruction::MatchAny)
            }

            GP::Predicate(expr) => {
                // Semantic predicate: evaluate expression, succeed if truthy
                // Lowered to IR:
                //   expr_result = compile(expr)
                //   if !truthy(expr_result) -> return Null (fail)
                //   else -> return true (success, don't consume input)
                //
                // Compile the predicate expression
                let expr_idx = self.compile_expr(expr)?;

                // Check if truthy - JumpIfFalse to failure case
                let jump_to_fail = self.code.emit(Instruction::JumpIfFalse {
                    cond: expr_idx,
                    target: InstrIndex(0), // placeholder
                });

                // Success case: return true (predicates succeed without producing meaningful value)
                let _success_idx = self.code.emit(Instruction::LoadBool(true));
                let jump_to_end = self.code.emit(Instruction::Jump {
                    target: InstrIndex(0), // placeholder
                });

                // Failure case: return Null to signal pattern failure
                let fail_idx = self.code.next_index();
                self.code.patch_jump_target(jump_to_fail, fail_idx);
                let _fail_null = self.code.emit(Instruction::LoadNull);

                let end_idx = self.code.next_index();
                self.code.patch_jump_target(jump_to_end, end_idx);

                end_idx
            }

            // Binary patterns
            GP::ByteRange(_, _)
            | GP::Bytes(_)
            | GP::UInt8
            | GP::UInt16BE
            | GP::UInt16LE
            | GP::UInt32BE
            | GP::UInt32LE
            | GP::Int8
            | GP::Int16BE
            | GP::Int16LE
            | GP::Int32BE
            | GP::Int32LE => {
                // For now, compile as MatchAny
                self.code.emit(Instruction::MatchAny)
            }

            // Object/value patterns - For simple patterns use MatchListWithBindings,
            // for complex patterns use OMeta-style tree descent
            GP::ListMatch(patterns, rest) => {
                // Check if any pattern is complex (not just Bind or Any)
                let has_complex_patterns = patterns
                    .iter()
                    .any(|p| !matches!(p, GP::Bind(_, _, _) | GP::Any))
                    || rest.as_ref().map_or(false, |r| {
                        !matches!(r.as_ref(), GP::Bind(_, _, _) | GP::Any)
                    });

                if has_complex_patterns {
                    // OMeta-style list matching with tree descent for complex patterns:
                    // For patterns like [%{x: a}, %{x: b}] or ['add' expr:x expr:y]
                    // 1. Get current input value (it IS the list, not from a stream)
                    // 2. Check if it's a list
                    // 3. Push the list as new input stream (ParsePush)
                    // 4. Match each element pattern in sequence
                    // 5. Check MatchEnd (unless rest pattern)
                    // 6. Pop back (ParsePop)

                    // 1. Get current input value using GetInput instruction
                    // We need to add this instruction - for now use MatchAny with special handling
                    // Actually, let's use the existing MatchList instruction for the IsList check
                    // and then push/descend

                    // For now, emit MatchList which handles the whole thing
                    // This will be refactored when we implement full OMeta tree descent
                    let compiled_patterns: Vec<InstrIndex> = patterns
                        .iter()
                        .map(|p| self.compile_grammar_pattern(p))
                        .collect::<Result<Vec<_>>>()?;

                    let compiled_rest = if let Some(rest_pat) = rest {
                        Some(self.compile_grammar_pattern(rest_pat)?)
                    } else {
                        None
                    };

                    self.code.emit(Instruction::MatchList {
                        patterns: compiled_patterns,
                        rest: compiled_rest,
                    })
                } else {
                    // Simple case - just extract variable names
                    let pattern_bindings: Vec<Option<SmolStr>> = patterns
                        .iter()
                        .map(|p| match p {
                            GP::Bind(_, name, _) => Some(name.clone()),
                            GP::Any => None,
                            _ => None,
                        })
                        .collect();

                    // Convert to constant indices
                    let pattern_consts: Vec<Option<ConstIndex>> = pattern_bindings
                        .iter()
                        .map(|opt| {
                            opt.as_ref()
                                .map(|name| self.code.add_constant(name.clone()))
                        })
                        .collect();

                    // Extract rest binding if present
                    let rest_binding = match rest {
                        Some(rest_pat) => match rest_pat.as_ref() {
                            GP::Bind(_, name, _) => Some(self.code.add_constant(name.clone())),
                            GP::Any => None,
                            _ => None,
                        },
                        None => None,
                    };

                    self.code.emit(Instruction::MatchListWithBindings {
                        patterns: pattern_consts,
                        rest: rest_binding,
                    })
                }
            }

            GP::MapMatch(entries) => {
                // Compile each key-value pattern pair
                let compiled_entries: Result<Vec<(MapKeyPattern, MapValuePattern)>> = entries
                    .iter()
                    .map(|(key, pattern)| {
                        // Compile key pattern (currently only specific keys, no wildcards)
                        let key_pattern =
                            MapKeyPattern::Specific(self.code.add_constant(key.clone()));

                        // Compile value pattern
                        let value_pattern = match pattern {
                            GP::Any => MapValuePattern::Wildcard,
                            GP::Bind(_, name, _) => {
                                MapValuePattern::Bind(self.code.add_constant(name.clone()))
                            }
                            GP::Literal(s) => {
                                // String literal - match as literal value
                                MapValuePattern::MatchLiteral(self.code.add_constant(s.clone()))
                            }
                            GP::MatchValue(value) => {
                                // Match a literal value (int, bool, etc.)
                                MapValuePattern::MatchLiteral(self.code.add_constant(value.clone()))
                            }
                            _ => {
                                // For nested patterns or complex patterns, compile as pattern instruction
                                let pattern_idx = self.compile_grammar_pattern(pattern)?;
                                MapValuePattern::Pattern(pattern_idx)
                            }
                        };

                        Ok((key_pattern, value_pattern))
                    })
                    .collect();

                let compiled_entries = compiled_entries?;
                self.code.emit(Instruction::MatchMap {
                    entries: compiled_entries,
                })
            }

            GP::MatchValue(_)
            | GP::MatchType(_)
            | GP::SymbolMatch(_)
            | GP::SymbolLiteral(_)
            | GP::Apply(_) => {
                // For now, compile as MatchAny
                // TODO: Implement proper compilation for these patterns
                self.code.emit(Instruction::MatchAny)
            }

            GP::TagMatch(tag, child_patterns) => {
                // Check if all patterns are simple (Bind or Any)
                let all_simple = child_patterns.iter().all(|p| {
                    matches!(p, GP::Bind(inner, _, _) if matches!(inner.as_ref(), GP::Any))
                        || matches!(p, GP::Any)
                });

                if all_simple {
                    // Simple case: all patterns are direct bindings or wildcards
                    // Use MatchTaggedWithBindings for efficiency
                    let tag_idx = self.code.add_constant(tag.clone());
                    let bindings: Vec<Option<ConstIndex>> = child_patterns
                        .iter()
                        .map(|p| match p {
                            GP::Bind(_, name, _) => Some(self.code.add_constant(name.clone())),
                            GP::Any => None,
                            _ => unreachable!(),
                        })
                        .collect();
                    self.code
                        .emit(Instruction::MatchTaggedWithBindings { tag_idx, bindings })
                } else {
                    // Complex case: nested patterns need full matching
                    // Use MatchTagged with compiled child patterns
                    let tag_idx = self.code.add_constant(tag.clone());
                    let compiled_patterns: Vec<InstrIndex> = child_patterns
                        .iter()
                        .map(|p| self.compile_grammar_pattern(p))
                        .collect::<Result<Vec<_>>>()?;
                    self.code.emit(Instruction::MatchTagged {
                        tag_idx,
                        patterns: compiled_patterns,
                    })
                }
            }
        })
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
