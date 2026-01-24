//! Tests for REPL commands
//!
//! Tests the REPL command functionality including object listing.

use fmpl_core::{Vm, eval};

/// Helper to verify that named objects are correctly tracked
#[test]
fn test_named_objects_tracking() {
    let mut vm = Vm::new();

    // Initially, no named objects
    let count = vm.objects.named_objects().count();
    assert_eq!(count, 0, "Expected no named objects initially");

    // The @name registration in object definitions creates named objects
    // Define an object with a name - this should register it
    let _ = eval(
        &mut vm,
        r#"
object test_obj {
  get_value(): 42
}
"#,
    )
    .expect("define object");

    // After defining an object, it should be registered by name
    let named: Vec<_> = vm.objects.named_objects().collect();
    assert!(
        named.len() > 0,
        "Expected at least 1 named object after object definition"
    );

    // One of them should be "test_obj" (the object we just defined)
    let names: Vec<_> = named.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        names.contains(&"test_obj"),
        "Expected 'test_obj' to be registered. Found: {:?}",
        names
    );
}

/// Test that multiple named objects can be tracked
#[test]
fn test_multiple_named_objects() {
    let mut vm = Vm::new();

    // Create multiple objects and register them
    let obj1_id = vm.objects.create(None);
    let obj2_id = vm.objects.create(None);
    let obj3_id = vm.objects.create(None);

    vm.objects.register_name("first".into(), obj1_id);
    vm.objects.register_name("second".into(), obj2_id);
    vm.objects.register_name("third".into(), obj3_id);

    let named: Vec<_> = vm.objects.named_objects().collect();
    assert_eq!(named.len(), 3);

    // Names should be present (order may vary due to HashMap)
    let names: Vec<_> = named.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"first"));
    assert!(names.contains(&"second"));
    assert!(names.contains(&"third"));
}

/// Test that named_objects handles empty case gracefully
#[test]
fn test_empty_named_objects() {
    let vm = Vm::new();
    let count = vm.objects.named_objects().count();
    assert_eq!(count, 0, "Expected no named objects in fresh VM");
}
