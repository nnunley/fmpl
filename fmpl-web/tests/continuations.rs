use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fmpl_web::continuations::{ContinuationStore, MAX_STREAM_PAYLOAD_BYTES, SnapshotEnvelope};
use serde_json::Value;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut path = std::env::temp_dir();
    path.push(format!("fmpl-web-continuations-{}-{}", nanos, counter));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

#[test]
fn test_store_and_load_continuation() {
    let dir = temp_path();
    let store = ContinuationStore::new(&dir).expect("create store");
    let token = store
        .save("session", SnapshotEnvelope::dummy())
        .expect("save");
    let loaded = store.load("session", &token).expect("load");
    assert_eq!(loaded.schema_version, 1);
}

#[test]
fn test_continuation_token_is_compact() {
    let dir = temp_path();
    let store = ContinuationStore::new(&dir).expect("create store");
    let token = store
        .save("session", SnapshotEnvelope::dummy())
        .expect("save");

    assert!(token.len() <= 22, "token too long: {}", token.len());
    assert!(
        token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "token contains non-url-safe characters: {}",
        token
    );
}

#[test]
fn test_continuation_payload_updates_with_last_action() {
    let dir = temp_path();
    let store = ContinuationStore::new(&dir).expect("create store");
    let mut env = SnapshotEnvelope::new(Vec::new(), "rkyv-v1");
    env.payload = br#"{}"#.to_vec();
    let token = store.save("session", env).expect("save");

    store
        .update_last_action("session", &token, "listen")
        .expect("update");

    let loaded = store.load("session", &token).expect("load");
    let payload: Value = serde_json::from_slice(&loaded.payload).expect("json");
    let events = payload["stream"]["source"].as_array().expect("source");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["choice"], "listen");
    assert!(events[0]["timestamp"].is_number());

    store
        .update_last_action("session", &token, "ask")
        .expect("update");
    let loaded = store.load("session", &token).expect("load");
    let payload: Value = serde_json::from_slice(&loaded.payload).expect("json");
    let events = payload["stream"]["source"].as_array().expect("source");
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["choice"], "ask");
}

#[test]
fn test_continuation_stream_rolls_over_with_prev_marker() {
    let dir = temp_path();
    let store = ContinuationStore::new(&dir).expect("create store");
    let env = SnapshotEnvelope::new(Vec::new(), "rkyv-v1");
    let token = store.save("session", env).expect("save");

    let big_choice = "x".repeat(512);
    for idx in 0..20 {
        store
            .update_last_action("session", &token, &format!("{}-{}", big_choice, idx))
            .expect("update");
    }

    let loaded = store.load("session", &token).expect("load");
    let payload: Value = serde_json::from_slice(&loaded.payload).expect("json");
    let stream = payload["stream"].as_object().expect("stream");
    let prev_token = stream["prev"]["token"].as_str().expect("prev token");
    assert!(loaded.payload.len() <= MAX_STREAM_PAYLOAD_BYTES);
    assert!(store.load("session", prev_token).is_ok());
}
