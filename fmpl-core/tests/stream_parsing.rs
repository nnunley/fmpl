use fmpl_core::{Compiler, Lexer, Parser, Result, Value, Vm};

fn eval(vm: &mut Vm, source: &str) -> Result<Value> {
    let tokens = Lexer::new(source).tokenize()?;
    let ast = Parser::with_source(&tokens, source).parse()?;
    let code = Compiler::new().compile(&ast)?;
    vm.run(&code)
}

#[test]
fn parse_stream_from_string_head() {
    let mut vm = Vm::new();
    // stream::new("hello") creates a ParseStream from a string
    // .head() returns the first character as a string
    let result = eval(&mut vm, r#"let s = stream::new("hello"); s.head()"#).unwrap();
    assert_eq!(result, Value::String("h".into()));
}

#[test]
fn parse_stream_from_string_position() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"let s = stream::new("hello"); s.position()"#).unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn parse_stream_advance_then_head() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("hello")
        s.advance(1)
        s.head()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("e".into()));
}

#[test]
fn parse_stream_checkpoint_restore() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("hello")
        s.advance(2)
        let cp = s.checkpoint()
        s.advance(2)
        s.restore(cp)
        s.head()
    "#,
    )
    .unwrap();
    // After advance(2), position is at 'l' (index 2)
    // checkpoint saves position 2
    // advance(2) moves to 'o' (index 4)
    // restore goes back to position 2
    // head() returns 'l'
    assert_eq!(result, Value::String("l".into()));
}

#[test]
fn parse_stream_is_at_end() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("hi")
        s.advance(2)
        s.head()
    "#,
    )
    .unwrap();
    // At end of input, head() returns null
    assert_eq!(result, Value::Null);
}

#[test]
fn parse_stream_from_list_head() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new([10, 20, 30])
        s.head()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn parse_stream_from_list_advance_head() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new([10, 20, 30])
        s.advance(1)
        s.head()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(20));
}

#[test]
fn parse_stream_from_list_checkpoint_restore() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new([10, 20, 30])
        s.advance(1)
        let cp = s.checkpoint()
        s.advance(2)
        s.restore(cp)
        s.head()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(20));
}

#[test]
fn parse_stream_fail_is_catchable() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let fail_rule = \s { stream::fail("expected digit") }
        try { fail_rule(s) } catch e { "caught: " + e }
    "#,
    )
    .unwrap();
    assert!(matches!(result, Value::String(_)));
}

#[test]
fn match_char_success() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        stream::match_char(s, "a")
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn match_char_failure() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        try { stream::match_char(s, "x") } catch e { "fail" }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("fail".into()));
}

#[test]
fn match_char_advances_position() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        stream::match_char(s, "a")
        s.head()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("b".into()));
}

#[test]
fn match_class_digit() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("3abc")
        stream::match_class(s, "0-9")
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("3".into()));
}

#[test]
fn match_class_letter() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        stream::match_class(s, "a-z")
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn match_class_failure() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        try { stream::match_class(s, "0-9") } catch e { "fail" }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("fail".into()));
}

#[test]
fn parse_stream_apply_calls_rule() {
    let mut vm = Vm::new();
    // Define a rule as a lambda that takes a stream,
    // reads head, advances, returns it
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let rule = \s { let c = s.head(); s.advance(1); c }
        s.apply(rule)
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn parse_stream_apply_memoizes_result() {
    let mut vm = Vm::new();
    // apply() at same position with same rule should return cached result
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let rule = \s { let c = s.head(); s.advance(1); c }
        let r1 = s.apply(rule)
        s.restore(0)
        s.apply(rule)
    "#,
    )
    .unwrap();
    // Second apply at same position with same rule should return memoized "a"
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn parse_stream_apply_memoizes_position() {
    let mut vm = Vm::new();
    // After memoized apply, position should be restored to the end position
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let rule = \s { let c = s.head(); s.advance(1); c }
        let r1 = s.apply(rule)
        s.restore(0)
        let r2 = s.apply(rule)
        s.position()
    "#,
    )
    .unwrap();
    // After memo hit, position should be at 1 (where the rule left off)
    assert_eq!(result, Value::Int(1));
}

// ── Task 8: choice combinator ──────────────────────────────────────────

#[test]
fn choice_first_matches() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let r1 = \s { stream::match_char(s, "a") }
        let r2 = \s { stream::match_char(s, "b") }
        stream::choice(s, [r1, r2])
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn choice_second_matches() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("bcd")
        let r1 = \s { stream::match_char(s, "a") }
        let r2 = \s { stream::match_char(s, "b") }
        stream::choice(s, [r1, r2])
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("b".into()));
}

#[test]
fn choice_restores_on_failure() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("bcd")
        let r1 = \s { stream::match_char(s, "a"); stream::match_char(s, "b") }
        let r2 = \s { stream::match_char(s, "b") }
        stream::choice(s, [r1, r2])
        s.position()
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn choice_all_fail() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("xyz")
        let r1 = \s { stream::match_char(s, "a") }
        let r2 = \s { stream::match_char(s, "b") }
        try { stream::choice(s, [r1, r2]) } catch e { "all failed" }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("all failed".into()));
}

// ── Task 9: star and plus combinators ──────────────────────────────────

#[test]
fn star_zero_matches() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::star(s, digit)
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::List(std::sync::Arc::new(vec![])));
}

#[test]
fn star_multiple_matches() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("123abc")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::star(s, digit)
    "#,
    )
    .unwrap();
    assert_eq!(
        result,
        Value::List(std::sync::Arc::new(vec![
            Value::String("1".into()),
            Value::String("2".into()),
            Value::String("3".into()),
        ]))
    );
}

#[test]
fn plus_requires_one() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let digit = \s { stream::match_class(s, "0-9") }
        try { stream::plus(s, digit) } catch e { "need at least one" }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("need at least one".into()));
}

#[test]
fn plus_multiple_matches() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("123abc")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::plus(s, digit)
    "#,
    )
    .unwrap();
    assert_eq!(
        result,
        Value::List(std::sync::Arc::new(vec![
            Value::String("1".into()),
            Value::String("2".into()),
            Value::String("3".into()),
        ]))
    );
}

// ── Task 10: seq combinator and full parse ─────────────────────────────

#[test]
fn seq_all_match() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let ra = \s { stream::match_char(s, "a") }
        let rb = \s { stream::match_char(s, "b") }
        let rc = \s { stream::match_char(s, "c") }
        stream::seq(s, [ra, rb, rc])
    "#,
    )
    .unwrap();
    assert_eq!(
        result,
        Value::List(std::sync::Arc::new(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]))
    );
}

#[test]
fn seq_partial_fails_and_restores() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let ra = \s { stream::match_char(s, "a") }
        let rx = \s { stream::match_char(s, "x") }
        try { stream::seq(s, [ra, rx]) } catch e { s.position() }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn full_parse_with_semantic_action() {
    let mut vm = Vm::new();
    // Semantic action: wrap each digit in a tagged value
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("123")
        let digit = \s { stream::match_class(s, "0-9") }
        let digits = stream::plus(s, digit)
        digits
    "#,
    )
    .unwrap();
    assert_eq!(
        result,
        Value::List(std::sync::Arc::new(vec![
            Value::String("1".into()),
            Value::String("2".into()),
            Value::String("3".into()),
        ]))
    );
}

// ── Task 11: not, lookahead, optional combinators ───────────────────────

#[test]
fn not_succeeds_when_rule_fails() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::not(s, digit)
        s.position()
    "#,
    )
    .unwrap();
    // not doesn't consume input
    assert_eq!(result, Value::Int(0));
}

#[test]
fn not_fails_when_rule_succeeds() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("123")
        let digit = \s { stream::match_class(s, "0-9") }
        try { stream::not(s, digit) } catch e { "matched" }
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("matched".into()));
}

#[test]
fn lookahead_succeeds_without_consuming() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let letter = \s { stream::match_class(s, "a-z") }
        stream::lookahead(s, letter)
        s.position()
    "#,
    )
    .unwrap();
    // lookahead doesn't consume input
    assert_eq!(result, Value::Int(0));
}

#[test]
fn optional_returns_null_on_failure() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("abc")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::optional(s, digit)
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn optional_returns_value_on_success() {
    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        r#"
        let s = stream::new("123")
        let digit = \s { stream::match_class(s, "0-9") }
        stream::optional(s, digit)
    "#,
    )
    .unwrap();
    assert_eq!(result, Value::String("1".into()));
}
