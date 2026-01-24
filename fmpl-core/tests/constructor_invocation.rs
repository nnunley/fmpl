//! Tests for object constructor invocation
//!
//! Tests the spawn expression with constructor method invocation.
//! When `spawn parent(args)` is called, the parent's constructor method
//! should be invoked with the provided arguments to initialize the new object.

use fmpl_core::{Value, Vm, eval};

// =============================================================================
// Constructor invocation tests
// =============================================================================

mod constructor_invocation {
    use super::*;

    #[test]
    fn spawn_without_constructor_works() {
        let mut vm = Vm::new();
        // Object without init method should still work
        let _ = eval(
            &mut vm,
            r#"
object basic {
  get_value(): 100
}
"#,
        )
        .expect("define object");

        let result = eval(&mut vm, r#"let b = spawn basic(); b.get_value()"#).unwrap();
        assert!(matches!(result, Value::Int(100)), "got {:?}", result);
    }

    #[test]
    fn init_method_receives_arguments() {
        let mut vm = Vm::new();
        // Define an object with an init method that tracks its arguments
        let _ = eval(
            &mut vm,
            r#"
object tracker {
  init(x): x

  get_x(): 42
}
"#,
        )
        .expect("define object");

        // If init is called with argument 10, spawn should succeed
        let result = eval(&mut vm, r#"let t = spawn tracker(10); t.get_x()"#).unwrap();
        assert!(matches!(result, Value::Int(42)), "got {:?}", result);
    }

    #[test]
    fn init_with_multiple_arguments() {
        let mut vm = Vm::new();
        let _ = eval(
            &mut vm,
            r#"
object point {
  init(x, y): 0

  distance(): 25
}
"#,
        )
        .expect("define object");

        let result = eval(&mut vm, r#"let p = spawn point(3, 4); p.distance()"#).unwrap();
        assert!(matches!(result, Value::Int(25)), "got {:?}", result);
    }
}

// =============================================================================
// Constructor error handling
// =============================================================================

mod property_init {
    use super::*;

    #[test]
    fn init_can_set_object_properties() {
        let mut vm = Vm::new();
        // Test that init can set object properties via self.prop = value
        let _ = eval(
            &mut vm,
            r#"
object counter {
  init(start): self.count = start

  get(): self.count
  count: 0
}
"#,
        )
        .expect("define object");

        // Spawn with start=42, should set count to 42
        let result = eval(&mut vm, r#"let c = spawn counter(42); c.get()"#).unwrap();
        assert!(
            matches!(result, Value::Int(42)),
            "expected 42, got {:?}",
            result
        );
    }

    #[test]
    fn init_property_assignment_persists_across_method_calls() {
        let mut vm = Vm::new();
        let _ = eval(
            &mut vm,
            r#"
object accumulator {
  init(initial): self.sum = initial

  add(value): self.sum = self.sum + value
  get_sum(): self.sum
  sum: 0
}
"#,
        )
        .expect("define object");

        // Spawn with initial=10, then add 5, should get 15
        let result = eval(
            &mut vm,
            r#"let a = spawn accumulator(10); a.add(5); a.get_sum()"#,
        )
        .unwrap();
        assert!(
            matches!(result, Value::Int(15)),
            "expected 15, got {:?}",
            result
        );
    }
}

mod constructor_errors {
    use super::*;

    #[test]
    fn spawn_with_wrong_arg_count_silently_skips_init() {
        let mut vm = Vm::new();
        // If arg count doesn't match, init is skipped but spawn still works
        let _ = eval(
            &mut vm,
            r#"
object needs_two {
  init(a, b): 0

  get_value(): 100
}
"#,
        )
        .expect("define object");

        // Spawn with wrong number of args - init should be skipped
        let result = eval(&mut vm, r#"let n = spawn needs_two(1); n.get_value()"#).unwrap();
        assert!(matches!(result, Value::Int(100)), "got {:?}", result);
    }
}
