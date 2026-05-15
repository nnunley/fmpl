//! Durable TupleSpace round-trip through the FMPL surface.
//!
//! Same-process test (not cross-process), but each phase uses a fresh
//! `Vm` opened against the same on-disk fjall path. Proves the FMPL
//! contract end-to-end: open path → out durable → drop space → open
//! same path → recover.
//!
//! Cross-process is exercised by the YAML demo scenario
//! `demo/scenarios/durable_tuplespace.yaml`.

#![cfg(feature = "persistence")]

use fmpl_core::{Value, Vm, eval};
use smol_str::SmolStr;
use tempfile::tempdir;

fn open_eval(vm: &mut Vm, dir: &std::path::Path) -> Value {
    // tuplespace.open returns a TupleSpace value bound to `world`.
    let src = format!(
        r#"let world = tuplespace.open({:?})"#,
        dir.display().to_string()
    );
    eval(vm, &src).expect("tuplespace.open eval")
}

#[test]
fn durable_round_trip_through_fmpl_surface() {
    let dir = tempdir().unwrap();

    // Phase A: fresh Vm, open, out two durable tuples, drop the Vm.
    {
        let mut vm = Vm::new();
        open_eval(&mut vm, dir.path());
        let _ = eval(
            &mut vm,
            r#"world.out(%{type: :room, data: %{name: "Rusty Flagon"}, durable: true})"#,
        )
        .expect("out room");
        let _ = eval(
            &mut vm,
            r#"world.out(%{type: :menu, data: %{ale: 3}, durable: true})"#,
        )
        .expect("out menu");
        // VM drops here; the TupleSpace inside it drops, closing fjall.
    }

    // Phase B: fresh Vm, reopen the same path, rd both tuples.
    let mut vm = Vm::new();
    open_eval(&mut vm, dir.path());

    let room_name = eval(&mut vm, r#"world.rd(:room).data.name"#).expect("rd room");
    assert_eq!(room_name, Value::String(SmolStr::new("Rusty Flagon")));

    let ale_price = eval(&mut vm, r#"world.rd(:menu).data.ale"#).expect("rd menu");
    assert_eq!(ale_price, Value::Int(3));
}

#[test]
fn non_durable_does_not_survive_restart() {
    let dir = tempdir().unwrap();
    {
        let mut vm = Vm::new();
        open_eval(&mut vm, dir.path());
        let _ = eval(
            &mut vm,
            r#"world.out(%{type: :ephemeral, data: "x"})"#, // no durable: true
        )
        .expect("out ephemeral");
    }

    let mut vm = Vm::new();
    open_eval(&mut vm, dir.path());
    // Today `rdp` errors on no-match instead of returning Null (the
    // surface comment in vm.rs claims `-> map | null` but dispatch
    // raises a runtime error). That's a pre-existing discrepancy; the
    // no-match error is sufficient evidence that the non-durable tuple
    // did not survive the restart.
    let err = eval(&mut vm, r#"world.rdp(:ephemeral)"#)
        .expect_err("non-durable tuple must not survive the close-reopen cycle");
    assert!(
        err.to_string().contains("no matching tuple"),
        "expected no-match error, got: {err}"
    );
}

#[test]
fn in_destructive_consume_removes_from_disk() {
    let dir = tempdir().unwrap();
    // Phase A: out durable, in destructively, exit.
    {
        let mut vm = Vm::new();
        open_eval(&mut vm, dir.path());
        let _ = eval(
            &mut vm,
            r#"world.out(%{type: :event, data: 1, durable: true})"#,
        )
        .expect("out");
        let _ = eval(&mut vm, r#"world.in(:event)"#).expect("in");
    }
    // Phase B: tuple should NOT resurrect on reopen.
    let mut vm = Vm::new();
    open_eval(&mut vm, dir.path());
    let err = eval(&mut vm, r#"world.rdp(:event)"#)
        .expect_err("in()'d durable tuple must stay consumed across restart");
    assert!(
        err.to_string().contains("no matching tuple"),
        "expected no-match error, got: {err}"
    );
}

#[test]
fn durable_on_non_persistent_space_errors_clearly() {
    let mut vm = Vm::new();
    let err = eval(
        &mut vm,
        r#"
        let space = tuplespace.new()
        space.out(%{type: :x, data: 1, durable: true})
        "#,
    )
    .expect_err("should error on durable+no-backing");
    let msg = err.to_string();
    assert!(
        msg.contains("durable") && msg.contains("no backing store"),
        "expected durable+no-backing error, got: {msg}"
    );
}
