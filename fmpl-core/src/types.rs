//! Type system representations for FMPL.
//!
//! Provides `Type` and `TypeConstraint` enums for layered type inference,
//! plus constraint generation from AST expressions.

use crate::ast::{Arg, BinOp, Expr, LetBinding};
use smol_str::SmolStr;

/// A type in FMPL's type system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Top type — every value inhabits Any.
    Any,
    /// Bottom type — subtype of everything, no values.
    None,
    Int,
    Float,
    String,
    Symbol,
    Bool,
    /// Homogeneous list with element type.
    List(Box<Type>),
    /// Map (untyped keys/values for now).
    Map,
    /// Function type: argument types → return type.
    Fun(Vec<Type>, Box<Type>),
    /// Union of types.
    Union(Vec<Type>),
    /// Fresh type variable for inference.
    Var(usize),
}

impl Type {
    /// Subtype check: is `self <: other`?
    pub fn is_subtype(&self, other: &Type) -> bool {
        if self == other {
            return true;
        }
        match (self, other) {
            // None is bottom — subtype of everything
            (Type::None, _) => true,
            // Everything is subtype of Any
            (_, Type::Any) => true,
            // List covariance
            (Type::List(a), Type::List(b)) => a.is_subtype(b),
            // A concrete type is subtype of a Union if it's subtype of any member
            (_, Type::Union(members)) => members.iter().any(|m| self.is_subtype(m)),
            // A Union is subtype of T if all members are subtypes of T
            (Type::Union(members), _) => members.iter().all(|m| m.is_subtype(other)),
            // Function: contravariant args, covariant return
            (Type::Fun(args1, ret1), Type::Fun(args2, ret2)) => {
                args1.len() == args2.len()
                    && args2.iter().zip(args1.iter()).all(|(a, b)| a.is_subtype(b))
                    && ret1.is_subtype(ret2)
            }
            _ => false,
        }
    }
}

/// A constraint generated during type inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    /// Two types must be equal.
    Eq(Type, Type),
    /// `lhs` must be a subtype of `rhs`.
    Subtype(Type, Type),
    /// Type must have a method with the given name and argument types.
    HasMethod(Type, SmolStr, Vec<Type>),
    /// Type must have a property with the given name.
    HasProperty(Type, SmolStr),
}

/// Constraint generator: walks an AST and emits type constraints.
pub struct ConstraintGenerator {
    next_var: usize,
}

impl Default for ConstraintGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstraintGenerator {
    pub fn new() -> Self {
        Self { next_var: 0 }
    }

    /// Create a fresh type variable.
    fn fresh(&mut self) -> Type {
        let v = self.next_var;
        self.next_var += 1;
        Type::Var(v)
    }

    /// Generate constraints for an expression, returning (type_of_expr, constraints).
    pub fn generate(&mut self, expr: &Expr) -> (Type, Vec<TypeConstraint>) {
        let mut constraints = Vec::new();
        let ty = self.walk(expr, &mut constraints);
        (ty, constraints)
    }

    pub fn walk(&mut self, expr: &Expr, cs: &mut Vec<TypeConstraint>) -> Type {
        match expr {
            // Literals have known types.
            Expr::Int(_) => Type::Int,
            Expr::Float(_) => Type::Float,
            Expr::String(_) => Type::String,
            Expr::Symbol(_) => Type::Symbol,
            Expr::Bool(_) => Type::Bool,
            Expr::Null => Type::None,

            // Variable reference → fresh type variable.
            Expr::Ident(_) => self.fresh(),

            // Binary operation: HasMethod constraint on lhs.
            Expr::Binary(lhs, op, rhs) => {
                let lhs_ty = self.walk(lhs, cs);
                let rhs_ty = self.walk(rhs, cs);
                let method_name = binop_method_name(*op);
                let result_ty = self.fresh();
                cs.push(TypeConstraint::HasMethod(
                    lhs_ty,
                    SmolStr::new(method_name),
                    vec![rhs_ty],
                ));
                result_ty
            }

            // Function call: typeof(f) = Fun([arg_types], result).
            Expr::Call(callee, args) => {
                let callee_ty = self.walk(callee, cs);
                let arg_types: Vec<Type> = args
                    .iter()
                    .map(|a| match a {
                        Arg::Expr(e) => self.walk(e, cs),
                        Arg::Placeholder => self.fresh(),
                    })
                    .collect();
                let result_ty = self.fresh();
                cs.push(TypeConstraint::Eq(
                    callee_ty,
                    Type::Fun(arg_types, Box::new(result_ty.clone())),
                ));
                result_ty
            }

            // Property access: HasProperty constraint.
            Expr::PropAccess(obj, name) => {
                let obj_ty = self.walk(obj, cs);
                cs.push(TypeConstraint::HasProperty(obj_ty, name.clone()));
                self.fresh()
            }

            // Let binding: typeof(name) = typeof(value).
            Expr::Let(bindings, body) => {
                for b in bindings {
                    if let LetBinding::Simple(_, Some(val)) = b {
                        let val_ty = self.walk(val, cs);
                        let name_ty = self.fresh();
                        cs.push(TypeConstraint::Eq(name_ty, val_ty));
                    }
                }
                self.walk(body, cs)
            }

            // Fallback: return fresh var for unhandled nodes.
            _ => self.fresh(),
        }
    }
}

fn binop_method_name(op: BinOp) -> &'static str {
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
        BinOp::In => "in",
    }
}
