//! FMPL shared cross-crate types.
//!
//! This crate hosts types that must be referenced from multiple
//! workspace members without creating a dependency cycle. Today:
//!
//! - [`VmVersion`] — semver triple stamped into every persisted envelope
//!   and used for cross-version compatibility checks.
//! - [`Hash`] — blake3-derived 32-byte content hash newtype; carrier
//!   for source-bytes content addressing. The hashing primitive
//!   (`Hash::compute`) is deferred to ITER-0005b when the source
//!   store is wired; this crate ships only the carrier type +
//!   [`Hash::NONE`] sentinel.
//! - [`SourceHash`] — type alias for [`Hash`] used in
//!   `EnvelopeHeader.source_hash` API-edge positions.
//! - [`parse_version_part`] — const fn parsing a single component out
//!   of a `CARGO_PKG_VERSION` string. Lets consumer crates derive
//!   their own version constants without runtime parsing.

mod hash;
mod vm_version;

pub use hash::{Hash, SourceHash, no_source_hash};
pub use vm_version::{VmVersion, parse_version_part};
