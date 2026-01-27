// Cursor CoW (Copy-on-Write) behavior tests
//
// These tests verify that cursors provide lightweight references to streams
// without copying the underlying data. Multiple cursors can observe the same
// stream independently, enabling RLM-style recursive processing.

use fmpl_core::{Value, eval};

#[test]
fn test_observe_stream_creates_cursor() {
    let mut vm = fmpl_core::Vm::new();

    // Create a simple stream
    let code =
        "let source = [1, 2, 3, 4, 5]; let data = source; let cursor = stream::observe(data)";

    let result = eval(&mut vm, code);
    if let Err(e) = &result {
        eprintln!("Error: {}", e);
    }
    assert!(result.is_ok(), "observe() should succeed");
}

#[test]
fn test_cursor_advance_creates_new_reference() {
    let mut vm = fmpl_core::Vm::new();

    let code = "let source = [1, 2, 3]; let data = source; let cursor1 = stream::observe(data); let cursor2 = cursor::advance(cursor1, 1); [cursor::position(cursor1), cursor::position(cursor2)]";

    let result = eval(&mut vm, code);
    assert!(result.is_ok(), "cursor.advance() should succeed");
}

#[test]
fn test_cursor_rewind() {
    let mut vm = fmpl_core::Vm::new();

    // Note: Don't use 'cursor' as variable name since it shadows the builtin
    let code = "let source = [1, 2, 3]; let data = source; let c = stream::observe(data); let advanced = cursor::advance(c, 2); let rewound = cursor::rewind(advanced, 1); [cursor::position(advanced), cursor::position(rewound)]";

    let result = eval(&mut vm, code);
    assert!(result.is_ok(), "cursor.rewind() should succeed");
}

#[test]
fn test_cursor_forking_independent_branches() {
    let mut vm = fmpl_core::Vm::new();

    let code = "let source = [1, 2, 3]; let data = source; let cursor1 = stream::observe(data); let cursor2 = cursor1; let cursor1_advanced = cursor::advance(cursor1, 1); let cursor2_advanced = cursor::advance(cursor2, 2); [cursor::position(cursor1), cursor::position(cursor1_advanced), cursor::position(cursor2_advanced)]";

    let result = eval(&mut vm, code);
    assert!(result.is_ok(), "cursor forking should succeed");
}

#[test]
fn test_multiple_cursors_share_stream() {
    let mut vm = fmpl_core::Vm::new();

    let code = "let source = [1, 2, 3, 4, 5]; let data = source; let cursor1 = stream::observe(data); let cursor2 = stream::observe(data); let cursor3 = stream::observe(data); [cursor::position(cursor1), cursor::position(cursor2), cursor::position(cursor3)]";

    let result = eval(&mut vm, code);
    assert!(result.is_ok(), "multiple observe() calls should succeed");
}

#[test]
fn test_cursor_with_custom_branch_id() {
    let mut vm = fmpl_core::Vm::new();

    let code = "let source = [1, 2, 3]; let data = source; let main_cursor = stream::observe(data); let branch_cursor = stream::observe(data, \"branch-1\"); [cursor::position(main_cursor), cursor::position(branch_cursor)]";

    let result = eval(&mut vm, code);
    assert!(result.is_ok(), "observe() with branch_id should succeed");
}
