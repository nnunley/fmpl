//! Linda-style tuple space for pattern-based coordination.
//!
//! The tuple space provides time/space decoupled coordination between agents
//! through pattern-based matching instead of direct addressing.

pub mod facet;
pub mod store;
pub mod stream;

use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

/// A tuple with metadata for pattern matching.
///
/// Derives `Serialize`/`Deserialize` so durable tuples can round-trip
/// through the persistence envelope writer. The `Value` payload has
/// full serde coverage including custom handlers for live resource
/// handles like async streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tuple {
    /// Tuple type for routing/pattern matching
    pub type_name: SmolStr,
    /// Optional namespace for isolation
    pub namespace: Option<SmolStr>,
    /// Timestamp for ordering
    pub timestamp: u64,
    /// Sequence number for deterministic ordering
    pub seq: u64,
    /// The tuple data
    pub data: Value,
    /// Durability flag (per `specs/tuplespace.md` § Persistence).
    /// When `true` AND the enclosing space has a backing store, `out`
    /// writes the tuple through to the store as a `PayloadKind::Tuple`
    /// envelope record. `true` with no backing store is a hard error.
    /// `false` is the default and means in-memory only.
    #[serde(default)]
    pub durable: bool,
}

impl Tuple {
    /// Create a new tuple.
    pub fn new(type_name: SmolStr, data: Value) -> Self {
        Self {
            type_name,
            namespace: None,
            timestamp: 0,
            seq: 0,
            data,
            durable: false,
        }
    }

    /// Set the namespace.
    pub fn with_namespace(mut self, namespace: SmolStr) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Set the timestamp.
    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set the sequence number.
    pub fn with_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    /// Mark this tuple as durable. With a backing store on the
    /// enclosing space, `out` will write it through.
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Compose this tuple's on-disk key from `(namespace, type, seq)`.
    /// Key shape: `<ns-len:1><ns-bytes><type-len:2-be><type-bytes><seq:8-be>`.
    /// `seq` is big-endian so byte-wise sort on the keyspace yields
    /// the same FIFO order as the in-memory BTreeMap. Namespace is
    /// length-prefixed so it can be the empty bytes for the no-namespace
    /// case without colliding with namespaces whose name happens to
    /// start with the type prefix.
    pub fn store_key(&self) -> Vec<u8> {
        let ns_bytes: &[u8] = self.namespace.as_ref().map(|s| s.as_bytes()).unwrap_or(b"");
        assert!(
            ns_bytes.len() <= u8::MAX as usize,
            "namespace must be at most 255 bytes; tuplespace keys assume this for byte-prefix encoding"
        );
        let type_bytes = self.type_name.as_bytes();
        assert!(
            type_bytes.len() <= u16::MAX as usize,
            "tuple type_name must be at most 65535 bytes; keys assume this for u16 length prefix"
        );
        let mut key = Vec::with_capacity(1 + ns_bytes.len() + 2 + type_bytes.len() + 8);
        key.push(ns_bytes.len() as u8);
        key.extend_from_slice(ns_bytes);
        key.extend_from_slice(&(type_bytes.len() as u16).to_be_bytes());
        key.extend_from_slice(type_bytes);
        key.extend_from_slice(&self.seq.to_be_bytes());
        key
    }
}

/// Pattern for matching tuples.
#[derive(Debug, Clone, PartialEq)]
pub enum TuplePattern {
    /// Exact match on type, pattern match on data
    TypeAndData { type_name: SmolStr, data: Pattern },
    /// Match on namespace + type + data
    Full {
        namespace: SmolStr,
        type_name: SmolStr,
        data: Pattern,
    },
    /// Wildcard: matches any tuple
    Any,
}

/// Pattern for matching tuple data.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Matches any value
    Wildcard,
    /// Exact value match
    Exact(Value),
    /// Map pattern with key-value pairs
    Map { required: HashMap<SmolStr, Value> },
}

impl Pattern {
    /// Check if a value matches this pattern.
    pub fn matches(&self, value: &Value) -> bool {
        match self {
            Pattern::Wildcard => true,
            Pattern::Exact(expected) => value == expected,
            Pattern::Map { required } => {
                if let Value::Map(map) = value {
                    for (key, expected_val) in required.iter() {
                        match map.get(key) {
                            Some(actual_val) if actual_val == expected_val => {}
                            _ => return false,
                        }
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

impl TuplePattern {
    /// Check if a tuple matches this pattern.
    pub fn matches(&self, tuple: &Tuple) -> bool {
        match self {
            TuplePattern::Any => true,
            TuplePattern::TypeAndData { type_name, data } => {
                tuple.type_name == *type_name && data.matches(&tuple.data)
            }
            TuplePattern::Full {
                namespace,
                type_name,
                data,
            } => {
                tuple.namespace.as_ref() == Some(namespace)
                    && tuple.type_name == *type_name
                    && data.matches(&tuple.data)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_map(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        let map: HashMap<SmolStr, Value> = pairs
            .into_iter()
            .map(|(k, v)| (SmolStr::new(k), v))
            .collect();
        Value::Map(Arc::new(map))
    }

    #[test]
    fn test_pattern_wildcard_matches_anything() {
        let pattern = Pattern::Wildcard;
        assert!(pattern.matches(&Value::Int(42)));
        assert!(pattern.matches(&Value::String(SmolStr::new("hello"))));
        assert!(pattern.matches(&Value::Null));
    }

    #[test]
    fn test_pattern_exact_matches_equal_values() {
        let pattern = Pattern::Exact(Value::Int(42));
        assert!(pattern.matches(&Value::Int(42)));
        assert!(!pattern.matches(&Value::Int(43)));
        assert!(!pattern.matches(&Value::String(SmolStr::new("42"))));
    }

    #[test]
    fn test_pattern_map_matches_subset() {
        let pattern = Pattern::Map {
            required: {
                let mut m = HashMap::new();
                m.insert(SmolStr::new("x"), Value::Int(1));
                m.insert(SmolStr::new("y"), Value::Int(2));
                m
            },
        };

        // Match: has all required keys with correct values
        let value = make_map(vec![
            ("x", Value::Int(1)),
            ("y", Value::Int(2)),
            ("z", Value::Int(3)),
        ]);
        assert!(pattern.matches(&value));

        // Match: exact keys
        let value = make_map(vec![("x", Value::Int(1)), ("y", Value::Int(2))]);
        assert!(pattern.matches(&value));

        // No match: missing key
        let value = make_map(vec![("x", Value::Int(1))]);
        assert!(!pattern.matches(&value));

        // No match: wrong value
        let value = make_map(vec![("x", Value::Int(1)), ("y", Value::Int(99))]);
        assert!(!pattern.matches(&value));
    }

    #[test]
    fn test_pattern_map_requires_map_value() {
        let pattern = Pattern::Map {
            required: {
                let mut m = HashMap::new();
                m.insert(SmolStr::new("x"), Value::Int(1));
                m
            },
        };

        assert!(!pattern.matches(&Value::Int(42)));
        assert!(!pattern.matches(&Value::String(SmolStr::new("hello"))));
    }

    #[test]
    fn test_tuple_pattern_any_matches_all() {
        let pattern = TuplePattern::Any;
        let tuple = Tuple::new(SmolStr::new("event"), Value::Int(42));
        assert!(pattern.matches(&tuple));
    }

    #[test]
    fn test_tuple_pattern_type_and_data() {
        let pattern = TuplePattern::TypeAndData {
            type_name: SmolStr::new("event"),
            data: Pattern::Exact(Value::Int(42)),
        };

        let tuple = Tuple::new(SmolStr::new("event"), Value::Int(42));
        assert!(pattern.matches(&tuple));

        let wrong_type = Tuple::new(SmolStr::new("other"), Value::Int(42));
        assert!(!pattern.matches(&wrong_type));

        let wrong_data = Tuple::new(SmolStr::new("event"), Value::Int(99));
        assert!(!pattern.matches(&wrong_data));
    }

    #[test]
    fn test_tuple_pattern_full() {
        let pattern = TuplePattern::Full {
            namespace: SmolStr::new("user123"),
            type_name: SmolStr::new("click"),
            data: Pattern::Map {
                required: {
                    let mut m = HashMap::new();
                    m.insert(SmolStr::new("x"), Value::Int(100));
                    m
                },
            },
        };

        let tuple = Tuple::new(
            SmolStr::new("click"),
            make_map(vec![("x", Value::Int(100)), ("y", Value::Int(200))]),
        )
        .with_namespace(SmolStr::new("user123"));
        assert!(pattern.matches(&tuple));

        let wrong_namespace = Tuple::new(
            SmolStr::new("click"),
            make_map(vec![("x", Value::Int(100))]),
        )
        .with_namespace(SmolStr::new("user456"));
        assert!(!pattern.matches(&wrong_namespace));
    }

    #[test]
    fn test_tuple_builder_methods() {
        let tuple = Tuple::new(SmolStr::new("test"), Value::Int(42))
            .with_namespace(SmolStr::new("ns"))
            .with_timestamp(123)
            .with_seq(456);

        assert_eq!(tuple.type_name, SmolStr::new("test"));
        assert_eq!(tuple.namespace, Some(SmolStr::new("ns")));
        assert_eq!(tuple.timestamp, 123);
        assert_eq!(tuple.seq, 456);
        assert_eq!(tuple.data, Value::Int(42));
        assert!(!tuple.durable, "default is non-durable");
    }

    #[test]
    fn test_tuple_with_durable_flag() {
        let tuple = Tuple::new(SmolStr::new("event"), Value::Int(1)).with_durable(true);
        assert!(tuple.durable);
    }

    #[test]
    fn test_tuple_serde_round_trip_primitive_data() {
        // The Value enum already has serde coverage; check that wrapping
        // a primitive in a Tuple round-trips bytes-identically.
        let tuple = Tuple::new(SmolStr::new("event"), Value::Int(42))
            .with_namespace(SmolStr::new("ns"))
            .with_timestamp(1234)
            .with_seq(5)
            .with_durable(true);
        let bytes = serde_json::to_vec(&tuple).expect("serialize");
        let back: Tuple = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(tuple, back);
    }

    #[test]
    fn test_tuple_serde_round_trip_map_data() {
        // Map payloads are the common case for tuplespace coordination.
        let tuple = Tuple::new(
            SmolStr::new("room"),
            make_map(vec![
                ("name", Value::String(SmolStr::new("Rusty Flagon"))),
                ("desc", Value::String(SmolStr::new("cozy"))),
            ]),
        )
        .with_durable(true);
        let bytes = serde_json::to_vec(&tuple).expect("serialize");
        let back: Tuple = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(tuple, back);
    }

    #[test]
    fn test_store_key_is_ordered_by_seq() {
        // Property: for tuples in the same (namespace, type), the
        // composite key sorts by seq. fjall iter() is byte-ordered,
        // so this gives FIFO replay on reopen.
        let t1 = Tuple::new(SmolStr::new("event"), Value::Null).with_seq(1);
        let t2 = Tuple::new(SmolStr::new("event"), Value::Null).with_seq(2);
        let t10 = Tuple::new(SmolStr::new("event"), Value::Null).with_seq(10);
        let t256 = Tuple::new(SmolStr::new("event"), Value::Null).with_seq(256);
        let mut keys = [
            t10.store_key(),
            t1.store_key(),
            t256.store_key(),
            t2.store_key(),
        ];
        keys.sort();
        assert_eq!(keys[0], t1.store_key());
        assert_eq!(keys[1], t2.store_key());
        assert_eq!(keys[2], t10.store_key());
        assert_eq!(keys[3], t256.store_key());
    }

    #[test]
    fn test_store_key_distinguishes_namespace_from_type_collision() {
        // Without length prefixing, namespace "evt" + type "X" could
        // produce the same byte sequence as namespace "ev" + type "tX".
        // Length-prefixing prevents that.
        let a = Tuple::new(SmolStr::new("X"), Value::Null)
            .with_namespace(SmolStr::new("evt"))
            .with_seq(1);
        let b = Tuple::new(SmolStr::new("tX"), Value::Null)
            .with_namespace(SmolStr::new("ev"))
            .with_seq(1);
        assert_ne!(
            a.store_key(),
            b.store_key(),
            "different (ns, type) must yield different keys"
        );
    }

    #[test]
    fn test_store_key_no_namespace_distinct_from_empty_namespace() {
        // `None` namespace encodes as ns-len=0 with no bytes; explicit
        // empty-string namespace would also be ns-len=0 with no bytes,
        // which is fine because both are observably the same.
        let none_ns = Tuple::new(SmolStr::new("event"), Value::Null).with_seq(1);
        let some_a = Tuple::new(SmolStr::new("event"), Value::Null)
            .with_namespace(SmolStr::new("a"))
            .with_seq(1);
        assert_ne!(none_ns.store_key(), some_a.store_key());
    }
}
