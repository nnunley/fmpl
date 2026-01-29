//! SEND + MORE = MONEY solver in pure FMPL
//!
//! Uses grammar backtracking with `when` guards for constraint satisfaction.
//! The input is a string of 8 digits representing S,E,N,D,M,O,R,Y assignments.
//! The `digit:?x` syntax marks each binding as a choice point for backtracking.

use fmpl_core::{Value, Vm, eval};
use std::time::Instant;

/// Test the SEND + MORE = MONEY solver with the known solution
#[test]
fn test_send_more_money_known_solution() {
    // Known solution: S=9 E=5 N=6 D=7 M=1 O=0 R=8 Y=2
    // Input: "95671082" (SENDMORY order)
    let source = r#"
        let Crypto = grammar Crypto {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4
                  | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9

            solution = digit:?s when s != 0
                       digit:?e when !(e in [s])
                       digit:?n when !(n in [s, e])
                       digit:?d when !(d in [s, e, n])
                       digit:?m when m != 0 && !(m in [s, e, n, d])
                       digit:?o when !(o in [s, e, n, d, m])
                       digit:?r when !(r in [s, e, n, d, m, o])
                       digit:?y when !(y in [s, e, n, d, m, o, r])
                                 && (s*1000 + e*100 + n*10 + d) + (m*1000 + o*100 + r*10 + e) == (m*10000 + o*1000 + n*100 + e*10 + y)
                       => [s, e, n, d, m, o, r, y]
        }

        "95671082" @ Crypto.solution
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);

    assert!(
        result.is_ok(),
        "Should solve with known solution: {:?}",
        result.err()
    );

    // Should return [9, 5, 6, 7, 1, 0, 8, 2]
    let solution = result.unwrap();
    println!("Result: {:?}", solution);

    // Verify it's a list with the expected values
    if let Value::List(list) = solution {
        assert_eq!(list.len(), 8, "Should have 8 values");
        let vals: Vec<i64> = list
            .iter()
            .filter_map(|v| {
                if let Value::Int(i) = v {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            vals,
            vec![9, 5, 6, 7, 1, 0, 8, 2],
            "Should be the known solution"
        );
    } else {
        panic!("Expected list result, got: {:?}", solution);
    }
}

/// Test that invalid solutions are rejected
#[test]
fn test_send_more_money_rejects_invalid() {
    // Invalid: all zeros (violates s != 0 and m != 0)
    let source = r#"
        let Crypto = grammar Crypto {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4
                  | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9

            solution = digit:?s when s != 0
                       digit:?e when !(e in [s])
                       digit:?n when !(n in [s, e])
                       digit:?d when !(d in [s, e, n])
                       digit:?m when m != 0 && !(m in [s, e, n, d])
                       digit:?o when !(o in [s, e, n, d, m])
                       digit:?r when !(r in [s, e, n, d, m, o])
                       digit:?y when !(y in [s, e, n, d, m, o, r])
                                 && (s*1000 + e*100 + n*10 + d) + (m*1000 + o*100 + r*10 + e) == (m*10000 + o*1000 + n*100 + e*10 + y)
                       => [s, e, n, d, m, o, r, y]
        }

        "00000000" @ Crypto.solution
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);

    // Should fail - s=0 violates first constraint
    assert!(result.is_err(), "Should reject all-zeros: {:?}", result);
}

/// Test that duplicate digits are rejected
#[test]
fn test_send_more_money_rejects_duplicates() {
    // Invalid: S=1, E=1 (duplicate)
    let source = r#"
        let Crypto = grammar Crypto {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4
                  | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9

            solution = digit:?s when s != 0
                       digit:?e when !(e in [s])
                       digit:?n when !(n in [s, e])
                       digit:?d when !(d in [s, e, n])
                       digit:?m when m != 0 && !(m in [s, e, n, d])
                       digit:?o when !(o in [s, e, n, d, m])
                       digit:?r when !(r in [s, e, n, d, m, o])
                       digit:?y when !(y in [s, e, n, d, m, o, r])
                                 && (s*1000 + e*100 + n*10 + d) + (m*1000 + o*100 + r*10 + e) == (m*10000 + o*1000 + n*100 + e*10 + y)
                       => [s, e, n, d, m, o, r, y]
        }

        "11234567" @ Crypto.solution
    "#;

    let mut vm = Vm::new();
    let result = eval(&mut vm, source);

    // Should fail - E duplicates S
    assert!(result.is_err(), "Should reject duplicates: {:?}", result);
}

/// Test solving with timing
#[test]
fn test_send_more_money_timing() {
    let source = r#"
        let Crypto = grammar Crypto {
            digit = "0" => 0 | "1" => 1 | "2" => 2 | "3" => 3 | "4" => 4
                  | "5" => 5 | "6" => 6 | "7" => 7 | "8" => 8 | "9" => 9

            solution = digit:?s when s != 0
                       digit:?e when !(e in [s])
                       digit:?n when !(n in [s, e])
                       digit:?d when !(d in [s, e, n])
                       digit:?m when m != 0 && !(m in [s, e, n, d])
                       digit:?o when !(o in [s, e, n, d, m])
                       digit:?r when !(r in [s, e, n, d, m, o])
                       digit:?y when !(y in [s, e, n, d, m, o, r])
                                 && (s*1000 + e*100 + n*10 + d) + (m*1000 + o*100 + r*10 + e) == (m*10000 + o*1000 + n*100 + e*10 + y)
                       => [s, e, n, d, m, o, r, y]
        }

        "95671082" @ Crypto.solution
    "#;

    let mut vm = Vm::new();
    let start = Instant::now();
    let result = eval(&mut vm, source);
    let elapsed = start.elapsed();

    println!("SEND+MORE=MONEY (FMPL) solved in {:?}", elapsed);

    assert!(result.is_ok(), "Should find solution: {:?}", result.err());

    // Verify arithmetic: 9567 + 1085 = 10652
    if let Ok(Value::List(list)) = &result {
        let vals: Vec<i64> = list
            .iter()
            .filter_map(|v| {
                if let Value::Int(i) = v {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect();

        let (s, e, n, d, m, o, r, y) = (
            vals[0], vals[1], vals[2], vals[3], vals[4], vals[5], vals[6], vals[7],
        );

        let send = s * 1000 + e * 100 + n * 10 + d;
        let more = m * 1000 + o * 100 + r * 10 + e;
        let money = m * 10000 + o * 1000 + n * 100 + e * 10 + y;

        println!("  {} + {} = {}", send, more, money);
        println!(
            "  S={} E={} N={} D={} M={} O={} R={} Y={}",
            s, e, n, d, m, o, r, y
        );

        assert_eq!(send + more, money, "Arithmetic should be correct");
    }
}
