//! Integration test for durable parse state suspension.
//!
//! Simulates the agent pause/resume scenario where a parse is suspended
//! mid-stream, persisted to a `Store`, and resumed in a new "session".

#![cfg(feature = "fjall-backend")]

mod tests {
    use fmpl_core::grammar::incremental::ParseState;
    use fmpl_core::value::Value;
    use fmpl_persistence::fjall_backend::FjallStore;
    use smol_str::SmolStr;
    use std::collections::HashMap;
    use tempfile::tempdir;

    /// Simulates suspending a parse mid-stream and resuming later.
    ///
    /// This tests the core durable suspension scenario:
    /// 1. Agent is mid-parse when it needs human approval
    /// 2. ParseState is saved to the Store
    /// 3. Process "restarts" (store closed and reopened)
    /// 4. ParseState is restored and parse can continue
    #[test]
    fn test_durable_suspension_scenario() {
        let temp_dir = tempdir().unwrap();
        let session_id = b"agent_session_abc123";

        // --- Session 1: Start parsing, get suspended waiting for human ---

        let store1 = FjallStore::open(temp_dir.path()).unwrap();

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
        suspended_state.save_to_store(&store1, session_id).unwrap();

        // Explicitly drop to simulate session end
        drop(store1);

        // --- Session 2: Human approved, resume the parse ---

        let store2 = FjallStore::open(temp_dir.path()).unwrap();

        // Restore suspended state
        let restored = ParseState::load_from_store(&store2, session_id)
            .unwrap()
            .expect("should find suspended state");

        // Verify state was preserved
        assert_eq!(restored.position_index, 15);
        assert_eq!(restored.rule_stack.len(), 2);
        assert_eq!(restored.rule_stack[0].0, "agent_turn");
        assert_eq!(restored.rule_stack[1].0, "tool_call");
        assert_eq!(
            restored.bindings.get(&SmolStr::new("pending_tool")),
            Some(&Value::String("search".into()))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("args")),
            Some(&Value::String("rust async".into()))
        );

        // Clean up: delete the state after successful resume.
        // The Store trait has no remove(); reach through to the inner
        // keyspace for this teardown step.
        store2.keyspace().remove(session_id).unwrap();
        assert!(
            ParseState::load_from_store(&store2, session_id)
                .unwrap()
                .is_none()
        );
    }

    /// Test that complex Value types roundtrip correctly through the Store.
    #[test]
    fn test_complex_bindings_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let store = FjallStore::open(temp_dir.path()).unwrap();

        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("int_val"), Value::Int(42));
        bindings.insert(SmolStr::new("float_val"), Value::Float(2.71));
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

        state.save_to_store(&store, b"complex").unwrap();
        let restored = ParseState::load_from_store(&store, b"complex")
            .unwrap()
            .unwrap();

        assert_eq!(
            restored.bindings.get(&SmolStr::new("int_val")),
            Some(&Value::Int(42))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("float_val")),
            Some(&Value::Float(2.71))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("bool_val")),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            restored.bindings.get(&SmolStr::new("null_val")),
            Some(&Value::Null)
        );

        // List comparison
        if let Some(Value::List(list)) = restored.bindings.get(&SmolStr::new("list_val")) {
            assert_eq!(list.len(), 3);
            assert_eq!(list[0], Value::Int(1));
            assert_eq!(list[1], Value::Int(2));
            assert_eq!(list[2], Value::String("three".into()));
        } else {
            panic!("list_val should be a List");
        }
    }

    /// Test multiple concurrent sessions can be persisted and restored.
    #[test]
    fn test_multiple_sessions() {
        let temp_dir = tempdir().unwrap();
        let store = FjallStore::open(temp_dir.path()).unwrap();

        // Create several sessions
        for i in 0..5 {
            let key = format!("session_{}", i);
            let mut bindings = HashMap::new();
            bindings.insert(SmolStr::new("session_num"), Value::Int(i));

            let state = ParseState {
                position_index: i as usize * 10,
                rule_stack: vec![(SmolStr::new("rule"), i as usize)],
                bindings,
            };

            state.save_to_store(&store, key.as_bytes()).unwrap();
        }

        // Verify each session independently
        for i in 0..5 {
            let key = format!("session_{}", i);
            let restored = ParseState::load_from_store(&store, key.as_bytes())
                .unwrap()
                .expect("should find session");

            assert_eq!(restored.position_index, i as usize * 10);
            assert_eq!(
                restored.bindings.get(&SmolStr::new("session_num")),
                Some(&Value::Int(i))
            );
        }
    }

    /// Test overwriting a session with updated state.
    #[test]
    fn test_session_update() {
        let temp_dir = tempdir().unwrap();
        let store = FjallStore::open(temp_dir.path()).unwrap();

        let key = b"evolving_session";

        // Initial state
        let state1 = ParseState {
            position_index: 10,
            rule_stack: vec![(SmolStr::new("rule1"), 0)],
            bindings: HashMap::new(),
        };
        state1.save_to_store(&store, key).unwrap();

        // Updated state
        let mut bindings = HashMap::new();
        bindings.insert(SmolStr::new("progress"), Value::String("advanced".into()));

        let state2 = ParseState {
            position_index: 50,
            rule_stack: vec![(SmolStr::new("rule1"), 0), (SmolStr::new("rule2"), 30)],
            bindings,
        };
        state2.save_to_store(&store, key).unwrap();

        // Should get the updated state
        let restored = ParseState::load_from_store(&store, key)
            .unwrap()
            .expect("should find session");

        assert_eq!(restored.position_index, 50);
        assert_eq!(restored.rule_stack.len(), 2);
        assert_eq!(
            restored.bindings.get(&SmolStr::new("progress")),
            Some(&Value::String("advanced".into()))
        );
    }

    /// Test that deep rule stacks are preserved.
    #[test]
    fn test_deep_rule_stack() {
        let temp_dir = tempdir().unwrap();
        let store = FjallStore::open(temp_dir.path()).unwrap();

        // Create a deep rule stack (simulating nested grammar rules)
        let rule_stack: Vec<(SmolStr, usize)> = (0..20)
            .map(|i| (SmolStr::new(format!("rule_{}", i)), i * 5))
            .collect();

        let state = ParseState {
            position_index: 100,
            rule_stack,
            bindings: HashMap::new(),
        };

        state.save_to_store(&store, b"deep_stack").unwrap();
        let restored = ParseState::load_from_store(&store, b"deep_stack")
            .unwrap()
            .unwrap();

        assert_eq!(restored.rule_stack.len(), 20);
        assert_eq!(restored.rule_stack[0], (SmolStr::new("rule_0"), 0));
        assert_eq!(restored.rule_stack[19], (SmolStr::new("rule_19"), 95));
    }
}
