//! Incremental parsing support for streaming grammars.
//!
//! Provides ParseState for suspension/resumption and ParseNext for
//! incremental parse results.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

/// State needed to resume an incremental parse.
///
/// Captures position, rule call stack, and bindings for serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Current position index in input.
    pub position_index: usize,
    /// Rule call stack: (rule_name, entry_position_index).
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Current variable bindings.
    pub bindings: HashMap<SmolStr, Value>,
}

/// Result of an incremental parse step.
#[derive(Debug, Clone)]
pub enum ParseNext {
    /// Rule matched, here's the result value.
    Match(Value),
    /// Need more input - here's state to resume from.
    NeedInput(ParseState),
    /// Input stream ended.
    End,
}

/// Errors for ParseState serialization/persistence.
#[derive(Debug)]
pub enum ParseStateError {
    /// Serialization failed.
    Serialize(serde_json::Error),
    /// Deserialization failed.
    Deserialize(serde_json::Error),
    /// Fjall operation failed.
    Fjall(fjall::Error),
}

impl std::fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "serialize error: {}", e),
            Self::Deserialize(e) => write!(f, "deserialize error: {}", e),
            Self::Fjall(e) => write!(f, "fjall error: {}", e),
        }
    }
}

impl std::error::Error for ParseStateError {}

/// Serialization support for ParseState.
impl ParseState {
    /// Serialize to bytes for Fjall storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Save parse state to Fjall keyspace.
    pub fn save_to_fjall(
        &self,
        keyspace: &fjall::Keyspace,
        key: &[u8],
    ) -> Result<(), ParseStateError> {
        let bytes = self.to_bytes().map_err(ParseStateError::Serialize)?;
        keyspace
            .insert(key, bytes)
            .map_err(ParseStateError::Fjall)?;
        Ok(())
    }

    /// Load parse state from Fjall keyspace.
    pub fn load_from_fjall(
        keyspace: &fjall::Keyspace,
        key: &[u8],
    ) -> Result<Option<Self>, ParseStateError> {
        match keyspace.get(key).map_err(ParseStateError::Fjall)? {
            Some(bytes) => {
                let state = Self::from_bytes(&bytes).map_err(ParseStateError::Deserialize)?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_next_variants() {
        let match_result: ParseNext = ParseNext::Match(Value::Int(42));
        assert!(matches!(match_result, ParseNext::Match(Value::Int(42))));

        let need_input: ParseNext = ParseNext::NeedInput(ParseState::default());
        assert!(matches!(need_input, ParseNext::NeedInput(_)));

        let end: ParseNext = ParseNext::End;
        assert!(matches!(end, ParseNext::End));
    }

    #[test]
    fn test_parse_state_serialization() {
        let state = ParseState {
            position_index: 5,
            rule_stack: vec![("digit".into(), 3), ("integer".into(), 0)],
            bindings: [("x".into(), Value::Int(42))].into_iter().collect(),
        };

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: ParseState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.position_index, 5);
        assert_eq!(deserialized.rule_stack.len(), 2);
        assert_eq!(deserialized.bindings.get("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_parse_state_binary_roundtrip() {
        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("x"), Value::Int(42));
        bindings.insert(SmolStr::new("name"), Value::String("test".into()));

        let state = ParseState {
            position_index: 100,
            rule_stack: vec![(SmolStr::new("outer"), 0), (SmolStr::new("inner"), 50)],
            bindings,
        };

        // Serialize to bytes
        let bytes = state.to_bytes().unwrap();

        // Deserialize
        let restored = ParseState::from_bytes(&bytes).unwrap();

        assert_eq!(restored.position_index, 100);
        assert_eq!(restored.rule_stack.len(), 2);
        assert_eq!(restored.rule_stack[0].0, "outer");
        assert_eq!(restored.rule_stack[1].0, "inner");
        assert_eq!(
            restored.bindings.get(&SmolStr::new("x")),
            Some(&Value::Int(42))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("name")),
            Some(&Value::String("test".into()))
        );
    }

    #[test]
    fn test_parse_state_empty_roundtrip() {
        let state = ParseState::default();
        let bytes = state.to_bytes().unwrap();
        let restored = ParseState::from_bytes(&bytes).unwrap();

        assert_eq!(restored.position_index, 0);
        assert!(restored.rule_stack.is_empty());
        assert!(restored.bindings.is_empty());
    }

    #[test]
    fn test_parse_state_fjall_roundtrip() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let db = fjall::Database::builder(temp_dir.path()).open().unwrap();
        let keyspace = db
            .keyspace("parse_states", || fjall::KeyspaceCreateOptions::default())
            .unwrap();

        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("result"), Value::Int(999));

        let state = ParseState {
            position_index: 42,
            rule_stack: vec![(SmolStr::new("expr"), 10)],
            bindings,
        };

        // Store in Fjall
        let key = b"test_session_123";
        state.save_to_fjall(&keyspace, key).unwrap();

        // Load from Fjall
        let restored = ParseState::load_from_fjall(&keyspace, key)
            .unwrap()
            .expect("should find saved state");

        assert_eq!(restored.position_index, 42);
        assert_eq!(
            restored.bindings.get(&SmolStr::new("result")),
            Some(&Value::Int(999))
        );
    }

    #[test]
    fn test_parse_state_fjall_not_found() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let db = fjall::Database::builder(temp_dir.path()).open().unwrap();
        let keyspace = db
            .keyspace("parse_states", || fjall::KeyspaceCreateOptions::default())
            .unwrap();

        let result = ParseState::load_from_fjall(&keyspace, b"nonexistent");
        assert!(result.unwrap().is_none());
    }
}
