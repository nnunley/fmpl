//! Time-related built-ins for FMPL.

use crate::error::Result;
use crate::value::Value;
use std::thread;
use std::time::Duration;

/// The time built-in object for time-related operations.
pub struct TimeBuiltin;

impl TimeBuiltin {
    /// Sleep for the specified number of milliseconds.
    ///
    /// Arguments:
    /// - ms: Duration to sleep in milliseconds (integer)
    ///
    /// Returns null after the sleep completes.
    ///
    /// # Notes
    ///
    /// - Negative durations are treated as 0 (no sleep)
    /// - This blocks the current thread during sleep
    /// - For async sleep, use the async stream operator `<-` with a future
    pub fn sleep(ms: i64) -> Result<Value> {
        let duration = if ms < 0 { 0 } else { ms };
        thread::sleep(Duration::from_millis(duration as u64));
        Ok(Value::Null)
    }
}
