//! Type system representation tests (AC-L1-1).

use fmpl_core::types::{Type, TypeConstraint};
use smol_str::SmolStr;

#[test]
fn int_is_subtype_of_any() {
    assert!(Type::Int.is_subtype(&Type::Any));
}

#[test]
fn int_is_subtype_of_self() {
    assert!(Type::Int.is_subtype(&Type::Int));
}

#[test]
fn none_is_subtype_of_everything() {
    assert!(Type::None.is_subtype(&Type::Any));
    assert!(Type::None.is_subtype(&Type::Int));
    assert!(Type::None.is_subtype(&Type::String));
}

#[test]
fn any_is_not_subtype_of_int() {
    assert!(!Type::Any.is_subtype(&Type::Int));
}

#[test]
fn list_subtyping() {
    let list_int = Type::List(Box::new(Type::Int));
    let list_any = Type::List(Box::new(Type::Any));
    assert!(list_int.is_subtype(&list_any));
    assert!(!list_any.is_subtype(&list_int));
}

#[test]
fn union_subtyping() {
    let int_or_string = Type::Union(vec![Type::Int, Type::String]);
    // Union is subtype of Any
    assert!(int_or_string.is_subtype(&Type::Any));
    // Int is subtype of Union(Int, String)
    assert!(Type::Int.is_subtype(&int_or_string));
    assert!(Type::String.is_subtype(&int_or_string));
    // Float is NOT subtype of Union(Int, String)
    assert!(!Type::Float.is_subtype(&int_or_string));
}

#[test]
fn tagged_subtyping() {
    let ok_int = Type::Tagged(SmolStr::new("ok"), vec![Type::Int]);
    let ok_any = Type::Tagged(SmolStr::new("ok"), vec![Type::Any]);
    let err_int = Type::Tagged(SmolStr::new("err"), vec![Type::Int]);
    assert!(ok_int.is_subtype(&ok_any));
    assert!(!ok_int.is_subtype(&err_int));
}

#[test]
fn fun_subtyping() {
    // Covariant return, contravariant args
    let f1 = Type::Fun(vec![Type::Any], Box::new(Type::Int));
    let f2 = Type::Fun(vec![Type::Int], Box::new(Type::Any));
    // f1: Any -> Int  is subtype of  f2: Int -> Any
    // because f1 accepts wider args and returns narrower result
    assert!(f1.is_subtype(&f2));
    assert!(!f2.is_subtype(&f1));
}

#[test]
fn type_constraint_construction() {
    let c1 = TypeConstraint::Subtype(Type::Int, Type::Any);
    let c2 = TypeConstraint::HasMethod(Type::Int, SmolStr::new("to_string"), vec![]);
    let c3 = TypeConstraint::HasProperty(Type::Map, SmolStr::new("length"));
    // Just verify they construct without panic
    assert!(matches!(c1, TypeConstraint::Subtype(..)));
    assert!(matches!(c2, TypeConstraint::HasMethod(..)));
    assert!(matches!(c3, TypeConstraint::HasProperty(..)));
}
