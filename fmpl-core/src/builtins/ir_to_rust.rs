// Suppress style lints for nested if-let inside match arms — refactoring would
// reduce clarity in this large dispatch table.
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::manual_checked_ops)]

//! IR to Rust transpiler.
//!
//! Generates Rust source code from IR tagged values.
//!
//! This module handles two types of IR:
//! 1. Computation IR: LoadInt, Add, Let, Lambda, etc. - for general computation
//! 2. Parsing IR: ParseChar, ParseLiteral, ParseChoice, etc. - for parser generation
//!
//! The parsing IR is used to convert Grammar structures to Rust parsing code.

use crate::error::{Error, Result};
use crate::value::Value;
use smol_str::SmolStr;
use std::collections::HashSet;

/// Transpile IR to Rust source code.
pub fn transpile(ir: &Value) -> Result<String> {
    // Special case: ParseGrammar generates a complete self-contained file
    // Support both Value::Tagged("ParseGrammar", ...) and [:ParseGrammar, ...]
    let is_grammar = match ir {
        Value::Tagged(tag, _) => tag.as_str() == "ParseGrammar",
        Value::List(items) if !items.is_empty() => {
            matches!(&items[0], Value::Symbol(tag) if tag.as_str() == "ParseGrammar")
        }
        _ => false,
    };

    if is_grammar {
        let mut transpiler = IrToRust::new_grammar_mode();
        return transpiler.transpile_ir(ir);
    }

    let mut transpiler = IrToRust::new();
    let expr = transpiler.transpile_ir(ir)?;

    // Generate complete Rust program
    let mut output = String::new();
    output.push_str("// Generated from FMPL IR\n");
    output.push_str("use std::rc::Rc;\n\n");

    // Include runtime support
    output.push_str(RUNTIME_PRELUDE);
    output.push('\n');

    // Main function
    output.push_str("fn main() {\n");
    output.push_str("    let result = ");
    output.push_str(&expr);
    output.push_str(";\n");
    output.push_str("    println!(\"{:?}\", result);\n");
    output.push_str("}\n");

    Ok(output)
}

/// Transpile IR to a Rust expression (without wrapper).
pub fn transpile_expr(ir: &Value) -> Result<String> {
    let mut transpiler = IrToRust::new();
    transpiler.transpile_ir(ir)
}

/// Helper functions for grammar semantic actions.
/// These implement the standard functions used in FMPL grammar actions.
const GRAMMAR_HELPERS: &str = r#"
// Helper functions for semantic actions

/// Prepend an item to a list, returning a new list
fn prepend(item: Value, list: Value) -> Value {
    match list {
        Value::List(items) => {
            let mut new_items = vec![item];
            new_items.extend(items.as_ref().clone());
            Value::List(Arc::new(new_items))
        }
        _ => {
            Value::List(Arc::new(vec![item, list]))
        }
    }
}

/// Join a list of characters/strings into a single string
fn join(list: Value) -> Value {
    match list {
        Value::List(items) => {
            let mut result = String::new();
            for item in items.as_ref() {
                match item {
                    Value::String(s) => result.push_str(s.as_str()),
                    _ => {}
                }
            }
            Value::String(SmolStr::new(&result))
        }
        Value::String(s) => Value::String(s),
        _ => Value::String(SmolStr::new("")),
    }
}

/// Convert a string to a symbol
fn symbol(s: Value) -> Value {
    match s {
        Value::String(s) => Value::Symbol(s),
        Value::Symbol(s) => Value::Symbol(s),
        _ => Value::Symbol(SmolStr::new("")),
    }
}

/// Parse a string as a float (used for float.parse in grammar actions)
fn float_parse(s: Value) -> Value {
    match s {
        Value::String(s) => {
            match s.as_str().parse::<f64>() {
                Ok(f) => Value::Float(f),
                Err(_) => Value::Null,
            }
        }
        _ => Value::Null,
    }
}

/// Fold a list with a native Rust closure
fn fold<F: Fn(Value, Value) -> Value>(f: F, init: Value, list: Value) -> Value {
    match list {
        Value::List(items) => {
            let mut acc = init;
            for item in items.as_ref() {
                acc = f(acc, item.clone());
            }
            acc
        }
        _ => init,
    }
}

/// Fold binary operations: fold_binary(first, [(op1, e1), (op2, e2), ...])
/// Produces :Binary(op, left, right) to match FMPL prelude's fold_binary
fn fold_binary(first: Value, rest: Value) -> Value {
    match rest {
        Value::List(items) if items.is_empty() => first,
        Value::List(items) => {
            let mut result = first;
            for pair in items.as_ref() {
                if let Value::List(pair_items) = pair {
                    if pair_items.len() >= 2 {
                        let op = pair_items[0].clone();
                        let rhs = pair_items[1].clone();
                        // FMPL order: :Binary(op, left, right)
                        result = Value::Tagged(
                            SmolStr::new("Binary"),
                            Arc::new(vec![op, result, rhs]),
                        );
                    }
                }
            }
            result
        }
        _ => first,
    }
}

/// Get the length of a list
fn length(list: Value) -> Value {
    match list {
        Value::List(items) => Value::Int(items.len() as i64),
        Value::String(s) => Value::Int(s.len() as i64),
        _ => Value::Int(0),
    }
}

/// Fold pipe and @ operations: fold_pipe_at(first, ops)
/// ops is a list of [op_type, ...args] pairs
fn fold_pipe_at(first: Value, ops: Value) -> Value {
    match ops {
        Value::List(items) if items.is_empty() => first,
        Value::List(items) => {
            let mut result = first;
            for op in items.as_ref() {
                if let Value::List(op_items) = op {
                    if !op_items.is_empty() {
                        match &op_items[0] {
                            Value::Symbol(s) if s.as_str() == "pipe" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("Binary"),
                                        Arc::new(vec![Value::Symbol(SmolStr::new("|>")), result, op_items[1].clone()]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "at_inline" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("AtInlineBlock"),
                                        Arc::new(vec![result, Value::Tagged(
                                            SmolStr::new("InlinePatternBlock"),
                                            Arc::new(vec![op_items[1].clone()]),
                                        )]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "at_grammar" => {
                                if op_items.len() >= 3 {
                                    result = Value::Tagged(
                                        SmolStr::new("AtGrammarApply"),
                                        Arc::new(vec![result, op_items[1].clone(), op_items[2].clone()]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "extend" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("GrammarExtend"),
                                        Arc::new(vec![result, op_items[1].clone()]),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            result
        }
        _ => first,
    }
}

/// Fold postfix operations: fold_postfix(base, [[:index, idx], [:call, args], ...])
fn fold_postfix(base: Value, ops: Value) -> Value {
    match ops {
        Value::List(items) if items.is_empty() => base,
        Value::List(items) => {
            let mut result = base;
            for op in items.as_ref() {
                if let Value::List(op_items) = op {
                    if !op_items.is_empty() {
                        match &op_items[0] {
                            Value::Symbol(s) if s.as_str() == "index" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("Index"),
                                        Arc::new(vec![result, op_items[1].clone()]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "call" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("Call"),
                                        Arc::new(vec![result, op_items[1].clone()]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "method" => {
                                if op_items.len() >= 3 {
                                    result = Value::Tagged(
                                        SmolStr::new("MethodCall"),
                                        Arc::new(vec![result, op_items[1].clone(), op_items[2].clone()]),
                                    );
                                }
                            }
                            Value::Symbol(s) if s.as_str() == "prop" => {
                                if op_items.len() >= 2 {
                                    result = Value::Tagged(
                                        SmolStr::new("PropAccess"),
                                        Arc::new(vec![result, op_items[1].clone()]),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            result
        }
        _ => base,
    }
}
"#;

const RUNTIME_PRELUDE: &str = r#"
#[derive(Clone)]
enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Lambda(Rc<dyn Fn(Vec<Value>) -> Value>),
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "Null"),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Int(n) => write!(f, "Int({})", n),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::String(s) => write!(f, "String({:?})", s),
            Value::List(items) => write!(f, "List({:?})", items),
            Value::Map(pairs) => write!(f, "Map({:?})", pairs),
            Value::Lambda(_) => write!(f, "Lambda(<fn>)"),
        }
    }
}

impl Value {
    fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            _ => true,
        }
    }

    fn add(&self, other: &Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
            (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
            _ => panic!("Cannot add {:?} and {:?}", self, other),
        }
    }

    fn sub(&self, other: &Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            _ => panic!("Cannot subtract {:?} and {:?}", self, other),
        }
    }

    fn mul(&self, other: &Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a * b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a * b),
            _ => panic!("Cannot multiply {:?} and {:?}", self, other),
        }
    }

    fn div(&self, other: &Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a / b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a / b),
            _ => panic!("Cannot divide {:?} and {:?}", self, other),
        }
    }

    fn modulo(&self, other: &Value) -> Value {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a % b),
            _ => panic!("Cannot modulo {:?} and {:?}", self, other),
        }
    }

    fn eq(&self, other: &Value) -> Value {
        Value::Bool(match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            _ => false,
        })
    }

    fn lt(&self, other: &Value) -> Value {
        Value::Bool(match (self, other) {
            (Value::Int(a), Value::Int(b)) => a < b,
            (Value::Float(a), Value::Float(b)) => a < b,
            _ => false,
        })
    }

    fn gt(&self, other: &Value) -> Value {
        Value::Bool(match (self, other) {
            (Value::Int(a), Value::Int(b)) => a > b,
            (Value::Float(a), Value::Float(b)) => a > b,
            _ => false,
        })
    }

    fn lteq(&self, other: &Value) -> Value {
        Value::Bool(match (self, other) {
            (Value::Int(a), Value::Int(b)) => a <= b,
            (Value::Float(a), Value::Float(b)) => a <= b,
            _ => false,
        })
    }

    fn gteq(&self, other: &Value) -> Value {
        Value::Bool(match (self, other) {
            (Value::Int(a), Value::Int(b)) => a >= b,
            (Value::Float(a), Value::Float(b)) => a >= b,
            _ => false,
        })
    }

    fn neg(&self) -> Value {
        match self {
            Value::Int(n) => Value::Int(-n),
            Value::Float(n) => Value::Float(-n),
            _ => panic!("Cannot negate {:?}", self),
        }
    }

    fn not(&self) -> Value {
        Value::Bool(!self.is_truthy())
    }

    fn index(&self, key: &Value) -> Value {
        match (self, key) {
            (Value::List(list), Value::Int(i)) => list[*i as usize].clone(),
            (Value::Map(pairs), k) => {
                for (mk, mv) in pairs {
                    if matches!(mk.eq(k), Value::Bool(true)) {
                        return mv.clone();
                    }
                }
                Value::Null
            }
            _ => panic!("Cannot index {:?} with {:?}", self, key),
        }
    }

    fn call(&self, args: Vec<Value>) -> Value {
        match self {
            Value::Lambda(f) => f(args),
            _ => panic!("Cannot call {:?}", self),
        }
    }
}
"#;

struct IrToRust {
    var_counter: usize,
    // When true, generating code for fmpl-core where Value methods return Result<Value>
    // When false, generating standalone code where RUNTIME_PRELUDE Value methods return Value
    is_grammar_mode: bool,
}

impl IrToRust {
    fn new() -> Self {
        Self {
            var_counter: 0,
            is_grammar_mode: false,
        }
    }

    fn new_grammar_mode() -> Self {
        Self {
            var_counter: 0,
            is_grammar_mode: true,
        }
    }

    fn fresh_var(&mut self) -> String {
        self.var_counter += 1;
        format!("__v{}", self.var_counter)
    }

    /// Extract tag and children from IR, supporting both formats:
    /// - Old: Value::Tagged(tag, children)
    /// - New: Value::List([Symbol(tag), ...children])
    fn extract_tag_children(&self, ir: &Value) -> Option<(SmolStr, Vec<Value>)> {
        match ir {
            Value::Tagged(tag, children) => Some((tag.clone(), (**children).clone())),
            Value::List(items) if !items.is_empty() => {
                // New format: [:Tag, arg1, arg2, ...]
                if let Value::Symbol(tag) = &items[0] {
                    let children: Vec<Value> = items.iter().skip(1).cloned().collect();
                    Some((tag.clone(), children))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn transpile_ir(&mut self, ir: &Value) -> Result<String> {
        match self.extract_tag_children(ir) {
            Some((tag, children)) => self.transpile_tagged(tag.as_str(), &children),
            None => Err(Error::Runtime(format!(
                "IR transpile expected tagged value or list syntax, got {}",
                ir.type_name()
            ))),
        }
    }

    /// Transpile an item within a sequence. Same as transpile_ir.
    fn transpile_seq_item(&mut self, ir: &Value) -> Result<String> {
        self.transpile_ir(ir)
    }

    /// Transpile a Lambda IR node as a native Rust closure.
    /// Used for semantic action lambdas passed to fold/reduce.
    /// Handles curried lambdas like \acc \d expr by flattening them.
    fn transpile_lambda_as_closure(
        &mut self,
        params_ir: &Value,
        body_ir: &Value,
    ) -> Result<String> {
        let mut all_params: Vec<String> = Vec::new();
        let mut current_body = body_ir;

        // Collect params from this lambda
        let params = self.expect_list(params_ir)?;
        for p in &params {
            if let Value::Symbol(name) = p {
                all_params.push(sanitize_ident(name));
            }
        }

        // Check if body is another Lambda (curried form)
        // Keep unwrapping nested Lambdas to flatten them
        // Use pattern matching to get references that live long enough
        loop {
            match current_body {
                Value::Tagged(tag, children) if tag.as_str() == "Lambda" && children.len() >= 2 => {
                    // Collect inner params
                    let inner_params = self.expect_list(&children[0])?;
                    for p in &inner_params {
                        if let Value::Symbol(name) = p {
                            all_params.push(sanitize_ident(name));
                        }
                    }
                    current_body = &children[1];
                }
                Value::List(items) if !items.is_empty() => {
                    if let Value::Symbol(tag) = &items[0]
                        && tag.as_str() == "Lambda"
                        && items.len() >= 3
                    {
                        // List format: [:Lambda, params, body]
                        let inner_params = self.expect_list(&items[1])?;
                        for p in &inner_params {
                            if let Value::Symbol(name) = p {
                                all_params.push(sanitize_ident(name));
                            }
                        }
                        current_body = &items[2];
                        continue;
                    }
                    break;
                }
                _ => break,
            }
        }

        // Transpile the final body
        let body = self.transpile_ir(current_body)?;

        // Build parameter list for Rust closure
        let params_str = all_params
            .iter()
            .map(|p| format!("{}: Value", p))
            .collect::<Vec<_>>()
            .join(", ");

        Ok(format!("|{}| {{ {} }}", params_str, body))
    }

    fn transpile_tagged(&mut self, tag: &str, children: &[Value]) -> Result<String> {
        match tag {
            "LoadNull" => Ok("Value::Null".to_string()),

            "LoadBool" => {
                let b = self.expect_bool(&children[0])?;
                Ok(format!("Value::Bool({})", b))
            }

            "LoadInt" => {
                let n = self.expect_int(&children[0])?;
                Ok(format!("Value::Int({})", n))
            }

            "LoadFloat" => {
                let n = self.expect_float(&children[0])?;
                Ok(format!("Value::Float({:?})", n))
            }

            "LoadString" => {
                let s = self.expect_string(&children[0])?;
                Ok(format!("Value::String(SmolStr::new({:?}))", s.as_str()))
            }

            "LoadSymbol" => {
                let s = self.expect_symbol(&children[0])?;
                Ok(format!("Value::Symbol(SmolStr::new({:?}))", s.as_str()))
            }

            "MakeTagged" => {
                // :MakeTagged(tag, [args...]) - create tagged value
                let tag = self.expect_symbol(&children[0])?;
                let args = self.expect_list(&children[1])?;
                let mut arg_strs = Vec::new();
                for arg in args {
                    arg_strs.push(self.transpile_ir(&arg)?);
                }
                Ok(format!(
                    "Value::Tagged(SmolStr::new({:?}), Arc::new(vec![{}]))",
                    tag.as_str(),
                    arg_strs.join(", ")
                ))
            }

            "Var" => {
                let name = self.expect_symbol(&children[0])?;
                Ok(format!("{}.clone()", sanitize_ident(&name)))
            }

            "Add" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                // In grammar mode, add() returns Result<Value> so we unwrap
                // In standalone mode, RUNTIME_PRELUDE's add() returns Value directly
                if self.is_grammar_mode {
                    Ok(format!("({}).add(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).add(&{})", lhs, rhs))
                }
            }

            "Sub" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).sub(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).sub(&{})", lhs, rhs))
                }
            }

            "Mul" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).mul(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).mul(&{})", lhs, rhs))
                }
            }

            "Div" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).div(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).div(&{})", lhs, rhs))
                }
            }

            "Mod" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).modulo(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).modulo(&{})", lhs, rhs))
                }
            }

            "Neg" => {
                let operand = self.transpile_ir(&children[0])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).neg().unwrap()", operand))
                } else {
                    Ok(format!("({}).neg()", operand))
                }
            }

            "Not" => {
                let operand = self.transpile_ir(&children[0])?;
                Ok(format!("({}).not()", operand))
            }

            "Eq" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                Ok(format!("({}).eq(&({}))", lhs, rhs))
            }

            "NotEq" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                Ok(format!("({}).eq(&({})).not()", lhs, rhs))
            }

            "Lt" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).lt(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).lt(&({})", lhs, rhs))
                }
            }

            "Gt" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).gt(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).gt(&({})", lhs, rhs))
                }
            }

            "LtEq" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).le(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).le(&({})", lhs, rhs))
                }
            }

            "GtEq" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                if self.is_grammar_mode {
                    Ok(format!("({}).ge(&({})).unwrap()", lhs, rhs))
                } else {
                    Ok(format!("({}).ge(&({})", lhs, rhs))
                }
            }

            "And" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                Ok(format!("if ({}).is_truthy() {{ {} }} else {{ Value::Bool(false) }}", lhs, rhs))
            }

            "Or" => {
                let lhs = self.transpile_ir(&children[0])?;
                let rhs = self.transpile_ir(&children[1])?;
                let tmp = self.fresh_var();
                Ok(format!("{{ let {} = {}; if ({}).is_truthy() {{ {} }} else {{ {} }} }}",
                    tmp, lhs, tmp, tmp, rhs))
            }

            "Let" => {
                let name = self.expect_symbol(&children[0])?;
                let value = self.transpile_ir(&children[1])?;
                let body = self.transpile_ir(&children[2])?;
                Ok(format!("{{ let {} = {}; {} }}", sanitize_ident(&name), value, body))
            }

            "If" => {
                let cond = self.transpile_ir(&children[0])?;
                let then_branch = self.transpile_ir(&children[1])?;
                let else_branch = self.transpile_ir(&children[2])?;
                Ok(format!("if ({}).is_truthy() {{ {} }} else {{ {} }}", cond, then_branch, else_branch))
            }

            "MakeList" => {
                let items = self.expect_list(&children[0])?;
                let mut item_strs = Vec::new();
                for item in items {
                    item_strs.push(self.transpile_ir(&item)?);
                }
                Ok(format!("Value::List(Arc::new(vec![{}]))", item_strs.join(", ")))
            }

            "MakeMap" => {
                let pairs = self.expect_list(&children[0])?;
                let mut pair_strs = Vec::new();
                for pair in pairs {
                    let pair_items = self.expect_list(&pair)?;
                    if pair_items.len() != 2 {
                        return Err(Error::Runtime("MakeMap pair must have 2 elements".to_string()));
                    }
                    let key = self.transpile_ir(&pair_items[0])?;
                    let val = self.transpile_ir(&pair_items[1])?;
                    pair_strs.push(format!("({}, {})", key, val));
                }
                Ok(format!("Value::Map(vec![{}])", pair_strs.join(", ")))
            }

            "Index" => {
                let collection = self.transpile_ir(&children[0])?;
                let key = self.transpile_ir(&children[1])?;
                Ok(format!("({}).index(&({})).unwrap_or(Value::Null)", collection, key))
            }

            "Call" => {
                let args = self.expect_list(&children[1])?;
                let mut arg_strs = Vec::new();
                for arg in &args {
                    arg_strs.push(self.transpile_ir(arg)?);
                }

                // Check if this is a call to a known helper function
                // Support both Value::Tagged and list syntax [:Tag, ...]
                if let Some((func_tag, func_children)) = self.extract_tag_children(&children[0]) {
                    // Check for qualified calls like float.parse(x)
                    // Structure: Call(GetProp(Var(float), parse), [x])
                    if func_tag.as_str() == "GetProp" && func_children.len() >= 2
                        && let Some((var_tag, var_children)) = self.extract_tag_children(&func_children[0])
                            && var_tag.as_str() == "Var"
                                && let (Some(Value::Symbol(obj)), Some(Value::Symbol(method))) =
                                    (var_children.first(), func_children.get(1)) {
                                    // Handle known qualified helpers
                                    match (obj.as_str(), method.as_str()) {
                                        ("float", "parse") if arg_strs.len() == 1 => {
                                            return Ok(format!("float_parse({})", arg_strs[0]));
                                        }
                                        _ => {}
                                    }
                                }
                    if func_tag.as_str() == "Var"
                        && let Some(Value::Symbol(name)) = func_children.first() {
                            match name.as_str() {
                                "prepend" if arg_strs.len() == 2 => {
                                    return Ok(format!("prepend({}, {})", arg_strs[0], arg_strs[1]));
                                }
                                "join" if arg_strs.len() == 1 => {
                                    return Ok(format!("join({})", arg_strs[0]));
                                }
                                "symbol" if arg_strs.len() == 1 => {
                                    return Ok(format!("symbol({})", arg_strs[0]));
                                }
                                "fold_binary" if arg_strs.len() == 2 => {
                                    return Ok(format!("fold_binary({}, {})", arg_strs[0], arg_strs[1]));
                                }
                                "fold_postfix" if arg_strs.len() == 2 => {
                                    return Ok(format!("fold_postfix({}, {})", arg_strs[0], arg_strs[1]));
                                }
                                "float_parse" if arg_strs.len() == 1 => {
                                    return Ok(format!("float_parse({})", arg_strs[0]));
                                }
                                "length" if arg_strs.len() == 1 => {
                                    return Ok(format!("length({})", arg_strs[0]));
                                }
                                "fold_pipe_at" if arg_strs.len() == 2 => {
                                    return Ok(format!("fold_pipe_at({}, {})", arg_strs[0], arg_strs[1]));
                                }
                                "reduce" | "fold" if args.len() == 3 => {
                                    // For fold/reduce, if the first arg is a Lambda, generate a native Rust closure
                                    if let Some((lambda_tag, lambda_children)) = self.extract_tag_children(&args[0])
                                        && lambda_tag.as_str() == "Lambda" && lambda_children.len() >= 2 {
                                            let closure = self.transpile_lambda_as_closure(&lambda_children[0], &lambda_children[1])?;
                                            return Ok(format!("fold({}, {}, {})", closure, arg_strs[1], arg_strs[2]));
                                        }
                                    // Fallback for non-lambda first arg
                                    return Ok(format!("fold({}, {}, {})", arg_strs[0], arg_strs[1], arg_strs[2]));
                                }
                                _ => {}
                            }
                        }
                }

                // Default: call via Value's call method
                let func = self.transpile_ir(&children[0])?;
                Ok(format!("({}).call(vec![{}])", func, arg_strs.join(", ")))
            }

            "Lambda" => {
                // For standalone Lambda nodes (not inside fold/reduce), generate a boxed closure
                // This is used when lambdas are returned as values from semantic actions
                self.transpile_lambda_as_closure(&children[0], &children[1])
            }

            "Return" => {
                // In Rust, last expression is the return value
                self.transpile_ir(&children[0])
            }

            "MethodCall" => {
                // MethodCall(obj, method_name, args)
                // Handle common methods used in grammar semantic actions
                let method = self.expect_symbol(&children[1])?;
                let args = self.expect_list(&children[2])?;
                let mut arg_strs = Vec::new();
                for arg in &args {
                    arg_strs.push(self.transpile_ir(arg)?);
                }

                // Check for known builtin method calls like float.parse
                if let Some((obj_tag, obj_children)) = self.extract_tag_children(&children[0])
                    && obj_tag.as_str() == "Var"
                        && let Some(Value::Symbol(obj_name)) = obj_children.first() {
                            match (obj_name.as_str(), method.as_str()) {
                                ("float", "parse") if arg_strs.len() == 1 => {
                                    return Ok(format!("float_parse({})", arg_strs[0]));
                                }
                                _ => {}
                            }
                        }

                let obj = self.transpile_ir(&children[0])?;
                match method.as_str() {
                    "push" if arg_strs.len() == 1 => {
                        // list.push(item) returns a new list with item appended
                        Ok(format!("{{ let mut result = match {} {{ Value::List(items) => items.as_ref().clone(), _ => vec![] }}; result.push({}); Value::List(Arc::new(result)) }}", obj, arg_strs[0]))
                    }
                    _ => {
                        Err(Error::Runtime(format!("Unknown method call in IR: {}", method)))
                    }
                }
            }

            // =================================================================
            // Parsing IR nodes - for parser generation from Grammar structures
            // =================================================================

            // :ParseChar(char) - match a single character
            "ParseChar" => {
                let c = self.expect_string(&children[0])?;
                let _c_char = c.chars().next().unwrap_or('\0');
                let escaped_for_comparison = escape_string_for_rust(c.as_ref());
                let escaped_for_error = escape_string_for_error_message(c.as_ref());
                Ok(format!(
                    "if input.get(pos..pos+1) == Some(\"{}\") {{ Ok((Value::String(SmolStr::new(\"{}\")), pos + 1)) }} else {{ Err(Error::Parser {{ token: pos, message: \"expected '{}'\".to_string() }}) }}",
                    escaped_for_comparison, escaped_for_comparison, escaped_for_error
                ))
            }

            // :ParseLiteral(string) - match a literal string
            "ParseLiteral" => {
                let s = self.expect_string(&children[0])?;
                let escaped = escape_string_for_rust(&s);
                let escaped_for_error = escape_string_for_error_message(&s);
                let len = s.len();
                Ok(format!(
                    "if input.get(pos..pos+{}).map(|s| s == \"{}\").unwrap_or(false) {{ Ok((Value::String(SmolStr::new(\"{}\")), pos + {})) }} else {{ Err(Error::Parser {{ token: pos, message: \"expected \\\"{}\\\"\".to_string() }}) }}",
                    len, escaped, escaped, len, escaped_for_error
                ))
            }

            // :ParseCharClass(ranges, negated) - match character class like [a-zA-Z]
            // ranges is a list of [start, end] pairs
            "ParseCharClass" => {
                let ranges = self.expect_list(&children[0])?;
                let negated = self.expect_bool(&children[1])?;

                let mut conditions = Vec::new();
                for range in ranges {
                    let pair = self.expect_list(&range)?;
                    if pair.len() == 2 {
                        let start = self.expect_string(&pair[0])?;
                        let end = self.expect_string(&pair[1])?;
                        let start_char = start.chars().next().unwrap_or('\0');
                        let end_char = end.chars().next().unwrap_or('\0');
                        if start_char == end_char {
                            conditions.push(format!("c == '{}'", escape_char_for_char_literal(start_char)));
                        } else {
                            conditions.push(format!("(c >= '{}' && c <= '{}')",
                                escape_char_for_char_literal(start_char),
                                escape_char_for_char_literal(end_char)));
                        }
                    }
                }

                let condition = if conditions.is_empty() {
                    "false".to_string()
                } else {
                    conditions.join(" || ")
                };

                let match_expr = if negated {
                    format!("!({})", condition)
                } else {
                    condition
                };

                Ok(format!(
                    "if let Some(c) = input.get(pos..pos+1).and_then(|s| s.chars().next()) {{ if {} {{ Ok((Value::String(SmolStr::new(&input[pos..pos+c.len_utf8()])), pos + c.len_utf8())) }} else {{ Err(Error::Parser {{ token: pos, message: \"character class mismatch\".to_string() }}) }} }} else {{ Err(Error::Parser {{ token: pos, message: \"unexpected end of input\".to_string() }}) }}",
                    match_expr
                ))
            }

            // :ParseAny - match any single character
            "ParseAny" => {
                Ok("if let Some(c) = input.get(pos..pos+1).and_then(|s| s.chars().next()) { Ok((Value::String(SmolStr::new(&input[pos..pos+c.len_utf8()])), pos + c.len_utf8())) } else { Err(Error::Parser { token: pos, message: \"unexpected end of input\".to_string() }) }".to_string())
            }

            // :ParseSeq([ir1, ir2, ...]) - sequence of parsers
            // Note: bindings are NOT declared here - they're declared by the enclosing ParseAction
            "ParseSeq" => {
                let items = self.expect_list(&children[0])?;
                if items.is_empty() {
                    return Ok("Ok((Value::List(Arc::new(vec![])), pos))".to_string());
                }

                let mut code = String::new();
                code.push_str("{\n    let mut current_pos = pos;\n    let mut results: Vec<Value> = Vec::new();\n");

                for (i, item) in items.iter().enumerate() {
                    let item_code = self.transpile_seq_item(item)?;
                    code.push_str(&format!(
                        "    match {{ let pos = current_pos; {} }} {{\n        Ok((v{}, new_pos)) => {{ results.push(v{}.clone()); current_pos = new_pos; }}\n        Err(e) => return Err(e),\n    }}\n",
                        item_code, i, i
                    ));
                }

                code.push_str("    Ok::<_, Error>((Value::List(Arc::new(results)), current_pos))\n}");
                Ok(code)
            }

            // :ParseChoice([ir1, ir2, ...]) - ordered choice of parsers
            // Each alternative is wrapped in a closure so `return` doesn't escape the whole function.
            // IMPORTANT: Do NOT use early returns that escape the outer context!
            // The choice may be part of a sequence (e.g., keyword = ("if" | ...) ~ident_rest)
            // so we must return the result normally, not via `return` statement that escapes the sequence.
            "ParseChoice" => {
                let items = self.expect_list(&children[0])?;
                if items.is_empty() {
                    return Ok("Err(Error::Parser { token: pos, message: \"empty choice\".to_string() })".to_string());
                }

                if items.len() == 1 {
                    // Single alternative - just try it
                    return self.transpile_ir(&items[0]);
                }

                let mut code = String::new();
                code.push_str("{\n    let start_pos = pos;\n");

                // Generate chained alternatives
                // Each alternative is wrapped in a closure to capture its internal early returns
                for (i, item) in items.iter().enumerate() {
                    let item_code = self.transpile_ir(item)?;

                    if i == 0 {
                        code.push_str(&format!("    let mut choice_result = (|| {{ let pos = start_pos; {} }})();\n", item_code));
                    } else {
                        code.push_str(&format!("    if choice_result.is_err() {{\n        choice_result = (|| {{ let pos = start_pos; {} }})();\n    }}\n", item_code));
                    }
                }

                code.push_str("    choice_result\n}");
                Ok(code)
            }

            // :ParseStar(ir) - zero or more repetitions
            // Inner is wrapped in closure so 'return' doesn't escape
            // Note: Like the grammar runtime, if all results are strings, join them (but not if empty)
            "ParseStar" => {
                let inner = self.transpile_ir(&children[0])?;
                Ok(format!(
                    "{{\n    let mut results: Vec<Value> = Vec::new();\n    let mut current_pos = pos;\n    loop {{\n        match (|| {{ let pos = current_pos; {} }})() {{\n            Ok((v, new_pos)) => {{\n                if new_pos == current_pos {{ break; }}\n                results.push(v);\n                current_pos = new_pos;\n            }}\n            Err(_) => break,\n        }}\n    }}\n    // Auto-join strings like the grammar runtime does (but keep empty list as list)\n    let result = if !results.is_empty() && results.iter().all(|v| matches!(v, Value::String(_))) {{\n        let s: String = results.iter().filter_map(|v| if let Value::String(s) = v {{ Some(s.as_str()) }} else {{ None }}).collect();\n        Value::String(SmolStr::new(&s))\n    }} else {{\n        Value::List(Arc::new(results))\n    }};\n    Ok::<_, Error>((result, current_pos))\n}}",
                    inner
                ))
            }

            // :ParsePlus(ir) - one or more repetitions
            // Inner is wrapped in closure so 'return' doesn't escape
            // Note: Like the grammar runtime, if all results are strings, join them
            "ParsePlus" => {
                let inner = self.transpile_ir(&children[0])?;
                Ok(format!(
                    "{{\n    let mut results: Vec<Value> = Vec::new();\n    let mut current_pos = pos;\n    match (|| {{ let pos = current_pos; {} }})() {{\n        Ok((v, new_pos)) => {{\n            results.push(v);\n            current_pos = new_pos;\n        }}\n        Err(e) => return Err(e),\n    }}\n    loop {{\n        match (|| {{ let pos = current_pos; {} }})() {{\n            Ok((v, new_pos)) => {{\n                if new_pos == current_pos {{ break; }}\n                results.push(v);\n                current_pos = new_pos;\n            }}\n            Err(_) => break,\n        }}\n    }}\n    // Auto-join strings like the grammar runtime does\n    let result = if results.iter().all(|v| matches!(v, Value::String(_))) {{\n        let s: String = results.iter().filter_map(|v| if let Value::String(s) = v {{ Some(s.as_str()) }} else {{ None }}).collect();\n        Value::String(SmolStr::new(&s))\n    }} else {{\n        Value::List(Arc::new(results))\n    }};\n    Ok::<_, Error>((result, current_pos))\n}}",
                    inner, inner
                ))
            }

            // :ParseOptional(ir) - zero or one
            "ParseOptional" => {
                let inner = self.transpile_ir(&children[0])?;
                Ok(format!(
                    "match {{ let pos = pos; {} }} {{ Ok((v, new_pos)) => Ok((v, new_pos)), Err(_) => Ok((Value::Null, pos)) }}",
                    inner
                ))
            }

            // :ParseNot(ir) - negative lookahead
            "ParseNot" => {
                let inner = self.transpile_ir(&children[0])?;
                Ok(format!(
                    "match {{ let pos = pos; {} }} {{ Ok(_) => Err(Error::Parser {{ token: pos, message: \"negative lookahead matched\".to_string() }}), Err(_) => Ok((Value::Null, pos)) }}",
                    inner
                ))
            }

            // :ParseLookahead(ir) - positive lookahead
            "ParseLookahead" => {
                let inner = self.transpile_ir(&children[0])?;
                Ok(format!(
                    "match {{ let pos = pos; {} }} {{ Ok((v, _)) => Ok((v, pos)), Err(e) => Err(e) }}",
                    inner
                ))
            }

            // :ParseRule(name) - call another parsing rule
            "ParseRule" => {
                let name = self.expect_symbol(&children[0])?;
                Ok(format!("parse_{}(input, pos)", sanitize_ident(&name)))
            }

            // :ParseBind(ir, name) - bind result to a name
            // Note: This is used within sequences where variables are declared upfront
            "ParseBind" => {
                let inner = self.transpile_ir(&children[0])?;
                let name = self.expect_symbol(&children[1])?;
                // Use a unique temp var name to avoid collision with bound variable names
                let temp_var = self.fresh_var();
                // Assign to the pre-declared variable and return the value
                Ok(format!(
                    "match {{ let pos = pos; {} }} {{ Ok(({}, new_pos)) => {{ {} = {}.clone(); Ok(({}, new_pos)) }} Err(e) => Err(e), }}",
                    inner, temp_var, sanitize_ident(&name), temp_var, temp_var
                ))
            }

            // :ParseAction(ir, action_expr) - apply semantic action to result
            // Collect all bindings from the inner pattern and declare them upfront
            "ParseAction" => {
                let inner = self.transpile_ir(&children[0])?;
                let action = self.transpile_ir(&children[1])?;

                // Collect bindings from the inner pattern
                let mut bindings = Vec::new();
                collect_bindings_from_ir(&children[0], &mut bindings);

                // Declare all bindings upfront with default values
                let mut decls = String::new();
                for binding in &bindings {
                    decls.push_str(&format!("let mut {} = Value::Null;\n", sanitize_ident(binding)));
                }

                // Directly inline the action expression with bindings declared
                Ok(format!(
                    "{{\n{}match {{ let pos = pos; {} }} {{ Ok((_v, new_pos)) => Ok(({}, new_pos)), Err(e) => Err(e), }}\n}}",
                    decls, inner, action
                ))
            }

            // :ParseGrammar(name, rules) - complete grammar with multiple rules
            // rules is a list of :ParseRuleDef(name, body) tuples
            "ParseGrammar" => {
                let grammar_name = self.expect_symbol(&children[0])?;
                let rules = self.expect_list(&children[1])?;

                let mut code = String::new();
                code.push_str(&format!("// Generated parser for grammar: {}\n", grammar_name));
                code.push_str("// DO NOT EDIT - generated from IR by ir_to_rust\n");
                code.push_str("// This file is included into parser.rs which provides all imports.\n\n");
                // Use inline Result type to avoid conflicts with crate::error::Result
                code.push_str("type ParseResult<T> = std::result::Result<(T, usize), Error>;\n\n");

                // Debug tracing support
                code.push_str(r#"
// Set to true to enable debug tracing
const DEBUG_PARSE: bool = false;

macro_rules! trace_enter {
    ($rule:expr, $input:expr, $pos:expr) => {
        if DEBUG_PARSE {
            let preview: String = $input.chars().skip($pos).take(20).collect();
            eprintln!("ENTER {} at {} {:?}...", $rule, $pos, preview);
        }
    };
}

macro_rules! trace_exit {
    ($rule:expr, $result:expr) => {
        if DEBUG_PARSE {
            match &$result {
                Ok((_, pos)) => eprintln!("EXIT {} => OK at {}", $rule, pos),
                Err(e) => eprintln!("EXIT {} => ERR: {:?}", $rule, e),
            }
        }
    };
}
"#);

                // Add helper functions used by semantic actions
                code.push_str(GRAMMAR_HELPERS);
                code.push('\n');

                for rule in rules {
                    if let Some((rule_tag, rule_children)) = self.extract_tag_children(&rule)
                        && rule_tag.as_str() == "ParseRuleDef" && rule_children.len() >= 2 {
                            let rule_name = self.expect_symbol(&rule_children[0])?;
                            let rule_body = self.transpile_ir(&rule_children[1])?;
                            let sanitized_name = sanitize_ident(&rule_name);

                            code.push_str(&format!(
                                "fn parse_{name}(input: &str, pos: usize) -> ParseResult<Value> {{\n    trace_enter!(\"{name}\", input, pos);\n    let __result = {body};\n    trace_exit!(\"{name}\", __result);\n    __result\n}}\n\n",
                                name = sanitized_name,
                                body = rule_body
                            ));
                        }
                }

                // Add public entry point that wraps the main parsing rule
                // This function is expected by tests and the legacy parser interface
                code.push_str(r#"
/// Parse FMPL source code using the generated parser.
/// Returns a Value::Tagged representation of the AST.
pub fn generated_parse(source: &str) -> Result<Expr> {
    match parse_code(source, 0) {
        Ok((value, _pos)) => value_to_expr(&value),
        Err(e) => Err(e),
    }
}

/// Convert a Value (tagged AST) to an Expr (typed AST).
/// Supports both formats:
/// - Old: Value::Tagged(tag, children)
/// - New: Value::List([Symbol(tag), ...children])
fn value_to_expr(value: &Value) -> Result<Expr> {
    // Helper to extract tag and children from either format
    let (tag, children) = match value {
        Value::Tagged(tag, children) => (tag.clone(), (**children).clone()),
        Value::List(items) if !items.is_empty() => {
            if let Value::Symbol(tag) = &items[0] {
                let children: Vec<Value> = items.iter().skip(1).cloned().collect();
                (tag.clone(), children)
            } else {
                return Err(Error::Runtime(format!("Expected list starting with symbol, got {:?}", value)));
            }
        }
        _ => return Err(Error::Runtime(format!("Expected tagged value or list, got {:?}", value))),
    };

    match tag.as_str() {
        "Int" => {
            if let Some(Value::Int(n)) = children.first() {
                Ok(Expr::Int(*n))
            } else {
                Err(Error::Runtime("Invalid Int node".to_string()))
            }
        }
        "Float" => {
            if let Some(Value::Float(f)) = children.first() {
                Ok(Expr::Float(*f))
            } else {
                Err(Error::Runtime("Invalid Float node".to_string()))
            }
        }
        "Bool" => {
            if let Some(Value::Bool(b)) = children.first() {
                Ok(Expr::Bool(*b))
            } else {
                Err(Error::Runtime("Invalid Bool node".to_string()))
            }
        }
        "Null" => Ok(Expr::Null),
        "String" => {
            // Handle both List of chars (from Star without auto-join) and String (from auto-join)
            match children.first() {
                Some(Value::List(chars)) => {
                    // Join characters into string
                    let s: String = chars.iter().filter_map(|v| {
                        if let Value::String(s) = v { Some(s.as_str()) } else { None }
                    }).collect();
                    Ok(Expr::String(SmolStr::new(&s)))
                }
                Some(Value::String(s)) => {
                    // Already a joined string
                    Ok(Expr::String(s.clone()))
                }
                _ => Err(Error::Runtime("Invalid String node".to_string()))
            }
        }
        "Symbol" => {
            if let Some(Value::String(s)) = children.first() {
                Ok(Expr::Symbol(s.clone()))
            } else if let Some(Value::Symbol(s)) = children.first() {
                Ok(Expr::Symbol(s.clone()))
            } else {
                Err(Error::Runtime("Invalid Symbol node".to_string()))
            }
        }
        "Var" => {
            if let Some(Value::Symbol(name)) = children.first() {
                Ok(Expr::Ident(name.clone()))
            } else {
                Err(Error::Runtime("Invalid Var node".to_string()))
            }
        }
        "List" => {
            if let Some(Value::List(items)) = children.first() {
                let exprs: Result<Vec<Expr>> = items.iter().map(value_to_expr).collect();
                Ok(Expr::List(exprs?))
            } else {
                Err(Error::Runtime("Invalid List node".to_string()))
            }
        }
        "Map" => {
            if let Some(Value::List(entries)) = children.first() {
                let mut map_entries = Vec::new();
                for entry in entries.iter() {
                    if let Value::List(pair) = entry {
                        if pair.len() >= 2 {
                            let key = match &pair[0] {
                                Value::String(s) => s.clone(),
                                _ => return Err(Error::Runtime("Map key must be string".to_string())),
                            };
                            let val = value_to_expr(&pair[1])?;
                            map_entries.push(MapEntry::Symbol(key, val));
                        }
                    }
                }
                Ok(Expr::Map(map_entries))
            } else {
                Err(Error::Runtime("Invalid Map node".to_string()))
            }
        }
        "Lambda" => {
            if children.len() >= 2 {
                let params = if let Value::List(ps) = &children[0] {
                    ps.iter().filter_map(|p| {
                        if let Value::Symbol(s) = p { Some(s.clone()) } else { None }
                    }).collect()
                } else {
                    Vec::new()
                };
                let body = value_to_expr(&children[1])?;
                Ok(Expr::Lambda(params, Box::new(body)))
            } else {
                Err(Error::Runtime("Invalid Lambda node".to_string()))
            }
        }
        "ShortLambda" => {
            // :ShortLambda(param, body) - single parameter short form
            if children.len() >= 2 {
                let param = if let Value::Symbol(s) = &children[0] {
                    s.clone()
                } else {
                    return Err(Error::Runtime("Invalid ShortLambda param".to_string()));
                };
                let body = value_to_expr(&children[1])?;
                Ok(Expr::ShortLambda(param, Box::new(body)))
            } else {
                Err(Error::Runtime("Invalid ShortLambda node".to_string()))
            }
        }
        "Binary" => {
            // FMPL's fold_binary produces :Binary(op, left, right)
            if children.len() >= 3 {
                let op = if let Value::Symbol(s) = &children[0] {
                    match s.as_str() {
                        "+" => BinOp::Add,
                        "-" => BinOp::Sub,
                        "*" => BinOp::Mul,
                        "/" => BinOp::Div,
                        "%" => BinOp::Mod,
                        "==" => BinOp::Eq,
                        "!=" => BinOp::NotEq,
                        "<" => BinOp::Lt,
                        ">" => BinOp::Gt,
                        "<=" => BinOp::LtEq,
                        ">=" => BinOp::GtEq,
                        "&&" => BinOp::And,
                        "||" => BinOp::Or,
                        "|>" => BinOp::Pipe,
                        "in" => BinOp::In,
                        _ => return Err(Error::Runtime(format!("Unknown binary operator: {}", s))),
                    }
                } else {
                    return Err(Error::Runtime("Invalid Binary operator".to_string()));
                };
                let lhs = value_to_expr(&children[1])?;
                let rhs = value_to_expr(&children[2])?;
                Ok(Expr::Binary(Box::new(lhs), op, Box::new(rhs)))
            } else {
                Err(Error::Runtime("Invalid Binary node".to_string()))
            }
        }
        "Unary" => {
            if children.len() >= 2 {
                let op = if let Value::Symbol(s) = &children[0] {
                    match s.as_str() {
                        "-" => UnaryOp::Neg,
                        "!" => UnaryOp::Not,
                        _ => return Err(Error::Runtime(format!("Unknown unary operator: {}", s))),
                    }
                } else {
                    return Err(Error::Runtime("Invalid Unary operator".to_string()));
                };
                let operand = value_to_expr(&children[1])?;
                Ok(Expr::Unary(op, Box::new(operand)))
            } else {
                Err(Error::Runtime("Invalid Unary node".to_string()))
            }
        }
        "If" => {
            if children.len() >= 3 {
                let cond = value_to_expr(&children[0])?;
                let then_branch = value_to_expr(&children[1])?;
                let else_branch = value_to_expr(&children[2])?;
                Ok(Expr::If(Box::new(cond), Box::new(then_branch), Some(Box::new(else_branch))))
            } else {
                Err(Error::Runtime("Invalid If node".to_string()))
            }
        }
        "Let" => {
            if children.len() >= 2 {
                if let Value::List(bindings) = &children[0] {
                    let mut let_bindings = Vec::new();
                    for binding in bindings.iter() {
                        // Support both Value::Tagged("Binding", ...) and [:Binding, ...]
                        let (tag, parts) = match binding {
                            Value::Tagged(tag, children) => (tag.clone(), (**children).clone()),
                            Value::List(items) if !items.is_empty() => {
                                if let Value::Symbol(tag) = &items[0] {
                                    let parts: Vec<Value> = items.iter().skip(1).cloned().collect();
                                    (tag.clone(), parts)
                                } else {
                                    continue;
                                }
                            }
                            _ => continue,
                        };
                        if tag.as_str() == "Binding" && parts.len() >= 2 {
                            if let Value::Symbol(name) = &parts[0] {
                                let value = value_to_expr(&parts[1])?;
                                let_bindings.push(LetBinding::Simple(name.clone(), Some(Box::new(value))));
                            }
                        }
                    }
                    let body = value_to_expr(&children[1])?;
                    Ok(Expr::Let(let_bindings, Box::new(body)))
                } else {
                    Err(Error::Runtime("Invalid Let bindings".to_string()))
                }
            } else {
                Err(Error::Runtime("Invalid Let node".to_string()))
            }
        }
        "LetSimple" => {
            if !children.is_empty() {
                let (tag, parts) = match &children[0] {
                    Value::Tagged(tag, children) => (tag.clone(), (**children).clone()),
                    _ => return Err(Error::Runtime("Invalid LetSimple binding".to_string())),
                };
                if tag.as_str() == "Binding" && parts.len() >= 2 {
                    if let Value::Symbol(name) = &parts[0] {
                        let value = value_to_expr(&parts[1])?;
                        Ok(Expr::LetStmt(name.clone(), Box::new(value)))
                    } else {
                        Err(Error::Runtime("Invalid LetSimple binding name".to_string()))
                    }
                } else {
                    Err(Error::Runtime("Invalid LetSimple binding format".to_string()))
                }
            } else {
                Err(Error::Runtime("Invalid LetSimple node".to_string()))
            }
        }
        "Do" => {
            if let Some(Value::List(stmts)) = children.first() {
                let mut exprs = Vec::new();
                for stmt in stmts.iter() {
                    let (tag, children) = match stmt {
                        Value::Tagged(tag, children) => (tag.as_str(), &**children),
                        _ => {
                            exprs.push(value_to_expr(&stmt)?);
                            continue;
                        }
                    };
                    if tag == "LetSimple" {
                        if !children.is_empty() {
                            let (btag, parts) = match &children[0] {
                                Value::Tagged(tag, children) => (tag.as_str(), &**children),
                                _ => {
                                    exprs.push(value_to_expr(&stmt)?);
                                    continue;
                                }
                            };
                            if btag == "Binding" && parts.len() >= 2 {
                                if let Value::Symbol(name) = &parts[0] {
                                    let value = value_to_expr(&parts[1])?;
                                    exprs.push(Expr::LetStmt(name.clone(), Box::new(value)));
                                    continue;
                                }
                            }
                        }
                        exprs.push(value_to_expr(&stmt)?);
                    } else {
                        exprs.push(value_to_expr(&stmt)?);
                    }
                }
                if exprs.len() == 1 {
                    Ok(exprs.pop().unwrap())
                } else {
                    Ok(Expr::Sequence(exprs))
                }
            } else {
                Err(Error::Runtime("Invalid Do node".to_string()))
            }
        }
        "Yield" => {
            if !children.is_empty() {
                let value = value_to_expr(&children[0])?;
                Ok(Expr::Yield(Box::new(value)))
            } else {
                Err(Error::Runtime("Invalid Yield node".to_string()))
            }
        }
        "Tagged" => {
            if children.len() >= 2 {
                if let Value::String(tag) = &children[0] {
                    if let Value::List(args) = &children[1] {
                        let exprs: Result<Vec<Expr>> = args.iter().map(value_to_expr).collect();
                        Ok(Expr::Tagged(tag.clone(), exprs?))
                    } else {
                        Err(Error::Runtime("Invalid Tagged args".to_string()))
                    }
                } else {
                    Err(Error::Runtime("Invalid Tagged tag".to_string()))
                }
            } else {
                Err(Error::Runtime("Invalid Tagged node".to_string()))
            }
        }
        "QualifiedName" => {
            if let Some(Value::List(parts)) = children.first() {
                let names: Vec<SmolStr> = parts.iter().filter_map(|p| {
                    if let Value::String(s) = p { Some(s.clone()) } else { None }
                }).collect();
                Ok(Expr::Qualified(QualifiedName { parts: names }))
            } else {
                Err(Error::Runtime("Invalid QualifiedName node".to_string()))
            }
        }
        "Call" | "Index" | "Slice" | "MethodCall" | "PropAccess" => {
            // These are generated by fold_postfix - need to handle them
            match tag.as_str() {
                "Call" if children.len() >= 2 => {
                    let func = value_to_expr(&children[0])?;
                    if let Value::List(args) = &children[1] {
                        let arg_exprs: Result<Vec<Expr>> = args.iter().map(value_to_expr).collect();
                        let args: Vec<Arg> = arg_exprs?.into_iter().map(Arg::Expr).collect();
                        // Merge Call(Spawn(x, []), args) → Spawn(x, args)
                        if let Expr::Spawn(ref constructor, ref spawn_args) = func {
                            if spawn_args.is_empty() {
                                return Ok(Expr::Spawn(constructor.clone(), args));
                            }
                        }
                        Ok(Expr::Call(Box::new(func), args))
                    } else {
                        Err(Error::Runtime("Invalid Call args".to_string()))
                    }
                }
                "Index" if children.len() >= 2 => {
                    let obj = value_to_expr(&children[0])?;
                    let idx = value_to_expr(&children[1])?;
                    Ok(Expr::Index(Box::new(obj), Box::new(idx)))
                }
                "Slice" if children.len() >= 3 => {
                    let obj = value_to_expr(&children[0])?;
                    let start = match &children[1] {
                        Value::Null => None,
                        v => Some(Box::new(value_to_expr(v)?)),
                    };
                    let end = match &children[2] {
                        Value::Null => None,
                        v => Some(Box::new(value_to_expr(v)?)),
                    };
                    Ok(Expr::Slice(Box::new(obj), start, end))
                }
                "PropAccess" if children.len() >= 2 => {
                    let obj = value_to_expr(&children[0])?;
                    if let Value::Symbol(prop) = &children[1] {
                        Ok(Expr::PropAccess(Box::new(obj), prop.clone()))
                    } else {
                        Err(Error::Runtime("Invalid PropAccess property".to_string()))
                    }
                }
                "MethodCall" if children.len() >= 3 => {
                    let obj = value_to_expr(&children[0])?;
                    if let Value::Symbol(method) = &children[1] {
                        if let Value::List(args) = &children[2] {
                            let arg_exprs: Result<Vec<Expr>> = args.iter().map(value_to_expr).collect();
                            let args: Vec<Arg> = arg_exprs?.into_iter().map(Arg::Expr).collect();
                            Ok(Expr::MethodCall(Box::new(obj), method.clone(), args))
                        } else {
                            Err(Error::Runtime("Invalid MethodCall args".to_string()))
                        }
                    } else {
                        Err(Error::Runtime("Invalid MethodCall method".to_string()))
                    }
                }
                _ => Err(Error::Runtime(format!("Invalid {} node", tag))),
            }
        }
        "Try" => {
            if children.len() >= 3 {
                let body = value_to_expr(&children[0])?;
                let binding = if let Value::Symbol(s) = &children[1] {
                    s.clone()
                } else {
                    return Err(Error::Runtime("Invalid Try binding".to_string()));
                };
                let catch_body = value_to_expr(&children[2])?;
                Ok(Expr::TryCatch {
                    body: Box::new(body),
                    error_binding: binding,
                    catch_body: Box::new(catch_body),
                })
            } else {
                Err(Error::Runtime("Invalid Try node".to_string()))
            }
        }
        "While" => {
            if children.len() >= 2 {
                let cond = value_to_expr(&children[0])?;
                let body = value_to_expr(&children[1])?;
                Ok(Expr::While(Box::new(cond), Box::new(body)))
            } else {
                Err(Error::Runtime("Invalid While node".to_string()))
            }
        }
        "DoWhile" => {
            if children.len() >= 2 {
                let body = value_to_expr(&children[0])?;
                let cond = value_to_expr(&children[1])?;
                Ok(Expr::DoWhile(Box::new(body), Box::new(cond)))
            } else {
                Err(Error::Runtime("Invalid DoWhile node".to_string()))
            }
        }
        "For" => {
            if children.len() >= 3 {
                let pattern = value_to_pattern(&children[0])?;
                let iterable = value_to_expr(&children[1])?;
                let body = value_to_expr(&children[2])?;
                Ok(Expr::For(pattern, Box::new(iterable), Box::new(body)))
            } else {
                Err(Error::Runtime("Invalid For node".to_string()))
            }
        }
        "Match" => {
            if children.len() >= 2 {
                let scrutinee = value_to_expr(&children[0])?;
                if let Value::List(cases) = &children[1] {
                    let match_cases = cases.iter().map(|c| {
                        let (tag, cs) = match c {
                            Value::Tagged(tag, children) => (tag.as_str(), &**children),
                            _ => return Err(Error::Runtime("Invalid MatchCase".to_string())),
                        };
                        if tag != "MatchCase" || cs.len() < 3 {
                            return Err(Error::Runtime("Invalid MatchCase".to_string()));
                        }
                        let pattern = value_to_pattern(&cs[0])?;
                        let guard = match &cs[1] {
                            Value::Null => None,
                            other => Some(Box::new(value_to_expr(other)?)),
                        };
                        let body = value_to_expr(&cs[2])?;
                        Ok(MatchCase { pattern, guard, body: Box::new(body) })
                    }).collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Match(Box::new(scrutinee), match_cases))
                } else {
                    Err(Error::Runtime("Invalid Match cases".to_string()))
                }
            } else {
                Err(Error::Runtime("Invalid Match node".to_string()))
            }
        }
        "Return" => {
            if !children.is_empty() {
                let value = value_to_expr(&children[0])?;
                Ok(Expr::Return(Some(Box::new(value))))
            } else {
                Ok(Expr::Return(None))
            }
        }
        "Throw" => {
            if !children.is_empty() {
                let value = value_to_expr(&children[0])?;
                Ok(Expr::Throw(Box::new(value)))
            } else {
                Err(Error::Runtime("Invalid Throw node".to_string()))
            }
        }
        "Assign" => {
            if children.len() >= 2 {
                let target = value_to_expr(&children[0])?;
                let value = value_to_expr(&children[1])?;
                Ok(Expr::Assignment(Box::new(target), Box::new(value)))
            } else {
                Err(Error::Runtime("Invalid Assign node".to_string()))
            }
        }
        "Sequence" => {
            if let Some(Value::List(items)) = children.first() {
                let exprs: Result<Vec<Expr>> = items.iter().map(value_to_expr).collect();
                Ok(Expr::Sequence(exprs?))
            } else {
                Err(Error::Runtime("Invalid Sequence node".to_string()))
            }
        }
        "AsyncCall" => {
            if !children.is_empty() {
                let expr = value_to_expr(&children[0])?;
                Ok(Expr::AsyncCall(Box::new(expr)))
            } else {
                Err(Error::Runtime("Invalid AsyncCall node".to_string()))
            }
        }
        "ListCons" => {
            if children.len() >= 2 {
                let head = value_to_expr(&children[0])?;
                let tail = value_to_expr(&children[1])?;
                Ok(Expr::ListCons(Box::new(head), Box::new(tail)))
            } else {
                Err(Error::Runtime("Invalid ListCons node".to_string()))
            }
        }
        "Placeholder" => Ok(Expr::Placeholder),
        "Self" => Ok(Expr::Self_),
        "Parent" => Ok(Expr::Parent),
        "Caller" => Ok(Expr::Caller),
        "User" => Ok(Expr::User),
        "Args" => Ok(Expr::Args),
        "ObjTag" => {
            if let Some(Value::String(name)) = children.first() {
                Ok(Expr::ObjTag(name.clone()))
            } else {
                Err(Error::Runtime("Invalid ObjTag node".to_string()))
            }
        }
        "Spawn" => {
            if !children.is_empty() {
                let constructor = value_to_expr(&children[0])?;
                Ok(Expr::Spawn(Box::new(constructor), Vec::new()))
            } else {
                Err(Error::Runtime("Invalid Spawn node".to_string()))
            }
        }
        "FacetAccess" => {
            if children.len() >= 2 {
                let obj = value_to_expr(&children[0])?;
                let facet = match &children[1] {
                    Value::Symbol(s) => s.clone(),
                    Value::Tagged(tag, inner) if tag.as_str() == "Symbol" => {
                        if let Some(Value::String(s)) = inner.first() {
                            SmolStr::new(s.as_str())
                        } else if let Some(Value::Symbol(s)) = inner.first() {
                            s.clone()
                        } else {
                            return Err(Error::Runtime("Invalid FacetAccess facet name".to_string()));
                        }
                    }
                    _ => return Err(Error::Runtime("Invalid FacetAccess facet".to_string())),
                };
                Ok(Expr::FacetAccess(Box::new(obj), facet))
            } else {
                Err(Error::Runtime("Invalid FacetAccess node".to_string()))
            }
        }
        "MapEach" if children.len() >= 2 => {
            let func = value_to_expr(&children[0])?;
            let iterable = value_to_expr(&children[1])?;
            Ok(Expr::MapEach {
                elem_var: SmolStr::new("_elem"),
                iterable: Box::new(iterable),
                body: Box::new(func),
            })
        }
        "FilterExpr" if children.len() >= 2 => {
            let pred = value_to_expr(&children[0])?;
            let iterable = value_to_expr(&children[1])?;
            Ok(Expr::Filter {
                elem_var: SmolStr::new("_elem"),
                iterable: Box::new(iterable),
                body: Box::new(pred),
            })
        }
        "Fold" if children.len() >= 3 => {
            let func = value_to_expr(&children[0])?;
            let initial = value_to_expr(&children[1])?;
            let iterable = value_to_expr(&children[2])?;
            Ok(Expr::Fold {
                initial: Box::new(initial),
                acc_var: SmolStr::new("_acc"),
                iterable: Box::new(iterable),
                body: Box::new(func),
            })
        }
        "Foldr" if children.len() >= 3 => {
            let func = value_to_expr(&children[0])?;
            let initial = value_to_expr(&children[1])?;
            let iterable = value_to_expr(&children[2])?;
            Ok(Expr::Foldr {
                initial: Box::new(initial),
                acc_var: SmolStr::new("_acc"),
                iterable: Box::new(iterable),
                body: Box::new(func),
            })
        }
        "AtInlineBlock" if children.len() >= 2 => {
            // AtInlineBlock needs pattern conversion which requires value_to_pattern_cases
            // For now, delegate to the standalone value_to_ast module at runtime
            Err(Error::Runtime("AtInlineBlock in generated parser template not yet supported - use standalone value_to_ast".to_string()))
        }
        "AtGrammarApply" if children.len() >= 3 => {
            let input = value_to_expr(&children[0])?;
            let grammar = value_to_expr(&children[1])?;
            let rule = if let Value::Symbol(s) = &children[2] {
                s.clone()
            } else {
                return Err(Error::Runtime("Invalid AtGrammarApply rule".to_string()));
            };
            Ok(Expr::GrammarApply {
                input: Box::new(input),
                grammar: Box::new(grammar),
                rule,
            })
        }
        "Object" if children.len() >= 2 => {
            let name = match &children[0] {
                Value::String(s) => QualifiedName::simple(SmolStr::new(s.as_str())),
                _ => return Err(Error::Runtime("Invalid Object name".to_string())),
            };
            let content = match &children[1] {
                Value::List(items) => items,
                _ => return Err(Error::Runtime("Invalid Object content".to_string())),
            };
            let mut bindings = Vec::new();
            let mut facets = Vec::new();
            for item in content.iter() {
                match item {
                    Value::Tagged(tag, cs) => match tag.as_str() {
                        "Section" if cs.len() >= 2 => {
                            let vis = match &cs[0] {
                                Value::Symbol(s) => match s.as_str() {
                                    "public" => Visibility::Public,
                                    "protected" => Visibility::Protected,
                                    _ => Visibility::Private,
                                },
                                _ => Visibility::Private,
                            };
                            if let Value::List(items) = &cs[1] {
                                for b in items.iter() {
                                    if let Value::Tagged(btag, bc) = b {
                                        if btag.as_str() == "ObjBinding" && bc.len() >= 4 {
                                            let bname = match &bc[0] {
                                                Value::String(s) => SmolStr::new(s.as_str()),
                                                _ => continue,
                                            };
                                            let params: Vec<SmolStr> = match &bc[1] {
                                                Value::List(ps) => ps.iter().filter_map(|p| {
                                                    if let Value::Symbol(s) = p { Some(s.clone()) } else { None }
                                                }).collect(),
                                                _ => Vec::new(),
                                            };
                                            let has_params = matches!(&bc[3], Value::Bool(true));
                                            let value = value_to_expr(&bc[2])?;
                                            bindings.push(Binding { name: bname, params, has_params, value, visibility: vis });
                                        }
                                    }
                                }
                            }
                        }
                        "FacetSection" if !cs.is_empty() => {
                            if let Value::List(items) = &cs[0] {
                                for f in items.iter() {
                                    if let Value::Tagged(ftag, fc) = f {
                                        if ftag.as_str() == "FacetDef" && fc.len() >= 3 {
                                            let fname = match &fc[0] {
                                                Value::String(s) => SmolStr::new(s.as_str()),
                                                _ => continue,
                                            };
                                            let members: Vec<SmolStr> = match &fc[1] {
                                                Value::List(ms) => ms.iter().filter_map(|m| {
                                                    if let Value::String(s) = m { Some(SmolStr::new(s.as_str())) } else { None }
                                                }).collect(),
                                                _ => Vec::new(),
                                            };
                                            let terminal = matches!(&fc[2], Value::Bool(true));
                                            facets.push(FacetDef { name: fname, members, terminal });
                                        }
                                    }
                                }
                            }
                        }
                        "ObjBinding" if cs.len() >= 4 => {
                            let bname = match &cs[0] {
                                Value::String(s) => SmolStr::new(s.as_str()),
                                _ => continue,
                            };
                            let params: Vec<SmolStr> = match &cs[1] {
                                Value::List(ps) => ps.iter().filter_map(|p| {
                                    if let Value::Symbol(s) = p { Some(s.clone()) } else { None }
                                }).collect(),
                                _ => Vec::new(),
                            };
                            let has_params = matches!(&cs[3], Value::Bool(true));
                            let value = value_to_expr(&cs[2])?;
                            bindings.push(Binding { name: bname, params, has_params, value, visibility: Visibility::Private });
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Ok(Expr::ObjectDef(ObjectDef {
                name,
                params: Vec::new(),
                parents: Vec::new(),
                bindings,
                facets,
                is_constructor: false,
            }))
        }
        _ => Err(Error::Runtime(format!("Unknown AST node type: {}", tag))),
    }
}

fn value_to_pattern(value: &Value) -> Result<Pattern> {
    let (tag, children) = match value {
        Value::Tagged(tag, children) => (tag.clone(), (**children).clone()),
        _ => return Err(Error::Runtime(format!("Expected pattern, got {:?}", value))),
    };
    match tag.as_str() {
        "PatternWildcard" => Ok(Pattern::Wildcard),
        "PatternVar" if !children.is_empty() => {
            if let Value::Symbol(name) = &children[0] {
                Ok(Pattern::Var(name.clone()))
            } else {
                Err(Error::Runtime("Invalid PatternVar name".to_string()))
            }
        }
        "PatternLiteral" if !children.is_empty() => {
            value_to_literal_pattern(&children[0])
        }
        "PatternTagged" if children.len() >= 2 => {
            let tag_name = if let Value::String(s) = &children[0] {
                s.clone()
            } else {
                return Err(Error::Runtime("Invalid PatternTagged tag".to_string()));
            };
            let sub_patterns = if let Value::List(pats) = &children[1] {
                pats.iter().map(value_to_pattern).collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };
            Ok(Pattern::Constructor(tag_name, sub_patterns))
        }
        "PatternList" if !children.is_empty() => {
            if let Value::List(items) = &children[0] {
                let patterns = items.iter().map(value_to_pattern).collect::<Result<Vec<_>>>()?;
                Ok(Pattern::List(patterns, None))
            } else {
                Ok(Pattern::List(Vec::new(), None))
            }
        }
        "PatternMap" if !children.is_empty() => {
            if let Value::List(entries) = &children[0] {
                let mut map_patterns = Vec::new();
                for entry in entries.iter() {
                    if let Value::List(pair) = entry {
                        if pair.len() >= 2 {
                            let key = if let Value::String(s) = &pair[0] {
                                s.clone()
                            } else {
                                return Err(Error::Runtime("Invalid PatternMap key".to_string()));
                            };
                            let val = value_to_pattern(&pair[1])?;
                            map_patterns.push((key, val));
                        }
                    }
                }
                Ok(Pattern::Map(map_patterns))
            } else {
                Ok(Pattern::Map(Vec::new()))
            }
        }
        _ => Err(Error::Runtime(format!("Unknown pattern type: {}", tag))),
    }
}

fn value_to_literal_pattern(value: &Value) -> Result<Pattern> {
    let (tag, children) = match value {
        Value::Tagged(tag, children) => (tag.clone(), (**children).clone()),
        _ => return Err(Error::Runtime(format!("Expected literal, got {:?}", value))),
    };
    match tag.as_str() {
        "Int" if !children.is_empty() => {
            if let Value::Int(n) = &children[0] {
                Ok(Pattern::Int(*n))
            } else {
                Err(Error::Runtime("Invalid Int literal".to_string()))
            }
        }
        "Bool" if !children.is_empty() => {
            if let Value::Bool(b) = &children[0] {
                Ok(Pattern::Int(if *b { 1 } else { 0 }))
            } else {
                Err(Error::Runtime("Invalid Bool literal".to_string()))
            }
        }
        "Null" => Ok(Pattern::Wildcard),
        "String" if !children.is_empty() => {
            match &children[0] {
                Value::String(s) => Ok(Pattern::String(s.clone())),
                Value::List(chars) => {
                    let s: String = chars.iter().filter_map(|v| {
                        if let Value::String(s) = v { Some(s.as_str()) } else { None }
                    }).collect();
                    Ok(Pattern::String(SmolStr::new(&s)))
                }
                _ => Err(Error::Runtime("Invalid String literal".to_string()))
            }
        }
        _ => Err(Error::Runtime(format!("Unknown literal pattern type: {}", tag))),
    }
}
"#);

                Ok(code)
            }

            _ => Err(Error::Runtime(format!("Unknown IR node for Rust transpile: {}", tag))),
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
}

/// Sanitize FMPL identifier to valid Rust identifier
fn sanitize_ident(name: &str) -> String {
    // Replace invalid chars and handle Rust keywords
    let sanitized = name
        .replace("-", "_")
        .replace("?", "_q")
        .replace("!", "_bang");

    // Handle Rust keywords
    match sanitized.as_str() {
        "type" | "match" | "loop" | "move" | "ref" | "self" | "Self" | "fn" | "let" | "mut"
        | "const" | "static" | "pub" | "mod" | "use" | "as" | "if" | "else" | "while" | "for"
        | "in" | "return" | "break" | "continue" | "struct" | "enum" | "trait" | "impl"
        | "where" | "async" | "await" | "dyn" | "true" | "false" => {
            format!("r#{}", sanitized)
        }
        _ => sanitized,
    }
}

/// Escape a character for use in a Rust char literal ('x')
fn escape_char_for_char_literal(c: char) -> String {
    match c {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\0' => "\\0".to_string(),
        c if c.is_ascii_control() => format!("\\x{:02x}", c as u8),
        c => c.to_string(),
    }
}

/// Escape a string for use in generated Rust code inside a format! macro.
///
/// The generated code will be inserted via {} into a format string.
/// The format string's escapes are processed first, then our string is inserted literally.
///
/// For example, to generate: Some("\"")
/// We use: format!("Some(\"{}\")", escape_string_for_rust("\""))
/// The format string in memory is: Some("{}")
/// We return: \"  (backslash quote - 2 chars)
/// Result: Some("\"")
fn escape_string_for_rust(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"), // \ -> \\
            '"' => result.push_str("\\\""),  // " -> \"
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_ascii_control() => result.push_str(&format!("\\x{:02x}", c as u8)),
            c => result.push(c),
        }
    }
    result
}

/// Escape a string for use inside an error message string in generated Rust.
/// Same as escape_string_for_rust since both are used the same way.
fn escape_string_for_error_message(s: &str) -> String {
    escape_string_for_rust(s)
}

/// Collect free variables from IR
#[allow(dead_code)]
fn collect_free_vars(ir: &Value, bound: &HashSet<String>, free: &mut HashSet<String>) {
    match ir {
        Value::Tagged(tag, children) => match tag.as_str() {
            "Var" => {
                if let Some(Value::Symbol(name)) = children.first() {
                    let name_str = sanitize_ident(name);
                    if !bound.contains(&name_str) {
                        free.insert(name_str);
                    }
                }
            }
            "Let" => {
                if children.len() >= 3 {
                    collect_free_vars(&children[1], bound, free);
                    if let Value::Symbol(name) = &children[0] {
                        let mut new_bound = bound.clone();
                        new_bound.insert(sanitize_ident(name));
                        collect_free_vars(&children[2], &new_bound, free);
                    }
                }
            }
            "Lambda" => {
                if children.len() >= 2
                    && let Value::List(params) = &children[0]
                {
                    let mut new_bound = bound.clone();
                    for p in params.iter() {
                        if let Value::Symbol(name) = p {
                            new_bound.insert(sanitize_ident(name));
                        }
                    }
                    collect_free_vars(&children[1], &new_bound, free);
                }
            }
            _ => {
                for child in children.iter() {
                    collect_free_vars(child, bound, free);
                }
            }
        },
        Value::List(items) => {
            for item in items.iter() {
                collect_free_vars(item, bound, free);
            }
        }
        _ => {}
    }
}

/// Collect all binding names from a list of IR items (ParseSeq items).
/// This extracts variable names bound via ParseBind so they can be declared upfront.
#[allow(dead_code)]
fn collect_bindings_from_ir_list(items: &[Value], bindings: &mut Vec<SmolStr>) {
    for item in items {
        collect_bindings_from_ir(item, bindings);
    }
}

/// Recursively collect binding names from an IR node.
fn collect_bindings_from_ir(ir: &Value, bindings: &mut Vec<SmolStr>) {
    if let Some((tag, children)) = ir.as_node() {
        match tag.as_str() {
            "ParseBind" => {
                // Extract the binding name (second child)
                if children.len() >= 2 {
                    if let Value::Symbol(name) = &children[1] {
                        bindings.push(name.clone());
                    }
                    // Also check for nested bindings in the inner pattern
                    collect_bindings_from_ir(&children[0], bindings);
                }
            }
            "ParseSeq" => {
                // Check items in sequence
                if let Some(Value::List(items)) = children.first() {
                    for item in items.iter() {
                        collect_bindings_from_ir(item, bindings);
                    }
                }
            }
            "ParseChoice" => {
                // Check all alternatives - bindings might come from any branch
                if let Some(Value::List(items)) = children.first() {
                    for item in items.iter() {
                        collect_bindings_from_ir(item, bindings);
                    }
                }
            }
            "ParseAction" => {
                // Check the inner pattern
                if !children.is_empty() {
                    collect_bindings_from_ir(&children[0], bindings);
                }
            }
            "ParseStar" | "ParsePlus" | "ParseOptional" | "ParseNot" | "ParseLookahead" => {
                // Check inner pattern
                if !children.is_empty() {
                    collect_bindings_from_ir(&children[0], bindings);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_transpile_add() {
        let ir = Value::list_node(
            "Add",
            vec![
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(1)])),
                Value::Tagged(SmolStr::new("LoadInt"), Arc::new(vec![Value::Int(2)])),
            ],
        );
        let result = transpile_expr(&ir).unwrap();
        assert!(result.contains("add"));
    }

    #[test]
    fn test_transpile_let() {
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
        let result = transpile_expr(&ir).unwrap();
        assert!(result.contains("let x"));
    }
}
