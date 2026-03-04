use fmpl_core::{Value, Vm, eval};

#[test]
fn test_grammar_optimizer() {
    // Change to workspace root so io::load paths work correctly
    std::env::set_current_dir(std::env::current_dir().unwrap().parent().unwrap()).unwrap();

    let mut vm = Vm::new();
    let result = eval(
        &mut vm,
        &format!(
            r#"
        io::load("{}")
    "#,
            "lib/core/grammar_optimizer_test.fmpl"
        ),
    )
    .unwrap();

    // The test file returns a summary map
    if let Value::Map(m) = &result {
        let ok = m.get("ok").cloned().unwrap_or(Value::Bool(false));
        let total = m.get("total").cloned().unwrap_or(Value::Int(0));
        let passed = m.get("passed").cloned().unwrap_or(Value::Int(0));
        let failed = m.get("failed").cloned().unwrap_or(Value::Int(0));

        println!("Grammar Optimizer Tests:");
        println!("  Total:  {:?}", total);
        println!("  Passed: {:?}", passed);
        println!("  Failed: {:?}", failed);

        // Print failures if any
        if let Some(Value::List(suites)) = m.get("suites") {
            for suite in suites.iter() {
                if let Value::Map(s) = suite {
                    if let Some(Value::List(failures)) = s.get("failures") {
                        if !failures.is_empty() {
                            if let Some(Value::String(name)) = s.get("suite") {
                                println!("\n  Failures in {}:", name);
                                for f in failures.iter() {
                                    if let Value::Map(fm) = f {
                                        if let Some(Value::String(n)) = fm.get("name") {
                                            println!("    - {}", n);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(ok, Value::Bool(true), "Some grammar optimizer tests failed");
    } else {
        panic!("Expected test result to be a Map, got: {:?}", result);
    }
}
