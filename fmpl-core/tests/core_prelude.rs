use fmpl_core::{Value, Vm, eval};

// Tests run from fmpl-core/ directory, so use relative path to workspace root
const PRELUDE_PATH: &str = "../lib/core/prelude.fmpl";

#[test]
fn test_join_empty_list() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        join([])
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::String("".into()));
}

#[test]
fn test_join_single_char() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        join(["a"])
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn test_join_multiple_chars() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        join(["h", "e", "l", "l", "o"])
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::String("hello".into()));
}

#[test]
fn test_to_int_digit_0() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        to_int("0")
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_to_int_digit_9() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        to_int("9")
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(9));
}

#[test]
fn test_to_int_digit_5() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
        to_int("5")
    "#,
            PRELUDE_PATH
        ),
    )
    .unwrap();
    assert_eq!(result, Value::Int(5));
}
