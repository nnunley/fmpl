//! Shared test helpers for fmpl-core integration tests.
//!
//! Each integration test file that wants these helpers declares `mod common;`
//! at its top, which Cargo compiles as a per-test-crate module. This is the
//! idiomatic Rust pattern for sharing helpers between `tests/*.rs` files.

#![allow(dead_code)]

pub mod comment_strip;
pub mod rust_string_scanner;
