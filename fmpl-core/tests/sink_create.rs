//! Tests for stream.sink builtin
//!
//! Tests the stream.sink() builtin which creates sink handles
//! for sending values asynchronously.

use fmpl_core::{Value, eval};

#[test]
fn test_stream_sink_creates_sink() {
    let mut vm = fmpl_core::Vm::new();

    // stream.sink() should create a sink
    let result = eval(&mut vm, "stream.sink()");
    assert!(
        result.is_ok(),
        "stream.sink() should succeed: {:?}",
        result.err()
    );

    match result.unwrap() {
        Value::Sink(s) => {
            // Verify the sink has a valid ID
            assert!(s.id() >= 1, "sink ID should be >= 1");
        }
        other => panic!("Expected Sink, got {:?}", other),
    }
}

#[test]
fn test_stream_sink_creates_unique_ids() {
    let mut vm = fmpl_core::Vm::new();

    // Multiple calls should create sinks with unique IDs
    let result1 = eval(&mut vm, "stream.sink()");
    let result2 = eval(&mut vm, "stream.sink()");

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    match (result1.unwrap(), result2.unwrap()) {
        (Value::Sink(s1), Value::Sink(s2)) => {
            assert_ne!(s1.id(), s2.id(), "sink IDs should be unique");
        }
        _ => panic!("Expected both values to be Sink"),
    }
}
