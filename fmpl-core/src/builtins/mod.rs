//! Built-in objects and functions for FMPL.

pub mod ast;
pub mod bridge;
pub mod bytes;
pub mod codegen;
#[cfg(all(feature = "curl-builtin", not(target_arch = "wasm32")))]
pub mod curl;
pub mod grammar_to_ir;
pub mod grammar_to_rust;
pub mod human;
pub mod io;
pub mod ir;
pub mod ir_to_rust;
pub mod rand;
pub mod runtime;
pub mod sse;
pub mod time;

pub use bridge::{CompiledExpr, FmplBridge, FunctionRegistry, RustFunction, eval_fmpl};
#[cfg(all(feature = "curl-builtin", not(target_arch = "wasm32")))]
pub use curl::CurlBuiltin;
pub use human::HumanBuiltin;
pub use io::{EnvBuiltin, IoBuiltin};
pub use rand::RandBuiltin;
pub use sse::SseBuiltin;
pub use time::TimeBuiltin;
