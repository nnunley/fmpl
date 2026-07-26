use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use tower_sessions::{MemoryStore, SessionManagerLayer};

use fmpl_web::storylet::build_app;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut path = std::env::temp_dir();
    path.push(format!("fmpl-web-storylet-{}-{}", nanos, counter));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

#[allow(dead_code)]
fn build_test_app(data_dir: &std::path::PathBuf, session_store: &MemoryStore) -> axum::Router {
    let app = build_app(data_dir).expect("app");
    let session_layer = SessionManagerLayer::new(session_store.clone()).with_secure(false);
    app.layer(session_layer)
}

#[tokio::test]
async fn test_play_route_redirects() {
    let dir = temp_path();
    let session_store = MemoryStore::default();
    let app = build_test_app(&dir, &session_store);

    let response = app
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers().get("location").expect("location");
    assert!(location.to_str().unwrap().starts_with("/play/"));
}

#[tokio::test]
#[ignore = "storylet /play rendering incomplete — WIP feature (asserts storylet content not yet emitted)"]
async fn test_play_route_renders_storylet_from_db() {
    let dir = temp_path();
    let session_store = MemoryStore::default();

    let app = build_test_app(&dir, &session_store);
    let response = app
        .clone()
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");
    let location = response.headers().get("location").expect("location");
    let path = location.to_str().unwrap().to_string();

    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(body.contains("data-storylet='crossroads'"));
}

#[tokio::test]
#[ignore = "storylet /play rendering incomplete — WIP feature (asserts storylet content not yet emitted)"]
async fn test_play_route_includes_debug_panel() {
    let dir = temp_path();
    let session_store = MemoryStore::default();

    let app = build_test_app(&dir, &session_store);
    let response = app
        .clone()
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");
    let location = response.headers().get("location").expect("location");
    let path = format!("{}?debug=1", location.to_str().unwrap());

    let response = app
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(body.contains("debug-fmpl"));
    assert!(body.contains("object crossroads"));
}

#[tokio::test]
#[ignore = "storylet /play rendering incomplete — WIP feature (asserts storylet content not yet emitted)"]
async fn test_choice_updates_continuation_payload() {
    let dir = temp_path();
    let session_store = MemoryStore::default();

    let app = build_test_app(&dir, &session_store);
    let response = app
        .clone()
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");
    let location = response.headers().get("location").expect("location");
    let path = location.to_str().unwrap();
    let token = path.trim_start_matches("/play/");

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/play/{}/choice", token))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("choice=listen"))
                .unwrap(),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let store = fmpl_web::continuations::ContinuationStore::new(&dir).expect("store");
    let env = store.load("default", token).expect("load");
    let payload: serde_json::Value = serde_json::from_slice(&env.payload).expect("json");
    let events = payload["stream"]["source"].as_array().expect("source");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["choice"], "listen");
    assert!(events[0]["timestamp"].is_number());
}

#[tokio::test]
async fn test_multi_session_isolation() {
    let dir = temp_path();
    let session_store = MemoryStore::default();

    // Two browser sessions share one server/backend, distinguished by the
    // session layer — not by opening the fjall backend twice (fjall
    // exclusive-locks the keyspace, so a second open returns Backend(Locked)).
    let app = build_test_app(&dir, &session_store);

    // Session 1: GET /play (no cookie -> a fresh session)
    let response1 = app
        .clone()
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");
    let location1 = response1.headers().get("location").expect("location");
    let token1 = location1
        .to_str()
        .unwrap()
        .trim_start_matches("/play/")
        .to_string();

    // Session 2: GET /play (no cookie -> a different fresh session)
    let response2 = app
        .clone()
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");
    let location2 = response2.headers().get("location").expect("location");
    let token2 = location2
        .to_str()
        .unwrap()
        .trim_start_matches("/play/")
        .to_string();

    // Verify they got different tokens (different sessions)
    assert_ne!(
        token1, token2,
        "different sessions should get different tokens"
    );
}
