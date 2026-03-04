//! Tests for RLM primitives: chunk and await_all

use fmpl_core::{Value, eval};

// Note: chunk() function tests are skipped because the @ pattern matching syntax
// for lists needs to be properly supported first. The chunk function is defined
// in lib/rlm.fmpl and can be tested manually in the REPL.

#[test]
fn test_await_all_empty_list() {
    let mut vm = fmpl_core::Vm::new();
    // await_all([]) should return []
    let code = r#"await_all([])"#;
    let result = eval(&mut vm, code);
    assert!(
        result.is_ok(),
        "await_all of empty list should succeed: {:?}",
        result.err()
    );
    match result {
        Ok(Value::List(results)) => {
            assert_eq!(results.len(), 0, "empty list should give empty results");
        }
        Ok(other) => panic!("Expected empty list, got {:?}", other),
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}
