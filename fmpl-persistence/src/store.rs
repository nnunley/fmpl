//! Abstract storage backend for the persistence layer.
//!
//! The `Store` trait sizes to exactly what the envelope writer and
//! loader need today: point-key get/insert and full-keyspace iter.
//! Implementations live in backend-specific modules behind feature
//! gates (e.g. [`crate::fjall_backend::FjallStore`] under
//! `fjall-backend`).
//!
//! ## Trait design notes
//!
//! - `iter()` returns `Box<dyn Iterator>` with owned `Vec<u8>` items.
//!   Trade-off: one Vec allocation per record at bootup-scan time, in
//!   exchange for stable Rust (no GATs), no lifetime acrobatics, and a
//!   trait that can be made into a trait-object. The loader is
//!   bootup-scale, not per-message — the allocation cost is acceptable.
//!   A future `iter_borrowed()` returning borrowed slices is
//!   non-breaking to add when a measured perf need surfaces.
//! - `Store: Send + Sync` supertrait makes the trait usable from
//!   `Arc<dyn Store + Send + Sync>` field positions (used by
//!   `OverflowStore`/`MemoStore` in fmpl-core's grammar/stream_input
//!   to avoid generic parameter cascades).
//! - `is_empty()` was added in ITER-0005a.6 when fmpl-web's
//!   `ImageStore::bootstrap_if_empty` became the first real consumer.
//!   Default impl propagates iterator errors (NOT `is_none()`, which
//!   would swallow them — per the 0005a.5 R3-C2 PAR finding).
//!   Backends with a native cheaper check should override.

use thiserror::Error;

/// Errors surfaced by `Store` implementations.
///
/// `Backend` boxes the underlying error to keep the trait
/// backend-agnostic. Implementations wrap their native errors (e.g.
/// `From<fjall::Error> for StoreError`) inside the backend variant.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("backend error: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync>),
}

impl StoreError {
    pub fn backend<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        Self::Backend(Box::new(err))
    }
}

/// One step of [`Store::iter`]: either a `(key, value)` pair or a
/// backend failure. Named so the trait's iterator type fits in one
/// line at call sites and doesn't trip clippy's type-complexity lint.
pub type StoreIterItem = Result<(Vec<u8>, Vec<u8>), StoreError>;

/// Boxed iterator returned by [`Store::iter`]. Lifetime-borrowed from
/// the `&self` receiver so iteration cannot outlive the store.
pub type StoreIter<'a> = Box<dyn Iterator<Item = StoreIterItem> + 'a>;

/// Abstract key-value store interface.
///
/// Sized to fmpl-core's existing persistence consumers (envelope
/// writer, loader, the 4 save/load paths in compiler.rs, object.rs,
/// grammar/{incremental,stream_input}.rs).
pub trait Store: Send + Sync {
    /// Point-key lookup. Returns `Ok(None)` if the key is absent;
    /// `Ok(Some(bytes))` if present; `Err` only on backend failure.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;

    /// Point-key write. Overwrites any existing value at `key`.
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), StoreError>;

    /// Iterate the entire keyspace. Each step yields either a
    /// `(key, value)` pair or an error. Iteration order is
    /// implementation-defined; consumers must not rely on it.
    fn iter(&self) -> StoreIter<'_>;

    /// Returns `true` iff the store currently has no records.
    ///
    /// Default impl walks one step of [`Self::iter`]; backends that
    /// can answer this cheaply (e.g. via a native length / row-count
    /// API) should override.
    ///
    /// Iterator errors propagate via `Err` — do NOT swallow them by
    /// returning `Ok(false)`. A backend failure on the first step is
    /// observably different from "the store has at least one record."
    /// Per ITER-0005a.5 R3-C2 PAR finding.
    fn is_empty(&self) -> Result<bool, StoreError> {
        match self.iter().next() {
            None => Ok(true),
            Some(Ok(_)) => Ok(false),
            Some(Err(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    //! In-source tests for the `Store` trait's default impls.
    //!
    //! Backend-specific tests (FjallStore round-trips, etc.) live as
    //! integration tests in `tests/`. These tests validate the default
    //! impl semantics via synthetic Store implementations.

    use super::*;

    /// Synthetic Store that returns the configured iterator outcomes
    /// from `iter().next()` and panics on `get`/`insert`. Lets the
    /// default-impl tests target `is_empty()` precisely.
    struct ScriptedStore {
        first_iter_step: fn() -> Option<StoreIterItem>,
    }

    impl Store for ScriptedStore {
        fn get(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
            unreachable!("ScriptedStore::get not used by is_empty()");
        }
        fn insert(&self, _key: &[u8], _value: &[u8]) -> Result<(), StoreError> {
            unreachable!("ScriptedStore::insert not used by is_empty()");
        }
        fn iter(&self) -> StoreIter<'_> {
            let first = (self.first_iter_step)();
            Box::new(first.into_iter())
        }
    }

    #[test]
    fn is_empty_returns_true_on_empty_iter() {
        let store = ScriptedStore {
            first_iter_step: || None,
        };
        assert!(store.is_empty().unwrap());
    }

    #[test]
    fn is_empty_returns_false_when_iter_yields_record() {
        let store = ScriptedStore {
            first_iter_step: || Some(Ok((b"k".to_vec(), b"v".to_vec()))),
        };
        assert!(!store.is_empty().unwrap());
    }

    /// R3-C2 regression guard: `is_empty()` must propagate iterator
    /// errors via `Err`, NOT swallow them by returning `Ok(false)`.
    /// A backend failure on the first step is observably different
    /// from "the store has at least one record."
    #[test]
    fn is_empty_propagates_iterator_error() {
        let store = ScriptedStore {
            first_iter_step: || {
                Some(Err(StoreError::backend(std::io::Error::other(
                    "synthetic backend failure",
                ))))
            },
        };
        let result = store.is_empty();
        assert!(
            result.is_err(),
            "is_empty must propagate iterator errors, not swallow them; got {:?}",
            result
        );
    }
}
