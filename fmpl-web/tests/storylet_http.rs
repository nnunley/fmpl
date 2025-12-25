use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use fmpl_web::storylet::build_app;

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("fmpl-web-storylet-{}", nanos));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

#[tokio::test]
async fn test_play_route_redirects() {
    let dir = temp_path();
    let app = build_app(&dir).expect("app");

    let response = app
        .oneshot(Request::get("/play").body(Body::empty()).unwrap())
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers().get("location").expect("location");
    assert!(location.to_str().unwrap().starts_with("/play/"));
}
