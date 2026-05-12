//! Tests for CoerceStream instruction
//!
//! The CoerceStream instruction enables the @ operator to work polymorphically
//! on different input types by coercing them to appropriate stream forms:
//! - String -> character stream (each char becomes an input element)
//! - List -> element stream (pass through as-is)
//! - Map/Tagged/other -> single-element stream (for pattern matching)

use fmpl_core::Vm;
use fmpl_core::compiler::{CompiledCode, InstrIndex, Instruction, StreamMode};
use fmpl_core::value::Value;
use smol_str::SmolStr;
use std::collections::HashMap;

/// Helper to create CompiledCode from instructions
fn make_code(instructions: Vec<Instruction>) -> CompiledCode {
    CompiledCode {
        instructions,
        nested: vec![],
        source: None,
        constants: vec![],
        rule_entry_points: HashMap::new(),
    }
}

/// Helper to execute compiled code and return the final value
fn execute(vm: &mut Vm, code: CompiledCode) -> fmpl_core::error::Result<Value> {
    vm.run(&code)
}

// =============================================================================
// StreamMode::Chars tests
// =============================================================================

mod chars_mode {
    use super::*;

    #[test]
    fn string_to_char_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("hello")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Chars,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(chars) = result {
            assert_eq!(chars.len(), 5);
            assert!(matches!(&chars[0], Value::String(s) if s == "h"));
            assert!(matches!(&chars[1], Value::String(s) if s == "e"));
            assert!(matches!(&chars[2], Value::String(s) if s == "l"));
            assert!(matches!(&chars[3], Value::String(s) if s == "l"));
            assert!(matches!(&chars[4], Value::String(s) if s == "o"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn empty_string_to_empty_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Chars,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(chars) = result {
            assert_eq!(chars.len(), 0);
        } else {
            panic!("expected empty list, got {:?}", result);
        }
    }

    #[test]
    fn unicode_string_to_char_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("日本")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Chars,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(chars) = result {
            assert_eq!(chars.len(), 2);
            assert!(matches!(&chars[0], Value::String(s) if s == "日"));
            assert!(matches!(&chars[1], Value::String(s) if s == "本"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn chars_mode_rejects_non_string() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(42),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Chars,
            },
        ]);
        let result = execute(&mut vm, code);
        assert!(result.is_err());
    }
}

// =============================================================================
// StreamMode::Items tests
// =============================================================================

mod items_mode {
    use super::*;

    #[test]
    fn list_passes_through() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(1),
            Instruction::LoadInt(2),
            Instruction::LoadInt(3),
            Instruction::MakeList {
                elements: vec![InstrIndex(0), InstrIndex(1), InstrIndex(2)],
            },
            Instruction::CoerceStream {
                value: InstrIndex(3),
                mode: StreamMode::Items,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 3);
            assert!(matches!(&items[0], Value::Int(1)));
            assert!(matches!(&items[1], Value::Int(2)));
            assert!(matches!(&items[2], Value::Int(3)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn empty_list_passes_through() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::MakeList { elements: vec![] },
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Items,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 0);
        } else {
            panic!("expected empty list, got {:?}", result);
        }
    }

    #[test]
    fn items_mode_rejects_non_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("not a list")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Items,
            },
        ]);
        let result = execute(&mut vm, code);
        assert!(result.is_err());
    }
}

// =============================================================================
// StreamMode::Once tests
// =============================================================================

mod once_mode {
    use super::*;

    #[test]
    fn int_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(42),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Once,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Int(42)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn string_wraps_in_list_not_chars() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("hello")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Once,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            // Once mode wraps, doesn't convert to chars
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::String(s) if s == "hello"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn map_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("x")),
            Instruction::LoadInt(1),
            Instruction::MakeMap {
                pairs: vec![(InstrIndex(0), InstrIndex(1))],
            },
            Instruction::CoerceStream {
                value: InstrIndex(2),
                mode: StreamMode::Once,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Map(_)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn tagged_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(42),
            Instruction::MakeListNode {
                tag: SmolStr::new("Some"),
                args: vec![InstrIndex(0)],
            },
            Instruction::CoerceStream {
                value: InstrIndex(1),
                mode: StreamMode::Once,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(items[0].as_node().is_some());
        } else {
            panic!("expected list, got {:?}", result);
        }
    }
}

// =============================================================================
// StreamMode::Auto tests
// =============================================================================

mod auto_mode {
    use super::*;

    #[test]
    fn auto_string_becomes_chars() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("hi")),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(chars) = result {
            assert_eq!(chars.len(), 2);
            assert!(matches!(&chars[0], Value::String(s) if s == "h"));
            assert!(matches!(&chars[1], Value::String(s) if s == "i"));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_list_passes_through() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(1),
            Instruction::LoadInt(2),
            Instruction::MakeList {
                elements: vec![InstrIndex(0), InstrIndex(1)],
            },
            Instruction::CoerceStream {
                value: InstrIndex(2),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 2);
            assert!(matches!(&items[0], Value::Int(1)));
            assert!(matches!(&items[1], Value::Int(2)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_int_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(99),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Int(99)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_map_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadString(SmolStr::new("key")),
            Instruction::LoadInt(123),
            Instruction::MakeMap {
                pairs: vec![(InstrIndex(0), InstrIndex(1))],
            },
            Instruction::CoerceStream {
                value: InstrIndex(2),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Map(_)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_tagged_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadInt(1),
            Instruction::MakeListNode {
                tag: SmolStr::new("X"),
                args: vec![InstrIndex(0)],
            },
            Instruction::CoerceStream {
                value: InstrIndex(1),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(items[0].as_node().is_some());
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_bool_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadBool(true),
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Bool(true)));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }

    #[test]
    fn auto_null_wraps_in_list() {
        let mut vm = Vm::new();
        let code = make_code(vec![
            Instruction::LoadNull,
            Instruction::CoerceStream {
                value: InstrIndex(0),
                mode: StreamMode::Auto,
            },
        ]);
        let result = execute(&mut vm, code).unwrap();
        if let Value::List(items) = result {
            assert_eq!(items.len(), 1);
            assert!(matches!(&items[0], Value::Null));
        } else {
            panic!("expected list, got {:?}", result);
        }
    }
}

// =============================================================================
// Integration tests using @ operator with FMPL source code
// =============================================================================
//
// NOTE: The @ operator already provides polymorphic stream coercion through
// the PegRuntime (apply_grammar_to_value_with_evaluator). The CoerceStream
// instruction is NOT emitted for GrammarApply because the runtime handles
// coercion internally. These tests verify the polymorphic behavior.

mod at_operator_integration {
    use fmpl_core::{Value, Vm, eval};

    // These tests verify that the @ operator handles different input types
    // polymorphically. The PegRuntime internally coerces inputs:
    // - String -> character stream (text parsing)
    // - List -> element stream OR single value (tries both)
    // - Other -> single-element stream (pattern matching)

    #[test]
    fn string_input_parses_as_text() {
        let mut vm = Vm::new();
        // String input should be parsed as a character stream
        let result = eval(&mut vm, r#""12345" @ base::parser.integer"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "12345"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn int_input_matches_as_single_value() {
        let mut vm = Vm::new();
        // Integer input should be treated as a single value
        let result = eval(&mut vm, r#"42 @ base::tree.int"#).unwrap();
        assert!(matches!(result, Value::Int(42)), "got {:?}", result);
    }

    #[test]
    fn bool_input_matches_as_single_value() {
        let mut vm = Vm::new();
        // Boolean input should be treated as a single value
        let result = eval(&mut vm, r#"true @ base::tree.bool"#).unwrap();
        assert!(matches!(result, Value::Bool(true)), "got {:?}", result);
    }

    #[test]
    fn string_with_char_class_pattern() {
        let mut vm = Vm::new();
        // String with character class pattern
        let result = eval(&mut vm, r#""hello" @ base::parser.word"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "hello"),
            "got {:?}",
            result
        );
    }

    #[test]
    fn string_digit_parsing() {
        let mut vm = Vm::new();
        // Parse a single digit from string
        let result = eval(&mut vm, r#""5" @ base::parser.digit"#).unwrap();
        assert!(
            matches!(result, Value::String(ref s) if s == "5"),
            "got {:?}",
            result
        );
    }
}
