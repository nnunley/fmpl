#![allow(unexpected_cfgs)]
// Direct comparison: FMPL VM vs execution_tape VM
// Cross-compiles FMPL IR to execution_tape and benchmarks both

fn main() {
    println!("=== FMPL VM vs execution_tape VM (Cross-Compiled) ===\n");

    // Test cases
    let tests = vec![
        ("7 + 9 * 5", "Arithmetic (7 + 9 * 5 = 52)"),
        ("1 + 2 + 3", "Simple addition"),
        ("10 * 5", "Multiplication"),
        ("100 - 50", "Subtraction"),
    ];

    let _iterations = 10000;

    for (source, name) in tests {
        println!("=== Test: {} ===", name);
        println!("Source: {}\n", source);

        // Compile to FMPL IR
        let tokens = fmpl_core::lexer::Lexer::new(source).tokenize().unwrap();
        let ast = fmpl_core::parser::Parser::with_source(&tokens, source)
            .parse()
            .unwrap();
        let _fmpl_code = fmpl_core::compiler::Compiler::new().compile(&ast).unwrap();

        #[cfg(feature = "cross_compile")]
        {
            // Cross-compile to execution_tape
            match fmpl_core::cross_compile::cross_compile(&fmpl_code) {
                Ok(exec_program) => {
                    // Benchmark FMPL VM
                    let mut vm = fmpl_core::vm::Vm::new();
                    for _ in 0..100 {
                        let _ = vm.run(&fmpl_code);
                    }

                    let start = Instant::now();
                    for _ in 0..iterations {
                        let _ = vm.run(&fmpl_code);
                    }
                    let fmpl_time = start.elapsed();
                    let fmpl_ns = fmpl_time.as_nanos() / iterations as u128;
                    let fmpl_ops = (iterations as f64 / fmpl_time.as_secs_f64()) as u64;

                    let fmpl_result = vm.run(&fmpl_code).unwrap();

                    // Benchmark execution_tape VM
                    use execution_tape::host::{Host, HostError, SigHash, ValueRef};
                    use execution_tape::trace::TraceMask;
                    use execution_tape::value::FuncId;
                    use execution_tape::vm::{Limits, Vm as ExecVm};

                    struct TestHost;
                    impl Host for TestHost {
                        fn call(
                            &mut self,
                            _symbol: &str,
                            _sig_hash: SigHash,
                            _args: &[ValueRef<'_>],
                        ) -> Result<(Vec<execution_tape::value::Value>, u64), HostError>
                        {
                            Err(HostError::UnknownSymbol)
                        }
                    }

                    let mut exec_vm = ExecVm::new(TestHost, Limits::default());

                    // Warmup
                    for _ in 0..100 {
                        let _ = exec_vm.run(&exec_program, FuncId(0), &[], TraceMask::NONE, None);
                    }

                    let start = Instant::now();
                    for _ in 0..iterations {
                        let _ = exec_vm.run(&exec_program, FuncId(0), &[], TraceMask::NONE, None);
                    }
                    let exec_time = start.elapsed();
                    let exec_ns = exec_time.as_nanos() / iterations as u128;
                    let exec_ops = (iterations as f64 / exec_time.as_secs_f64()) as u64;

                    let exec_result = exec_vm
                        .run(&exec_program, FuncId(0), &[], TraceMask::NONE, None)
                        .unwrap();

                    println!("FMPL VM:");
                    println!("  Result: {:?}", fmpl_result);
                    println!("  Time: {:.2}s", fmpl_time.as_secs_f64());
                    println!("  {:.2} ns/op", fmpl_ns);
                    println!("  {:.2} M ops/sec", fmpl_ops as f64 / 1_000_000.0);

                    println!("\nexecution_tape VM:");
                    println!("  Result: {:?}", exec_result[0]);
                    println!("  Time: {:.2}s", exec_time.as_secs_f64());
                    println!("  {:.2} ns/op", exec_ns);
                    println!("  {:.2} M ops/sec", exec_ops as f64 / 1_000_000.0);

                    let speedup = fmpl_time.as_secs_f64() / exec_time.as_secs_f64();
                    println!("\nSpeedup: {:.2}x", speedup);
                }
                Err(e) => {
                    println!("Cross-compilation failed: {}", e);
                    println!(
                        "Note: This is expected - cross_compile feature not fully implemented yet"
                    );
                }
            }
        }

        #[cfg(not(feature = "cross_compile"))]
        {
            println!("Cross-compilation not enabled. Run with:");
            println!("  cargo run --bin cross_compile_bench --features cross_compile");
        }

        println!();
    }
}
