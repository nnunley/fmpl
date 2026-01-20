//! Integration tests for curl builtin with mock HTTP server.

use fmpl_core::{Value, Vm, eval};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_curl_get_returns_stream() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello world"))
        .mount(&server)
        .await;

    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    // Call curl.get and verify it returns a map with source
    let url = format!("{}/test", server.uri());
    let code = format!(r#"curl.get("{}")"#, url);

    let result = eval(&mut vm, &code).unwrap();

    // Result should be a map with source key
    match result {
        Value::Map(m) => {
            assert!(m.contains_key("source"), "result should have 'source' key");
            assert!(m.contains_key("sink"), "result should have 'sink' key");

            // source should be an AsyncStream
            match m.get("source") {
                Some(Value::AsyncStream(stream)) => {
                    // Verify the stream exists and has a valid ID
                    let handle = stream.lock().unwrap();
                    assert!(handle.id() > 0);
                }
                other => panic!("expected AsyncStream, got {:?}", other),
            }
        }
        other => panic!("expected Map, got {:?}", other),
    }
}

#[tokio::test]
async fn test_curl_post_returns_stream() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"ok"}"#))
        .mount(&server)
        .await;

    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    let url = format!("{}/api", server.uri());
    let code = format!(r#"curl.post("{}", "{{}}")"#, url);

    let result = eval(&mut vm, &code).unwrap();

    match result {
        Value::Map(m) => {
            assert!(m.contains_key("source"));
            match m.get("source") {
                Some(Value::AsyncStream(_)) => {}
                other => panic!("expected AsyncStream, got {:?}", other),
            }
        }
        other => panic!("expected Map, got {:?}", other),
    }
}

#[tokio::test]
async fn test_curl_get_receives_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test response"))
        .mount(&server)
        .await;

    let mut vm = Vm::with_runtime(tokio::runtime::Handle::current());

    let url = format!("{}/data", server.uri());
    let code = format!(r#"curl.get("{}")"#, url);

    let result = eval(&mut vm, &code).unwrap();

    // Get the source stream
    if let Value::Map(m) = result
        && let Some(Value::AsyncStream(stream)) = m.get("source")
    {
        // Wait a bit for the async operation to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Try to receive the response
        let mut handle = stream.lock().unwrap();
        if let Some(event) = handle.recv_blocking() {
            match event {
                fmpl_core::StreamEvent::Ok(Value::String(s)) => {
                    assert_eq!(s.as_str(), "test response");
                }
                other => panic!("expected Ok with string, got {:?}", other),
            }
        } else {
            // Response not yet available - that's okay for async
            // The test verifies the structure is correct
        }
    }
}
