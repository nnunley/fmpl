//! Capability security tests for TupleSpaceFacet.
//!
//! Tests namespace isolation and permission-based access control.

use fmpl_core::eval;

#[test]
fn test_facet_namespace_isolation() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let system = tuplespace.new()
        let user1 = system.namespace(:user_1)
        let user2 = system.namespace(:user_2)

        user1.out("event", "from_user1")
        let result = user2.in("event")
        result
    "#;
    // user2 should NOT see user1's tuple (different namespaces)
    let result = eval(&mut vm, source);
    assert!(
        result.is_err(),
        "user2 should not be able to read user1's tuple"
    );
}

#[test]
fn test_facet_same_namespace_can_read() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let system = tuplespace.new()
        let space_a = system.namespace(:ns_a)
        let space_b = system.namespace(:ns_a)

        space_a.out("event", "from_a")
        let result = space_b.in("event")
        result.data
    "#;
    // Same namespace facets should see each other's tuples
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::String("from_a".into()));
}

#[test]
fn test_facet_readonly_cannot_write() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let system = tuplespace.new()
        let readonly = system.readonly()

        readonly.out("event", "should_fail")
    "#;
    // Readonly facet should not allow out operations
    let result = eval(&mut vm, source);
    assert!(result.is_err(), "readonly facet should not allow out()");
}

#[test]
fn test_facet_writeonly_cannot_read() {
    let mut vm = fmpl_core::Vm::new();
    let source = r#"
        let system = tuplespace.new()
        let writeonly = system.writeonly()

        writeonly.out("event", "data")
        let result = writeonly.in("event")
        result
    "#;
    // Writeonly facet should not allow in operations
    let result = eval(&mut vm, source);
    assert!(result.is_err(), "writeonly facet should not allow in()");
}

#[test]
fn test_facet_default_allows_all() {
    let mut vm = fmpl_core::Vm::new();
    // `out` on a TupleSpace value takes a single tagged map. Facet-
    // bound `out` (the other tests in this file) still takes two args
    // because facets dispatch separately; shape unification on facets
    // is a follow-up.
    let source = r#"
        let system = tuplespace.new()

        system.out(%{type: "event", data: "data"})
        let result = system.in("event")
        result.data
    "#;
    // Default facet (no namespace restriction) should work normally
    let result = eval(&mut vm, source).unwrap();
    assert_eq!(result, fmpl_core::value::Value::String("data".into()));
}
