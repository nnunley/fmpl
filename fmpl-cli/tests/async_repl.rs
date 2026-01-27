//! Tests for REPL async support
//!
//! These tests verify that REPL's wait_for_async function
//! and async execution flow work correctly.

use fmpl_core::stream::StreamEvent;
use fmpl_core::{Value, Vm, eval};
use tokio::sync::mpsc;

/// Block and wait for an async stream to complete.
/// Returns the final result value or an error.
/// (Copied from main.rs for testing purposes since fmpl-cli is a binary crate)
fn wait_for_async(value: Value) -> Result<Value, String> {
    match value {
        Value::AsyncStream(handle) => {
            let mut handle = handle.lock().map_err(|e| format!("Lock error: {}", e))?;

            // Collect all events from the stream
            let mut final_value = Value::Null;

            loop {
                match handle.recv_blocking() {
                    Some(StreamEvent::Data(v)) => {
                        // Intermediate data - keep last value
                        final_value = v;
                    }
                    Some(StreamEvent::Ok(v)) => {
                        // Terminal success - return result
                        return Ok(v);
                    }
                    Some(StreamEvent::Err(e)) => {
                        // Terminal error - return error
                        return Err(format!("Async error: {}", e));
                    }
                    Some(StreamEvent::Done) => {
                        // Stream completed without value - return final data or null
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Ok(Value::Null);
                    }
                    None => {
                        // Channel closed without Ok/Err/Done
                        if final_value != Value::Null {
                            return Ok(final_value);
                        }
                        return Err("Async stream completed without result".to_string());
                    }
                }
            }
        }
        _ => Ok(value),
    }
}

#[tokio::test]
async fn test_wait_for_async_handles_ok() {
    // Test that wait_for_async correctly handles Ok events
    let (tx, rx) = mpsc::channel(1);
    let stream = fmpl_core::stream::StreamHandle::new(rx, 1);

    // Send an Ok event
    tx.send(StreamEvent::Ok(Value::Int(42))).await.unwrap();

    let value = Value::AsyncStream(std::sync::Arc::new(std::sync::Mutex::new(stream)));
    let result = wait_for_async(value);

    assert!(
        result.is_ok(),
        "wait_for_async should succeed: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Int(42));
}

#[tokio::test]
async fn test_wait_for_async_handles_data() {
    // Test that wait_for_async correctly handles Data events followed by Done
    let (tx, rx) = mpsc::channel(1);
    let stream = fmpl_core::stream::StreamHandle::new(rx, 2);

    // Send Data then Done in a background task to avoid blocking
    tokio::spawn(async move {
        tx.send(StreamEvent::Data(Value::Int(1))).await.unwrap();
        tx.send(StreamEvent::Done).await.unwrap();
    });

    // Give the background task time to send
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let value = Value::AsyncStream(std::sync::Arc::new(std::sync::Mutex::new(stream)));
    let result = wait_for_async(value);

    assert!(
        result.is_ok(),
        "wait_for_async should succeed: {:?}",
        result.err()
    );
    // Should return the last data value
    assert_eq!(result.unwrap(), Value::Int(1));
}

#[tokio::test]
async fn test_wait_for_async_handles_error() {
    // Test that wait_for_async correctly handles Err events
    let (tx, rx) = mpsc::channel(1);
    let stream = fmpl_core::stream::StreamHandle::new(rx, 3);

    let error_msg = Value::String("test error".into());
    tx.send(StreamEvent::Err(error_msg.clone())).await.unwrap();

    let value = Value::AsyncStream(std::sync::Arc::new(std::sync::Mutex::new(stream)));
    let result = wait_for_async(value);

    assert!(result.is_err(), "wait_for_async should return error");
    assert!(result.unwrap_err().contains("test error"));
}

#[tokio::test]
async fn test_wait_for_async_handles_done_with_no_data() {
    // Test that wait_for_async returns null when stream completes with Done and no data
    let (tx, rx) = mpsc::channel(1);
    let stream = fmpl_core::stream::StreamHandle::new(rx, 4);

    // Send only Done
    tx.send(StreamEvent::Done).await.unwrap();

    let value = Value::AsyncStream(std::sync::Arc::new(std::sync::Mutex::new(stream)));
    let result = wait_for_async(value);

    assert!(
        result.is_ok(),
        "wait_for_async should succeed: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), Value::Null);
}

#[tokio::test]
async fn test_repl_vm_with_runtime_supports_async() {
    // Test that REPL can create a VM with runtime and execute async operations
    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    // stream.create should work with runtime
    let result = eval(&mut vm, "stream.create(lambda () 42)");
    assert!(result.is_ok(), "stream.create should succeed with runtime");

    match result.unwrap() {
        Value::AsyncStream(_stream) => {
            // Successfully created AsyncStream - REPL can handle async
        }
        other => panic!("Expected AsyncStream, got {:?}", other),
    }
}

#[tokio::test]
async fn test_repl_vm_with_runtime_and_wait_for_async() {
    // Test full async flow: create stream and wait for result
    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    // Create a simple async stream that returns a value
    // Use block syntax for lambda to ensure it compiles correctly
    let result = eval(&mut vm, "stream.create(lambda () { 42 })");
    assert!(result.is_ok());

    // The REPL should be able to wait for this stream
    let value = result.unwrap();
    match value {
        Value::AsyncStream(stream) => {
            // Give the async task time to execute
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            // REPL would call wait_for_async here
            // Verify the stream can be locked and read
            let mut handle = stream.lock().unwrap();
            match handle.recv_blocking() {
                Some(StreamEvent::Ok(Value::Int(42))) => {
                    // Lambda executed successfully and returned 42
                }
                other => panic!("Unexpected stream event: {:?}, expected Ok(Int(42))", other),
            }
        }
        other => panic!("Expected AsyncStream, got {:?}", other),
    }
}
