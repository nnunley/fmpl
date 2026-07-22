//! Re-export shim for the persistence layer.
//!
//! The on-disk image persistence implementation lives in the
//! [`fmpl_persistence`] crate (extracted in ITER-0005a.5 T0). This
//! module preserves the qualified paths `fmpl_core::persistence::*`
//! that pre-extraction consumers used, so downstream code keeps
//! compiling without per-call-site path rewrites.
//!
//! New code is free to import from `fmpl_persistence::*` directly;
//! both routes resolve to the same items.

pub use fmpl_persistence::{Store, StoreError};
pub use fmpl_persistence::{checksum, envelope, loader, schema};

#[cfg(feature = "persistence")]
pub use fmpl_persistence::fjall_backend;
