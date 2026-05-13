//! Fjall-backed [`Store`] implementation.
//!
//! Wraps `fjall::Keyspace` behind the abstract `Store` trait. This is
//! the only module in the crate that names `fjall::*` directly.

use crate::{Store, StoreError};

impl From<fjall::Error> for StoreError {
    fn from(err: fjall::Error) -> Self {
        StoreError::backend(err)
    }
}

/// `Store` implementation backed by a fjall keyspace.
///
/// Construction takes an opened `fjall::Keyspace`; the caller owns the
/// open path and lifecycle.
pub struct FjallStore {
    keyspace: fjall::Keyspace,
}

impl FjallStore {
    /// Wrap an already-opened fjall keyspace.
    pub fn new(keyspace: fjall::Keyspace) -> Self {
        Self { keyspace }
    }

    /// Open a fjall database + default keyspace at `path` and return a
    /// `FjallStore` wrapping it.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let db = fjall::Database::builder(path.as_ref()).open()?;
        let keyspace = db.keyspace("default", fjall::KeyspaceCreateOptions::default)?;
        Ok(Self { keyspace })
    }

    /// Escape hatch for callers that need a fjall capability the
    /// abstract [`crate::Store`] trait deliberately doesn't expose.
    ///
    /// Two current use cases:
    /// 1. **Integration tests** under `fmpl-persistence/tests/` that
    ///    seed synthetic records by bypassing the envelope writer.
    /// 2. **Source-store compaction** ([`crate::source_store::
    ///    SourceStore::compact`]) needs `Keyspace::remove`. Promoting
    ///    `remove` to the `Store` trait would force every future
    ///    backend (read-only, append-only, in-memory) to implement
    ///    delete semantics they may not have. Per ITER-0005b R-J-C-1
    ///    PAR finding, the escape hatch is the principled call: one
    ///    concrete consumer that will always be FjallStore-backed.
    ///
    /// Hidden from rustdoc and explicitly NOT part of the API contract.
    /// Production code MUST go through the `Store` trait; consumers of
    /// `fmpl-persistence` MUST NOT name `fjall::*` in their surface.
    /// The method survives only because integration tests + the
    /// in-crate `SourceStore::compact` need it.
    #[doc(hidden)]
    pub fn keyspace(&self) -> &fjall::Keyspace {
        &self.keyspace
    }
}

impl Store for FjallStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        let bytes = self.keyspace.get(key)?;
        Ok(bytes.map(|s| s.to_vec()))
    }

    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.keyspace.insert(key, value)?;
        Ok(())
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>), StoreError>> + '_> {
        Box::new(self.keyspace.iter().map(|guard| {
            let (key, value) = guard.into_inner()?;
            Ok((key.to_vec(), value.to_vec()))
        }))
    }

    /// Native fjall v3 `is_empty()` — answered cheaply without
    /// walking the keyspace.
    fn is_empty(&self) -> Result<bool, StoreError> {
        Ok(self.keyspace.is_empty()?)
    }
}
