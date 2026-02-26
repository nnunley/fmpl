//! Integration test for the end-to-end tavern demo.
//!
//! Validates that demo/tavern.fmpl exercises all major FMPL features:
//! objects, facets, grammars, pattern matching, tuple space.

use fmpl_core::{Vm, eval};

fn demo_source() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("demo/tavern.fmpl"),
    )
    .expect("demo/tavern.fmpl should exist")
}

#[test]
fn tavern_demo_returns_summary_map() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, &demo_source()).expect("tavern demo should run without error");
    assert_eq!(
        result.type_name(),
        "map",
        "demo should return a map, got: {}",
        result
    );
}

#[test]
fn tavern_demo_look_command() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, &demo_source()).expect("tavern demo");
    let s = format!("{}", result);
    assert!(
        s.contains("cozy tavern") || s.contains("roaring fire"),
        "room description missing from result: {}",
        s
    );
}

#[test]
fn tavern_demo_greet_command() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, &demo_source()).expect("tavern demo");
    let s = format!("{}", result);
    assert!(
        s.contains("Welcome to The Rusty Flagon, Traveler!"),
        "greet missing from result: {}",
        s
    );
}

#[test]
fn tavern_demo_order_command() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, &demo_source()).expect("tavern demo");
    let s = format!("{}", result);
    assert!(
        s.contains("Here is your ale"),
        "order missing from result: {}",
        s
    );
}

#[test]
fn tavern_demo_menu() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, &demo_source()).expect("tavern demo");
    let s = format!("{}", result);
    assert!(
        s.contains("ale") && s.contains("mead") && s.contains("stew"),
        "menu missing from result: {}",
        s
    );
}

#[test]
fn tavern_demo_facet_denial() {
    let mut vm = Vm::new();
    // Run the demo first to set up barkeep
    eval(&mut vm, &demo_source()).expect("tavern demo");
    // Now try to restock through customer facet — should fail
    let err = eval(&mut vm, "barkeep.as(:customer).restock(5)").unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("facet :customer does not expose method 'restock'"),
        "expected facet denial, got: {}",
        msg
    );
}
