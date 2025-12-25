use std::time::{SystemTime, UNIX_EPOCH};

use fmpl_web::image_store::ImageStore;

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("fmpl-web-seed-{}", nanos));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn seed_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("seed")
        .join("seed.fmpl")
}

#[test]
fn test_seed_loader_bootstraps_image() {
    let dir = temp_path();
    let store = ImageStore::new(&dir).expect("create store");
    store
        .bootstrap_if_empty(seed_path().to_str().expect("seed path"))
        .expect("bootstrap");
    assert!(store.has_object("storylet").expect("lookup"));
}
