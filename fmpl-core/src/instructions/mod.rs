//! Instruction execution using trait-based dispatch with macro helpers
//!
//! This module provides:
//! - Trait-based instruction dispatch for clean separation
//! - Macros to reduce boilerplate in handler implementations
//! - Category-specific handler modules

// Make macros available to this module's children
pub mod macros;

// Instruction execution trait and dispatch
mod dispatch;

// Handler modules organized by category
pub mod arithmetic;
pub mod control_flow;
pub mod functions;
pub mod objects;

// Re-export the key types
pub use dispatch::{ExecuteResult, InstructionHandler};
