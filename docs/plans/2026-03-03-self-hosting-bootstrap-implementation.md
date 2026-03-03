# Self-Hosting Bootstrap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make FMPL self-hosting: the parser is already generated from FMPL grammar, now make it the default, complete the compiler cutover, add image persistence, and prepare the MLIR backend.

**Architecture:** The bootstrap pipeline is `fmpl_parser.fmpl` → `codegen.grammar_to_ir()` → `optimize_grammar()` → `codegen.ir_to_rust()` → generated Rust parser. This already works and passes all tests. The next steps are: (1) make the generated parser the default, (2) wire `ast_to_ir.fmpl` into the compilation path, (3) persist compiler state in the image, (4) add MLIR FFI builtins.

**Tech Stack:** Rust, FMPL grammars, Fjall (persistence), MLIR C API (future)

**Design doc:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`

---

## Phase 1: Make Generated Parser the Default

### Task 1: ✅ Flip parser default to generated (COMPLETE)

**Status:** Completed in commit TBD
**Files:** `fmpl-core/src/lib.rs:53-66`

The generated parser from `fmpl_parser.fmpl` is now the default. The environment variable has been flipped from `FMPL_USE_GENERATED_PARSER=1` (opt-in) to `FMPL_USE_LEGACY_PARSER=1` (opt-out).

All 900+ tests pass with the generated parser as the default.

### Task 2: Update REPL and web server to use generated parser

**Files:**
- Modify: `fmpl-cli/src/main.rs` (find where `eval` is called)
- Modify: `fmpl-web/src/main.rs` (find where `eval` is called)
- Verify: Both already use `fmpl_core::eval()`, so they get the new default automatically

**Step 1: Verify REPL uses fmpl_core::eval()**

Read `fmpl-cli/src/main.rs` and confirm it calls `fmpl_core::eval()` (not a custom pipeline).

**Step 2: Verify web server uses fmpl_core::eval()**

Read `fmpl-web/src/main.rs` and confirm it calls `fmpl_core::eval()`.

**Step 3: Manual test**

Run: `cargo run -p fmpl-cli` and type `1 + 2 * 3`. Expect `=> 7`.
Run: `cargo run -p fmpl-web` and POST to `/eval`. Expect same.

**Step 4: Commit (if any changes needed)**

```
fix(bootstrap): ensure REPL and web use generated parser
```

### Task 3: ✅ Remove FMPL_USE_GENERATED_PARSER env var references (COMPLETE)

**Status:** Completed as part of Task 1
**Files:** `fmpl-core/src/lib.rs`

The old `FMPL_USE_GENERATED_PARSER` environment variable has been removed from the code and replaced with `FMPL_USE_LEGACY_PARSER` for the reverse opt-in.

---

## Phase 2: Compiler Cutover (ast_to_ir.fmpl)

### Task 4: Test ast_to_ir.fmpl parity with Rust compiler

**Files:**
- Create: `fmpl-core/tests/ast_to_ir_parity.rs`

**Step 1: Write parity tests**

These tests compile the same source via both paths and compare results:

```rust
//! Verify ast_to_ir.fmpl produces equivalent IR to the Rust compiler
use fmpl_core::{eval, Value, Vm};

fn run(src: &str) -> Value {
    let mut vm = Vm::new();
    eval(&mut vm, src).expect("runtime error")
}

/// Helper: compile via FMPL pipeline (ast::parse → ast_to_ir → ir::compile → code::eval)
fn run_fmpl_pipeline(src: &str) -> Value {
    let mut vm = Vm::new();
    eval(&mut vm, &format!(r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/ast_to_ir.fmpl")
        let (ast = ast::parse({:?}))
        let (ir = ast @ ast_to_ir.expr)
        code::eval(ir::compile(ir))
    "#, src)).expect("fmpl pipeline error")
}

#[test]
fn parity_integer() {
    assert_eq!(run("42"), run_fmpl_pipeline("42"));
}

#[test]
fn parity_arithmetic() {
    assert_eq!(run("1 + 2 * 3"), run_fmpl_pipeline("1 + 2 * 3"));
}

#[test]
fn parity_string() {
    assert_eq!(run(r#""hello""#), run_fmpl_pipeline(r#""hello""#));
}

#[test]
fn parity_let_binding() {
    assert_eq!(
        run("let (x = 42) x + 1"),
        run_fmpl_pipeline("let (x = 42) x + 1")
    );
}

#[test]
fn parity_if_expr() {
    assert_eq!(
        run("if true then 1 else 2"),
        run_fmpl_pipeline("if true then 1 else 2")
    );
}

#[test]
fn parity_lambda() {
    assert_eq!(
        run("let (f = \\x x + 1) f(41)"),
        run_fmpl_pipeline("let (f = \\x x + 1) f(41)")
    );
}

#[test]
fn parity_list() {
    assert_eq!(run("[1, 2, 3]"), run_fmpl_pipeline("[1, 2, 3]"));
}

#[test]
fn parity_map() {
    assert_eq!(
        run(r#"%{a: 1, b: 2}"#),
        run_fmpl_pipeline(r#"%{a: 1, b: 2}"#)
    );
}
```

**Step 2: Run tests**

Run: `cargo test -p fmpl-core --test ast_to_ir_parity -v`
Expected: Some pass, some fail (ast_to_ir.fmpl may not cover all AST node types)

**Step 3: Document which tests fail**

Create a checklist of failing tests — these are the gaps in `ast_to_ir.fmpl`.

**Step 4: Commit**

```
test(bootstrap): add ast_to_ir.fmpl parity tests

Tests that compare Rust compiler output with FMPL compiler pipeline
(ast::parse → ast_to_ir.fmpl → ir::compile → code::eval).
Some tests may be ignored until ast_to_ir.fmpl is completed.
```

### Task 5: Fix ast_to_ir.fmpl gaps

**Files:**
- Modify: `lib/core/ast_to_ir.fmpl`

**Step 1: Read current ast_to_ir.fmpl**

Read `lib/core/ast_to_ir.fmpl` and identify which AST node types are handled.

**Step 2: For each failing parity test, add the missing rule**

Each rule follows the pattern:
```fmpl
:NodeType(args...) => :IrOp(transformed_args...)
```

Refer to `fmpl-core/src/builtins/ir.rs` (the `IrCompiler`) for the IR format that `ir::compile()` expects.

**Step 3: Run parity tests after each fix**

Run: `cargo test -p fmpl-core --test ast_to_ir_parity -v`

**Step 4: Commit per batch of fixes**

```
feat(bootstrap): add [node types] to ast_to_ir.fmpl
```

### Task 6: Expand parity test coverage

**Files:**
- Modify: `fmpl-core/tests/ast_to_ir_parity.rs`

**Step 1: Add tests for remaining language features**

Cover: while loops, do-while, for loops, try/catch, pattern matching with `@`, objects, grammars, async `<-`, spawn, pipe `|>`, method calls, property access, indexing, slicing, symbols, tagged values.

**Step 2: Fix ast_to_ir.fmpl for each failure**

**Step 3: Commit**

```
test(bootstrap): expand ast_to_ir parity to full language coverage
```

---

## Phase 3: Image Persistence

### Task 7: Persist ObjectDb to Fjall

**Files:**
- Modify: `fmpl-core/src/object.rs`
- Create: `fmpl-core/tests/object_persistence.rs`

**Step 1: Write failing test**

```rust
#[test]
fn object_survives_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(&dir).open().unwrap();

    let mut db = ObjectDb::new();
    let id = db.create();
    db.set_property(id, "name".into(), Value::String("test".into()));

    db.save_to_fjall(&keyspace).unwrap();

    let mut db2 = ObjectDb::new();
    db2.load_from_fjall(&keyspace).unwrap();

    assert_eq!(
        db2.get_property(id, "name"),
        Some(&Value::String("test".into()))
    );
}
```

**Step 2: Implement save_to_fjall / load_from_fjall on ObjectDb**

Objects already derive Serialize/Deserialize. Serialize each object as JSON, store in a Fjall partition keyed by ObjectId.

**Step 3: Run test**

Run: `cargo test -p fmpl-core --test object_persistence -v`
Expected: PASS

**Step 4: Commit**

```
feat(persistence): persist ObjectDb to Fjall
```

### Task 8: Persist compiled bytecode to Fjall

**Files:**
- Modify: `fmpl-core/src/compiler.rs` (or new `fmpl-core/src/bytecode_cache.rs`)
- Create: `fmpl-core/tests/bytecode_persistence.rs`

**Step 1: Write failing test**

```rust
#[test]
fn bytecode_survives_save_restore() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(&dir).open().unwrap();

    let code = Compiler::new()
        .compile(&parse("1 + 2"))
        .unwrap();

    save_bytecode(&keyspace, "test_key", &code).unwrap();
    let restored = load_bytecode(&keyspace, "test_key").unwrap();

    let mut vm = Vm::new();
    assert_eq!(vm.run(&restored).unwrap(), Value::Int(3));
}
```

**Step 2: Implement save/load using rkyv or serde**

CompiledCode contains Instructions which have rkyv support. Serialize to bytes and store in Fjall.

**Step 3: Run test**

Expected: PASS

**Step 4: Commit**

```
feat(persistence): persist compiled bytecode to Fjall
```

### Task 9: Persist GrammarRegistry to Fjall

**Files:**
- Modify: `fmpl-core/src/grammar/mod.rs`
- Create: `fmpl-core/tests/grammar_persistence.rs`

**Step 1: Write failing test**

Test that a grammar defined in one session can be loaded in another.

**Step 2: Implement save/load on GrammarRegistry**

Grammars need serialization. If Grammar contains AST expressions (for semantic actions), these must be serializable too.

**Step 3: Run test**

**Step 4: Commit**

```
feat(persistence): persist GrammarRegistry to Fjall
```

### Task 10: Implement Vm::snapshot() and Vm::restore()

**Files:**
- Modify: `fmpl-core/src/vm.rs`
- Create: `fmpl-core/tests/vm_snapshot.rs`

**Step 1: Write failing test**

```rust
#[test]
fn vm_snapshot_restore() {
    let dir = tempfile::tempdir().unwrap();

    // Create VM, define variable, snapshot
    let mut vm = Vm::new();
    eval(&mut vm, "let x = 42").unwrap();
    vm.snapshot(&dir).unwrap();

    // Create fresh VM, restore, verify variable
    let mut vm2 = Vm::new();
    vm2.restore(&dir).unwrap();
    let result = eval(&mut vm2, "x").unwrap();
    assert_eq!(result, Value::Int(42));
}
```

**Step 2: Implement snapshot/restore**

Snapshot saves: scope (variable bindings), ObjectDb, GrammarRegistry, compiled code cache.
Uses Tasks 7-9 as building blocks.

**Step 3: Run test**

**Step 4: Commit**

```
feat(persistence): implement Vm::snapshot() and Vm::restore()
```

---

## Phase 4: Self-Compile and Seed

### Task 11: Create seed snapshot from current compiler

**Files:**
- Modify: `fmpl-bootstrap/src/main.rs`
- Create: `bootstrap/seed.bin` (binary artifact, gitignored)
- Create: `fmpl-bootstrap/src/seed.rs`

**Step 1: Add `--snapshot` flag to fmpl-bootstrap**

When invoked with `--snapshot <output>`, fmpl-bootstrap:
1. Creates a VM
2. Loads the FMPL compiler pipeline (`prelude.fmpl`, `fmpl_parser.fmpl`, `ast_to_ir.fmpl`, etc.)
3. Snapshots the VM state to the output file

**Step 2: Add `--from-seed` flag**

When invoked with `--from-seed <seed>`, fmpl-bootstrap:
1. Creates a VM
2. Restores from the seed file
3. The FMPL compiler is available as loaded grammars/functions

**Step 3: Test round-trip**

```bash
./target/debug/fmpl-bootstrap --snapshot bootstrap/seed.bin
./target/debug/fmpl-bootstrap --from-seed bootstrap/seed.bin -e "1 + 2"
```

Expected: `3`

**Step 4: Commit**

```
feat(bootstrap): add seed snapshot/restore to fmpl-bootstrap
```

### Task 12: Verify self-compilation fixpoint

**Files:**
- Create: `fmpl-core/tests/bootstrap_fixpoint.rs`

**Step 1: Write fixpoint test**

```rust
#[test]
#[ignore = "requires full bootstrap pipeline"]
fn compiler_compiles_itself_to_same_output() {
    // Stage 0: Rust compiler compiles parser_generator.fmpl → Rust source
    let stage0_output = run_bootstrap("lib/core/parser_generator.fmpl");

    // Stage 1: Load seed, run parser_generator.fmpl through FMPL pipeline
    let stage1_output = run_from_seed("lib/core/parser_generator.fmpl");

    // Fixpoint: both should produce identical output
    assert_eq!(stage0_output, stage1_output);
}
```

**Step 2: Implement and debug until fixpoint achieved**

This is the hardest task — any difference means the FMPL compiler doesn't faithfully replicate the Rust compiler's behavior.

**Step 3: Commit**

```
feat(bootstrap): verify self-compilation fixpoint
```

---

## Phase 5: MLIR FFI Builtins (Future)

### Task 13: Add mlir-sys dependency

**Files:**
- Modify: `fmpl-core/Cargo.toml` (add optional `mlir-sys` or `melior` dependency)
- Modify: `fmpl-core/src/lib.rs` (add `mlir` module behind feature flag)

**Step 1: Add feature flag**

```toml
[features]
mlir = ["dep:mlir-sys"]

[dependencies]
mlir-sys = { version = "0.3", optional = true }
```

**Step 2: Create stub module**

Create `fmpl-core/src/mlir/mod.rs` with basic context creation.

**Step 3: Test**

```bash
cargo build -p fmpl-core --features mlir
```

**Step 4: Commit**

```
feat(mlir): add mlir-sys dependency behind feature flag
```

### Task 14: Implement mlir::context and mlir::module builtins

**Files:**
- Create: `fmpl-core/src/builtins/mlir.rs`
- Modify: `fmpl-core/src/vm.rs` (register `mlir` builtin module)

**Step 1: Write test**

```rust
#[test]
#[cfg(feature = "mlir")]
fn mlir_parse_and_emit() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        let ctx = mlir::context.create()
        let m = mlir::module.parse(ctx, "module {}")
        mlir::module.to_string(m)
    "#).unwrap();
    assert!(matches!(result, Value::String(_)));
}
```

**Step 2: Implement builtins**

Wrap MLIR C API calls for context creation, module parsing, and text emission.

**Step 3: Run test**

**Step 4: Commit**

```
feat(mlir): implement mlir::context and mlir::module builtins
```

### Task 15: Implement mlir::pass_manager and mlir::pass.from_lambda

**Files:**
- Modify: `fmpl-core/src/builtins/mlir.rs`

**Step 1: Write test**

Test that an FMPL lambda can be registered as an MLIR pass and invoked.

**Step 2: Implement pass registration**

The MLIR C API supports `mlirPassManagerAddOwnedPass`. Wrap a Rust closure that calls back into the FMPL VM to execute the lambda.

**Step 3: Run test**

**Step 4: Commit**

```
feat(mlir): implement pass_manager and lambda-based passes
```

### Task 16: Write ir_to_mlir.fmpl

**Files:**
- Create: `lib/core/ir_to_mlir.fmpl`
- Create: `fmpl-core/tests/ir_to_mlir.rs`

**Step 1: Write test**

```rust
#[test]
#[cfg(feature = "mlir")]
fn ir_to_mlir_simple_add() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/ir_to_mlir.fmpl")
        let ast = ast::parse("1 + 2")
        let ir = ast @ ast_to_ir.expr
        ir @ ir_to_mlir.expr
    "#).unwrap();
    // Should produce MLIR text like:
    // %0 = arith.constant 1 : i64
    // %1 = arith.constant 2 : i64
    // %2 = arith.addi %0, %1 : i64
    assert!(matches!(result, Value::String(_)));
}
```

**Step 2: Implement as tree grammar**

```fmpl
let ir_to_mlir = grammar ir_to_mlir {
    expr = :LoadInt(n) => "arith.constant " ++ string(n) ++ " : i64"
         | :Add(expr:l, expr:r) => ...
}
```

**Step 3: Run test**

**Step 4: Commit**

```
feat(mlir): add ir_to_mlir.fmpl tree grammar for MLIR text emission
```

---

## Summary

| Phase | Tasks | Status |
|-------|-------|--------|
| Phase 1: Parser default | Tasks 1-3 | ✅ **COMPLETE** — Generated parser is default |
| Phase 2: Compiler cutover | Tasks 4-6 | Ready now — ast_to_ir.fmpl exists |
| Phase 3: Image persistence | Tasks 7-10 | Needs ObjectDb/bytecode/grammar serialization |
| Phase 4: Self-compile | Tasks 11-12 | Depends on Phase 2-3 |
| Phase 5: MLIR backend | Tasks 13-16 | Future — depends on Phase 2 |

**Critical path:** ~~Tasks 1 →~~ 4 → 5 → 7 → 10 → 11 → 12

**Independent work:** Tasks 13-16 (MLIR) can start in parallel with Phase 3 once Phase 2 is done.
