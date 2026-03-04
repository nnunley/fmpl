//! Tests for ObjectDb persistence to Fjall storage (AC-P3-1).

#![cfg(feature = "fjall-persistence")]

use fmpl_core::object::ObjectDb;
use fmpl_core::value::Value;
use smol_str::SmolStr;

/// AC-1: ObjectDb.save_to_fjall() serializes all objects to Fjall partition.
/// AC-2: ObjectDb.load_from_fjall() restores objects with properties intact.
/// AC-3: Object IDs are preserved across save/restore.
#[test]
fn object_survives_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let _db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = _db
        .keyspace("objects", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    // Create object with property
    let mut db = ObjectDb::new();
    let id = db.create(None);
    db.set_property(id, SmolStr::new("name"), Value::String("test".into()))
        .unwrap();
    db.set_property(id, SmolStr::new("count"), Value::Int(42))
        .unwrap();

    // Save to Fjall
    db.save_to_fjall(&keyspace).unwrap();

    // Create new ObjectDb and load from Fjall
    let mut db2 = ObjectDb::new();
    db2.load_from_fjall(&keyspace).unwrap();

    // Verify object restored with same ID and properties
    assert_eq!(db2.get(id).unwrap().id, id);
    assert_eq!(
        db2.get_property(id, "name"),
        Some(Value::String("test".into()))
    );
    assert_eq!(db2.get_property(id, "count"), Some(Value::Int(42)));
}

/// AC-4: Prototype chains (parent references) survive round-trip.
#[test]
fn prototype_chain_survives_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let _db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = _db
        .keyspace("objects", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    // Create parent object
    let mut db = ObjectDb::new();
    let parent_id = db.create(None);
    db.set_property(
        parent_id,
        SmolStr::new("inherited"),
        Value::String("value".into()),
    )
    .unwrap();

    // Create child object with parent
    let child_id = db.create(Some(parent_id));
    db.set_property(
        child_id,
        SmolStr::new("own"),
        Value::String("property".into()),
    )
    .unwrap();

    // Save and restore
    db.save_to_fjall(&keyspace).unwrap();

    let mut db2 = ObjectDb::new();
    db2.load_from_fjall(&keyspace).unwrap();

    // Verify prototype chain is restored
    assert_eq!(db2.get(child_id).unwrap().parent, Some(parent_id));
    assert_eq!(
        db2.get_property(child_id, "own"),
        Some(Value::String("property".into()))
    );
    assert_eq!(
        db2.get_property(child_id, "inherited"),
        Some(Value::String("value".into()))
    );
}

/// AC-5: Multiple objects survive save/restore.
#[test]
fn multiple_objects_survive_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let _db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = _db
        .keyspace("objects", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let mut db = ObjectDb::new();
    let id1 = db.create(None);
    let id2 = db.create(None);
    let id3 = db.create(Some(id1)); // child of id1

    db.set_property(id1, SmolStr::new("x"), Value::Int(1))
        .unwrap();
    db.set_property(id2, SmolStr::new("y"), Value::Int(2))
        .unwrap();
    db.set_property(id3, SmolStr::new("z"), Value::Int(3))
        .unwrap();

    db.save_to_fjall(&keyspace).unwrap();

    let mut db2 = ObjectDb::new();
    db2.load_from_fjall(&keyspace).unwrap();

    // All objects present with correct IDs and properties
    assert!(db2.get(id1).is_some());
    assert!(db2.get(id2).is_some());
    assert!(db2.get(id3).is_some());

    assert_eq!(db2.get_property(id1, "x"), Some(Value::Int(1)));
    assert_eq!(db2.get_property(id2, "y"), Some(Value::Int(2)));
    assert_eq!(db2.get_property(id3, "z"), Some(Value::Int(3)));
    assert_eq!(db2.get(id3).unwrap().parent, Some(id1));
}

/// AC-3: next_id restored correctly so new objects don't collide.
#[test]
fn next_id_restored_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let _db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = _db
        .keyspace("objects", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let mut db = ObjectDb::new();
    let id1 = db.create(None);
    let id2 = db.create(None);
    let id3 = db.create(None);

    db.save_to_fjall(&keyspace).unwrap();

    let mut db2 = ObjectDb::new();
    db2.load_from_fjall(&keyspace).unwrap();

    // New objects created should not reuse old IDs
    let new_id = db2.create(None);
    assert!(new_id > id3);
    assert!(db2.get(new_id).is_some());
}
