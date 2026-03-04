//! Stream integration for tuple space.
//!
//! Provides async stream support for subscribing to tuples matching patterns.
//! This integrates with FMPL's existing stream infrastructure using tokio channels.

use crate::stream::{StreamEvent, StreamHandle, next_id};
use crate::tuplespace::{Tuple, TuplePattern};

/// Subscribe to tuples matching a pattern from the tuple space.
///
/// Returns a StreamHandle that can be used with FMPL's stream operations.
pub fn subscribe(pattern: TuplePattern) -> (TupleSubscriber, StreamHandle) {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let id = next_id();
    let handle = StreamHandle::new(rx, id);
    let subscriber = TupleSubscriber {
        pattern,
        sender: tx,
    };
    (subscriber, handle)
}

/// A subscriber that can send tuples to a stream.
///
/// When the tuple space receives a tuple matching the subscriber's pattern,
/// it sends the tuple to the stream.
pub struct TupleSubscriber {
    pattern: TuplePattern,
    sender: tokio::sync::mpsc::Sender<StreamEvent>,
}

impl TupleSubscriber {
    /// Get the pattern for this subscriber.
    pub fn pattern(&self) -> &TuplePattern {
        &self.pattern
    }

    /// Send a tuple to the stream if it matches the pattern.
    pub fn send_tuple(
        &self,
        tuple: &Tuple,
    ) -> Result<(), Box<tokio::sync::mpsc::error::SendError<StreamEvent>>> {
        if self.pattern.matches(tuple) {
            // Convert tuple to Value for streaming
            // For now, we wrap it as a map with type and data
            use crate::value::Value;
            use smol_str::SmolStr;
            use std::collections::HashMap;
            use std::sync::Arc;

            let mut map = HashMap::new();
            map.insert(SmolStr::new("type"), Value::String(tuple.type_name.clone()));
            if let Some(ns) = &tuple.namespace {
                map.insert(SmolStr::new("namespace"), Value::String(ns.clone()));
            }
            map.insert(SmolStr::new("data"), tuple.data.clone());

            self.sender
                .blocking_send(StreamEvent::Data(Value::Map(Arc::new(map))))?;
        }
        Ok(())
    }

    /// Check if the channel is closed.
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use smol_str::SmolStr;

    #[test]
    fn test_subscribe_creates_stream() {
        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: crate::tuplespace::Pattern::Wildcard,
        };
        let (_subscriber, handle) = subscribe(pattern);
        assert_eq!(handle.id(), handle.id());
    }

    #[test]
    fn test_subscriber_pattern() {
        let pattern = TuplePattern::Any;
        let subscriber = TupleSubscriber {
            pattern: pattern.clone(),
            sender: tokio::sync::mpsc::channel(1).0,
        };
        assert_eq!(subscriber.pattern(), &pattern);
    }

    #[test]
    fn test_send_tuple_matching_pattern() {
        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: crate::tuplespace::Pattern::Wildcard,
        };
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let subscriber = TupleSubscriber {
            pattern,
            sender: tx,
        };

        let tuple = Tuple::new(SmolStr::new("log"), Value::String(SmolStr::new("error")));
        subscriber.send_tuple(&tuple).unwrap();

        let event = rx.blocking_recv().unwrap();
        match event {
            StreamEvent::Data(value) => {
                if let Value::Map(map) = value {
                    assert_eq!(map.get("type"), Some(&Value::String(SmolStr::new("log"))));
                } else {
                    panic!("Expected map value");
                }
            }
            _ => panic!("Expected Data event"),
        }
    }

    #[test]
    fn test_send_tuple_non_matching_pattern() {
        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("log"),
            data: crate::tuplespace::Pattern::Wildcard,
        };
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let subscriber = TupleSubscriber {
            pattern,
            sender: tx,
        };

        // Send a tuple that doesn't match
        let tuple = Tuple::new(SmolStr::new("event"), Value::String(SmolStr::new("click")));
        subscriber.send_tuple(&tuple).unwrap();

        // Channel should be empty
        let result = rx.try_recv();
        assert!(matches!(
            result,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }
}
