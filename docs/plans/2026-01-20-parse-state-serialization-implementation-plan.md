# ParseState Serialization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add binary serialization to `ParseState` for durable suspension/resumption of incremental parses across process restarts.

**Architecture:** Add `to_bytes()`/`from_bytes()` methods to `ParseState` using serde_json (already a dependency). This enables storing suspended parse states in Fjall for agents that need to pause mid-parse (e.g., waiting for human approval).

**Tech Stack:** Rust, serde/serde_json (existing), Fjall (for storage)

---

## Context

This is Task 7 from the streaming-grammar-push-model-implementation-plan. Tasks 1-6 are complete:
- ParseState/ParseNext types ✅
- Fjall backing for StreamPosition ✅
- Incremental parse API (start/resume) ✅
- ParseDriver for streaming pipelines ✅
- AsyncParse stream operation ✅
- Fjall persistence for memo tables ✅

This task enables the durable pause/resume pattern from the unified-grammars-and-agents-design:

```fmpl
pause_for_human(action) = {
  let (request = spawn ^approval_request(action, current_continuation()))
  <- request.decision  -- suspends here, resumes when human responds
}
```

---

## Task 1: Add to_bytes/from_bytes to ParseState

**Files:**
- Modify: `fmpl-core/src/grammar/incremental.rs:14-22`

**Step 1: Write the failing test**

Add to `fmpl-core/src/grammar/incremental.rs` in the `tests` module:

```rust
#[test]
fn test_parse_state_binary_roundtrip() {
    let mut bindings = HashMap::new();
    bindings.insert(SmolStr::new("x"), Value::Int(42));
    bindings.insert(SmolStr::new("name"), Value::String("test".into()));

    let state = ParseState {
        position_index: 100,
        rule_stack: vec![
            (SmolStr::new("outer"), 0),
            (SmolStr::new("inner"), 50),
        ],
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
    assert_eq!(restored.bindings.get(&SmolStr::new("x")), Some(&Value::Int(42)));
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
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_parse_state_binary --no-run 2>&1 | head -20`
Expected: Compilation error - `to_bytes` and `from_bytes` don't exist

**Step 3: Write minimal implementation**

Add to `ParseState` impl in `fmpl-core/src/grammar/incremental.rs`:

```rust
impl ParseState {
    /// Serialize to bytes for Fjall storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_parse_state_binary -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/incremental.rs
git commit -m "feat(grammar): add to_bytes/from_bytes for ParseState serialization"
```

---

## Task 2: Add Fjall Storage Helper for ParseState

**Files:**
- Modify: `fmpl-core/src/grammar/incremental.rs`

**Step 1: Write the failing test**

Add to tests module:

```rust
#[cfg(feature = "persistence")]
#[test]
fn test_parse_state_fjall_roundtrip() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(temp_dir.path())
        .open()
        .unwrap();
    let partition = keyspace
        .open_partition("parse_states", Default::default())
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
    state.save_to_fjall(&partition, key).unwrap();

    // Load from Fjall
    let restored = ParseState::load_from_fjall(&partition, key)
        .unwrap()
        .expect("should find saved state");

    assert_eq!(restored.position_index, 42);
    assert_eq!(restored.bindings.get(&SmolStr::new("result")), Some(&Value::Int(999)));
}

#[cfg(feature = "persistence")]
#[test]
fn test_parse_state_fjall_not_found() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(temp_dir.path())
        .open()
        .unwrap();
    let partition = keyspace
        .open_partition("parse_states", Default::default())
        .unwrap();

    let result = ParseState::load_from_fjall(&partition, b"nonexistent");
    assert!(result.unwrap().is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core test_parse_state_fjall --features persistence --no-run 2>&1 | head -20`
Expected: Compilation error - `save_to_fjall` and `load_from_fjall` don't exist

**Step 3: Write minimal implementation**

Add to `ParseState` impl:

```rust
#[cfg(feature = "persistence")]
impl ParseState {
    /// Save parse state to Fjall partition.
    pub fn save_to_fjall(
        &self,
        partition: &fjall::PartitionHandle,
        key: &[u8],
    ) -> Result<(), ParseStateError> {
        let bytes = self.to_bytes().map_err(ParseStateError::Serialize)?;
        partition.insert(key, bytes).map_err(ParseStateError::Fjall)?;
        Ok(())
    }

    /// Load parse state from Fjall partition.
    pub fn load_from_fjall(
        partition: &fjall::PartitionHandle,
        key: &[u8],
    ) -> Result<Option<Self>, ParseStateError> {
        match partition.get(key).map_err(ParseStateError::Fjall)? {
            Some(bytes) => {
                let state = Self::from_bytes(&bytes).map_err(ParseStateError::Deserialize)?;
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }
}

/// Errors for ParseState serialization/persistence.
#[derive(Debug)]
pub enum ParseStateError {
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    #[cfg(feature = "persistence")]
    Fjall(fjall::Error),
}

impl std::fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "serialize error: {}", e),
            Self::Deserialize(e) => write!(f, "deserialize error: {}", e),
            #[cfg(feature = "persistence")]
            Self::Fjall(e) => write!(f, "fjall error: {}", e),
        }
    }
}

impl std::error::Error for ParseStateError {}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core test_parse_state_fjall --features persistence -v`
Expected: PASS

**Step 5: Commit**

```bash
git add fmpl-core/src/grammar/incremental.rs
git commit -m "feat(grammar): add Fjall storage helpers for ParseState"
```

---

## Task 3: Integration Test - Durable Parse Suspension

**Files:**
- Create: `fmpl-core/tests/parse_state_persistence.rs`

**Step 1: Write the integration test**

```rust
//! Integration test for durable parse state suspension.
//!
//! Simulates the agent pause/resume scenario where a parse is suspended
//! mid-stream, persisted to Fjall, and resumed in a new "session".

#[cfg(feature = "persistence")]
mod tests {
    use fmpl_core::grammar::incremental::{ParseState, ParseStateError};
    use fmpl_core::value::Value;
    use smol_str::SmolStr;
    use std::collections::HashMap;
    use tempfile::tempdir;

    /// Simulates suspending a parse mid-stream and resuming later.
    #[test]
    fn test_durable_suspension_scenario() {
        let temp_dir = tempdir().unwrap();
        let session_id = b"agent_session_abc123";

        // --- Session 1: Start parsing, get suspended waiting for human ---

        let keyspace1 = fjall::Config::new(temp_dir.path()).open().unwrap();
        let partition1 = keyspace1
            .open_partition("parse_states", Default::default())
            .unwrap();

        // Simulate parse in progress
        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("pending_tool"), Value::String("search".into()));
        bindings.insert(SmolStr::new("args"), Value::String("rust async".into()));

        let suspended_state = ParseState {
            position_index: 15, // Mid-stream
            rule_stack: vec![
                (SmolStr::new("agent_turn"), 0),
                (SmolStr::new("tool_call"), 10),
            ],
            bindings,
        };

        // Persist before "human approval" (simulated process shutdown)
        suspended_state.save_to_fjall(&partition1, session_id).unwrap();

        // Explicitly drop to simulate session end
        drop(partition1);
        drop(keyspace1);

        // --- Session 2: Human approved, resume the parse ---

        let keyspace2 = fjall::Config::new(temp_dir.path()).open().unwrap();
        let partition2 = keyspace2
            .open_partition("parse_states", Default::default())
            .unwrap();

        // Restore suspended state
        let restored = ParseState::load_from_fjall(&partition2, session_id)
            .unwrap()
            .expect("should find suspended state");

        // Verify state was preserved
        assert_eq!(restored.position_index, 15);
        assert_eq!(restored.rule_stack.len(), 2);
        assert_eq!(restored.rule_stack[1].0, "tool_call");
        assert_eq!(
            restored.bindings.get(&SmolStr::new("pending_tool")),
            Some(&Value::String("search".into()))
        );

        // Clean up: delete the state after successful resume
        partition2.remove(session_id).unwrap();
        assert!(ParseState::load_from_fjall(&partition2, session_id).unwrap().is_none());
    }

    /// Test that complex Value types roundtrip correctly.
    #[test]
    fn test_complex_bindings_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let keyspace = fjall::Config::new(temp_dir.path()).open().unwrap();
        let partition = keyspace
            .open_partition("parse_states", Default::default())
            .unwrap();

        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("int_val"), Value::Int(42));
        bindings.insert(SmolStr::new("float_val"), Value::Float(3.14));
        bindings.insert(SmolStr::new("bool_val"), Value::Bool(true));
        bindings.insert(SmolStr::new("null_val"), Value::Null);
        bindings.insert(
            SmolStr::new("list_val"),
            Value::List(std::sync::Arc::new(vec![
                Value::Int(1),
                Value::Int(2),
                Value::String("three".into()),
            ])),
        );

        let state = ParseState {
            position_index: 0,
            rule_stack: vec![],
            bindings,
        };

        state.save_to_fjall(&partition, b"complex").unwrap();
        let restored = ParseState::load_from_fjall(&partition, b"complex")
            .unwrap()
            .unwrap();

        assert_eq!(
            restored.bindings.get(&SmolStr::new("int_val")),
            Some(&Value::Int(42))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("bool_val")),
            Some(&Value::Bool(true))
        );

        // List comparison
        if let Some(Value::List(list)) = restored.bindings.get(&SmolStr::new("list_val")) {
            assert_eq!(list.len(), 3);
            assert_eq!(list[0], Value::Int(1));
        } else {
            panic!("list_val should be a List");
        }
    }
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test -p fmpl-core parse_state_persistence --features persistence -v`
Expected: PASS

**Step 3: Commit**

```bash
git add fmpl-core/tests/parse_state_persistence.rs
git commit -m "test(grammar): add integration tests for durable parse state persistence"
```

---

## Task 4: Update Implementation Plan Status

**Files:**
- Modify: `docs/plans/2026-01-20-streaming-grammar-push-model-implementation-plan.md`

**Step 1: Update status table**

Change the status section to reflect completed tasks:

```markdown
## Implementation Status (as of 2026-01-20)

| Task | Status | Commit |
|------|--------|--------|
| Task 1: ParseState/ParseNext types | ✅ Complete | `53b27a0` |
| Task 2: Fjall backing for StreamPosition | ✅ Complete | `b2c5daf` |
| Task 3: Incremental parse API | ✅ Complete | `67536dc` |
| Task 4: ParseDriver for streaming pipelines | ✅ Complete | `d137df4` |
| Task 5: Wire |> operator to ParseDriver | ✅ Complete | `18991d1` |
| Task 6: Fjall persistence for memo tables | ✅ Complete | `04949ff` |
| Task 7: ParseState serialization | ✅ Complete | (this PR) |
| Task 8: Integration tests | ✅ Complete | (this PR) |
| Task 9: Documentation | ⏳ Pending | - |

**To continue:** Complete Task 9 - add module documentation for streaming grammars.
```

**Step 2: Commit**

```bash
git add docs/plans/2026-01-20-streaming-grammar-push-model-implementation-plan.md
git commit -m "docs: update streaming grammar implementation status"
```

---

## Task 5: Update grammar-system.md Spec

**Files:**
- Modify: `specs/grammar-system.md`

The streaming grammar content should be consolidated into the single grammar spec rather than maintained as a separate document.

**Step 1: Update implementation status**

Add/update the "Implementation Status" section at the bottom of `specs/grammar-system.md`:

```markdown
---

## Implementation Status

| Component | Status |
|-----------|--------|
| Base grammar types (Grammar, Rule, Pattern) | Complete |
| PegRuntime with packrat memoization | Complete |
| Grammar inheritance | Complete |
| ParseState/ParseNext types | Complete |
| StreamPosition with Fjall overflow | Complete |
| Incremental API (start/resume) | Complete |
| ParseDriver for async pipelines | Complete |
| AsyncParse StreamOp | Complete |
| Memo table persistence | Complete |
| ParseState serialization | Complete |
| Integration tests | Complete |
```

**Step 2: Add streaming section**

Add a "Streaming Grammars" section covering:
- Push-based incremental parsing
- ParseState serialization for durable suspension
- Fjall backing for large streams

**Step 3: Commit**

```bash
git add specs/grammar-system.md
git commit -m "docs(specs): add streaming grammar and ParseState persistence to grammar spec"
```

---

## Summary

| Task | Component | Description |
|------|-----------|-------------|
| 1 | incremental.rs | `to_bytes()`/`from_bytes()` for ParseState |
| 2 | incremental.rs | Fjall storage helpers (`save_to_fjall`/`load_from_fjall`) |
| 3 | tests/ | Integration test for durable suspension scenario |
| 4 | docs/plans/ | Update implementation status |
| 5 | specs/ | Consolidate streaming into grammar-system.md spec |

After completing all tasks, run the full test suite:

```bash
cargo test -p fmpl-core --features persistence
```
