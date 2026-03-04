use fmpl_core::{Value, Vm, eval};

#[test]
fn test_new_constructor_basic() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object cell {
  .#public
  init(v): self.val = v
  get(): self.val
  val: 0
}
"#,
    )
    .expect("define object");

    let result = eval(&mut vm, "let c = new ^cell(42)\nc.get()").expect("new constructor");
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_new_constructor_no_args() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object counter {
  .#public
  get(): self.count
  inc(): self.count = self.count + 1
  count: 0
}
"#,
    )
    .expect("define object");

    let result = eval(&mut vm, "let c = new ^counter()\nc.get()").expect("new no-arg constructor");
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_new_constructor_with_methods() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object cell {
  .#public
  init(v): self.val = v
  get(): self.val
  set(v): self.val = v
  val: 0
}
"#,
    )
    .expect("define object");

    let result = eval(
        &mut vm,
        r#"
let c = new ^cell(10)
c.set(20)
c.get()
"#,
    )
    .expect("new constructor with methods");
    assert_eq!(result, Value::Int(20));
}
