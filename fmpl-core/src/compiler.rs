//! Compiler: AST to Indexed RPN bytecode.

use crate::ast::*;
use crate::error::{Error, Result};
use smol_str::SmolStr;

/// Bytecode instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Literals
    LoadNull,
    LoadBool(bool),
    LoadInt(i64),
    LoadFloat(f64),
    LoadString(SmolStr),
    LoadSymbol(SmolStr),

    // Variable access
    LoadVar(SmolStr),
    StoreVar(SmolStr),

    // Special references
    LoadSelf,
    LoadParent,
    LoadCaller,
    LoadUser,
    LoadArgs,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,

    // Comparison
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    // Logical
    Not,
    And,
    Or,

    // Control flow
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),

    // Functions and calls
    Call(usize),                // arg count
    TailCall(usize),            // arg count
    MethodCall(SmolStr, usize), // method name, arg count
    Return,

    // Objects
    GetProp(SmolStr),
    SetProp(SmolStr),
    Spawn(usize), // arg count
    GetFacet(SmolStr),

    // Sync/Async
    SyncCall,
    AsyncCall,

    // Data structures
    MakeList(usize),
    MakeMap(usize),
    Index,
    Slice,

    // Binding
    PushScope,
    PopScope,
    Bind(SmolStr),

    // Lambda
    MakeLambda(Vec<SmolStr>, usize), // param names, code index

    // Stack manipulation
    Pop,
    Dup,

    // Pipe operator (function application)
    Pipe,

    // Pattern matching
    MatchPattern(usize), // jump target if no match

    // Object definition (creates object in DB)
    DefineObject(SmolStr),
    DefineMethod(SmolStr, usize), // method name, param count
    DefineProp(SmolStr),
    DefineFacet(SmolStr, usize, bool), // facet name, member count, terminal

    // Grammar application
    GrammarApply(SmolStr, SmolStr), // grammar name, rule name
}

/// Compiled bytecode.
#[derive(Debug, Clone)]
pub struct CompiledCode {
    pub instructions: Vec<Instruction>,
    /// Nested code blocks (for lambdas, methods).
    pub nested: Vec<CompiledCode>,
}

impl CompiledCode {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            nested: Vec::new(),
        }
    }

    fn emit(&mut self, instr: Instruction) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(instr);
        idx
    }

    fn current_pos(&self) -> usize {
        self.instructions.len()
    }

    fn patch_jump(&mut self, idx: usize, target: usize) {
        match &mut self.instructions[idx] {
            Instruction::Jump(t)
            | Instruction::JumpIfFalse(t)
            | Instruction::JumpIfTrue(t)
            | Instruction::MatchPattern(t) => {
                *t = target;
            }
            _ => panic!("not a jump instruction"),
        }
    }
}

impl Default for CompiledCode {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiler.
pub struct Compiler {
    code: CompiledCode,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            code: CompiledCode::new(),
        }
    }

    /// Compile an expression.
    pub fn compile(mut self, expr: &Expr) -> Result<CompiledCode> {
        self.compile_expr(expr)?;
        Ok(self.code)
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Int(n) => {
                self.code.emit(Instruction::LoadInt(*n));
            }
            Expr::Float(f) => {
                self.code.emit(Instruction::LoadFloat(*f));
            }
            Expr::String(s) => {
                self.code.emit(Instruction::LoadString(s.clone()));
            }
            Expr::Symbol(s) => {
                self.code.emit(Instruction::LoadSymbol(s.clone()));
            }
            Expr::Bool(b) => {
                self.code.emit(Instruction::LoadBool(*b));
            }
            Expr::Null => {
                self.code.emit(Instruction::LoadNull);
            }
            Expr::Ident(name) => {
                self.code.emit(Instruction::LoadVar(name.clone()));
            }
            Expr::Qualified(qn) => {
                // For now, treat as simple name lookup
                // TODO: proper namespace resolution
                self.code
                    .emit(Instruction::LoadVar(SmolStr::new(qn.to_string())));
            }
            Expr::ObjTag(name) => {
                // Object constructor reference
                self.code
                    .emit(Instruction::LoadVar(SmolStr::new(format!("^{}", name))));
            }
            Expr::FnTag(name) => {
                // Function reference
                self.code
                    .emit(Instruction::LoadVar(SmolStr::new(format!("@{}", name))));
            }
            Expr::Self_ => {
                self.code.emit(Instruction::LoadSelf);
            }
            Expr::Parent => {
                self.code.emit(Instruction::LoadParent);
            }
            Expr::Caller => {
                self.code.emit(Instruction::LoadCaller);
            }
            Expr::User => {
                self.code.emit(Instruction::LoadUser);
            }
            Expr::Args => {
                self.code.emit(Instruction::LoadArgs);
            }
            Expr::Placeholder => {
                // Placeholder for partial application - handled at call site
                self.code.emit(Instruction::LoadNull);
            }
            Expr::List(items) => {
                for item in items {
                    self.compile_expr(item)?;
                }
                self.code.emit(Instruction::MakeList(items.len()));
            }
            Expr::ListCons(head, tail) => {
                self.compile_expr(head)?;
                self.compile_expr(tail)?;
                // TODO: proper list cons instruction
                self.code.emit(Instruction::MakeList(2));
            }
            Expr::Map(entries) => {
                for entry in entries {
                    match entry {
                        MapEntry::Symbol(key, val) => {
                            self.code.emit(Instruction::LoadSymbol(key.clone()));
                            self.compile_expr(val)?;
                        }
                        MapEntry::Computed(key, val) => {
                            self.compile_expr(key)?;
                            self.compile_expr(val)?;
                        }
                    }
                }
                self.code.emit(Instruction::MakeMap(entries.len()));
            }
            Expr::Binary(left, op, right) => {
                // Special handling for short-circuit operators
                match op {
                    BinOp::And => {
                        self.compile_expr(left)?;
                        let jump = self.code.emit(Instruction::JumpIfFalse(0));
                        self.code.emit(Instruction::Pop);
                        self.compile_expr(right)?;
                        let end = self.code.current_pos();
                        self.code.patch_jump(jump, end);
                    }
                    BinOp::Or => {
                        self.compile_expr(left)?;
                        let jump = self.code.emit(Instruction::JumpIfTrue(0));
                        self.code.emit(Instruction::Pop);
                        self.compile_expr(right)?;
                        let end = self.code.current_pos();
                        self.code.patch_jump(jump, end);
                    }
                    BinOp::Pipe => {
                        // x |> f compiles to f(x)
                        self.compile_expr(left)?;
                        self.compile_expr(right)?;
                        self.code.emit(Instruction::Pipe);
                    }
                    _ => {
                        self.compile_expr(left)?;
                        self.compile_expr(right)?;
                        let instr = match op {
                            BinOp::Add => Instruction::Add,
                            BinOp::Sub => Instruction::Sub,
                            BinOp::Mul => Instruction::Mul,
                            BinOp::Div => Instruction::Div,
                            BinOp::Mod => Instruction::Mod,
                            BinOp::Eq => Instruction::Eq,
                            BinOp::NotEq => Instruction::NotEq,
                            BinOp::Lt => Instruction::Lt,
                            BinOp::Gt => Instruction::Gt,
                            BinOp::LtEq => Instruction::LtEq,
                            BinOp::GtEq => Instruction::GtEq,
                            BinOp::And | BinOp::Or | BinOp::Pipe => unreachable!(),
                        };
                        self.code.emit(instr);
                    }
                }
            }
            Expr::Unary(op, expr) => {
                self.compile_expr(expr)?;
                match op {
                    UnaryOp::Neg => self.code.emit(Instruction::Neg),
                    UnaryOp::Not => self.code.emit(Instruction::Not),
                };
            }
            Expr::Index(expr, idx) => {
                self.compile_expr(expr)?;
                self.compile_expr(idx)?;
                self.code.emit(Instruction::Index);
            }
            Expr::Slice(expr, start, end) => {
                self.compile_expr(expr)?;
                self.compile_expr(start)?;
                self.compile_expr(end)?;
                self.code.emit(Instruction::Slice);
            }
            Expr::Call(func, args) => {
                // Check for partial application (any placeholder args)
                let has_placeholder = args.iter().any(|a| matches!(a, Arg::Placeholder));
                if has_placeholder {
                    // TODO: compile as partial application
                    return Err(Error::Compiler(
                        "partial application not yet implemented".to_string(),
                    ));
                }

                for arg in args {
                    match arg {
                        Arg::Expr(e) => self.compile_expr(e)?,
                        Arg::Placeholder => unreachable!(),
                    }
                }
                self.compile_expr(func)?;
                self.code.emit(Instruction::Call(args.len()));
            }
            Expr::PropAccess(expr, name) => {
                self.compile_expr(expr)?;
                self.code.emit(Instruction::GetProp(name.clone()));
            }
            Expr::MethodCall(expr, name, args) => {
                self.compile_expr(expr)?;
                for arg in args {
                    match arg {
                        Arg::Expr(e) => self.compile_expr(e)?,
                        Arg::Placeholder => {
                            return Err(Error::Compiler(
                                "partial application not yet implemented".to_string(),
                            ));
                        }
                    }
                }
                self.code
                    .emit(Instruction::MethodCall(name.clone(), args.len()));
            }
            Expr::If(cond, then_branch, else_branch) => {
                self.compile_expr(cond)?;
                let else_jump = self.code.emit(Instruction::JumpIfFalse(0));

                self.compile_expr(then_branch)?;

                if let Some(else_expr) = else_branch {
                    let end_jump = self.code.emit(Instruction::Jump(0));
                    let else_start = self.code.current_pos();
                    self.code.patch_jump(else_jump, else_start);

                    self.compile_expr(else_expr)?;
                    let end = self.code.current_pos();
                    self.code.patch_jump(end_jump, end);
                } else {
                    let end = self.code.current_pos();
                    self.code.patch_jump(else_jump, end);
                }
            }
            Expr::While(cond, body) => {
                let loop_start = self.code.current_pos();
                self.compile_expr(cond)?;
                let exit_jump = self.code.emit(Instruction::JumpIfFalse(0));

                self.compile_expr(body)?;
                self.code.emit(Instruction::Pop); // discard body result
                self.code.emit(Instruction::Jump(loop_start));

                let end = self.code.current_pos();
                self.code.patch_jump(exit_jump, end);
                self.code.emit(Instruction::LoadNull); // while returns null
            }
            Expr::DoWhile(body, cond) => {
                let loop_start = self.code.current_pos();
                self.compile_expr(body)?;
                self.code.emit(Instruction::Pop);

                self.compile_expr(cond)?;
                self.code.emit(Instruction::JumpIfTrue(loop_start));
                self.code.emit(Instruction::LoadNull);
            }
            Expr::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e)?;
                } else {
                    self.code.emit(Instruction::LoadNull);
                }
                self.code.emit(Instruction::Return);
            }
            Expr::Lambda(params, body) => {
                // Compile lambda body to nested code
                let mut lambda_compiler = Compiler::new();
                lambda_compiler.compile_expr(body)?;
                lambda_compiler.code.emit(Instruction::Return);

                let nested_idx = self.code.nested.len();
                self.code.nested.push(lambda_compiler.code);

                self.code
                    .emit(Instruction::MakeLambda(params.clone(), nested_idx));
            }
            Expr::ShortLambda(param, body) => {
                // \x expr is equivalent to lambda (x) expr
                let mut lambda_compiler = Compiler::new();
                lambda_compiler.compile_expr(body)?;
                lambda_compiler.code.emit(Instruction::Return);

                let nested_idx = self.code.nested.len();
                self.code.nested.push(lambda_compiler.code);

                self.code
                    .emit(Instruction::MakeLambda(vec![param.clone()], nested_idx));
            }
            Expr::Let(bindings, body) => {
                self.code.emit(Instruction::PushScope);

                for binding in bindings {
                    match binding {
                        LetBinding::Simple(name, init) => {
                            if let Some(expr) = init {
                                self.compile_expr(expr)?;
                            } else {
                                self.code.emit(Instruction::LoadNull);
                            }
                            self.code.emit(Instruction::Bind(name.clone()));
                        }
                        LetBinding::Destructure(_pattern, _expr) => {
                            // TODO: pattern destructuring
                            return Err(Error::Compiler(
                                "pattern destructuring not yet implemented".to_string(),
                            ));
                        }
                    }
                }

                self.compile_expr(body)?;
                self.code.emit(Instruction::PopScope);
            }
            Expr::Sequence(exprs) => {
                for (i, expr) in exprs.iter().enumerate() {
                    self.compile_expr(expr)?;
                    if i < exprs.len() - 1 {
                        self.code.emit(Instruction::Pop);
                    }
                }
                if exprs.is_empty() {
                    self.code.emit(Instruction::LoadNull);
                }
            }
            Expr::ObjectDef(def) => {
                self.compile_object_def(def)?;
            }
            Expr::Match(scrutinee, cases) => {
                self.compile_expr(scrutinee)?;

                let mut end_jumps = Vec::new();

                for (i, case) in cases.iter().enumerate() {
                    if i > 0 {
                        // Previous case didn't match, duplicate scrutinee
                        self.code.emit(Instruction::Dup);
                    }

                    // TODO: proper pattern compilation
                    // For now, just support simple variable patterns
                    match &case.pattern {
                        Pattern::Var(name) => {
                            self.code.emit(Instruction::PushScope);
                            self.code.emit(Instruction::Dup);
                            self.code.emit(Instruction::Bind(name.clone()));
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
                        self.compile_expr(guard)?;
                        let skip = self.code.emit(Instruction::JumpIfFalse(0));
                        self.compile_expr(&case.body)?;
                        self.code.emit(Instruction::PopScope);
                        end_jumps.push(self.code.emit(Instruction::Jump(0)));
                        self.code.patch_jump(skip, self.code.current_pos());
                        self.code.emit(Instruction::PopScope);
                    } else {
                        self.compile_expr(&case.body)?;
                        self.code.emit(Instruction::PopScope);
                        end_jumps.push(self.code.emit(Instruction::Jump(0)));
                    }
                }

                // If no pattern matched, result is null
                self.code.emit(Instruction::Pop);
                self.code.emit(Instruction::LoadNull);

                let end = self.code.current_pos();
                for jump in end_jumps {
                    self.code.patch_jump(jump, end);
                }
            }
            Expr::Spawn(constructor, args) => {
                self.compile_expr(constructor)?;
                for arg in args {
                    match arg {
                        Arg::Expr(e) => self.compile_expr(e)?,
                        Arg::Placeholder => {
                            return Err(Error::Compiler(
                                "partial application not yet implemented".to_string(),
                            ));
                        }
                    }
                }
                self.code.emit(Instruction::Spawn(args.len()));
            }
            Expr::SyncCall(expr) => {
                self.compile_expr(expr)?;
                self.code.emit(Instruction::SyncCall);
            }
            Expr::AsyncCall(expr) => {
                self.compile_expr(expr)?;
                self.code.emit(Instruction::AsyncCall);
            }
            Expr::FacetAccess(expr, facet) => {
                self.compile_expr(expr)?;
                self.code.emit(Instruction::GetFacet(facet.clone()));
            }
            Expr::GrammarApply {
                input,
                grammar,
                rule,
            } => {
                self.compile_expr(input)?;
                self.code.emit(Instruction::GrammarApply(
                    SmolStr::new(grammar.to_string()),
                    rule.clone(),
                ));
            }
        }
        Ok(())
    }

    fn compile_object_def(&mut self, def: &ObjectDef) -> Result<()> {
        // Create the object
        self.code.emit(Instruction::DefineObject(SmolStr::new(
            def.name.to_string(),
        )));

        // Compile and set properties/methods
        for binding in &def.bindings {
            if binding.params.is_empty() {
                // Property
                self.compile_expr(&binding.value)?;
                self.code
                    .emit(Instruction::DefineProp(binding.name.clone()));
            } else {
                // Method
                let mut method_compiler = Compiler::new();
                method_compiler.compile_expr(&binding.value)?;
                method_compiler.code.emit(Instruction::Return);

                // TODO: pass nested_idx to DefineMethod for proper method lookup
                self.code.nested.push(method_compiler.code);

                self.code.emit(Instruction::DefineMethod(
                    binding.name.clone(),
                    binding.params.len(),
                ));
            }
        }

        // Define facets
        for facet in &def.facets {
            for member in &facet.members {
                self.code.emit(Instruction::LoadSymbol(member.clone()));
            }
            self.code.emit(Instruction::DefineFacet(
                facet.name.clone(),
                facet.members.len(),
                facet.terminal,
            ));
        }

        Ok(())
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
                Instruction::Add
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
                .any(|i| matches!(i, Instruction::JumpIfFalse(_)))
        );
    }

    #[test]
    fn test_compile_lambda() {
        let code = compile("lambda (x) x + 1").unwrap();
        // Check that MakeLambda has the param name "x" and points to nested code 0
        assert!(matches!(
            &code.instructions[0],
            Instruction::MakeLambda(params, 0) if params.len() == 1 && params[0] == "x"
        ));
        assert_eq!(code.nested.len(), 1);
    }
}
