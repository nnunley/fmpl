# Persistence Patterns

Feature flag: `persistence` (optional, enabled in tests with `--features persistence`). Fjall is the underlying KV store used by this feature.

## Serialization: serde JSON currently, migrating to rkyv

Current persistence uses **serde_json**. The target is **rkyv** (zero-copy deserialization) which is already a dependency and has derives on some types. New persistence code should use rkyv where possible — `rkyv::to_bytes` / `rkyv::from_bytes`. Fall back to serde only if a type doesn't yet have `Archive, RkyvSerialize, RkyvDeserialize` derives.

## Save/Load Pattern

All persistence follows the same pattern:

```rust
#[cfg(feature = "persistence")]
pub fn save_to_fjall(&self, keyspace: &fjall::Keyspace, key: &str) -> Result<()> {
    let bytes = serde_json::to_vec(self)?;
    keyspace.insert(key.as_bytes(), bytes)?;
    Ok(())
}

#[cfg(feature = "persistence")]
pub fn load_from_fjall(keyspace: &fjall::Keyspace, key: &str) -> Result<Option<Self>> {
    match keyspace.get(key.as_bytes())? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}
```

Errors wrap into `Error::BytecodePersistenceError(String)` or `Error::ObjectPersistenceError(String)`.

## Keyspaces

| Keyspace | Type | Key Format | Location |
|----------|------|------------|----------|
| `"bytecode"` | `CompiledCode` | `&str` as bytes | `compiler.rs:694-718` |
| `"objects"` | `ObjectDb` | `"obj:{id}"` + `"__object_ids__"` | `object.rs:187-243` |
| `"parse_states"` | `ParseState` | session ID bytes | `grammar/incremental.rs:71-93` |

## Test Pattern

```rust
#![cfg(feature = "persistence")]

#[test]
fn test_name() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let keyspace = db
        .keyspace("name", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    // ... use keyspace, auto-cleanup on drop
}
```

## Types with serde derives

**Fully serializable:** `CompiledCode`, `Instruction`, `InstrIndex`, `ConstIndex`, `Value` (most variants), `Object`, `Method`, `Facet`, `Lambda`, `Stream`, `StreamOp`, `ParseState`.

**Skipped (`#[serde(skip)]`):** `Facet` (value variant), `AsyncStream`, `Sink`, `TupleSpace`, `TupleSpaceFacet`, `Cursor`, `Code`, `ParseStream`. These are live handles that can't be restored.

## Gotchas

- `AsyncStream` and `Sink` serialize to suspended variants with reconnection metadata, not the live handle.
- `ObjectDb.next_id` must be restored correctly to prevent ID collisions.
- JSON is the current format but rkyv is the target. When adding new persistence, use `rkyv::to_bytes::<_, 256>(value)` / `rkyv::from_bytes::<T>(&bytes)`. Add `#[derive(Archive, rkyv::Serialize, rkyv::Deserialize)]` to types that don't have it yet.
