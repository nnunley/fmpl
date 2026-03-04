//! Type system representations for FMPL.
//!
//! Provides `Type` and `TypeConstraint` enums for layered type inference.

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
    /// Tagged constructor with name and child types.
    Tagged(SmolStr, Vec<Type>),
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
            // Tagged: same name, covariant children
            (Type::Tagged(n1, c1), Type::Tagged(n2, c2)) => {
                n1 == n2
                    && c1.len() == c2.len()
                    && c1.iter().zip(c2.iter()).all(|(a, b)| a.is_subtype(b))
            }
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
    /// `lhs` must be a subtype of `rhs`.
    Subtype(Type, Type),
    /// Type must have a method with the given name and argument types.
    HasMethod(Type, SmolStr, Vec<Type>),
    /// Type must have a property with the given name.
    HasProperty(Type, SmolStr),
}
