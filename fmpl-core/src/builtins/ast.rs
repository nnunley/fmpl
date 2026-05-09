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
        Expr::Int(n) => Value::list_node("Int", vec![Value::Int(*n)]),

        Expr::Float(n) => Value::list_node("Float", vec![Value::Float(*n)]),

        Expr::String(s) => Value::list_node("String", vec![Value::String(s.clone())]),

        Expr::Symbol(s) => Value::list_node("Symbol", vec![Value::Symbol(s.clone())]),

        Expr::Tagged(tag, children) => Value::list_node(
            "Tagged",
            vec![
                Value::Symbol(tag.clone()),
                Value::List(Arc::new(children.iter().map(expr_to_value).collect())),
            ],
        ),

        Expr::Bool(b) => Value::list_node("Bool", vec![Value::Bool(*b)]),

        Expr::Null => Value::list_node("Null", vec![]),

        Expr::Ident(name) => Value::list_node("Var", vec![Value::Symbol(name.clone())]),

        Expr::Qualified(qn) => Value::list_node(
            "Qualified",
            vec![Value::List(Arc::new(
                qn.parts.iter().map(|p| Value::Symbol(p.clone())).collect(),
            ))],
        ),

        Expr::Binary(lhs, op, rhs) => Value::list_node(
            "Binary",
            vec![
                Value::Symbol(SmolStr::new(binop_to_str(*op))),
                expr_to_value(lhs),
                expr_to_value(rhs),
            ],
        ),

        Expr::Unary(op, e) => Value::list_node(
            "Unary",
            vec![
                Value::Symbol(SmolStr::new(unaryop_to_str(*op))),
                expr_to_value(e),
            ],
        ),

        Expr::Lambda(params, body) => Value::list_node(
            "Lambda",
            vec![
                Value::List(Arc::new(
                    params.iter().map(|p| Value::Symbol(p.clone())).collect(),
                )),
                expr_to_value(body),
            ],
        ),

        Expr::ShortLambda(param, body) => Value::list_node(
            "Lambda",
            vec![
                Value::List(Arc::new(vec![Value::Symbol(param.clone())])),
                expr_to_value(body),
            ],
        ),

        Expr::Call(func, args) => Value::list_node(
            "Call",
            vec![
                expr_to_value(func),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ],
        ),

        Expr::MethodCall(receiver, method, args) => Value::list_node(
            "MethodCall",
            vec![
                expr_to_value(receiver),
                Value::Symbol(method.clone()),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ],
        ),

        Expr::PropAccess(obj, prop) => Value::list_node(
            "PropAccess",
            vec![expr_to_value(obj), Value::Symbol(prop.clone())],
        ),

        Expr::If(cond, then_branch, else_branch) => Value::list_node(
            "If",
            vec![
                expr_to_value(cond),
                expr_to_value(then_branch),
                else_branch
                    .as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ],
        ),

        Expr::Let(bindings, body) => {
            let binding_values: Vec<Value> = bindings.iter().map(let_binding_to_value).collect();
            Value::list_node(
                "Let",
                vec![Value::List(Arc::new(binding_values)), expr_to_value(body)],
            )
        }

        Expr::LetStmt(name, expr) => Value::list_node(
            "LetStmt",
            vec![Value::Symbol(name.clone()), expr_to_value(expr)],
        ),

        Expr::Assignment(target, value) => Value::list_node(
            "Assignment",
            vec![expr_to_value(target), expr_to_value(value)],
        ),

        Expr::List(items) => Value::list_node(
            "List",
            vec![Value::List(Arc::new(
                items.iter().map(expr_to_value).collect(),
            ))],
        ),

        Expr::ListCons(head, tail) => {
            Value::list_node("ListCons", vec![expr_to_value(head), expr_to_value(tail)])
        }

        Expr::Map(entries) => Value::list_node(
            "Map",
            vec![Value::List(Arc::new(
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
            ))],
        ),

        Expr::Index(obj, idx) => {
            Value::list_node("Index", vec![expr_to_value(obj), expr_to_value(idx)])
        }

        Expr::Slice(obj, start, end) => Value::list_node(
            "Slice",
            vec![
                expr_to_value(obj),
                start
                    .as_ref()
                    .map(|s| expr_to_value(s))
                    .unwrap_or(Value::Null),
                end.as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ],
        ),

        Expr::While(cond, body) => {
            Value::list_node("While", vec![expr_to_value(cond), expr_to_value(body)])
        }

        Expr::DoWhile(body, cond) => {
            Value::list_node("DoWhile", vec![expr_to_value(body), expr_to_value(cond)])
        }

        Expr::For(pattern, iterable, body) => Value::list_node(
            "For",
            vec![
                pattern_to_value(pattern),
                expr_to_value(iterable),
                expr_to_value(body),
            ],
        ),

        Expr::Return(expr) => Value::list_node(
            "Return",
            vec![
                expr.as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ],
        ),

        Expr::Self_ => Value::list_node("Self", vec![]),
        Expr::Parent => Value::list_node("Parent", vec![]),
        Expr::Caller => Value::list_node("Caller", vec![]),
        Expr::User => Value::list_node("User", vec![]),
        Expr::Args => Value::list_node("Args", vec![]),

        Expr::AsyncCall(e) => Value::list_node("AsyncCall", vec![expr_to_value(e)]),

        Expr::SyncCall(e) => Value::list_node("SyncCall", vec![expr_to_value(e)]),

        Expr::FacetAccess(obj, facet) => Value::list_node(
            "FacetAccess",
            vec![expr_to_value(obj), Value::Symbol(facet.clone())],
        ),

        Expr::Spawn(expr, args) => Value::list_node(
            "Spawn",
            vec![
                expr_to_value(expr),
                Value::List(Arc::new(args.iter().map(arg_to_value).collect())),
            ],
        ),

        Expr::ObjectDef(def) => Value::list_node(
            "ObjectDef",
            vec![
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
            ],
        ),

        Expr::Match(expr, cases) => Value::list_node(
            "Match",
            vec![
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
            ],
        ),

        Expr::TryCatch {
            body,
            error_binding,
            catch_body,
        } => Value::list_node(
            "TryCatch",
            vec![
                expr_to_value(body),
                Value::Symbol(error_binding.clone()),
                expr_to_value(catch_body),
            ],
        ),

        Expr::Throw(expr) => Value::list_node("Throw", vec![expr_to_value(expr)]),

        Expr::Sequence(exprs) => Value::list_node(
            "Sequence",
            vec![Value::List(Arc::new(
                exprs.iter().map(expr_to_value).collect(),
            ))],
        ),

        // Inline pattern block: x @ { pat => body, ... } is sugar for match
        Expr::InlinePatternBlock { input, cases } => Value::list_node(
            "Match",
            vec![
                expr_to_value(input),
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
            ],
        ),

        // Catch-all for less common cases
        _ => Value::list_node(
            "Unknown",
            vec![Value::String(SmolStr::new(format!("{:?}", expr)))],
        ),
    }
}

fn arg_to_value(arg: &Arg) -> Value {
    match arg {
        Arg::Expr(e) => expr_to_value(e),
        Arg::Placeholder => Value::list_node("Placeholder", vec![]),
    }
}

fn let_binding_to_value(binding: &LetBinding) -> Value {
    match binding {
        LetBinding::Simple(name, expr) => Value::list_node(
            "Binding",
            vec![
                Value::Symbol(name.clone()),
                expr.as_ref()
                    .map(|e| expr_to_value(e))
                    .unwrap_or(Value::Null),
            ],
        ),
        LetBinding::Destructure(pat, expr) => Value::list_node(
            "Destructure",
            vec![pattern_to_value(pat), expr_to_value(expr)],
        ),
    }
}

fn pattern_to_value(pat: &Pattern) -> Value {
    match pat {
        Pattern::Var(name) => Value::list_node("PatVar", vec![Value::Symbol(name.clone())]),
        Pattern::Wildcard => Value::list_node("PatWildcard", vec![]),
        Pattern::Int(n) => Value::list_node("PatInt", vec![Value::Int(*n)]),
        Pattern::Float(n) => Value::list_node("PatFloat", vec![Value::Float(*n)]),
        Pattern::String(s) => Value::list_node("PatString", vec![Value::String(s.clone())]),
        Pattern::Symbol(s) => Value::list_node("PatSymbol", vec![Value::Symbol(s.clone())]),
        Pattern::List(pats, tail) => Value::list_node(
            "PatList",
            vec![
                Value::List(Arc::new(pats.iter().map(pattern_to_value).collect())),
                tail.as_ref()
                    .map(|t| Value::Symbol(t.clone()))
                    .unwrap_or(Value::Null),
            ],
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
        Pattern::Constructor(tag, pats) => Value::list_node(
            "PatConstructor",
            vec![
                Value::Symbol(tag.clone()),
                Value::List(Arc::new(pats.iter().map(pattern_to_value).collect())),
            ],
        ),
        Pattern::As(pat, name) => Value::list_node(
            "PatAs",
            vec![pattern_to_value(pat), Value::Symbol(name.clone())],
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

    fn assert_node_tag(value: &Value, expected_tag: &str) {
        let (tag, _) = value.as_node().unwrap_or_else(|| {
            panic!(
                "expected list-shaped node {:?}, got {:?}",
                expected_tag, value
            )
        });
        assert_eq!(tag.as_str(), expected_tag);
    }

    #[test]
    fn test_parse_int() {
        let result = parse("42").unwrap();
        assert_node_tag(&result, "Int");
    }

    #[test]
    fn test_parse_binary() {
        let result = parse("1 + 2").unwrap();
        let (tag, children) = result.as_node().expect("list-shaped node");
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn test_parse_lambda() {
        let result = parse("\\x x + 1").unwrap();
        assert_node_tag(&result, "Lambda");
    }

    #[test]
    fn test_parse_let() {
        let result = parse("let (x = 1) x + 1").unwrap();
        assert_node_tag(&result, "Let");
    }
}
