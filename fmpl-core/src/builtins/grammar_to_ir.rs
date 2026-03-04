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
        rules.push(Value::Tagged(
            SmolStr::new("ParseRuleDef"),
            Arc::new(vec![Value::Symbol(name.clone()), body_ir]),
        ));
    }

    Ok(Value::Tagged(
        SmolStr::new("ParseGrammar"),
        Arc::new(vec![
            Value::Symbol(grammar.name.clone()),
            Value::List(Arc::new(rules)),
        ]),
    ))
}

/// Convert a Rule to parsing IR.
fn rule_to_ir(rule: &Rule) -> Result<Value> {
    let pattern_ir = pattern_to_ir(&rule.pattern)?;

    // If there's an action, wrap it
    if let Some(action) = &rule.action {
        let action_ir = expr_to_ir(action)?;
        Ok(Value::Tagged(
            SmolStr::new("ParseAction"),
            Arc::new(vec![pattern_ir, action_ir]),
        ))
    } else {
        Ok(pattern_ir)
    }
}

/// Convert a single Pattern to parsing IR.
fn pattern_to_ir(pattern: &Pattern) -> Result<Value> {
    match pattern {
        Pattern::Empty => {
            // Match nothing, always succeed - return empty sequence
            Ok(Value::Tagged(
                SmolStr::new("ParseSeq"),
                Arc::new(vec![Value::List(Arc::new(vec![]))]),
            ))
        }

        Pattern::StringLiteral(s) => {
            if s.len() == 1 {
                Ok(Value::Tagged(
                    SmolStr::new("ParseChar"),
                    Arc::new(vec![Value::String(s.clone())]),
                ))
            } else {
                Ok(Value::Tagged(
                    SmolStr::new("ParseLiteral"),
                    Arc::new(vec![Value::String(s.clone())]),
                ))
            }
        }

        Pattern::Char(cp) => {
            match cp {
                CharPattern::Exact(c) => Ok(Value::Tagged(
                    SmolStr::new("ParseChar"),
                    Arc::new(vec![Value::String(SmolStr::new(&c.to_string()))]),
                )),
                CharPattern::Class(ranges) => {
                    let range_values: Vec<Value> = ranges
                        .iter()
                        .map(|range| char_range_to_value(range))
                        .collect();

                    Ok(Value::Tagged(
                        SmolStr::new("ParseCharClass"),
                        Arc::new(vec![
                            Value::List(Arc::new(range_values)),
                            Value::Bool(false), // not negated
                        ]),
                    ))
                }
                CharPattern::NegatedClass(ranges) => {
                    let range_values: Vec<Value> = ranges
                        .iter()
                        .map(|range| char_range_to_value(range))
                        .collect();

                    Ok(Value::Tagged(
                        SmolStr::new("ParseCharClass"),
                        Arc::new(vec![
                            Value::List(Arc::new(range_values)),
                            Value::Bool(true), // negated
                        ]),
                    ))
                }
            }
        }

        Pattern::Any => Ok(Value::Tagged(SmolStr::new("ParseAny"), Arc::new(vec![]))),

        Pattern::Seq(patterns) => {
            let items: Result<Vec<Value>> = patterns.iter().map(pattern_to_ir).collect();

            Ok(Value::Tagged(
                SmolStr::new("ParseSeq"),
                Arc::new(vec![Value::List(Arc::new(items?))]),
            ))
        }

        Pattern::Choice(alternatives) => {
            // Each alternative is (Pattern, bool) where bool is backtracking flag
            let items: Result<Vec<Value>> = alternatives
                .iter()
                .map(|(p, _backtrack)| pattern_to_ir(p))
                .collect();

            Ok(Value::Tagged(
                SmolStr::new("ParseChoice"),
                Arc::new(vec![Value::List(Arc::new(items?))]),
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
            Ok(Value::Tagged(SmolStr::new(tag), Arc::new(vec![inner_ir])))
        }

        Pattern::Optional(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseOptional"),
                Arc::new(vec![inner_ir]),
            ))
        }

        Pattern::Lookahead(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseLookahead"),
                Arc::new(vec![inner_ir]),
            ))
        }

        Pattern::Not(inner) => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseNot"),
                Arc::new(vec![inner_ir]),
            ))
        }

        Pattern::ApplyRule(name) => Ok(Value::Tagged(
            SmolStr::new("ParseRule"),
            Arc::new(vec![Value::Symbol(name.clone())]),
        )),

        Pattern::Super(name) => {
            // Super rule call - for now treat as regular rule call
            Ok(Value::Tagged(
                SmolStr::new("ParseRule"),
                Arc::new(vec![Value::Symbol(name.clone())]),
            ))
        }

        Pattern::Bind {
            pattern: inner,
            name,
            is_choice: _,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseBind"),
                Arc::new(vec![inner_ir, Value::Symbol(name.clone())]),
            ))
        }

        Pattern::Action {
            pattern: inner,
            action,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            let action_ir = expr_to_ir(action)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseAction"),
                Arc::new(vec![inner_ir, action_ir]),
            ))
        }

        Pattern::Predicate(expr) => {
            // Semantic predicate - evaluate expression
            let expr_ir = expr_to_ir(expr)?;
            Ok(Value::Tagged(
                SmolStr::new("ParsePredicate"),
                Arc::new(vec![expr_ir]),
            ))
        }

        Pattern::Guard {
            pattern: inner,
            predicate,
        } => {
            let inner_ir = pattern_to_ir(inner)?;
            let guard_ir = expr_to_ir(predicate)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseGuard"),
                Arc::new(vec![inner_ir, guard_ir]),
            ))
        }

        Pattern::Apply(inner) => {
            // Apply pattern to element (for tree walking)
            let inner_ir = pattern_to_ir(inner)?;
            Ok(Value::Tagged(
                SmolStr::new("ParseApply"),
                Arc::new(vec![inner_ir]),
            ))
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
            Value::String(SmolStr::new(&start.to_string())),
            Value::String(SmolStr::new(&end.to_string())),
        ])),
    }
}

/// Convert an action expression to computation IR.
/// This handles the semantic actions in grammar rules like `=> :Int(n)`.
fn expr_to_ir(expr: &crate::ast::Expr) -> Result<Value> {
    use crate::ast::Expr;

    match expr {
        Expr::Null => Ok(Value::Tagged(SmolStr::new("LoadNull"), Arc::new(vec![]))),

        Expr::Int(n) => Ok(Value::Tagged(
            SmolStr::new("LoadInt"),
            Arc::new(vec![Value::Int(*n)]),
        )),

        Expr::Float(f) => Ok(Value::Tagged(
            SmolStr::new("LoadFloat"),
            Arc::new(vec![Value::Float(*f)]),
        )),

        Expr::Bool(b) => Ok(Value::Tagged(
            SmolStr::new("LoadBool"),
            Arc::new(vec![Value::Bool(*b)]),
        )),

        Expr::String(s) => Ok(Value::Tagged(
            SmolStr::new("LoadString"),
            Arc::new(vec![Value::String(s.clone())]),
        )),

        Expr::Symbol(s) => Ok(Value::Tagged(
            SmolStr::new("LoadSymbol"),
            Arc::new(vec![Value::Symbol(s.clone())]),
        )),

        Expr::Ident(name) => Ok(Value::Tagged(
            SmolStr::new("Var"),
            Arc::new(vec![Value::Symbol(name.clone())]),
        )),

        Expr::Qualified(qn) => {
            // For qualified names, generate a sequence of property accesses
            if qn.parts.is_empty() {
                return Err(Error::Runtime("Empty qualified name".to_string()));
            }
            let mut result = Value::Tagged(
                SmolStr::new("Var"),
                Arc::new(vec![Value::Symbol(qn.parts[0].clone())]),
            );
            for part in &qn.parts[1..] {
                result = Value::Tagged(
                    SmolStr::new("GetProp"),
                    Arc::new(vec![result, Value::Symbol(part.clone())]),
                );
            }
            Ok(result)
        }

        Expr::Tagged(tag, args) => {
            let mut arg_irs = Vec::new();
            for arg in args {
                arg_irs.push(expr_to_ir(arg)?);
            }
            Ok(Value::Tagged(
                SmolStr::new("MakeTagged"),
                Arc::new(vec![
                    Value::Symbol(tag.clone()),
                    Value::List(Arc::new(arg_irs)),
                ]),
            ))
        }

        Expr::List(items) => {
            let mut item_irs = Vec::new();
            for item in items {
                item_irs.push(expr_to_ir(item)?);
            }
            Ok(Value::Tagged(
                SmolStr::new("MakeList"),
                Arc::new(vec![Value::List(Arc::new(item_irs))]),
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
            Ok(Value::Tagged(
                SmolStr::new(op_tag),
                Arc::new(vec![lhs_ir, rhs_ir]),
            ))
        }

        Expr::Unary(op, operand) => {
            let operand_ir = expr_to_ir(operand)?;
            let op_tag = match op {
                UnaryOp::Neg => "Neg",
                UnaryOp::Not => "Not",
            };
            Ok(Value::Tagged(
                SmolStr::new(op_tag),
                Arc::new(vec![operand_ir]),
            ))
        }

        Expr::If(condition, then_branch, else_branch) => {
            let cond_ir = expr_to_ir(condition)?;
            let then_ir = expr_to_ir(then_branch)?;
            let else_ir = if let Some(eb) = else_branch {
                expr_to_ir(eb)?
            } else {
                Value::Tagged(SmolStr::new("LoadNull"), Arc::new(vec![]))
            };
            Ok(Value::Tagged(
                SmolStr::new("If"),
                Arc::new(vec![cond_ir, then_ir, else_ir]),
            ))
        }

        Expr::Let(bindings, body) => {
            // Build nested Let IR for multiple bindings
            let mut result = expr_to_ir(body)?;
            for binding in bindings.iter().rev() {
                match binding {
                    LetBinding::Simple(name, Some(value)) => {
                        let value_ir = expr_to_ir(value)?;
                        result = Value::Tagged(
                            SmolStr::new("Let"),
                            Arc::new(vec![Value::Symbol(name.clone()), value_ir, result]),
                        );
                    }
                    LetBinding::Simple(name, None) => {
                        // No value - bind to null
                        result = Value::Tagged(
                            SmolStr::new("Let"),
                            Arc::new(vec![
                                Value::Symbol(name.clone()),
                                Value::Tagged(SmolStr::new("LoadNull"), Arc::new(vec![])),
                                result,
                            ]),
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
            Ok(Value::Tagged(
                SmolStr::new("Call"),
                Arc::new(vec![func_ir, Value::List(Arc::new(arg_irs))]),
            ))
        }

        Expr::Lambda(params, body) => {
            let body_ir = expr_to_ir(body)?;
            let param_symbols: Vec<Value> =
                params.iter().map(|p| Value::Symbol(p.clone())).collect();
            Ok(Value::Tagged(
                SmolStr::new("Lambda"),
                Arc::new(vec![Value::List(Arc::new(param_symbols)), body_ir]),
            ))
        }

        Expr::ShortLambda(param, body) => {
            let body_ir = expr_to_ir(body)?;
            Ok(Value::Tagged(
                SmolStr::new("Lambda"),
                Arc::new(vec![
                    Value::List(Arc::new(vec![Value::Symbol(param.clone())])),
                    body_ir,
                ]),
            ))
        }

        Expr::Index(collection, index) => {
            let coll_ir = expr_to_ir(collection)?;
            let idx_ir = expr_to_ir(index)?;
            Ok(Value::Tagged(
                SmolStr::new("Index"),
                Arc::new(vec![coll_ir, idx_ir]),
            ))
        }

        Expr::PropAccess(object, property) => {
            let obj_ir = expr_to_ir(object)?;
            Ok(Value::Tagged(
                SmolStr::new("GetProp"),
                Arc::new(vec![obj_ir, Value::Symbol(property.clone())]),
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
            Ok(Value::Tagged(
                SmolStr::new("MethodCall"),
                Arc::new(vec![
                    obj_ir,
                    Value::Symbol(method.clone()),
                    Value::List(Arc::new(arg_irs)),
                ]),
            ))
        }

        Expr::Map(entries) => {
            let mut pair_irs = Vec::new();
            for entry in entries {
                match entry {
                    MapEntry::Symbol(key, value) => {
                        let key_ir = Value::Tagged(
                            SmolStr::new("LoadString"),
                            Arc::new(vec![Value::String(key.clone())]),
                        );
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
            Ok(Value::Tagged(
                SmolStr::new("MakeMap"),
                Arc::new(vec![Value::List(Arc::new(pair_irs))]),
            ))
        }

        Expr::Sequence(stmts) => {
            let mut stmt_irs = Vec::new();
            for stmt in stmts {
                stmt_irs.push(expr_to_ir(stmt)?);
            }
            Ok(Value::Tagged(
                SmolStr::new("Seq"),
                Arc::new(vec![Value::List(Arc::new(stmt_irs))]),
            ))
        }

        Expr::Return(val) => {
            let val_ir = if let Some(v) = val {
                expr_to_ir(v)?
            } else {
                Value::Tagged(SmolStr::new("LoadNull"), Arc::new(vec![]))
            };
            Ok(Value::Tagged(
                SmolStr::new("Return"),
                Arc::new(vec![val_ir]),
            ))
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

        if let Value::Tagged(tag, children) = ir {
            assert_eq!(tag.as_str(), "ParseLiteral");
            assert_eq!(children.len(), 1);
            if let Value::String(s) = &children[0] {
                assert_eq!(s.as_str(), "hello");
            } else {
                panic!("Expected string child");
            }
        } else {
            panic!("Expected tagged value");
        }
    }

    #[test]
    fn test_char_to_ir() {
        let pattern = Pattern::Char(CharPattern::Exact('x'));
        let ir = pattern_to_ir(&pattern).unwrap();

        if let Value::Tagged(tag, _) = ir {
            assert_eq!(tag.as_str(), "ParseChar");
        } else {
            panic!("Expected tagged value");
        }
    }

    #[test]
    fn test_seq_to_ir() {
        let pattern = Pattern::Seq(vec![
            Pattern::Char(CharPattern::Exact('a')),
            Pattern::Char(CharPattern::Exact('b')),
        ]);
        let ir = pattern_to_ir(&pattern).unwrap();

        if let Value::Tagged(tag, children) = ir {
            assert_eq!(tag.as_str(), "ParseSeq");
            if let Value::List(items) = &children[0] {
                assert_eq!(items.len(), 2);
            } else {
                panic!("Expected list child");
            }
        } else {
            panic!("Expected tagged value");
        }
    }
}
