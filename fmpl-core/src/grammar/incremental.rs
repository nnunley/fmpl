//! Incremental parsing primitives for streaming grammars.
//!
//! A [`ParseState`] is a serializable snapshot of an in-flight parse: input
//! cursor, rule call stack, and bound variables. A [`ParseNext`] is the
//! tri-state outcome of one driver step (matched / needs more input /
//! end-of-stream). Together they let a grammar suspend on a short read,
//! persist to durable storage, and resume later — possibly in a different
//! process — without rerunning prefix work.

use crate::value::Value;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

/// Resumable snapshot of an in-flight parse.
///
/// All fields are public because `ParseState` is the wire format between the
/// parser driver, the persistence layer, and any resumption logic. The state
/// is by-value `Clone` and serde-serializable so it can be checkpointed
/// mid-parse and rehydrated later.
///
/// # Invariants
///
/// * `position_index` indexes into the input stream that produced this state.
///   Resuming against a *different* stream (or a stream whose prefix has
///   changed) is undefined behavior at the grammar level — the driver must
///   match states to streams.
/// * `rule_stack` is innermost-last; each entry records the position at which
///   that rule was entered, which is what packrat memoization keys on.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Cursor into the input stream this state was captured from.
    pub position_index: usize,
    /// Active rule stack as `(rule_name, entry_position_index)`, innermost
    /// last.
    pub rule_stack: Vec<(SmolStr, usize)>,
    /// Variable bindings visible to the innermost rule.
    pub bindings: HashMap<SmolStr, Value>,
}

/// Outcome of one step of incremental parsing.
///
/// The driver loop matches on this to decide whether to consume the result,
/// suspend (persist the embedded [`ParseState`] and wait for more input), or
/// stop.
#[derive(Debug, Clone)]
pub enum ParseNext {
    /// A top-level rule matched; carries the produced value.
    Match(Value),
    /// The parse cannot proceed without more input. The embedded state is the
    /// resumption point and is safe to serialize.
    NeedInput(ParseState),
    /// The input stream terminated before any further match was possible.
    End,
}

/// Failure modes for [`ParseState`] persistence.
///
/// `Serialize` / `Deserialize` indicate a JSON-level shape mismatch (typically
/// a struct version skew). `Store` is the underlying key-value store
/// returning an I/O or storage-engine error.
#[derive(Debug)]
pub enum ParseStateError {
    /// JSON serialization of the state failed.
    Serialize(serde_json::Error),
    /// JSON deserialization of stored bytes failed (likely shape drift or
    /// corrupted record).
    Deserialize(serde_json::Error),
    /// Underlying Store error (wraps backend-specific failure like fjall).
    Store(crate::persistence::StoreError),
}

impl std::fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "serialize error: {}", e),
            Self::Deserialize(e) => write!(f, "deserialize error: {}", e),
            Self::Store(e) => write!(f, "store error: {}", e),
        }
    }
}

impl std::error::Error for ParseStateError {}

impl From<crate::persistence::StoreError> for ParseStateError {
    fn from(e: crate::persistence::StoreError) -> Self {
        Self::Store(e)
    }
}

impl ParseState {
    /// Returns the JSON encoding of this state.
    ///
    /// This is the raw payload (no envelope header). Use [`save_to_store`] to
    /// write a record that round-trips through [`load_from_store`].
    ///
    /// [`save_to_store`]: Self::save_to_store
    /// [`load_from_store`]: Self::load_from_store
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Reconstructs a state from its raw JSON encoding (no envelope header).
    ///
    /// Pair with [`to_bytes`]. Reads from durable storage should go through
    /// [`load_from_store`] instead — it strips the envelope first.
    ///
    /// [`to_bytes`]: Self::to_bytes
    /// [`load_from_store`]: Self::load_from_store
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Persists this state under `key` in `store`, wrapped in the
    /// standard persistence envelope.
    ///
    /// The on-disk record is `envelope_header || json(self)` where the header
    /// carries the payload kind ([`PayloadKind::ParseState`]) and a source
    /// hash slot. Returns `Err(Serialize)` for a JSON encoding failure and
    /// `Err(Store)` for an I/O / storage failure.
    ///
    /// [`PayloadKind::ParseState`]: crate::persistence::schema::PayloadKind::ParseState
    pub fn save_to_store<S: crate::persistence::Store>(
        &self,
        store: &S,
        key: &[u8],
    ) -> Result<(), ParseStateError> {
        use crate::persistence::envelope::write;
        use crate::persistence::schema::PayloadKind;
        use fmpl_types::Hash;
        match write(
            store,
            key,
            self,
            PayloadKind::ParseState,
            crate::VM_VERSION,
            Hash::NONE,
        ) {
            Ok(()) => Ok(()),
            Err(crate::persistence::envelope::EnvelopeWriteError::Serialize(e)) => {
                Err(ParseStateError::Serialize(e))
            }
            Err(crate::persistence::envelope::EnvelopeWriteError::Store(e)) => {
                Err(ParseStateError::Store(e))
            }
        }
    }

    /// Loads a previously saved state from `store`, stripping the envelope
    /// header.
    ///
    /// Returns `Ok(None)` if no record exists at `key` (a normal "not found"
    /// — the caller treats this as a cache miss). Returns `Err(Deserialize)`
    /// if a record is present but cannot be decoded — that signals either
    /// schema drift across versions or on-disk corruption, and the caller
    /// must decide whether to invalidate the slot.
    ///
    /// A record shorter than [`ENVELOPE_HEADER_SIZE`] is treated as
    /// corruption: we floor the slice at the header size so the deserializer
    /// rejects an empty payload rather than the function panicking on an
    /// out-of-bounds index.
    ///
    /// TODO(ITER-0005a.4): Replace the manual header strip with
    /// `loader::decode(&bytes)` once the loader API lands.
    ///
    /// [`ENVELOPE_HEADER_SIZE`]: crate::persistence::envelope::ENVELOPE_HEADER_SIZE
    pub fn load_from_store<S: crate::persistence::Store>(
        store: &S,
        key: &[u8],
    ) -> Result<Option<Self>, ParseStateError> {
        use crate::persistence::envelope::ENVELOPE_HEADER_SIZE;
        match store.get(key)? {
            Some(bytes) => {
                let payload_start = bytes.len().min(ENVELOPE_HEADER_SIZE);
                let payload = &bytes[payload_start..];
                let state = Self::from_bytes(payload).map_err(ParseStateError::Deserialize)?;
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

    // The Store-backed round-trip and not-found cases exercise the
    // FjallStore impl and live as integration tests at
    // `fmpl-core/tests/parse_state_persistence.rs`. Keeping them out of
    // this `#[cfg(test)] mod tests` block keeps the
    // no-fjall-in-fmpl-core source gate (T5) satisfied.
}
