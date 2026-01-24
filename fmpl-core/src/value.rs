//! Runtime values for FMPL.

use crate::compiler::CompiledCode;
use crate::error::{Error, Result};
use crate::grammar::Grammar;
use crate::object::ObjectId;
use crate::stream::{SinkHandle, SinkSource, StreamHandle, StreamSource};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Runtime value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(SmolStr),
    Symbol(SmolStr),
    List(Arc<Vec<Value>>),
    Map(Arc<HashMap<SmolStr, Value>>),
    Object(ObjectId),
    Lambda(Arc<Lambda>),
    /// Partially applied function.
    Partial(Arc<Partial>),
    /// First-class grammar.
    Grammar(Arc<Grammar>),
    /// Stream value with lazy operations.
    Stream(Arc<Stream>),
    /// Async stream handle (source) - live connection.
    /// Serializes to SuspendedStream with source metadata for reconnection.
    #[serde(skip_deserializing)]
    #[serde(serialize_with = "serialize_async_stream")]
    AsyncStream(Arc<std::sync::Mutex<StreamHandle>>),
    /// Sink handle (destination) - live connection.
    /// Serializes to SuspendedSink with source metadata for reconnection.
    #[serde(skip_deserializing)]
    #[serde(serialize_with = "serialize_sink")]
    Sink(Arc<SinkHandle>),
    /// Suspended async stream awaiting reconnection.
    /// Created when deserializing a serialized AsyncStream.
    SuspendedStream(StreamSource),
    /// Suspended sink awaiting reconnection.
    /// Created when deserializing a serialized Sink.
    SuspendedSink(SinkSource),
}

/// Serialize AsyncStream by extracting its source metadata.
fn serialize_async_stream<S>(
    stream: &Arc<std::sync::Mutex<StreamHandle>>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let guard = stream.lock().unwrap();
    let source = guard.source().clone();
    drop(guard);
    // Serialize as SuspendedStream variant so it deserializes correctly
    Value::SuspendedStream(source).serialize(serializer)
}

/// Serialize Sink by extracting its source metadata.
fn serialize_sink<S>(sink: &Arc<SinkHandle>, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let source = sink.source().clone();
    // Serialize as SuspendedSink variant so it deserializes correctly
    Value::SuspendedSink(source).serialize(serializer)
}

/// Stream operation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub source: Value,
    pub ops: Vec<StreamOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamOp {
    Map(Value),
    Filter(Value),
    FlatMap(Value),
    Reduce(Value),
    Parse {
        grammar: Value,
        rule: SmolStr,
    },
    /// Async streaming parse - emits matches incrementally as they occur.
    AsyncParse {
        grammar: Value,
        rule: SmolStr,
    },
}

/// A lambda closure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lambda {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
    pub captures: HashMap<SmolStr, Value>,
}

/// A partially applied function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partial {
    pub func: Value,
    pub args: Vec<Option<Value>>,
    pub remaining: usize,
}

impl Value {
    /// Check if value is truthy.
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Map(m) => !m.is_empty(),
            _ => true,
        }
    }

    /// Check if value is falsy.
    pub fn is_falsy(&self) -> bool {
        !self.is_truthy()
    }

    /// Get type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Object(_) => "object",
            Value::Lambda(_) => "lambda",
            Value::Partial(_) => "partial",
            Value::Grammar(_) => "grammar",
            Value::Stream(_) => "stream",
            Value::AsyncStream(_) => "async_stream",
            Value::Sink(_) => "sink",
            Value::SuspendedStream(_) => "suspended_stream",
            Value::SuspendedSink(_) => "suspended_sink",
        }
    }

    /// Add two values.
    pub fn add(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => {
                Ok(Value::String(SmolStr::new(format!("{}{}", a, b))))
            }
            (Value::List(a), Value::List(b)) => {
                let mut result = (**a).clone();
                result.extend((**b).iter().cloned());
                Ok(Value::List(Arc::new(result)))
            }
            _ => Err(Error::Type {
                expected: "numeric or string".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Subtract two values.
    pub fn sub(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
            _ => Err(Error::Type {
                expected: "numeric".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Multiply two values.
    pub fn mul(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err(Error::Type {
                expected: "numeric".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Divide two values.
    pub fn div(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (_, Value::Int(0)) => Err(Error::DivisionByZero),
            (_, Value::Float(f)) if *f == 0.0 => Err(Error::DivisionByZero),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),
            _ => Err(Error::Type {
                expected: "numeric".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Modulo two values.
    pub fn modulo(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (_, Value::Int(0)) => Err(Error::DivisionByZero),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            _ => Err(Error::Type {
                expected: "int".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Negate a value.
    pub fn neg(&self) -> Result<Value> {
        match self {
            Value::Int(n) => Ok(Value::Int(-n)),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(Error::Type {
                expected: "numeric".to_string(),
                got: self.type_name().to_string(),
            }),
        }
    }

    /// Logical not.
    pub fn not(&self) -> Value {
        Value::Bool(!self.is_truthy())
    }

    /// Compare for equality.
    pub fn eq(&self, other: &Value) -> Value {
        Value::Bool(self.equals(other))
    }

    /// Compare for inequality.
    pub fn ne(&self, other: &Value) -> Value {
        Value::Bool(!self.equals(other))
    }

    /// Internal equality check.
    fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).map_or(false, |bv| v.equals(bv)))
            }
            _ => false,
        }
    }

    /// Less than comparison.
    pub fn lt(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) < *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a < (*b as f64))),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a < b)),
            _ => Err(Error::Type {
                expected: "comparable".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Greater than comparison.
    pub fn gt(&self, other: &Value) -> Result<Value> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) > *b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Bool(*a > (*b as f64))),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(a > b)),
            _ => Err(Error::Type {
                expected: "comparable".to_string(),
                got: format!("{} and {}", self.type_name(), other.type_name()),
            }),
        }
    }

    /// Less than or equal comparison.
    pub fn le(&self, other: &Value) -> Result<Value> {
        let gt = self.gt(other)?;
        Ok(gt.not())
    }

    /// Greater than or equal comparison.
    pub fn ge(&self, other: &Value) -> Result<Value> {
        let lt = self.lt(other)?;
        Ok(lt.not())
    }

    /// Index into a value.
    pub fn index(&self, idx: &Value) -> Result<Value> {
        match (self, idx) {
            (Value::List(list), Value::Int(i)) => {
                let i = if *i < 0 {
                    (list.len() as i64 + i) as usize
                } else {
                    *i as usize
                };
                list.get(i)
                    .cloned()
                    .ok_or_else(|| Error::Runtime(format!("index {} out of bounds", i)))
            }
            (Value::Map(map), Value::Symbol(key)) => map
                .get(key)
                .cloned()
                .ok_or_else(|| Error::UndefinedProperty(key.to_string())),
            (Value::Map(map), Value::String(key)) => map
                .get(key.as_str())
                .cloned()
                .ok_or_else(|| Error::UndefinedProperty(key.to_string())),
            (Value::String(s), Value::Int(i)) => {
                let i = if *i < 0 {
                    (s.len() as i64 + i) as usize
                } else {
                    *i as usize
                };
                s.chars()
                    .nth(i)
                    .map(|c| Value::String(SmolStr::new(c.to_string())))
                    .ok_or_else(|| Error::Runtime(format!("index {} out of bounds", i)))
            }
            _ => Err(Error::Type {
                expected: "indexable".to_string(),
                got: format!("{}[{}]", self.type_name(), idx.type_name()),
            }),
        }
    }

    /// Slice a value with optional start and end indices.
    /// For lists: returns a new list with elements from start..end
    /// For strings: returns a new string with characters from start..end
    /// Null start means 0, null end means len
    pub fn slice(&self, start: Option<&Value>, end: Option<&Value>) -> Result<Value> {
        fn normalize_idx(idx: i64, len: usize) -> usize {
            if idx < 0 {
                (len as i64 + idx).max(0) as usize
            } else {
                (idx as usize).min(len)
            }
        }

        match self {
            Value::List(list) => {
                let len = list.len();
                let start_idx = match start {
                    None => 0,
                    Some(Value::Int(i)) => normalize_idx(*i, len),
                    Some(v) => {
                        return Err(Error::Type {
                            expected: "int or null".to_string(),
                            got: format!("{}", v.type_name()),
                        });
                    }
                };
                let end_idx = match end {
                    None => len,
                    Some(Value::Int(i)) => normalize_idx(*i, len),
                    Some(v) => {
                        return Err(Error::Type {
                            expected: "int or null".to_string(),
                            got: format!("{}", v.type_name()),
                        });
                    }
                };
                if start_idx > end_idx {
                    return Ok(Value::List(Arc::new(vec![])));
                }
                Ok(Value::List(Arc::new(list[start_idx..end_idx].to_vec())))
            }
            Value::String(s) => {
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len();
                let start_idx = match start {
                    None => 0,
                    Some(Value::Int(i)) => normalize_idx(*i, len),
                    Some(v) => {
                        return Err(Error::Type {
                            expected: "int or null".to_string(),
                            got: format!("{}", v.type_name()),
                        });
                    }
                };
                let end_idx = match end {
                    None => len,
                    Some(Value::Int(i)) => normalize_idx(*i, len),
                    Some(v) => {
                        return Err(Error::Type {
                            expected: "int or null".to_string(),
                            got: format!("{}", v.type_name()),
                        });
                    }
                };
                if start_idx > end_idx {
                    return Ok(Value::String(SmolStr::new("")));
                }
                let sliced: String = chars[start_idx..end_idx].iter().collect();
                Ok(Value::String(SmolStr::new(sliced)))
            }
            _ => Err(Error::Type {
                expected: "sliceable (list or string)".to_string(),
                got: format!("{}", self.type_name()),
            }),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Symbol(s) => write!(f, ":{}", s),
            Value::List(l) => {
                write!(f, "[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(m) => {
                write!(f, "%{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Object(id) => write!(f, "<object #{}>", id),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::Partial(_) => write!(f, "<partial>"),
            Value::Grammar(g) => write!(f, "<grammar {}>", g.name),
            Value::Stream(_) => write!(f, "<stream>"),
            Value::AsyncStream(s) => write!(f, "<async_stream #{}>", s.lock().unwrap().id()),
            Value::Sink(s) => write!(f, "<sink #{}>", s.id()),
            Value::SuspendedStream(source) => write!(f, "<suspended_stream {:?}>", source),
            Value::SuspendedSink(source) => write!(f, "<suspended_sink {:?}>", source),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::Grammar;

    #[test]
    fn test_grammar_value_type_name() {
        let grammar = Grammar::new(SmolStr::new("test"));
        let val = Value::Grammar(Arc::new(grammar));
        assert_eq!(val.type_name(), "grammar");
    }

    #[test]
    fn test_grammar_value_display() {
        let grammar = Grammar::new(SmolStr::new("my::grammar"));
        let val = Value::Grammar(Arc::new(grammar));
        assert_eq!(format!("{}", val), "<grammar my::grammar>");
    }

    #[test]
    fn test_grammar_value_is_truthy() {
        let grammar = Grammar::new(SmolStr::new("test"));
        let val = Value::Grammar(Arc::new(grammar));
        assert!(val.is_truthy());
    }
}
