//! Compile-time VM version constants stamped into every persisted record.
//!
//! Derived from `env!("CARGO_PKG_VERSION")` at fmpl-core compile time,
//! parsed via [`fmpl_types::parse_version_part`]. Per ITER-0005a.5
//! R2-C1, the version constants stay in fmpl-core: fmpl-persistence is
//! version-agnostic, so the writer takes a [`VmVersion`] argument and
//! callers pass [`VM_VERSION`] explicitly. The major-version
//! compatibility check at the loader uses [`VM_VERSION_MAJOR`].

use fmpl_types::{VmVersion, parse_version_part};

pub const VM_VERSION_MAJOR: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 0);
pub const VM_VERSION_MINOR: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 1);
pub const VM_VERSION_PATCH: u16 = parse_version_part(env!("CARGO_PKG_VERSION"), 2);

/// VM version this fmpl-core build was compiled at. Stamped by every
/// envelope writer; the loader's compatibility check compares the
/// `major` field against the running VM's [`VM_VERSION_MAJOR`].
pub const VM_VERSION: VmVersion =
    VmVersion::new(VM_VERSION_MAJOR, VM_VERSION_MINOR, VM_VERSION_PATCH);
