use axum::Extension;
use axum::Router;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use fmpl_core::Vm;
use std::path::Path as FsPath;
use std::sync::{Arc, Mutex};

use crate::continuations::{ContinuationStore, SnapshotEnvelope};
use crate::image_store::ImageStore;

#[derive(Clone)]
pub struct AppState {
    pub vm: Arc<Mutex<Vm>>,
    pub continuations: Arc<ContinuationStore>,
    pub image: Arc<ImageStore>,
}

pub fn build_app(data_dir: impl AsRef<FsPath>) -> crate::continuations::Result<Router> {
    let image = ImageStore::new(&data_dir)?;
    let seed_path = FsPath::new(env!("CARGO_MANIFEST_DIR"))
        .join("seed")
        .join("seed.fmpl");
    image.bootstrap_if_empty(seed_path.to_str().unwrap_or("fmpl-web/seed/seed.fmpl"))?;
    let continuations = ContinuationStore::new(&data_dir)?;
    let vm = Arc::new(Mutex::new(Vm::new()));

    let state = AppState {
        vm,
        continuations: Arc::new(continuations),
        image: Arc::new(image),
    };

    Ok(Router::new()
        .route("/play", get(play_start))
        .route("/play/{token}", get(play_token))
        .layer(Extension(state)))
}

async fn play_start(Extension(state): Extension<AppState>) -> impl IntoResponse {
    let session_id = "default";
    let token = match state
        .continuations
        .save(session_id, SnapshotEnvelope::new(Vec::new(), "rkyv-v1"))
    {
        Ok(token) => token,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Redirect::to(&format!("/play/{}", token)).into_response()
}

async fn play_token(
    Extension(state): Extension<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    let session_id = "default";
    match state.continuations.load(session_id, &token) {
        Ok(_env) => (StatusCode::OK, "<div>storylet</div>").into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
