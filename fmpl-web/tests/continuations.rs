use std::time::{SystemTime, UNIX_EPOCH};

use fmpl_web::continuations::{ContinuationStore, SnapshotEnvelope};

fn temp_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("fmpl-web-continuations-{}", nanos));
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
