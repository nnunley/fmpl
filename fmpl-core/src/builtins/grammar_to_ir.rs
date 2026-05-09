//! Grammar to parsing IR converter.
//!
//! Converts Grammar structures (from fmpl_parser.fmpl) into parsing IR
//! that can be transpiled to Rust by ir_to_rust.
//!
//! The parsing IR uses tagged values:
//! - :ParseChar(char) - match single character
//! - :ParseLiteral(string) - match literal string
//! - :ParseCharClass(ranges, negated) - character class
//! - :ParseAny - match any character
//! - :ParseSeq([ir...]) - sequence
//! - :ParseChoice([ir...]) - ordered choice
//! - :ParseStar(ir) - zero or more
//! - :ParsePlus(ir) - one or more
//! - :ParseOptional(ir) - zero or one
//! - :ParseNot(ir) - negative lookahead
//! - :ParseLookahead(ir) - positive lookahead
//! - :ParseRule(name) - call another rule
//! - :ParseBind(ir, name) - bind result to name
//! - :ParseAction(ir, expr) - semantic action
//! - :ParseGrammar(name, rules) - complete grammar
//! - :ParseRuleDef(name, body) - rule definition

use crate::ast::{Arg, BinOp, LetBinding, MapEntry, UnaryOp};
use crate::error::{Error, Result};
use crate::grammar::{Grammar, Rule};
use crate::pattern::{CharPattern, CharRange, Pattern, RepeatKind};
use crate::value::Value;
use smol_str::SmolStr;
use std::sync::Arc;

/// Convert a Grammar to parsing IR.
pub fn grammar_to_ir(grammar: &Grammar) -> Result<Value> {
    let mut rules = Vec::new();

    for (name, rule) in &grammar.rules {
        let body_ir = rule_to_ir(rule)?;
        rules.push(Value::list_node(
            "ParseRuleDef",
            vec![Value::Symbol(name.clone()), body_ir],
        ));
    }

    Ok(Value::list_node(
        "ParseGrammar",
        vec![
            Value::Symbol(grammar.name.clone()),
            Value::List(Arc::new(rules)),
        ],
    ))
}

/// Convert a Rule to parsing IR.
fn rule_to_ir(rule: &Rule) -> Result<Value> {
    let pattern_ir = pattern_to_ir(&rule.pattern)?;

    // If there's an action, wrap it
    if let Some(action) = &rule.action {
        let action_ir = expr_to_ir(action)?;
        Ok(Value::list_node("ParseAction", vec![pattern_ir, action_ir]))
    } else {
        Ok(pattern_ir)
    }
}

/// Convert a single Pattern to parsing IR.
fn pattern_to_ir(pattern: &Pattern) -> Result<Value> {
    match pattern {
        Pattern::Empty => {
            // Match nothing, always succeed - return empty sequence
            Ok(Value::list_node(
                "ParseSeq",
                vec![Value::List(Arc::new(vec![]))],
            ))
        }

        Pattern::StringLiteral(s) => {
            if s.len() == 1 {
                Ok(Value::list_node(
                    "ParseChar",
                    vec![Value::String(s.clone())],
                ))
            } else {
                Ok(Value::list_node(
                    "ParseLiteral",
                    vec![Value::String(s.clone())],
                ))
            }
        }

        Pattern::Char(cp) => {
            match cp {
                CharPattern::Exact(c) => Ok(Value::list_node(
                    "ParseChar",
                    vec![Value::String(SmolStr::new(c.to_string()))],
                )),
                CharPattern::Class(ranges) => {
                    let range_values: Vec<Value> = ranges.iter().map(char_range_to_value).collect();

                    Ok(Value::list_node(
                        "ParseCharClass",
                        vec![
                            Value::List(Arc::new(range_values)),
                            Value::Bool(false), // not negated
                        ],
                    ))
                }
                CharPattern::NegatedClass(ranges) => {
                    let range_values: Vec<Value> = ranges.iter().map(char_range_to_value).collect();

                    Ok(Value::list_node(
                        "ParseCharClass",
                        vec![
                            Value::List(Arc::new(range_values)),
                            Value::Bool(true), // negated
                        ],
                    ))
                }
            }
        }

        Pattern::Any => Ok(Value::list_node("ParseAny", vec![])),

        Pattern::Seq(patterns) => {
            let items: Result<Vec<Value>> = patterns.iter().map(pattern_to_ir).collect();

            Ok(Value::list_node(
                "ParseSeq",
                vec![Value::List(Arc::new(items?))],
            ))
        }

        Pattern::Choice(alternatives) => {
            // Each alternative is (Pattern, bool) where bool is backtracking flag
            let items: Result<Vec<Value>> = alternatives
                .iter()
                .map(|(p, _backtrack)| pattern_to_ir(p))
                .collect();

            Ok(Value::list_node(
                "ParseChoice",
                vec![Value::List(Arc::new(items?))],
            ))
        }

        Pattern::Repeat {
            pattern: inner,
            kind,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            let tag = match kind {
                RepeatKind::ZeroOrMore => "ParseStar",
                RepeatKind::OneOrMore => "ParsePlus",
            };
            Ok(Value::list_node(tag, vec![inner_ir]))
        }

        Pattern::Optional(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::list_node("ParseOptional", vec![inner_ir]))
        }

        Pattern::Lookahead(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::list_node("ParseLookahead", vec![inner_ir]))
        }

        Pattern::Not(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::list_node("ParseNot", vec![inner_ir]))
        }

        Pattern::ApplyRule(name) => Ok(Value::list_node(
            "ParseRule",
            vec![Value::Symbol(name.clone())],
        )),

        Pattern::Super(name) => {
            // Super rule call - for now treat as regular rule call
            Ok(Value::list_node(
                "ParseRule",
                vec![Value::Symbol(name.clone())],
            ))
        }

        Pattern::Bind {
            pattern: inner,
            name,
            is_choice: _,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::list_node(
                "ParseBind",
                vec![inner_ir, Value::Symbol(name.clone())],
            ))
        }

        Pattern::Action {
            pattern: inner,
            action,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            let action_ir = expr_to_ir(action)?;
            Ok(Value::list_node("ParseAction", vec![inner_ir, action_ir]))
        }

        Pattern::Predicate(expr) => {
            // Semantic predicate - evaluate expression
            let expr_ir = expr_to_ir(expr)?;
            Ok(Value::list_node("ParsePredicate", vec![expr_ir]))
        }

        Pattern::Guard {
            pattern: inner,
            predicate,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            let guard_ir = expr_to_ir(predicate)?;
            Ok(Value::list_node("ParseGuard", vec![inner_ir, guard_ir]))
        }

        Pattern::Apply(inner) => {
            // Apply pattern to element (for tree walking)
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::list_node("ParseApply", vec![inner_ir]))
        }

        // Binary patterns (not yet supported in IR)
        Pattern::Binary(_) => Err(Error::Runtime(
            "Binary patterns not yet supported in grammar_to_ir".to_string(),
        )),

        // Tree patterns and other patterns not yet supported
        Pattern::TagMatch(_, _)
        | Pattern::End
        | Pattern::MatchValue(_)
        | Pattern::MatchType(_)
        | Pattern::ListMatch(_, _)
        | Pattern::MapMatch(_)
        | Pattern::SymbolMatch(_)
        | Pattern::SymbolLiteral(_) => Err(Error::Runtime(
            "Pattern type not yet supported in grammar_to_ir".to_string(),
        )),

        // Patterns from let bindings that shouldn't appear in grammars
        Pattern::Var(_)
        | Pattern::Literal(_)
        | Pattern::Map(_)
        | Pattern::List(_)
        | Pattern::Tagged { .. } => Err(Error::Runtime(
            "Let binding patterns not supported in grammar_to_ir".to_string(),
        )),
    }
}

/// Convert a CharRange to a Value (list of [start, end])
fn char_range_to_value(range: &CharRange) -> Value {
    match range {
        CharRange::Char(c) => {
            let s = c.to_string();
            Value::List(Arc::new(vec![
                Value::String(SmolStr::new(&s)),
                Value::String(SmolStr::new(&s)),
            ]))
        }
        CharRange::Range(start, end) => Value::List(Arc::new(vec![
            Value::String(SmolStr::new(start.to_string())),
            Value::String(SmolStr::new(end.to_string())),
        ])),
    }
}

/// Convert an action expression to computation IR.
/// This handles the semantic actions in grammar rules like `=> :Int(n)`.
fn expr_to_ir(expr: &crate::ast::Expr) -> Result<Value> {
    use crate::ast::Expr;

    match expr {
        Expr::Null => Ok(Value::list_node("LoadNull", vec![])),

        Expr::Int(n) => Ok(Value::list_node("LoadInt", vec![Value::Int(*n)])),

        Expr::Float(f) => Ok(Value::list_node("LoadFloat", vec![Value::Float(*f)])),

        Expr::Bool(b) => Ok(Value::list_node("LoadBool", vec![Value::Bool(*b)])),

        Expr::String(s) => Ok(Value::list_node(
            "LoadString",
            vec![Value::String(s.clone())],
        )),

        Expr::Symbol(s) => Ok(Value::list_node(
            "LoadSymbol",
            vec![Value::Symbol(s.clone())],
        )),

        Expr::Ident(name) => Ok(Value::list_node("Var", vec![Value::Symbol(name.clone())])),

        Expr::Qualified(qn) => {
            // For qualified names, generate a sequence of property accesses
            if qn.parts.is_empty() {
                return Err(Error::Runtime("Empty qualified name".to_string()));
            }
            let mut result = Value::list_node("Var", vec![Value::Symbol(qn.parts[0].clone())]);
            for part in &qn.parts[1..] {
                result = Value::list_node("GetProp", vec![result, Value::Symbol(part.clone())]);
            }
            Ok(result)
        }

        Expr::Tagged(tag, args) => {
            let mut arg_irs = Vec::new();
            for arg in args {
                arg_irs.push(expr_to_ir(arg)?);
            }
            Ok(Value::list_node(
                "MakeTagged",
                vec![Value::Symbol(tag.clone()), Value::List(Arc::new(arg_irs))],
            ))
        }

        Expr::List(items) => {
            let mut item_irs = Vec::new();
            for item in items {
                item_irs.push(expr_to_ir(item)?);
            }
            Ok(Value::list_node(
                "MakeList",
                vec![Value::List(Arc::new(item_irs))],
            ))
        }

        Expr::Binary(lhs, op, rhs) => {
            let lhs_ir = expr_to_ir(lhs)?;
            let rhs_ir = expr_to_ir(rhs)?;
            let op_tag = match op {
                BinOp::Add => "Add",
                BinOp::Sub => "Sub",
                BinOp::Mul => "Mul",
                BinOp::Div => "Div",
                BinOp::Mod => "Mod",
                BinOp::Eq => "Eq",
                BinOp::NotEq => "NotEq",
                BinOp::Lt => "Lt",
                BinOp::Gt => "Gt",
                BinOp::LtEq => "LtEq",
                BinOp::GtEq => "GtEq",
                BinOp::And => "And",
                BinOp::Or => "Or",
                BinOp::Pipe => "Pipe",
                BinOp::In => "In",
            };
            Ok(Value::list_node(op_tag, vec![lhs_ir, rhs_ir]))
        }

        Expr::Unary(op, operand) => {
            let operand_ir = expr_to_ir(operand)?;
            let op_tag = match op {
                UnaryOp::Neg => "Neg",
                UnaryOp::Not => "Not",
            };
            Ok(Value::list_node(op_tag, vec![operand_ir]))
        }

        Expr::If(condition, then_branch, else_branch) => {
            let cond_ir = expr_to_ir(condition)?;
            let then_ir = expr_to_ir(then_branch)?;
            let else_ir = if let Some(eb) = else_branch {
                expr_to_ir(eb)?
            } else {
                Value::list_node("LoadNull", vec![])
            };
            Ok(Value::list_node("If", vec![cond_ir, then_ir, else_ir]))
        }

        Expr::Let(bindings, body) => {
            // Build nested Let IR for multiple bindings
            let mut result = expr_to_ir(body)?;
            for binding in bindings.iter().rev() {
                match binding {
                    LetBinding::Simple(name, Some(value)) => {
                        let value_ir = expr_to_ir(value)?;
                        result = Value::list_node(
                            "Let",
                            vec![Value::Symbol(name.clone()), value_ir, result],
                        );
                    }
                    LetBinding::Simple(name, None) => {
                        // No value - bind to null
                        result = Value::list_node(
                            "Let",
                            vec![
                                Value::Symbol(name.clone()),
                                Value::Tagged(SmolStr::new("LoadNull"), Arc::new(vec![])),
                                result,
                            ],
                        );
                    }
                    LetBinding::Destructure(_, _) => {
                        return Err(Error::Runtime(
                            "Destructuring let bindings not yet supported in grammar_to_ir"
                                .to_string(),
                        ));
                    }
                }
            }
            Ok(result)
        }

        Expr::Call(function, args) => {
            let func_ir = expr_to_ir(function)?;
            let mut arg_irs = Vec::new();
            for arg in args {
                match arg {
                    Arg::Expr(e) => arg_irs.push(expr_to_ir(e)?),
                    Arg::Placeholder => {
                        return Err(Error::Runtime(
                            "Placeholder arguments not supported in grammar_to_ir".to_string(),
                        ));
                    }
                }
            }
            Ok(Value::list_node(
                "Call",
                vec![func_ir, Value::List(Arc::new(arg_irs))],
            ))
        }

        Expr::Lambda(params, body) => {
            let body_ir = expr_to_ir(body)?;
            let param_symbols: Vec<Value> =
                params.iter().map(|p| Value::Symbol(p.clone())).collect();
            Ok(Value::list_node(
                "Lambda",
                vec![Value::List(Arc::new(param_symbols)), body_ir],
            ))
        }

        Expr::ShortLambda(param, body) => {
            let body_ir = expr_to_ir(body)?;
            Ok(Value::list_node(
                "Lambda",
                vec![
                    Value::List(Arc::new(vec![Value::Symbol(param.clone())])),
                    body_ir,
                ],
            ))
        }

        Expr::Index(collection, index) => {
            let coll_ir = expr_to_ir(collection)?;
            let idx_ir = expr_to_ir(index)?;
            Ok(Value::list_node("Index", vec![coll_ir, idx_ir]))
        }

        Expr::PropAccess(object, property) => {
            let obj_ir = expr_to_ir(object)?;
            Ok(Value::list_node(
                "GetProp",
                vec![obj_ir, Value::Symbol(property.clone())],
            ))
        }

        Expr::MethodCall(object, method, args) => {
            let obj_ir = expr_to_ir(object)?;
            let mut arg_irs = Vec::new();
            for arg in args {
                match arg {
                    Arg::Expr(e) => arg_irs.push(expr_to_ir(e)?),
                    Arg::Placeholder => {
                        return Err(Error::Runtime(
                            "Placeholder arguments not supported in grammar_to_ir".to_string(),
                        ));
                    }
                }
            }
            Ok(Value::list_node(
                "MethodCall",
                vec![
                    obj_ir,
                    Value::Symbol(method.clone()),
                    Value::List(Arc::new(arg_irs)),
                ],
            ))
        }

        Expr::Map(entries) => {
            let mut pair_irs = Vec::new();
            for entry in entries {
                match entry {
                    MapEntry::Symbol(key, value) => {
                        let key_ir =
                            Value::list_node("LoadString", vec![Value::String(key.clone())]);
                        let val_ir = expr_to_ir(value)?;
                        pair_irs.push(Value::List(Arc::new(vec![key_ir, val_ir])));
                    }
                    MapEntry::Computed(key, value) => {
                        let key_ir = expr_to_ir(key)?;
                        let val_ir = expr_to_ir(value)?;
                        pair_irs.push(Value::List(Arc::new(vec![key_ir, val_ir])));
                    }
                }
            }
            Ok(Value::list_node(
                "MakeMap",
                vec![Value::List(Arc::new(pair_irs))],
            ))
        }

        Expr::Sequence(stmts) => {
            let mut stmt_irs = Vec::new();
            for stmt in stmts {
                stmt_irs.push(expr_to_ir(stmt)?);
            }
            Ok(Value::list_node(
                "Seq",
                vec![Value::List(Arc::new(stmt_irs))],
            ))
        }

        Expr::Return(val) => {
            let val_ir = if let Some(v) = val {
                expr_to_ir(v)?
            } else {
                Value::list_node("LoadNull", vec![])
            };
            Ok(Value::list_node("Return", vec![val_ir]))
        }

        // For complex expressions, provide a placeholder
        _ => Err(Error::Runtime(format!(
            "Expression type not yet supported in grammar_to_ir: {:?}",
            expr
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_to_ir() {
        let pattern = Pattern::StringLiteral(SmolStr::new("hello"));
        let ir = pattern_to_ir(&pattern).unwrap();

        if let Some((tag, children)) = ir.as_node() {
            assert_eq!(tag.as_str(), "ParseLiteral");
            assert_eq!(children.len(), 1);
            if let Value::String(s) = &children[0] {
                assert_eq!(s.as_str(), "hello");
            } else {
                panic!("Expected string child");
            }
        }
    }

    #[test]
    fn test_char_to_ir() {
        let pattern = Pattern::Char(CharPattern::Exact('x'));
        let ir = pattern_to_ir(&pattern).unwrap();

        let (tag, _) = ir.as_node().expect("list-shaped node");
        assert_eq!(tag.as_str(), "ParseChar");
    }

    #[test]
    fn test_seq_to_ir() {
        let pattern = Pattern::Seq(vec![
            Pattern::Char(CharPattern::Exact('a')),
            Pattern::Char(CharPattern::Exact('b')),
        ]);
        let ir = pattern_to_ir(&pattern).unwrap();

        if let Some((tag, children)) = ir.as_node() {
            assert_eq!(tag.as_str(), "ParseSeq");
            if let Value::List(items) = &children[0] {
                assert_eq!(items.len(), 2);
            } else {
                panic!("Expected list child");
            }
        }
    }
}
