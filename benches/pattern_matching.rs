// Pattern matching benchmarks for FMPL VM
// Benchmarks the three key pattern extraction operations:
// - ExtractMapKey: Map key extraction
// - ExtractListIndex: List index extraction
// - ExtractTaggedChild: Tagged value child extraction

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use fmpl_core::{compiler::Compiler, value::Value, vm::Vm};

// Helper to compile FMPL source to bytecode
fn compile_fmpl(source: &str) -> fmpl_core::compiler::CompiledCode {
    let tokens = fmpl_core::lexer::Lexer::new(source).tokenize().unwrap();
    let ast = fmpl_core::parser::Parser::with_source(&tokens, source)
        .parse()
        .unwrap();
    Compiler::new().compile(&ast).unwrap()
}

// Helper to run compiled code
fn run_code(code: &fmpl_core::compiler::CompiledCode) -> Value {
    let mut vm = Vm::new();
    vm.run(code).unwrap()
}

// =============================================================================
// Map Extraction Benchmarks (ExtractMapKey)
// =============================================================================

fn bench_map_extraction_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_extraction");

    // Small map (3 keys)
    let code = compile_fmpl(r#"%{a: 1, b: 2, c: 3} @ { %{a: _:x, b: _:y} => x + y }"#);

    group.bench_function("small_3keys", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_map_extraction_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_extraction");

    // Medium map (10 keys)
    let code = compile_fmpl(
        r#"%{k0: 0, k1: 1, k2: 2, k3: 3, k4: 4, k5: 5, k6: 6, k7: 7, k8: 8, k9: 9} @ { %{k0: _:a, k5: _:b, k9: _:c} => a + b + c }"#,
    );

    group.bench_function("medium_10keys", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_map_extraction_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_extraction");

    // Nested map
    let code = compile_fmpl(
        r#"%{outer: %{inner: %{value: 42}}} @ { %{outer: %{inner: %{value: _:v}}} => v }"#,
    );

    group.bench_function("nested_3_levels", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_map_extraction_single_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_extraction");

    // Single key extraction (minimal overhead test)
    let code = compile_fmpl(r#"%{x: 100} @ { %{x: _:v} => v }"#);

    group.bench_function("single_key", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// List Extraction Benchmarks (ExtractListIndex)
// =============================================================================

fn bench_list_extraction_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_extraction");

    // Small list (3 elements)
    let code = compile_fmpl(r#"[1, 2, 3] @ { [ _:a, _:b, _:c] => a + b + c }"#);

    group.bench_function("small_3elements", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_list_extraction_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_extraction");

    // Medium list (10 elements)
    let code = compile_fmpl(
        r#"[0, 1, 2, 3, 4, 5, 6, 7, 8, 9] @ { [ _:a, _, _, _, _, _:b, _, _, _, _:c] => a + b + c }"#,
    );

    group.bench_function("medium_10elements", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_list_extraction_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_extraction");

    // Nested list
    let code = compile_fmpl(r#"[[[42]]] @ { [[[ _:v]]] => v }"#);

    group.bench_function("nested_3_levels", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_list_extraction_head(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_extraction");

    // Head extraction (first element)
    let code = compile_fmpl(r#"[100, 200, 300] @ { [ _:first | rest] => first }"#);

    group.bench_function("head_tail", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_list_extraction_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_extraction");

    // Single element extraction (minimal overhead)
    let code = compile_fmpl(r#"[42] @ { [ _:v] => v }"#);

    group.bench_function("single_element", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// Tagged Value Extraction Benchmarks (ExtractTaggedChild)
// =============================================================================

fn bench_tagged_extraction_single_child(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_extraction");

    // Single child extraction - tagged patterns use direct variable binding
    let code = compile_fmpl(r#":Wrapper(42) @ { :Wrapper(v) => v }"#);

    group.bench_function("single_child", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_tagged_extraction_multiple_children(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_extraction");

    // Multiple children extraction
    let code = compile_fmpl(r#":Triple(1, 2, 3) @ { :Triple(a, b, c) => a + b + c }"#);

    group.bench_function("three_children", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_tagged_extraction_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_extraction");

    // Nested tagged values (AST-like structure)
    let code = compile_fmpl(
        r#":Binary(:Add, :Int(1), :Int(2)) @ { :Binary(op, :Int(a), :Int(b)) => [op, a, b] }"#,
    );

    group.bench_function("nested_ast", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_tagged_extraction_deep_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("tagged_extraction");

    // Deep nesting (3 levels)
    let code = compile_fmpl(r#":A(:B(:C(42))) @ { :A(:B(:C(v))) => v }"#);

    group.bench_function("deep_3_levels", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// Let Binding Destructuring Benchmarks (Fast path)
// =============================================================================
// These test the fast-path pattern compilation used in let bindings

fn bench_let_map_destructuring(c: &mut Criterion) {
    let mut group = c.benchmark_group("let_destructuring");

    // Map destructuring in let binding (use newline separator, not semicolon)
    let code = compile_fmpl(
        r#"let (%{a: x, b: y} = %{a: 1, b: 2})
x + y"#,
    );

    group.bench_function("map", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_let_list_destructuring(c: &mut Criterion) {
    let mut group = c.benchmark_group("let_destructuring");

    // List destructuring in let binding
    let code = compile_fmpl(
        r#"let ([a, b, c] = [1, 2, 3])
a + b + c"#,
    );

    group.bench_function("list", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_let_tagged_destructuring(c: &mut Criterion) {
    let mut group = c.benchmark_group("let_destructuring");

    // Tagged destructuring in let binding
    let code = compile_fmpl(
        r#"let (:Pair(a, b) = :Pair(10, 20))
a + b"#,
    );

    group.bench_function("tagged", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// Multiple Match Arms Benchmarks
// =============================================================================
// Test backtracking performance with multiple match arms

fn bench_multiple_arms_first_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_arms");

    // First arm matches with guard on integer
    let code = compile_fmpl(
        r#"%{status: 200} @ { %{status: _:s} when s == 200 => "success"; %{status: _:s} => "other"; _ => "unknown" }"#,
    );

    group.bench_function("first_arm", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_multiple_arms_last_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_arms");

    // Last arm (wildcard) matches after backtracking
    let code = compile_fmpl(r#"42 @ { %{status: _:s} => "map"; [ _:x] => "list"; _ => "other" }"#);

    group.bench_function("last_arm_wildcard", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_multiple_arms_guard_fallthrough(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_arms");

    // Guard fails, falls through to next arm
    let code = compile_fmpl(
        r#"%{status: 404} @ { %{status: _:s} when s == 200 => "ok"; %{status: _:s} when s == 404 => "not found"; _ => "error" }"#,
    );

    group.bench_function("guard_fallthrough", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// Comparison Benchmarks (baseline operations)
// =============================================================================

fn bench_baseline_map_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    // Just construct a map (no pattern matching)
    let code = compile_fmpl(r#"%{a: 1, b: 2, c: 3}"#);

    group.bench_function("map_construction", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_baseline_list_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    // Just construct a list (no pattern matching)
    let code = compile_fmpl(r#"[1, 2, 3]"#);

    group.bench_function("list_construction", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_baseline_tagged_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    // Just construct a tagged value (no pattern matching)
    let code = compile_fmpl(r#":Triple(1, 2, 3)"#);

    group.bench_function("tagged_construction", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

fn bench_baseline_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");

    // Simple arithmetic (reference point)
    let code = compile_fmpl(r#"1 + 2 + 3"#);

    group.bench_function("arithmetic", |b| {
        b.iter(|| run_code(black_box(&code)));
    });

    group.finish();
}

// =============================================================================
// Scaling Benchmarks
// =============================================================================

fn bench_map_key_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.throughput(Throughput::Elements(1));

    for n_keys in [1, 5, 10, 20, 50].iter() {
        // Build a map with n_keys keys and extract the last one
        let keys: Vec<String> = (0..*n_keys).map(|i| format!("k{}: {}", i, i)).collect();
        let map_literal = format!("%{{{}}}", keys.join(", "));
        let last_key = format!("k{}", n_keys - 1);
        let source = format!("{} @ {{ %{{{}: _:v}} => v }}", map_literal, last_key);

        let code = compile_fmpl(&source);

        group.bench_with_input(BenchmarkId::new("map_keys", n_keys), n_keys, |b, _| {
            b.iter(|| run_code(black_box(&code)));
        });
    }

    group.finish();
}

fn bench_list_length_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.throughput(Throughput::Elements(1));

    for n_elements in [1, 3, 5, 10].iter() {
        // Build a list with n_elements and extract all of them
        let elements: Vec<String> = (0..*n_elements).map(|i| i.to_string()).collect();
        let list_literal = format!("[{}]", elements.join(", "));
        // Pattern: use wildcards to match all elements and extract first
        let wildcards: Vec<&str> = (0..*n_elements)
            .map(|i| if i == 0 { "_:first" } else { "_" })
            .collect();
        let pattern = format!("[ {}]", wildcards.join(", "));
        let source = format!("{} @ {{ {} => first }}", list_literal, pattern);

        let code = compile_fmpl(&source);

        group.bench_with_input(
            BenchmarkId::new("list_length", n_elements),
            n_elements,
            |b, _| {
                b.iter(|| run_code(black_box(&code)));
            },
        );
    }

    group.finish();
}

fn bench_tagged_children_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.throughput(Throughput::Elements(1));

    for n_children in [1, 3, 5, 10].iter() {
        // Build a tagged value with n_children and extract the first
        let children: Vec<String> = (0..*n_children).map(|i| i.to_string()).collect();
        let tagged_literal = format!(":Node({})", children.join(", "));
        // Pattern: extract first child only (use wildcards for rest)
        let wildcards: Vec<&str> = (0..*n_children)
            .map(|i| if i == 0 { "first" } else { "_" })
            .collect();
        let pattern = format!(":Node({})", wildcards.join(", "));
        let source = format!("{} @ {{ {} => first }}", tagged_literal, pattern);

        let code = compile_fmpl(&source);

        group.bench_with_input(
            BenchmarkId::new("tagged_children", n_children),
            n_children,
            |b, _| {
                b.iter(|| run_code(black_box(&code)));
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = map_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_map_extraction_small,
        bench_map_extraction_medium,
        bench_map_extraction_nested,
        bench_map_extraction_single_key,
}

criterion_group! {
    name = list_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_list_extraction_small,
        bench_list_extraction_medium,
        bench_list_extraction_nested,
        bench_list_extraction_head,
        bench_list_extraction_single,
}

criterion_group! {
    name = tagged_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_tagged_extraction_single_child,
        bench_tagged_extraction_multiple_children,
        bench_tagged_extraction_nested,
        bench_tagged_extraction_deep_nested,
}

criterion_group! {
    name = let_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_let_map_destructuring,
        bench_let_list_destructuring,
        bench_let_tagged_destructuring,
}

criterion_group! {
    name = multiple_arms_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_multiple_arms_first_match,
        bench_multiple_arms_last_match,
        bench_multiple_arms_guard_fallthrough,
}

criterion_group! {
    name = baseline_benches;
    config = Criterion::default().sample_size(100);
    targets =
        bench_baseline_map_construction,
        bench_baseline_list_construction,
        bench_baseline_tagged_construction,
        bench_baseline_arithmetic,
}

criterion_group! {
    name = scaling_benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_map_key_scaling,
        bench_list_length_scaling,
        bench_tagged_children_scaling,
}

criterion_main!(
    map_benches,
    list_benches,
    tagged_benches,
    let_benches,
    multiple_arms_benches,
    baseline_benches,
    scaling_benches
);
