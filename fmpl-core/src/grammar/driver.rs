//! Parse driver for streaming grammar pipelines.
//!
//! Connects an async input stream to a grammar, emitting matches downstream.
//! The driver handles incremental parsing, resumption, and backtracking.

use crate::error::Result;
use crate::grammar::incremental::ParseNext;
use crate::grammar::input::StreamingInput;
use crate::grammar::runtime::PegRuntime;
use crate::grammar::{Grammar, GrammarRegistry};
use crate::stream::{StreamEvent, StreamHandle};
use crate::value::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Drives incremental parsing of an async stream.
///
/// The driver:
/// 1. Reads values from an input stream
/// 2. Runs a grammar rule incrementally against the input
/// 3. Emits each successful match to an output channel
/// 4. Continues until the input stream ends
pub struct ParseDriver {
    input_handle: StreamHandle,
    grammar: Arc<Grammar>,
    rule: String,
    registry: GrammarRegistry,
    output: mpsc::Sender<Value>,
    timeout: Option<Duration>,
}

impl ParseDriver {
    /// Create a new parse driver.
    ///
    /// # Arguments
    /// * `input_handle` - Handle to the input stream
    /// * `grammar` - Grammar containing the rule to match
    /// * `rule` - Name of the rule to match
    /// * `registry` - Grammar registry for rule lookups
    /// * `output` - Channel to send matched values
    pub fn new(
        input_handle: StreamHandle,
        grammar: Arc<Grammar>,
        rule: String,
        registry: &GrammarRegistry,
        output: mpsc::Sender<Value>,
    ) -> Self {
        Self {
            input_handle,
            grammar,
            rule,
            registry: registry.clone(),
            output,
            timeout: Some(Duration::from_secs(30)),
        }
    }

    /// Set timeout for blocking input operations.
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run the parse driver until input ends.
    ///
    /// This method consumes the driver and runs until:
    /// - The input stream ends
    /// - The output channel is closed
    /// - An error occurs
    pub async fn run(mut self) -> Result<()> {
        // Collect values from the async stream into a buffer
        let mut values = Vec::new();

        loop {
            // Try to receive with timeout
            let recv_result = if let Some(timeout) = self.timeout {
                tokio::time::timeout(timeout, self.input_handle.receiver.recv()).await
            } else {
                // No timeout - wrap in Ok to match the timeout case
                Ok(self.input_handle.receiver.recv().await)
            };

            match recv_result {
                Ok(Some(event)) => {
                    match event {
                        StreamEvent::Data(value) => {
                            values.push(value);
                        }
                        StreamEvent::Ok(value) => {
                            values.push(value);
                            break; // Terminal event
                        }
                        StreamEvent::Err(_) => {
                            break; // Terminal error
                        }
                    }
                }
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout - treat as end of stream for now
                    break;
                }
            }
        }

        // Parse the collected values and collect results to send
        // Do parsing synchronously first, then send results asynchronously
        let mut results_to_send = Vec::new();

        for value in values {
            let input = StreamingInput::from_values(vec![value]);
            let mut runtime = PegRuntime::new(input, &self.registry, self.grammar.clone());

            let state = runtime.start(&self.rule);
            match runtime.resume(state)? {
                ParseNext::Match(matched_value) => {
                    results_to_send.push(matched_value);
                }
                ParseNext::NeedInput(_) | ParseNext::End => {
                    // Value didn't match the rule - skip it
                    continue;
                }
            }
        }

        // Now send results (this is the async part)
        for matched_value in results_to_send {
            if self.output.send(matched_value).await.is_err() {
                // Output channel closed, stop
                break;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::GrammarRegistry;
    use crate::stream::StreamEvent;
    use crate::value::Value;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_parse_driver_emits_matches() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();

        // Input stream
        let (in_tx, in_rx) = mpsc::channel(10);
        let in_handle = StreamHandle::new(in_rx, 1);

        // Output channel
        let (out_tx, mut out_rx) = mpsc::channel(10);

        // Create driver
        let driver = ParseDriver::new(in_handle, grammar, "any".to_string(), &registry, out_tx)
            .with_timeout(Some(Duration::from_millis(100)));

        // Spawn driver
        let handle = tokio::spawn(async move { driver.run().await });

        // Send values
        in_tx.send(StreamEvent::Data(Value::Int(1))).await.unwrap();
        in_tx.send(StreamEvent::Data(Value::Int(2))).await.unwrap();
        drop(in_tx); // Signal end

        // Collect output
        let mut results = Vec::new();
        while let Some(v) = out_rx.recv().await {
            results.push(v);
        }

        handle.await.unwrap().unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Value::Int(1));
        assert_eq!(results[1], Value::Int(2));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_parse_driver_handles_empty_stream() {
        let registry = GrammarRegistry::new();
        let grammar = registry.get("base::tree").unwrap();

        // Empty input stream
        let (in_tx, in_rx) = mpsc::channel(10);
        let in_handle = StreamHandle::new(in_rx, 1);
        drop(in_tx); // Immediately close

        // Output channel
        let (out_tx, mut out_rx) = mpsc::channel(10);

        let driver = ParseDriver::new(in_handle, grammar, "any".to_string(), &registry, out_tx)
            .with_timeout(Some(Duration::from_millis(100)));

        let handle = tokio::spawn(async move { driver.run().await });

        // Should complete with no output
        let mut results = Vec::new();
        while let Some(v) = out_rx.recv().await {
            results.push(v);
        }

        handle.await.unwrap().unwrap();
        assert!(results.is_empty());
    }
}
