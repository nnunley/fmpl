//! VM submodules
//!
//! This directory contains extracted modules from vm.rs
//! to reduce the main file size and improve organization.

mod frame;
mod parse_state;

// Re-export public types
pub use frame::Frame;
pub use parse_state::{ParseCheckpoint, ParseState};
