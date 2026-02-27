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
