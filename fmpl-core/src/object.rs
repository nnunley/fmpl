//! In-memory object database for FMPL.

use crate::compiler::CompiledCode;
use crate::error::{Error, Result};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Object identifier.
pub type ObjectId = u64;

/// A stored method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
}

/// A facet definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facet {
    pub members: Vec<SmolStr>,
    pub terminal: bool,
}

/// An FMPL object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: ObjectId,
    pub parent: Option<ObjectId>,
    pub properties: HashMap<SmolStr, Value>,
    pub methods: HashMap<SmolStr, Method>,
    pub facets: HashMap<SmolStr, Facet>,
}

impl Object {
    pub fn new(id: ObjectId, parent: Option<ObjectId>) -> Self {
        Self {
            id,
            parent,
            properties: HashMap::new(),
            methods: HashMap::new(),
            facets: HashMap::new(),
        }
    }
}

/// The object database.
#[derive(Debug)]
pub struct ObjectDb {
    objects: HashMap<ObjectId, Object>,
    next_id: ObjectId,
    /// Named objects (like @merchant, ^thing).
    named: HashMap<SmolStr, ObjectId>,
}

impl ObjectDb {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_id: 1,
            named: HashMap::new(),
        }
    }

    /// Create a new object with optional parent.
    pub fn create(&mut self, parent: Option<ObjectId>) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(id, Object::new(id, parent));
        id
    }

    /// Register a named object.
    pub fn register_name(&mut self, name: SmolStr, id: ObjectId) {
        self.named.insert(name, id);
    }

    /// Look up a named object.
    pub fn lookup_name(&self, name: &str) -> Option<ObjectId> {
        self.named.get(name).copied()
    }

    /// Iterate named objects (name, id).
    pub fn named_objects(&self) -> impl Iterator<Item = (&SmolStr, &ObjectId)> {
        self.named.iter()
    }

    /// Get an object by ID.
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Get an object mutably by ID.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    /// Get a property, following the prototype chain.
    pub fn get_property(&self, id: ObjectId, name: &str) -> Option<Value> {
        let obj = self.objects.get(&id)?;

        if let Some(val) = obj.properties.get(name) {
            return Some(val.clone());
        }

        if let Some(parent) = obj.parent {
            return self.get_property(parent, name);
        }

        None
    }

    /// Set a property on an object.
    pub fn set_property(&mut self, id: ObjectId, name: SmolStr, value: Value) -> Result<()> {
        let obj = self.objects.get_mut(&id).ok_or(Error::ObjectNotFound(id))?;
        obj.properties.insert(name, value);
        Ok(())
    }

    /// Get a method, following the prototype chain.
    pub fn get_method(&self, id: ObjectId, name: &str) -> Option<&Method> {
        let obj = self.objects.get(&id)?;

        if let Some(method) = obj.methods.get(name) {
            return Some(method);
        }

        if let Some(parent) = obj.parent {
            return self.get_method(parent, name);
        }

        None
    }

    /// Define a method on an object.
    pub fn define_method(&mut self, id: ObjectId, name: SmolStr, method: Method) -> Result<()> {
        let obj = self.objects.get_mut(&id).ok_or(Error::ObjectNotFound(id))?;
        obj.methods.insert(name, method);
        Ok(())
    }

    /// Get a facet definition, following the prototype chain.
    pub fn get_facet(&self, id: ObjectId, name: &str) -> Option<&Facet> {
        let obj = self.objects.get(&id)?;

        if let Some(facet) = obj.facets.get(name) {
            return Some(facet);
        }

        if let Some(parent) = obj.parent {
            return self.get_facet(parent, name);
        }

        None
    }

    /// Define a facet on an object.
    pub fn define_facet(&mut self, id: ObjectId, name: SmolStr, facet: Facet) -> Result<()> {
        let obj = self.objects.get_mut(&id).ok_or(Error::ObjectNotFound(id))?;
        obj.facets.insert(name, facet);
        Ok(())
    }

    /// Check if a member is accessible through a facet.
    pub fn facet_allows(&self, id: ObjectId, facet_name: &str, member: &str) -> bool {
        if let Some(facet) = self.get_facet(id, facet_name) {
            facet.members.iter().any(|m| m == member)
        } else {
            false
        }
    }

    /// Save all objects to a [`Store`][crate::persistence::Store].
    ///
    /// Routes through [`fmpl_persistence::envelope::write`]. Two
    /// PayloadKind variants are emitted per save:
    /// `PayloadKind::ObjectIndex` (0x02) for the `__object_ids__` index,
    /// then `PayloadKind::ObjectRecord` (0x01) per object. Both shapes
    /// carry [`Hash::NONE`][fmpl_types::Hash::NONE] until the
    /// content-addressed source store ships.
    pub fn save_to_store<S: crate::persistence::Store>(&self, store: &S) -> Result<()> {
        use crate::persistence::envelope::write;
        use crate::persistence::schema::PayloadKind;
        use fmpl_types::Hash;

        // Index record: list of object IDs for efficient loading
        let ids: Vec<u64> = self.objects.keys().copied().collect();
        write(
            store,
            b"__object_ids__",
            &ids,
            PayloadKind::ObjectIndex,
            crate::VM_VERSION,
            Hash::NONE,
        )
        .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?;

        // Per-object records
        for (id, object) in &self.objects {
            let key = format!("obj:{}", id);
            write(
                store,
                key.as_bytes(),
                object,
                PayloadKind::ObjectRecord,
                crate::VM_VERSION,
                Hash::NONE,
            )
            .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?;
        }

        Ok(())
    }

    /// Load all objects from a [`Store`][crate::persistence::Store].
    ///
    /// **TODO(ITER-0005a.4):** Transitional manual prefix-strip. On-disk
    /// values have a 56-byte envelope header followed by the serialized
    /// payload; this will be replaced with `loader::decode(&bytes)`.
    pub fn load_from_store<S: crate::persistence::Store>(&mut self, store: &S) -> Result<()> {
        use crate::persistence::envelope::ENVELOPE_HEADER_SIZE;

        fn strip_envelope(bytes: &[u8]) -> Result<&[u8]> {
            if bytes.len() < ENVELOPE_HEADER_SIZE {
                return Err(Error::ObjectPersistenceError(format!(
                    "value too short for envelope header: {} bytes",
                    bytes.len()
                )));
            }
            Ok(&bytes[ENVELOPE_HEADER_SIZE..])
        }

        // Load object IDs list (index record)
        let ids_bytes = store
            .get(b"__object_ids__")
            .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?;

        let ids: Vec<u64> = match ids_bytes {
            Some(bytes) => {
                let payload = strip_envelope(&bytes)?;
                serde_json::from_slice(payload)
                    .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?
            }
            None => Vec::new(), // No objects saved yet
        };

        // Load each object record
        for id in ids {
            let key = format!("obj:{}", id);
            let obj_bytes = store
                .get(key.as_bytes())
                .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?;

            if let Some(bytes) = obj_bytes {
                let payload = strip_envelope(&bytes)?;
                let object: Object = serde_json::from_slice(payload)
                    .map_err(|e| Error::ObjectPersistenceError(e.to_string()))?;
                self.objects.insert(id, object);

                // Update next_id to avoid collisions
                if id >= self.next_id {
                    self.next_id = id + 1;
                }
            }
        }

        Ok(())
    }
}

impl Default for ObjectDb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_object() {
        let mut db = ObjectDb::new();
        let id = db.create(None);
        assert!(db.get(id).is_some());
    }

    #[test]
    fn test_property_inheritance() {
        let mut db = ObjectDb::new();
        let parent = db.create(None);
        let child = db.create(Some(parent));

        db.set_property(
            parent,
            SmolStr::new("name"),
            Value::String(SmolStr::new("parent")),
        )
        .unwrap();

        // Child inherits parent's property
        let val = db.get_property(child, "name");
        assert!(matches!(val, Some(Value::String(s)) if s == "parent"));

        // Child can override
        db.set_property(
            child,
            SmolStr::new("name"),
            Value::String(SmolStr::new("child")),
        )
        .unwrap();
        let val = db.get_property(child, "name");
        assert!(matches!(val, Some(Value::String(s)) if s == "child"));
    }

    #[test]
    fn test_named_objects() {
        let mut db = ObjectDb::new();
        let id = db.create(None);
        db.register_name(SmolStr::new("merchant"), id);

        assert_eq!(db.lookup_name("merchant"), Some(id));
        assert_eq!(db.lookup_name("unknown"), None);
    }
}
