//! Built-in objects and functions for FMPL.

pub mod ast;
pub mod curl;
pub mod io;
pub mod ir;
pub mod rand;
pub mod sse;
pub mod time;

pub use curl::CurlBuiltin;
pub use io::{EnvBuiltin, IoBuiltin};
pub use rand::RandBuiltin;
pub use sse::SseBuiltin;
pub use time::TimeBuiltin;
