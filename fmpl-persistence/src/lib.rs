//! FMPL persistence layer.
//!
//! Hosts the envelope writer, the loader (decode + iter_store), the
//! `Store` trait abstraction over backend key-value stores, and the
//! `FjallStore` impl (gated behind the `fjall-backend` feature).
//!
//! Consumers depend on this crate to read/write FMPL persisted records
//! without naming `fjall::*` in their public API. The storage backend
//! is an implementation detail.

pub mod checksum;
pub mod envelope;
pub mod hash_compute;
pub mod loader;
pub mod schema;
mod store;

#[cfg(feature = "fjall-backend")]
pub mod fjall_backend;
#[cfg(feature = "fjall-backend")]
pub mod recovery;
#[cfg(feature = "fjall-backend")]
pub mod source_store;

pub use fmpl_types::Hash;
pub use hash_compute::hash_bytes;
#[cfg(feature = "fjall-backend")]
pub use recovery::{RecoveryError, RecoveryStats, recover_incompatible};
#[cfg(feature = "fjall-backend")]
pub use source_store::{CompactStats, SourceStore};
pub use store::{Store, StoreError};
