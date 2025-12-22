//! Runtime values for FMPL.

use crate::compiler::CompiledCode;
use crate::error::{Error, Result};
use crate::object::ObjectId;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Runtime value.
#[derive(Debug, Clone)]
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
}

/// A lambda closure.
#[derive(Debug, Clone)]
pub struct Lambda {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
    pub captures: HashMap<SmolStr, Value>,
}

/// A partially applied function.
#[derive(Debug, Clone)]
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
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}
