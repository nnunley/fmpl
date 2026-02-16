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
    /// Facet-restricted view of an object (sealed capability).
    #[serde(skip)]
    Facet(ObjectId, SmolStr),
    Lambda(Arc<Lambda>),
    /// Partially applied function.
    Partial(Arc<Partial>),
    /// First-class grammar.
    Grammar(Arc<Grammar>),
    /// Tagged/constructor value with symbol name and children.
    Tagged(SmolStr, Arc<Vec<Value>>),
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
    /// Tuple space for pattern-based coordination.
    #[serde(skip)]
    TupleSpace(Arc<std::sync::Mutex<crate::tuplespace::store::TupleSpace>>),
    /// Facet-restricted tuple space with capability security.
    #[serde(skip)]
    TupleSpaceFacet(Arc<std::sync::Mutex<crate::tuplespace::facet::TupleSpaceFacet>>),
    /// Cursor into a stream - CoW reference for RLM-style observation.
    #[serde(skip)]
    Cursor(Arc<Cursor>),
    /// Compiled bytecode (opaque, executable).
    #[serde(skip)]
    Code(Arc<CompiledCode>),
}

/// A cursor into a stream - lightweight CoW reference.
///
/// Cursors provide observable access to streams without copying the underlying data.
/// Multiple cursors can observe the same stream independently, enabling:
/// - RLM-style recursive processing
/// - Multi-agent coordination through shared observation
/// - Time travel debugging (immutable history)
/// - Forking without copying
#[derive(Debug, Clone)]
pub struct Cursor {
    /// The stream being observed (Arc for cheap sharing)
    pub stream: Arc<Stream>,
    /// Current position in the stream
    pub position: CursorPosition,
    /// Branch identifier (for tracking fork history)
    pub branch_id: SmolStr,
}

/// Position within a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorPosition {
    /// Index in the stream's value sequence
    pub index: usize,
    /// Generation counter (for tracking stream mutations)
    pub generation: u64,
}

impl CursorPosition {
    /// Create a new position at the start of a stream.
    pub fn start() -> Self {
        Self {
            index: 0,
            generation: 0,
        }
    }

    /// Advance position by n steps.
    pub fn advance(&self, n: usize) -> Self {
        Self {
            index: self.index + n,
            generation: self.generation,
        }
    }

    /// Rewind position by n steps.
    pub fn rewind(&self, n: usize) -> Self {
        Self {
            index: self.index.saturating_sub(n),
            generation: self.generation,
        }
    }
}

impl Cursor {
    /// Create a new cursor observing a stream.
    pub fn new(stream: Arc<Stream>) -> Self {
        Self {
            stream,
            position: CursorPosition::start(),
            branch_id: SmolStr::new("main"),
        }
    }

    /// Fork this cursor at the current position, creating a new branch.
    pub fn fork(&self, new_branch_id: SmolStr) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
            position: self.position,
            branch_id: new_branch_id,
        }
    }

    /// Advance this cursor by n positions.
    pub fn advance(&self, n: usize) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
            position: self.position.advance(n),
            branch_id: self.branch_id.clone(),
        }
    }

    /// Rewind this cursor by n positions.
    pub fn rewind(&self, n: usize) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
            position: self.position.rewind(n),
            branch_id: self.branch_id.clone(),
        }
    }

    /// Get the current position as a value.
    pub fn get_position(&self) -> Value {
        Value::Int(self.position.index as i64)
    }
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
    Collect,
    Take {
        n: Value,
    },
    Drop {
        n: Value,
    },
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
            Value::Tagged(_, _) => true,
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
            Value::Facet(_, _) => "facet",
            Value::Lambda(_) => "lambda",
            Value::Partial(_) => "partial",
            Value::Grammar(_) => "grammar",
            Value::Tagged(_, _) => "tagged",
            Value::Stream(_) => "stream",
            Value::AsyncStream(_) => "async_stream",
            Value::Sink(_) => "sink",
            Value::SuspendedStream(_) => "suspended_stream",
            Value::SuspendedSink(_) => "suspended_sink",
            Value::TupleSpace(_) => "tuplespace",
            Value::TupleSpaceFacet(_) => "tuplespace_facet",
            Value::Cursor(_) => "cursor",
            Value::Code(_) => "code",
        }
    }

    /// Check if value is a stream or cursor.
    pub fn is_stream_like(&self) -> bool {
        match self {
            Value::Stream(_) | Value::AsyncStream(_) | Value::Cursor(_) => true,
            _ => false,
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
            (Value::Facet(a, an), Value::Facet(b, bn)) => a == b && an == bn,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Map(a), Value::Map(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).map_or(false, |bv| v.equals(bv)))
            }
            (Value::Tagged(tag_a, children_a), Value::Tagged(tag_b, children_b)) => {
                tag_a == tag_b
                    && children_a.len() == children_b.len()
                    && children_a
                        .iter()
                        .zip(children_b.iter())
                        .all(|(x, y)| x.equals(y))
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
            Value::Facet(id, name) => write!(f, "<facet :{} of object #{}>", name, id),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::Partial(_) => write!(f, "<partial>"),
            Value::Grammar(g) => write!(f, "<grammar {}>", g.name),
            Value::Tagged(tag, children) => {
                write!(f, ":{}", tag)?;
                if !children.is_empty() {
                    write!(f, "(")?;
                    for (i, child) in children.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", child)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Value::Stream(_) => write!(f, "<stream>"),
            Value::AsyncStream(s) => write!(f, "<async_stream #{}>", s.lock().unwrap().id()),
            Value::Sink(s) => write!(f, "<sink #{}>", s.id()),
            Value::SuspendedStream(source) => write!(f, "<suspended_stream {:?}>", source),
            Value::SuspendedSink(source) => write!(f, "<suspended_sink {:?}>", source),
            Value::TupleSpace(_) => write!(f, "<tuplespace>"),
            Value::TupleSpaceFacet(_) => write!(f, "<tuplespace_facet>"),
            Value::Cursor(c) => write!(
                f,
                "<cursor branch:{} pos:{}>",
                c.branch_id, c.position.index
            ),
            Value::Code(_) => write!(f, "<code>"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

// Into implementations for ergonomic constant creation
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(SmolStr::new(v))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(SmolStr::new(v))
    }
}

impl From<SmolStr> for Value {
    fn from(v: SmolStr) -> Self {
        Value::String(v)
    }
}

impl From<&Value> for Value {
    fn from(v: &Value) -> Self {
        v.clone()
    }
}

// TryFrom implementations for extracting values from Value enum
impl TryFrom<Value> for SmolStr {
    type Error = Error;

    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s),
            Value::Symbol(s) => Ok(s),
            other => Err(Error::Type {
                expected: "String or Symbol".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = Error;

    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Int(n) => Ok(n),
            other => Err(Error::Type {
                expected: "Int".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = Error;

    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Bool(b) => Ok(b),
            other => Err(Error::Type {
                expected: "Bool".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = Error;

    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Float(n) => Ok(n),
            other => Err(Error::Type {
                expected: "Float".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = Error;

    fn try_from(v: Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s.into()),
            Value::Symbol(s) => Ok(s.into()),
            other => Err(Error::Type {
                expected: "String or Symbol".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for SmolStr {
    type Error = Error;

    fn try_from(v: &'a Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s.clone()),
            Value::Symbol(s) => Ok(s.clone()),
            other => Err(Error::Type {
                expected: "String or Symbol".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for i64 {
    type Error = Error;

    fn try_from(v: &'a Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Int(n) => Ok(*n),
            other => Err(Error::Type {
                expected: "Int".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for bool {
    type Error = Error;

    fn try_from(v: &'a Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Bool(b) => Ok(*b),
            other => Err(Error::Type {
                expected: "Bool".to_string(),
                got: other.type_name().to_string(),
            }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for f64 {
    type Error = Error;

    fn try_from(v: &'a Value) -> std::result::Result<Self, Self::Error> {
        match v {
            Value::Float(n) => Ok(*n),
            other => Err(Error::Type {
                expected: "Float".to_string(),
                got: other.type_name().to_string(),
            }),
        }
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

    #[test]
    fn test_tagged_value_type_name() {
        let val = Value::Tagged(
            SmolStr::new("Binary"),
            Arc::new(vec![
                Value::Symbol(SmolStr::new("+")),
                Value::Int(1),
                Value::Int(2),
            ]),
        );
        assert_eq!(val.type_name(), "tagged");
    }

    #[test]
    fn test_tagged_value_display() {
        let val = Value::Tagged(SmolStr::new("Int"), Arc::new(vec![Value::Int(42)]));
        assert_eq!(format!("{}", val), ":Int(42)");
    }

    #[test]
    fn test_tagged_value_is_truthy() {
        let val = Value::Tagged(SmolStr::new("Foo"), Arc::new(vec![]));
        assert!(val.is_truthy());
    }

    #[test]
    fn test_tagged_value_nested() {
        let inner = Value::Tagged(SmolStr::new("Int"), Arc::new(vec![Value::Int(1)]));
        let outer = Value::Tagged(
            SmolStr::new("Binary"),
            Arc::new(vec![
                Value::Symbol(SmolStr::new("+")),
                inner,
                Value::Tagged(SmolStr::new("Int"), Arc::new(vec![Value::Int(2)])),
            ]),
        );
        assert_eq!(format!("{}", outer), ":Binary(:+, :Int(1), :Int(2))");
    }

    #[test]
    fn test_code_value_type_name() {
        use crate::compiler::CompiledCode;
        let code = CompiledCode::default();
        let val = Value::Code(Arc::new(code));
        assert_eq!(val.type_name(), "code");
    }

    #[test]
    fn test_code_value_display() {
        use crate::compiler::CompiledCode;
        let code = CompiledCode::default();
        let val = Value::Code(Arc::new(code));
        assert_eq!(format!("{}", val), "<code>");
    }

    #[test]
    fn test_code_value_is_truthy() {
        use crate::compiler::CompiledCode;
        let code = CompiledCode::default();
        let val = Value::Code(Arc::new(code));
        assert!(val.is_truthy());
    }
}
