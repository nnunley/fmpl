//! Convert Value::Tagged AST to typed Expr AST
//!
//! This module provides the conversion layer between the generated parser's
//! Value::Tagged output and the typed Expr AST used by the compiler.
//!
//! The generated parser produces Value::Tagged nodes like:
//!   - :Int(42)
//!   - :Binary(:+, :Int(1), :Int(2))
//!   - :Let([bindings], body)
//!
//! This module converts them to the typed Expr enum used elsewhere.

use crate::ast::*;
use crate::error::Error;
use crate::grammar::{Grammar, Rule};
use crate::pattern::{CharPattern, CharRange, Pattern as GrammarPattern, RepeatKind};
use crate::value::Value;
use smol_str::SmolStr;

type Result<T> = std::result::Result<T, Error>;

/// Convert a Value (tagged AST) to an Expr (typed AST).
/// Supports both formats:
/// - Old: Value::Tagged(tag, children)
/// - New: Value::List([Symbol(tag), ...children])
pub fn value_to_expr(value: &Value) -> Result<Expr> {
    // Helper to extract tag and children from either format
    let (tag, children) = match value {
        Value::List(items) if !items.is_empty() => {
            if let Value::Symbol(tag) = &items[0] {
                let children: Vec<Value> = items.iter().skip(1).cloned().collect();
                (tag.clone(), children)
            } else {
                return Err(Error::Runtime(format!(
                    "Expected list starting with symbol, got {:?}",
                    value
                )));
            }
        }
        _ => {
            return Err(Error::Runtime(format!(
                "Expected tagged value or list, got {:?}",
                value
            )));
        }
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
                    let s: String = chars
                        .iter()
                        .filter_map(|v| {
                            if let Value::String(s) = v {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    Ok(Expr::String(SmolStr::new(&s)))
                }
                Some(Value::String(s)) => {
                    // Already a joined string
                    Ok(Expr::String(s.clone()))
                }
                _ => Err(Error::Runtime("Invalid String node".to_string())),
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
                                _ => {
                                    return Err(Error::Runtime(
                                        "Map key must be string".to_string(),
                                    ));
                                }
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
                    ps.iter()
                        .filter_map(|p| {
                            if let Value::Symbol(s) = p {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect()
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
                Ok(Expr::If(
                    Box::new(cond),
                    Box::new(then_branch),
                    Some(Box::new(else_branch)),
                ))
            } else {
                Err(Error::Runtime("Invalid If node".to_string()))
            }
        }
        "Fold" => {
            // :Fold(func, init, iter) - left fold
            // func is typically a lambda like \acc \elem ...
            // Convert to Expr::Fold { initial, acc_var, iterable, body }
            if children.len() >= 3 {
                let func = value_to_expr(&children[0])?;
                let initial = value_to_expr(&children[1])?;
                let iterable = value_to_expr(&children[2])?;
                Ok(Expr::Fold {
                    initial: Box::new(initial),
                    acc_var: SmolStr::new("_acc"),
                    iterable: Box::new(iterable),
                    body: Box::new(func),
                })
            } else {
                Err(Error::Runtime("Invalid Fold node".to_string()))
            }
        }
        "Foldr" => {
            // :Foldr(func, init, iter) - right fold
            if children.len() >= 3 {
                let func = value_to_expr(&children[0])?;
                let initial = value_to_expr(&children[1])?;
                let iterable = value_to_expr(&children[2])?;
                Ok(Expr::Foldr {
                    initial: Box::new(initial),
                    acc_var: SmolStr::new("_acc"),
                    iterable: Box::new(iterable),
                    body: Box::new(func),
                })
            } else {
                Err(Error::Runtime("Invalid Foldr node".to_string()))
            }
        }
        "Yield" => {
            // :Yield(expr) - yield value to sink in grammar apply
            if !children.is_empty() {
                let value = value_to_expr(&children[0])?;
                Ok(Expr::Yield(Box::new(value)))
            } else {
                Err(Error::Runtime("Invalid Yield node".to_string()))
            }
        }
        "Let" => {
            if children.len() >= 2 {
                if let Value::List(bindings) = &children[0] {
                    let mut let_bindings = Vec::new();
                    for binding in bindings.iter() {
                        // Support both Value::Tagged("Binding", ...) and [:Binding, ...]
                        let (tag, parts) = match binding {
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
                                let_bindings
                                    .push(LetBinding::Simple(name.clone(), Some(Box::new(value))));
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
        // LetSimple - let without parentheses, no explicit body (appears in Do block)
        // Use LetStmt which binds to current scope without PushScope/PopScope.
        // This allows top-level bindings to persist after evaluation.
        "LetSimple" => {
            if !children.is_empty() {
                // The binding is :Binding(name, value)
                let (tag, parts) = match &children[0] {
                    _ => return Err(Error::Runtime("Invalid LetSimple binding".to_string())),
                };
                if tag.as_str() == "Binding" && parts.len() >= 2 {
                    if let Value::Symbol(name) = &parts[0] {
                        let value = value_to_expr(&parts[1])?;
                        // Use LetStmt to bind to current scope (not a new nested scope)
                        Ok(Expr::LetStmt(name.clone(), Box::new(value)))
                    } else {
                        Err(Error::Runtime("Invalid LetSimple binding name".to_string()))
                    }
                } else {
                    Err(Error::Runtime(
                        "Invalid LetSimple binding format".to_string(),
                    ))
                }
            } else {
                Err(Error::Runtime("Invalid LetSimple node".to_string()))
            }
        }
        // Do - statement sequence: transform LetSimple statements into nested Let expressions
        "Do" => {
            if let Some(Value::List(stmts)) = children.first() {
                // Convert each statement, then chain LetSimple into nested Lets
                transform_do_to_nested_lets(stmts)
            } else {
                Err(Error::Runtime("Invalid Do node".to_string()))
            }
        }
        "QualifiedName" => {
            if let Some(Value::List(parts)) = children.first() {
                let names: Vec<SmolStr> = parts
                    .iter()
                    .filter_map(|p| {
                        if let Value::String(s) = p {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Expr::Qualified(QualifiedName { parts: names }))
            } else {
                Err(Error::Runtime("Invalid QualifiedName node".to_string()))
            }
        }
        "Call" | "Index" | "MethodCall" | "PropAccess" | "Slice" => {
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
                            let arg_exprs: Result<Vec<Expr>> =
                                args.iter().map(value_to_expr).collect();
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
                    let match_cases = cases
                        .iter()
                        .map(|c| {
                            let (tag, cs) = match c {
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
                            Ok(MatchCase {
                                pattern,
                                guard,
                                body: Box::new(body),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(Expr::Match(Box::new(scrutinee), match_cases))
                } else {
                    Err(Error::Runtime("Invalid Match cases".to_string()))
                }
            } else {
                Err(Error::Runtime("Invalid Match node".to_string()))
            }
        }
        "PatternAs" => Err(Error::Runtime(
            "PatternAs cannot be used as expression".to_string(),
        )),
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
                // The facet is a :Symbol(name) tagged value
                let facet = match &children[1] {
                    Value::Symbol(s) => s.clone(),
                    v if matches!(v.as_node(), Some((t, _)) if t.as_str() == "Symbol") => {
                        let (_, inner) = v.as_node().unwrap();
                        if let Some(Value::String(s)) = inner.first() {
                            SmolStr::new(s.as_str())
                        } else if let Some(Value::Symbol(s)) = inner.first() {
                            s.clone()
                        } else {
                            return Err(Error::Runtime(
                                "Invalid FacetAccess facet name".to_string(),
                            ));
                        }
                    }
                    _ => return Err(Error::Runtime("Invalid FacetAccess facet".to_string())),
                };
                Ok(Expr::FacetAccess(Box::new(obj), facet))
            } else {
                Err(Error::Runtime("Invalid FacetAccess node".to_string()))
            }
        }
        // Object(name, content_items) -> Expr::ObjectDef
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
                if let Some((tag, cs)) = item.as_node() {
                    match tag.as_str() {
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
                                    if let Some(binding) = value_to_obj_binding(b, vis)? {
                                        bindings.push(binding);
                                    }
                                }
                            }
                        }
                        "FacetSection" if !cs.is_empty() => {
                            if let Value::List(items) = &cs[0] {
                                for f in items.iter() {
                                    if let Some(facet) = value_to_facet_def(f)? {
                                        facets.push(facet);
                                    }
                                }
                            }
                        }
                        "ObjBinding" => {
                            // Top-level binding (no visibility section) defaults to Private
                            if let Some(binding) = value_to_obj_binding(item, Visibility::Private)?
                            {
                                bindings.push(binding);
                            }
                        }
                        _ => {}
                    }
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
        // AtInlineBlock(input, InlinePatternBlock(cases)) -> Expr::InlinePatternBlock
        "AtInlineBlock" if children.len() >= 2 => {
            let input = value_to_expr(&children[0])?;
            let cases = value_to_pattern_cases(&children[1])?;
            Ok(Expr::InlinePatternBlock {
                input: Box::new(input),
                cases,
            })
        }
        // AtGrammarApply(input, grammar, rule) -> Expr::GrammarApply
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
        // MapEach(func, iterable) -> Expr::MapEach
        "MapEach" if children.len() >= 2 => {
            let func = value_to_expr(&children[0])?;
            let iterable = value_to_expr(&children[1])?;
            Ok(Expr::MapEach {
                elem_var: SmolStr::new("_elem"),
                iterable: Box::new(iterable),
                body: Box::new(func),
            })
        }
        // FilterExpr(pred, iterable) -> Expr::Filter
        "FilterExpr" if children.len() >= 2 => {
            let pred = value_to_expr(&children[0])?;
            let iterable = value_to_expr(&children[1])?;
            Ok(Expr::Filter {
                elem_var: SmolStr::new("_elem"),
                iterable: Box::new(iterable),
                body: Box::new(pred),
            })
        }
        // GrammarExtend(base, rules) -> Expr::GrammarExtend
        "GrammarExtend" if children.len() >= 2 => {
            let base = value_to_expr(&children[0])?;
            let mut grammar = Grammar::new(SmolStr::new(""));
            if let Value::List(rules) = &children[1] {
                for rule_val in rules.iter() {
                    if let Some((tag, rule_children)) = rule_val.as_node() {
                        if tag.as_str() == "Rule" && rule_children.len() >= 2 {
                            let rule_name = match &rule_children[0] {
                                Value::String(s) => SmolStr::new(s.as_str()),
                                _ => continue,
                            };
                            let pattern = value_to_grammar_pattern(&rule_children[1])?;
                            let backtracking = if rule_children.len() > 2 {
                                matches!(&rule_children[2], Value::Bool(true))
                            } else {
                                false
                            };
                            grammar.add_rule(
                                rule_name,
                                Rule {
                                    pattern,
                                    backtracking,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
            Ok(Expr::GrammarExtend {
                base: Box::new(base),
                rules: grammar,
            })
        }
        // GrammarDef(name, parent, rules) -> Expr::GrammarLiteral
        // Now rules is a list of :Rule(name, pattern, backtracking) values
        "GrammarDef" if !children.is_empty() => {
            // Extract grammar name
            let name = match &children[0] {
                Value::String(s) => SmolStr::new(s.as_str()),
                _ => return Err(Error::Runtime("Invalid GrammarDef name".to_string())),
            };

            // Extract optional parent (may be null or a string)
            let parent = if children.len() > 1 {
                match &children[1] {
                    Value::String(s) if !s.is_empty() => Some(SmolStr::new(s.as_str())),
                    Value::List(items) if items.len() == 1 => {
                        if let Value::String(s) = &items[0] {
                            Some(SmolStr::new(s.as_str()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };

            // Build the grammar from the parsed rules
            let mut grammar = match parent {
                Some(p) => Grammar::with_parent(name, p),
                None => Grammar::new(name),
            };

            // Extract rules from children[2]
            if children.len() > 2 {
                match &children[2] {
                    Value::List(rules) => {
                        for rule_val in rules.iter() {
                            if let Some((tag, rule_children)) = rule_val.as_node() {
                                if tag.as_str() == "Rule" && rule_children.len() >= 2 {
                                    let rule_name = match &rule_children[0] {
                                        Value::String(s) => SmolStr::new(s.as_str()),
                                        _ => continue,
                                    };
                                    let pattern = value_to_grammar_pattern(&rule_children[1])?;
                                    let backtracking = if rule_children.len() > 2 {
                                        matches!(&rule_children[2], Value::Bool(true))
                                    } else {
                                        false
                                    };
                                    grammar.add_rule(
                                        rule_name,
                                        Rule {
                                            pattern,
                                            backtracking,
                                            ..Default::default()
                                        },
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            Ok(Expr::GrammarLiteral(grammar))
        }
        _ => Err(Error::Runtime(format!("Unknown AST node type: {}", tag))),
    }
}

/// Convert a Value representing a PEG pattern to a GrammarPattern struct
/// Handles the AST nodes produced by peg_* rules in fmpl_parser.fmpl
fn value_to_grammar_pattern(value: &Value) -> Result<GrammarPattern> {
    let (tag, children) = match value {
        // Simple pattern types
        Value::String(s) => {
            // String literal in pattern
            if s.len() == 1 {
                return Ok(GrammarPattern::Char(CharPattern::Exact(
                    s.chars().next().unwrap(),
                )));
            }
            return Ok(GrammarPattern::StringLiteral(SmolStr::new(s.as_str())));
        }
        _ => {
            return Err(Error::Runtime(format!(
                "Expected pattern AST, got {:?}",
                value
            )));
        }
    };

    match tag {
        "Any" => Ok(GrammarPattern::Any),
        "StringLit" => {
            // Extract the string value - could be a plain String or a :String(value) tagged value
            let s = match children.first() {
                Some(Value::String(s)) => s.clone(),
                Some(v) if matches!(v.as_node(), Some((t, _)) if t.as_str() == "String") => {
                    // :String(value) - extract the inner string
                    let (_, inner) = v.as_node().unwrap();
                    if let Some(Value::String(s)) = inner.first() {
                        s.clone()
                    } else {
                        return Err(Error::Runtime(
                            "Invalid StringLit: inner String has no value".to_string(),
                        ));
                    }
                }
                _ => return Err(Error::Runtime("Invalid StringLit".to_string())),
            };
            if s.len() == 1 {
                Ok(GrammarPattern::Char(CharPattern::Exact(
                    s.chars().next().unwrap(),
                )))
            } else {
                Ok(GrammarPattern::StringLiteral(SmolStr::new(s.as_str())))
            }
        }
        "CharLit" => {
            if let Some(Value::String(s)) = children.first() {
                let c = s
                    .chars()
                    .next()
                    .ok_or_else(|| Error::Runtime("Empty CharLit".to_string()))?;
                Ok(GrammarPattern::Char(CharPattern::Exact(c)))
            } else {
                Err(Error::Runtime("Invalid CharLit".to_string()))
            }
        }
        "Class" => {
            if let Some(Value::List(ranges)) = children.first() {
                let char_ranges = value_to_char_ranges(ranges)?;
                Ok(GrammarPattern::Char(CharPattern::Class(char_ranges)))
            } else {
                Err(Error::Runtime("Invalid Class".to_string()))
            }
        }
        "NegatedClass" => {
            if let Some(Value::List(ranges)) = children.first() {
                let char_ranges = value_to_char_ranges(ranges)?;
                Ok(GrammarPattern::Char(CharPattern::NegatedClass(char_ranges)))
            } else {
                Err(Error::Runtime("Invalid NegatedClass".to_string()))
            }
        }
        "RuleRef" => {
            if let Some(Value::String(name)) = children.first() {
                Ok(GrammarPattern::ApplyRule(SmolStr::new(name.as_str())))
            } else {
                Err(Error::Runtime("Invalid RuleRef".to_string()))
            }
        }
        "Super" => {
            if let Some(Value::String(name)) = children.first() {
                Ok(GrammarPattern::Super(SmolStr::new(name.as_str())))
            } else {
                Err(Error::Runtime("Invalid Super".to_string()))
            }
        }
        "Lookahead" => {
            if let Some(inner) = children.first() {
                Ok(GrammarPattern::Lookahead(Box::new(
                    value_to_grammar_pattern(inner)?,
                )))
            } else {
                Err(Error::Runtime("Invalid Lookahead".to_string()))
            }
        }
        "Not" => {
            if let Some(inner) = children.first() {
                Ok(GrammarPattern::Not(Box::new(value_to_grammar_pattern(
                    inner,
                )?)))
            } else {
                Err(Error::Runtime("Invalid Not".to_string()))
            }
        }
        "Star" => {
            if let Some(inner) = children.first() {
                Ok(GrammarPattern::Repeat {
                    pattern: Box::new(value_to_grammar_pattern(inner)?),
                    kind: RepeatKind::ZeroOrMore,
                })
            } else {
                Err(Error::Runtime("Invalid Star".to_string()))
            }
        }
        "Plus" => {
            if let Some(inner) = children.first() {
                Ok(GrammarPattern::Repeat {
                    pattern: Box::new(value_to_grammar_pattern(inner)?),
                    kind: RepeatKind::OneOrMore,
                })
            } else {
                Err(Error::Runtime("Invalid Plus".to_string()))
            }
        }
        "Optional" => {
            if let Some(inner) = children.first() {
                Ok(GrammarPattern::Optional(Box::new(
                    value_to_grammar_pattern(inner)?,
                )))
            } else {
                Err(Error::Runtime("Invalid Optional".to_string()))
            }
        }
        "Bind" => {
            if children.len() >= 2 {
                let inner = value_to_grammar_pattern(&children[0])?;
                let name = match &children[1] {
                    Value::Symbol(s) => SmolStr::new(s.as_str()),
                    Value::String(s) => SmolStr::new(s.as_str()),
                    _ => return Err(Error::Runtime("Invalid Bind name".to_string())),
                };
                Ok(GrammarPattern::Bind {
                    pattern: Box::new(inner),
                    name,
                    is_choice: false,
                })
            } else {
                Err(Error::Runtime("Invalid Bind".to_string()))
            }
        }
        "BindChoice" => {
            // Choice point binding: digit:?x
            if children.len() >= 2 {
                let inner = value_to_grammar_pattern(&children[0])?;
                let name = match &children[1] {
                    Value::Symbol(s) => SmolStr::new(s.as_str()),
                    Value::String(s) => SmolStr::new(s.as_str()),
                    _ => return Err(Error::Runtime("Invalid BindChoice name".to_string())),
                };
                Ok(GrammarPattern::Bind {
                    pattern: Box::new(inner),
                    name,
                    is_choice: true,
                })
            } else {
                Err(Error::Runtime("Invalid BindChoice".to_string()))
            }
        }
        "Guard" => {
            // when guard on a pattern
            if children.len() >= 2 {
                let pattern = value_to_grammar_pattern(&children[0])?;
                let predicate = value_to_expr(&children[1])?;
                Ok(GrammarPattern::Guard {
                    pattern: Box::new(pattern),
                    predicate,
                })
            } else {
                Err(Error::Runtime("Invalid Guard".to_string()))
            }
        }
        "Seq" => {
            if let Some(Value::List(items)) = children.first() {
                let patterns: Result<Vec<GrammarPattern>> =
                    items.iter().map(value_to_grammar_pattern).collect();
                Ok(GrammarPattern::Seq(patterns?))
            } else {
                Err(Error::Runtime("Invalid Seq".to_string()))
            }
        }
        "Choice" => {
            if let Some(Value::List(items)) = children.first() {
                let patterns: Result<Vec<(GrammarPattern, bool)>> = items
                    .iter()
                    .map(|p| Ok((value_to_grammar_pattern(p)?, false)))
                    .collect();
                Ok(GrammarPattern::Choice(patterns?))
            } else {
                Err(Error::Runtime("Invalid Choice".to_string()))
            }
        }
        "Action" => {
            if children.len() >= 2 {
                let pattern = value_to_grammar_pattern(&children[0])?;
                let action = value_to_expr(&children[1])?;
                Ok(GrammarPattern::Action {
                    pattern: Box::new(pattern),
                    action,
                })
            } else {
                Err(Error::Runtime("Invalid Action".to_string()))
            }
        }
        "Predicate" => {
            if let Some(expr_val) = children.first() {
                let expr = value_to_expr(expr_val)?;
                Ok(GrammarPattern::Predicate(expr))
            } else {
                Err(Error::Runtime("Invalid Predicate".to_string()))
            }
        }
        _ => Err(Error::Runtime(format!("Unknown pattern type: {}", tag))),
    }
}

/// Convert a list of :Char or :Range values to CharRange vec
fn value_to_char_ranges(ranges: &[Value]) -> Result<Vec<CharRange>> {
    ranges
        .iter()
        .map(|r| {
            let (tag, children) = match r {
                _ => {
                    return Err(Error::Runtime(format!(
                        "Expected Char or Range, got {:?}",
                        r
                    )));
                }
            };
            match tag {
                "Char" => {
                    if let Some(Value::String(s)) = children.first() {
                        let c = s
                            .chars()
                            .next()
                            .ok_or_else(|| Error::Runtime("Empty Char".to_string()))?;
                        Ok(CharRange::Char(c))
                    } else {
                        Err(Error::Runtime("Invalid Char".to_string()))
                    }
                }
                "Range" => {
                    if children.len() >= 2 {
                        let start = match &children[0] {
                            Value::String(s) => s
                                .chars()
                                .next()
                                .ok_or_else(|| Error::Runtime("Empty Range start".to_string()))?,
                            _ => return Err(Error::Runtime("Invalid Range start".to_string())),
                        };
                        let end = match &children[1] {
                            Value::String(s) => s
                                .chars()
                                .next()
                                .ok_or_else(|| Error::Runtime("Empty Range end".to_string()))?,
                            _ => return Err(Error::Runtime("Invalid Range end".to_string())),
                        };
                        Ok(CharRange::Range(start, end))
                    } else {
                        Err(Error::Runtime("Invalid Range".to_string()))
                    }
                }
                _ => Err(Error::Runtime(format!(
                    "Expected Char or Range, got {}",
                    tag
                ))),
            }
        })
        .collect()
}

/// Convert :InlinePatternBlock(cases) to Vec<PatternCase>
fn value_to_pattern_cases(value: &Value) -> Result<Vec<PatternCase>> {
    let (tag, children) = match value {
        _ => {
            return Err(Error::Runtime(format!(
                "Expected InlinePatternBlock, got {:?}",
                value
            )));
        }
    };

    if tag != "InlinePatternBlock" {
        return Err(Error::Runtime(format!(
            "Expected InlinePatternBlock, got {}",
            tag
        )));
    }

    if let Some(Value::List(cases)) = children.first() {
        cases.iter().map(value_to_pattern_case).collect()
    } else {
        Err(Error::Runtime(
            "Invalid InlinePatternBlock cases".to_string(),
        ))
    }
}

/// Convert :PatternCase(pattern, action) to PatternCase
fn value_to_pattern_case(value: &Value) -> Result<PatternCase> {
    let (tag, children) = match value {
        _ => {
            return Err(Error::Runtime(format!(
                "Expected PatternCase, got {:?}",
                value
            )));
        }
    };

    if tag != "PatternCase" {
        return Err(Error::Runtime(format!("Expected PatternCase, got {}", tag)));
    }

    if children.len() < 2 {
        return Err(Error::Runtime(
            "Invalid PatternCase: need pattern and action".to_string(),
        ));
    }

    let pattern = value_to_pattern(&children[0])?;
    let (guard, body) = value_to_pattern_action(&children[1])?;

    Ok(PatternCase {
        pattern,
        guard,
        body: Box::new(body),
    })
}

/// Convert :PatternCaseSimple(body) or :PatternCaseGuard(guard, body) to (Option<Expr>, Expr)
fn value_to_pattern_action(value: &Value) -> Result<(Option<Box<Expr>>, Expr)> {
    let (tag, children) = match value {
        _ => {
            return Err(Error::Runtime(format!(
                "Expected pattern action, got {:?}",
                value
            )));
        }
    };

    match tag {
        "PatternCaseSimple" if !children.is_empty() => {
            let body = value_to_expr(&children[0])?;
            Ok((None, body))
        }
        "PatternCaseGuard" if children.len() >= 2 => {
            let guard = value_to_expr(&children[0])?;
            let body = value_to_expr(&children[1])?;
            Ok((Some(Box::new(guard)), body))
        }
        _ => Err(Error::Runtime(format!("Invalid pattern action: {}", tag))),
    }
}

/// Convert pattern values to ast::Pattern
fn value_to_pattern(value: &Value) -> Result<Pattern> {
    let (tag, children) = match value {
        _ => return Err(Error::Runtime(format!("Expected pattern, got {:?}", value))),
    };

    match tag {
        "PatternWildcard" => Ok(Pattern::Wildcard),
        "PatternVar" if !children.is_empty() => {
            if let Value::Symbol(name) = &children[0] {
                Ok(Pattern::Var(name.clone()))
            } else {
                Err(Error::Runtime("Invalid PatternVar name".to_string()))
            }
        }
        "PatternLiteral" if !children.is_empty() => {
            // The child is an AST node like :Int(n), :Bool(b), etc.
            value_to_literal_pattern(&children[0])
        }
        "PatternList" if !children.is_empty() => {
            if let Value::List(items) = &children[0] {
                let patterns = items
                    .iter()
                    .map(value_to_pattern)
                    .collect::<Result<Vec<_>>>()?;
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

/// Convert a literal AST node to a Pattern
fn value_to_literal_pattern(value: &Value) -> Result<Pattern> {
    let (tag, children) = match value {
        _ => return Err(Error::Runtime(format!("Expected literal, got {:?}", value))),
    };

    match tag {
        "Int" if !children.is_empty() => {
            if let Value::Int(n) = &children[0] {
                Ok(Pattern::Int(*n))
            } else {
                Err(Error::Runtime("Invalid Int literal".to_string()))
            }
        }
        "Bool" if !children.is_empty() => {
            // For bool patterns, we use Int(1) for true, Int(0) for false
            if let Value::Bool(b) = &children[0] {
                Ok(Pattern::Int(if *b { 1 } else { 0 }))
            } else {
                Err(Error::Runtime("Invalid Bool literal".to_string()))
            }
        }
        "Null" => {
            // Null pattern - using a special representation
            Ok(Pattern::Wildcard) // Simplified for now
        }
        "String" if !children.is_empty() => match &children[0] {
            Value::String(s) => Ok(Pattern::String(s.clone())),
            Value::List(chars) => {
                let s: String = chars
                    .iter()
                    .filter_map(|v| {
                        if let Value::String(s) = v {
                            Some(s.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(Pattern::String(SmolStr::new(&s)))
            }
            _ => Err(Error::Runtime("Invalid String literal".to_string())),
        },
        _ => Err(Error::Runtime(format!(
            "Unknown literal pattern type: {}",
            tag
        ))),
    }
}

/// Transform a Do block (statement sequence) into nested Let expressions.
/// This handles the case where let bindings in a sequence need to be available
/// to subsequent statements.
///
/// For example:
///   let x = 1
///   let y = 2
///   x + y
///
/// Becomes:
///   Sequence([LetStmt(x, 1), LetStmt(y, 2), x + y])
///
/// All LetSimple statements become LetStmt (which binds to current scope without
/// creating a new nested scope). This allows bindings to persist after evaluation,
/// which is essential for file loading to work correctly.
fn transform_do_to_nested_lets(stmts: &[Value]) -> Result<Expr> {
    if stmts.is_empty() {
        return Ok(Expr::Null);
    }

    // Convert all statements, using LetStmt for LetSimple
    let mut exprs = Vec::new();

    for stmt in stmts {
        let (tag, children) = match stmt {
            _ => {
                // Not a tagged value, try to convert it directly
                exprs.push(value_to_expr(stmt)?);
                continue;
            }
        };

        if tag == "LetSimple" {
            // Extract binding from LetSimple
            if !children.is_empty() {
                let (btag, parts) = match &children[0] {
                    _ => {
                        // Not a proper binding, treat as expression
                        exprs.push(value_to_expr(stmt)?);
                        continue;
                    }
                };

                if btag == "Binding" && parts.len() >= 2 {
                    if let Value::Symbol(name) = &parts[0] {
                        let value = value_to_expr(&parts[1])?;
                        // Use LetStmt to bind to current scope
                        exprs.push(Expr::LetStmt(name.clone(), Box::new(value)));
                        continue;
                    }
                }
            }
            // Fallback: treat as regular expression
            exprs.push(value_to_expr(stmt)?);
        } else {
            // Not a LetSimple, convert normally
            exprs.push(value_to_expr(stmt)?);
        }
    }

    // Return single expression or sequence
    if exprs.len() == 1 {
        Ok(exprs.pop().unwrap())
    } else {
        Ok(Expr::Sequence(exprs))
    }
}

/// Convert an :ObjBinding tagged value to a Binding
fn value_to_obj_binding(value: &Value, visibility: Visibility) -> Result<Option<Binding>> {
    let (tag, children) = match value {
        _ => return Ok(None),
    };

    if tag != "ObjBinding" || children.len() < 4 {
        return Ok(None);
    }

    let name = match &children[0] {
        Value::String(s) => SmolStr::new(s.as_str()),
        _ => return Err(Error::Runtime("Invalid ObjBinding name".to_string())),
    };

    let params = match &children[1] {
        Value::List(ps) => ps
            .iter()
            .filter_map(|p| {
                if let Value::Symbol(s) = p {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let has_params = match &children[3] {
        Value::Bool(b) => *b,
        _ => false,
    };

    let value_expr = value_to_expr(&children[2])?;

    Ok(Some(Binding {
        name,
        params,
        has_params,
        value: value_expr,
        visibility,
    }))
}

/// Convert a :FacetDef tagged value to a FacetDef
fn value_to_facet_def(value: &Value) -> Result<Option<FacetDef>> {
    let (tag, children) = match value {
        _ => return Ok(None),
    };

    if tag != "FacetDef" || children.len() < 3 {
        return Ok(None);
    }

    let name = match &children[0] {
        Value::String(s) => SmolStr::new(s.as_str()),
        _ => return Err(Error::Runtime("Invalid FacetDef name".to_string())),
    };

    let members = match &children[1] {
        Value::List(ms) => ms
            .iter()
            .filter_map(|m| {
                if let Value::String(s) = m {
                    Some(SmolStr::new(s.as_str()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let terminal = match &children[2] {
        Value::Bool(b) => *b,
        _ => false,
    };

    Ok(Some(FacetDef {
        name,
        members,
        terminal,
    }))
}
