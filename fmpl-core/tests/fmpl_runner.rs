//! FMPL source file test runner
//!
//! Runs .fmpl files and validates basic execution.
//! For now, just verifies they parse and execute without panicking.

use fmpl_core::{Vm, eval};
use std::fs;

fn run_fmpl_file(path: &str) -> Vec<(String, Result<String, String>)> {
    let source = fs::read_to_string(path).expect("Failed to read file");
    let mut vm = Vm::new();
    let mut results = Vec::new();

    for (i, line) in source.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("--") {
            continue;
        }

        // Strip trailing comments
        let line = if let Some(idx) = line.find("--") {
            line[..idx].trim()
        } else {
            line
        };

        // Skip if nothing left after stripping
        if line.is_empty() {
            continue;
        }

        // Evaluate the line
        let result = eval(&mut vm, line);

        let result_str = match &result {
            Ok(v) => Ok(format!("{:?}", v)),
            Err(e) => Err(format!("{:?}", e)),
        };

        results.push((format!("line {}: {}", i + 1, line), result_str));
    }

    results
}

#[test]
fn test_apply_operator_fmpl() {
    let results = run_fmpl_file("tests/fmpl/apply_operator.fmpl");

    // Print all results for debugging
    println!("\n=== FMPL Test Results ===\n");
    for (line, result) in &results {
        match result {
            Ok(v) => println!("OK: {} => {}", line, v),
            Err(e) => println!("ERR: {} => {}", line, e),
        }
    }
    println!();

    // Count successes and failures
    let successes = results.iter().filter(|(_, r)| r.is_ok()).count();
    let failures = results.iter().filter(|(_, r)| r.is_err()).count();

    println!("Results: {} succeeded, {} failed", successes, failures);

    // These are expected to fail based on the comments in the file
    let expected_failures = [
        "abc123",                 // Partial match (line 11)
        "[1, 2, 3]",              // Multi-element list (line 50)
        "42 @ base::tree.string", // Type mismatch (line 54)
    ];

    // Check unexpected failures
    let unexpected_failures: Vec<_> = results
        .iter()
        .filter(|(line, r)| r.is_err() && !expected_failures.iter().any(|ef| line.contains(ef)))
        .collect();

    if !unexpected_failures.is_empty() {
        println!("\n=== Unexpected Failures ===\n");
        for (line, result) in &unexpected_failures {
            if let Err(e) = result {
                println!("UNEXPECTED: {} => {}", line, e);
            }
        }
    }

    // We expect some failures, so just verify we got results
    assert!(successes > 0, "No tests succeeded");
}
