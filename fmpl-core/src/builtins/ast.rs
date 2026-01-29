//! AST manipulation builtins.
//!
//! Provides functions to parse FMPL source code into tagged value AST representation.

use crate::ast::{Arg, BinOp, Expr, LetBinding, MapEntry, Pattern, UnaryOp};
use crate::error::Result;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::value::Value;
use smol_str::SmolStr;
use std::sync::Arc;

/// Convert an Expr AST node to a Value::Tagged representation.
pub fn expr_to_value(expr: &Expr) -> Value {
    match expr {
        Expr::Int(n) => Value::Tagged(SmolStr::new("Int"), Arc::new(vec![Value::Int(*n)])),

        Expr::Float(n) => Value::Tagged(SmolStr::new("Float"), Arc::new(vec![Value::Float(*n)])),

        Expr::String(s) => Value::Tagged(
            SmolStr::new("String"),
            Arc::new(vec![Value::String(s.clone())]),
        ),

        Expr::Symbol(s) => Value::Tagged(
            SmolStr::new("Symbol"),
            Arc::new(vec![Value::Symbol(s.clone())]),
        ),

        Expr::Tagged(tag, children) => Value::Tagged(
            SmolStr::new("Tagged"),
            Arc::new(vec![
                Value::Symbol(tag.clone()),
                Value::List(Arc::new(children.iter().map(expr_to_value).collect())),
            ]),
        ),

        Expr::Bool(b) => Value::Tagged(SmolStr::new("Bool"), Arc::new(vec![Value::Bool(*b)])),

        Expr::Null => Value::Tagged(SmolStr::new("Null"), Arc::new(vec![])),

        Expr::Ident(name) => Value::Tagged(
            SmolStr::new("Var"),
            Arc::new(vec![Value::Symbol(name.clone())]),
        ),

        Expr::Qualified(qn) => Value::Tagged(
            SmolStr::new("Qualified"),
            Arc::new(vec![Value::List(Arc::new(
                qn.parts.iter().map(|p| Value::Symbol(p.clone())).collect(),
            ))]),
        ),

        Expr::Binary(lhs, op, rhs) => Value::Tagged(
            SmolStr::new("Binary"),
            Arc::new(vec![
                Value::Symbol(SmolStr::new(binop_to_str(*op))),
                expr_to_value(lhs),
                expr_to_value(rhs),
            ]),
        ),

        Expr::Unary(op, e) => Value::Tagged(
            SmolStr::new("Unary"),
            Arc::new(vec![
                Value::Symbol(SmolStr::new(unaryop_to_str(*op))),
                expr_to_value(e),
            ]),
        ),

        Expr::Lambda(params, body) => Value::Tagged(
            SmolStr::new("Lambda"),
            Arc::new(vec![
                Value::List(Arc::new(
                    params.iter().map(|p| Value::Symbol(p.clone())).collect(),
                )),
                expr_to_value(body),
            ]),
        ),

        Expr::ShortLambda(param, body) => Value::Tagged(
            SmolStr::new("Lambda"),
            Arc::new(vec![
                Value::List(Arc::new(vec![Value::Symbol(param.clone())])),
                expr_to_value(body),
            ]),
        ),

        Expr::Call(func, args) => Value::Tagged(
            SmolStr::new("Call"),
            Arc::new(vec![
                expr_to_value(func),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ]),
        ),

        Expr::MethodCall(receiver, method, args) => Value::Tagged(
            SmolStr::new("MethodCall"),
            Arc::new(vec![
                expr_to_value(receiver),
                Value::Symbol(method.clone()),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ]),
        ),

        Expr::PropAccess(obj, prop) => Value::Tagged(
            SmolStr::new("PropAccess"),
            Arc::new(vec![expr_to_value(obj), Value::Symbol(prop.clone())]),
        ),

        Expr::If(cond, then_branch, else_branch) => Value::Tagged(
            SmolStr::new("If"),
            Arc::new(vec![
                expr_to_value(cond),
                expr_to_value(then_branch),
                else_branch
                    .as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ]),
        ),

        Expr::Let(bindings, body) => {
            let binding_values: Vec<Value> = bindings.iter().map(let_binding_to_value).collect();
            Value::Tagged(
                SmolStr::new("Let"),
                Arc::new(vec![
                    Value::List(Arc::new(binding_values)),
                    expr_to_value(body),
                ]),
            )
        }

        Expr::LetStmt(name, expr) => Value::Tagged(
            SmolStr::new("LetStmt"),
            Arc::new(vec![Value::Symbol(name.clone()), expr_to_value(expr)]),
        ),

        Expr::Assignment(target, value) => Value::Tagged(
            SmolStr::new("Assignment"),
            Arc::new(vec![expr_to_value(target), expr_to_value(value)]),
        ),

        Expr::List(items) => Value::Tagged(
            SmolStr::new("List"),
            Arc::new(vec![Value::List(Arc::new(
                items.iter().map(expr_to_value).collect(),
            ))]),
        ),

        Expr::ListCons(head, tail) => Value::Tagged(
            SmolStr::new("ListCons"),
            Arc::new(vec![expr_to_value(head), expr_to_value(tail)]),
        ),

        Expr::Map(entries) => Value::Tagged(
            SmolStr::new("Map"),
            Arc::new(vec![Value::List(Arc::new(
                entries
                    .iter()
                    .map(|e| match e {
                        MapEntry::Symbol(key, value) => Value::List(Arc::new(vec![
                            Value::Symbol(key.clone()),
                            expr_to_value(value),
                        ])),
                        MapEntry::Computed(key, value) => {
                            Value::List(Arc::new(vec![expr_to_value(key), expr_to_value(value)]))
                        }
                    })
                    .collect(),
            ))]),
        ),

        Expr::Index(obj, idx) => Value::Tagged(
            SmolStr::new("Index"),
            Arc::new(vec![expr_to_value(obj), expr_to_value(idx)]),
        ),

        Expr::Slice(obj, start, end) => Value::Tagged(
            SmolStr::new("Slice"),
            Arc::new(vec![
                expr_to_value(obj),
                expr_to_value(start),
                expr_to_value(end),
            ]),
        ),

        Expr::While(cond, body) => Value::Tagged(
            SmolStr::new("While"),
            Arc::new(vec![expr_to_value(cond), expr_to_value(body)]),
        ),

        Expr::DoWhile(body, cond) => Value::Tagged(
            SmolStr::new("DoWhile"),
            Arc::new(vec![expr_to_value(body), expr_to_value(cond)]),
        ),

        Expr::For(pattern, iterable, body) => Value::Tagged(
            SmolStr::new("For"),
            Arc::new(vec![
                pattern_to_value(pattern),
                expr_to_value(iterable),
                expr_to_value(body),
            ]),
        ),

        Expr::Return(expr) => Value::Tagged(
            SmolStr::new("Return"),
            Arc::new(vec![
                expr.as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ]),
        ),

        Expr::Self_ => Value::Tagged(SmolStr::new("Self"), Arc::new(vec![])),
        Expr::Parent => Value::Tagged(SmolStr::new("Parent"), Arc::new(vec![])),
        Expr::Caller => Value::Tagged(SmolStr::new("Caller"), Arc::new(vec![])),
        Expr::User => Value::Tagged(SmolStr::new("User"), Arc::new(vec![])),
        Expr::Args => Value::Tagged(SmolStr::new("Args"), Arc::new(vec![])),

        Expr::AsyncCall(e) => {
            Value::Tagged(SmolStr::new("AsyncCall"), Arc::new(vec![expr_to_value(e)]))
        }

        Expr::SyncCall(e) => {
            Value::Tagged(SmolStr::new("SyncCall"), Arc::new(vec![expr_to_value(e)]))
        }

        Expr::FacetAccess(obj, facet) => Value::Tagged(
            SmolStr::new("FacetAccess"),
            Arc::new(vec![expr_to_value(obj), Value::Symbol(facet.clone())]),
        ),

        Expr::Spawn(expr, args) => Value::Tagged(
            SmolStr::new("Spawn"),
            Arc::new(vec![
                expr_to_value(expr),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ]),
        ),

        Expr::ObjectDef(def) => Value::Tagged(
            SmolStr::new("ObjectDef"),
            Arc::new(vec![
                // Object name as qualified name
                Value::List(Arc::new(
                    def.name
                        .parts
                        .iter()
                        .map(|p| Value::Symbol(p.clone()))
                        .collect(),
                )),
                // Parameters
                Value::List(Arc::new(
                    def.params
                        .iter()
                        .map(|p| Value::Symbol(p.clone()))
                        .collect(),
                )),
                // Parents
                Value::List(Arc::new(
                    def.parents
                        .iter()
                        .map(|p| {
                            Value::List(Arc::new(
                                p.parts.iter().map(|s| Value::Symbol(s.clone())).collect(),
                            ))
                        })
                        .collect(),
                )),
            ]),
        ),

        Expr::Match(expr, cases) => Value::Tagged(
            SmolStr::new("Match"),
            Arc::new(vec![
                expr_to_value(expr),
                Value::List(Arc::new(
                    cases
                        .iter()
                        .map(|c| {
                            Value::Tagged(
                                SmolStr::new("Case"),
                                Arc::new(vec![
                                    pattern_to_value(&c.pattern),
                                    c.guard
                                        .as_ref()
                                        .map(|g| expr_to_value(g))
                                        .unwrap_or(Value::Null),
                                    expr_to_value(&c.body),
                                ]),
                            )
                        })
                        .collect(),
                )),
            ]),
        ),

        Expr::TryCatch {
            body,
            error_binding,
            catch_body,
        } => Value::Tagged(
            SmolStr::new("TryCatch"),
            Arc::new(vec![
                expr_to_value(body),
                Value::Symbol(error_binding.clone()),
                expr_to_value(catch_body),
            ]),
        ),

        Expr::Throw(expr) => {
            Value::Tagged(SmolStr::new("Throw"), Arc::new(vec![expr_to_value(expr)]))
        }

        Expr::Sequence(exprs) => Value::Tagged(
            SmolStr::new("Sequence"),
            Arc::new(vec![Value::List(Arc::new(
                exprs.iter().map(expr_to_value).collect(),
            ))]),
        ),

        // Catch-all for less common cases
        _ => Value::Tagged(
            SmolStr::new("Unknown"),
            Arc::new(vec![Value::String(SmolStr::new(format!("{:?}", expr)))]),
        ),
    }
}

fn arg_to_value(arg: &Arg) -> Value {
    match arg {
        Arg::Expr(e) => expr_to_value(e),
        Arg::Placeholder => Value::Tagged(SmolStr::new("Placeholder"), Arc::new(vec![])),
    }
}

fn let_binding_to_value(binding: &LetBinding) -> Value {
    match binding {
        LetBinding::Simple(name, expr) => Value::Tagged(
            SmolStr::new("Binding"),
            Arc::new(vec![
                Value::Symbol(name.clone()),
                expr.as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ]),
        ),
        LetBinding::Destructure(pat, expr) => Value::Tagged(
            SmolStr::new("Destructure"),
            Arc::new(vec![pattern_to_value(pat), expr_to_value(expr)]),
        ),
    }
}

fn pattern_to_value(pat: &Pattern) -> Value {
    match pat {
        Pattern::Var(name) => Value::Tagged(
            SmolStr::new("PatVar"),
            Arc::new(vec![Value::Symbol(name.clone())]),
        ),
        Pattern::Wildcard => Value::Tagged(SmolStr::new("PatWildcard"), Arc::new(vec![])),
        Pattern::Int(n) => Value::Tagged(SmolStr::new("PatInt"), Arc::new(vec![Value::Int(*n)])),
        Pattern::Float(n) => {
            Value::Tagged(SmolStr::new("PatFloat"), Arc::new(vec![Value::Float(*n)]))
        }
        Pattern::String(s) => Value::Tagged(
            SmolStr::new("PatString"),
            Arc::new(vec![Value::String(s.clone())]),
        ),
        Pattern::Symbol(s) => Value::Tagged(
            SmolStr::new("PatSymbol"),
            Arc::new(vec![Value::Symbol(s.clone())]),
        ),
        Pattern::List(pats, tail) => Value::Tagged(
            SmolStr::new("PatList"),
            Arc::new(vec![
                Value::List(Arc::new(pats.iter().map(pattern_to_value).collect())),
                tail.as_ref()
                    .map(|t| Value::Symbol(t.clone()))
                    .unwrap_or(Value::Null),
            ]),
        ),
        Pattern::Map(entries) => Value::Tagged(
            SmolStr::new("PatMap"),
            Arc::new(
                entries
                    .iter()
                    .map(|(k, v)| {
                        Value::List(Arc::new(vec![
                            Value::Symbol(k.clone()),
                            pattern_to_value(v),
                        ]))
                    })
                    .collect(),
            ),
        ),
        Pattern::Constructor(tag, pats) => Value::Tagged(
            SmolStr::new("PatConstructor"),
            Arc::new(vec![
                Value::Symbol(tag.clone()),
                Value::List(Arc::new(pats.iter().map(pattern_to_value).collect())),
            ]),
        ),
        Pattern::As(pat, name) => Value::Tagged(
            SmolStr::new("PatAs"),
            Arc::new(vec![pattern_to_value(pat), Value::Symbol(name.clone())]),
        ),
    }
}

fn binop_to_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::LtEq => "<=",
        BinOp::GtEq => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Pipe => "|>",
        BinOp::In => " in ",
    }
}

fn unaryop_to_str(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

/// Parse FMPL source code and return AST as tagged values.
pub fn parse(source: &str) -> Result<Value> {
    let lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(&tokens);
    let expr = parser.parse()?;
    Ok(expr_to_value(&expr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int() {
        let result = parse("42").unwrap();
        assert!(matches!(result, Value::Tagged(tag, _) if tag == "Int"));
    }

    #[test]
    fn test_parse_binary() {
        let result = parse("1 + 2").unwrap();
        if let Value::Tagged(tag, children) = result {
            assert_eq!(tag.as_str(), "Binary");
            assert_eq!(children.len(), 3);
        } else {
            panic!("expected Tagged");
        }
    }

    #[test]
    fn test_parse_lambda() {
        let result = parse("\\x x + 1").unwrap();
        assert!(matches!(result, Value::Tagged(tag, _) if tag == "Lambda"));
    }

    #[test]
    fn test_parse_let() {
        let result = parse("let (x = 1) x + 1").unwrap();
        assert!(matches!(result, Value::Tagged(tag, _) if tag == "Let"));
    }
}
