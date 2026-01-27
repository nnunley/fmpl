//! JSON conversion utilities
//!
//! Provides conversion between serde_json::Value and fmpl_core::Value.

use crate::error::Result;
use crate::value::Value;

/// Convert serde_json::Value to FMPL Value.
pub fn from_json(json: serde_json::Value) -> Result<Value> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(crate::error::Error::Runtime(
                    "Number out of range".to_string(),
                ))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s.into())),
        serde_json::Value::Array(arr) => {
            let items: Result<Vec<Value>> = arr.into_iter().map(from_json).collect();
            Ok(Value::List(std::sync::Arc::new(items?)))
        }
        serde_json::Value::Object(obj) => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in obj {
                map.insert(k.into(), from_json(v)?);
            }
            Ok(Value::Map(std::sync::Arc::new(map)))
        }
    }
}

/// Convert FMPL Value to serde_json::Value.
pub fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.to_string()),
        Value::List(items) => {
            let arr: Vec<serde_json::Value> = items.iter().map(to_json).collect();
            serde_json::Value::Array(arr)
        }
        Value::Map(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.to_string(), to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        // For unsupported types, convert to null
        _ => serde_json::Value::Null,
    }
}
