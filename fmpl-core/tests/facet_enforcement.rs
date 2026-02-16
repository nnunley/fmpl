use fmpl_core::{Value, Vm, eval};

#[test]
fn test_facet_allows_exposed_method() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object barkeep {
  .#facets
    customer: [greet]
  .#public
  greet(): "Welcome!"
  restock(): "Restocked"
}
"#,
    )
    .expect("define object");

    let result = eval(&mut vm, "barkeep.as(:customer).greet()").expect("greet");
    assert!(matches!(result, Value::String(s) if s == "Welcome!"));
}

#[test]
fn test_facet_denies_hidden_method() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object barkeep {
  .#facets
    customer: [greet]
  .#public
  greet(): "Welcome!"
  restock(): "Restocked"
}
"#,
    )
    .expect("define object");

    let err = eval(&mut vm, "barkeep.as(:customer).restock()").unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("facet :customer does not expose method 'restock'"),
        "got: {}",
        msg
    );
}

#[test]
fn test_facet_returns_facet_type() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object thing {
  .#facets
    view: [look]
  .#public
  look(): "seen"
}
"#,
    )
    .expect("define object");

    let facet = eval(&mut vm, "thing.as(:view)").expect("get facet");
    assert_eq!(facet.type_name(), "facet");
}

#[test]
fn test_facet_denies_property_write() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object shop {
  .#facets
    customer: [price]
  .#public
  price: 10
}
"#,
    )
    .expect("define object");

    let err = eval(
        &mut vm,
        r#"
let f = shop.as(:customer)
f.price = 99
"#,
    )
    .unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("cannot set properties through facet :customer"),
        "got: {}",
        msg
    );
}

#[test]
fn test_facet_property_read() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object shop {
  .#facets
    customer: [price]
  .#public
  price: 42
}
"#,
    )
    .expect("define object");

    let val = eval(&mut vm, "shop.as(:customer).price").expect("read price");
    assert!(matches!(val, Value::Int(42)));
}

#[test]
fn test_facet_denies_property_read() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object shop {
  .#facets
    customer: [price]
  .#public
  price: 42
  secret: "hidden"
}
"#,
    )
    .expect("define object");

    let err = eval(&mut vm, "shop.as(:customer).secret").unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("facet :customer does not expose property 'secret'"),
        "got: {}",
        msg
    );
}

#[test]
fn test_facet_undefined_errors() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object thing {
  greet(): "hi"
}
"#,
    )
    .expect("define object");

    let err = eval(&mut vm, "thing.as(:nonexistent)").unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("undefined facet"), "got: {}", msg);
}

#[test]
fn test_facet_on_spawned_object() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        r#"
object counter {
  .#facets
    reader: [get]
  .#public
  init(n): self.count = n
  get(): self.count
  inc(): self.count = self.count + 1
  count: 0
}
"#,
    )
    .expect("define object");

    let result = eval(
        &mut vm,
        r#"
let c = spawn counter(5)
let r = c.as(:reader)
r.get()
"#,
    )
    .expect("read via facet on spawned");
    assert!(matches!(result, Value::Int(5)), "got: {}", result);

    let err = eval(&mut vm, "r.inc()").unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("facet :reader does not expose method 'inc'"),
        "got: {}",
        msg
    );
}
