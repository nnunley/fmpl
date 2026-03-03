# Self-Hosting Bootstrap Design

**Date**: 2026-03-03
**Status**: Design approved

---

## Goal

FMPL compiles itself. The implementation lives in the persistent image, with a serialized bytecode seed for cold bootstrap. Two compilation backends: execution_tape (interactive) and MLIR (optimization/native).

## Design Principles

1. **Image is the compiler** -- develop the compiler interactively in the REPL, persist via Fjall
2. **Seed for reproducibility** -- serialized bytecode snapshot enables cold bootstrap without a running image
3. **Progressive replacement** -- swap Rust components for FMPL equivalents one at a time, keeping the system working at every step
4. **MLIR as optimization infrastructure** -- leverage MLIR's pass system rather than building optimization passes from scratch
5. **FFI over shelling out** -- call MLIR C API through builtins, don't generate text and invoke external tools

## Current State

### What exists in FMPL (lib/core/)

| File | Purpose | Status |
|------|---------|--------|
| `fmpl_parser.fmpl` | Scannerless PEG parser, full language coverage | Working |
| `ast_to_ir.fmpl` | AST -> tree-structured IR via tree grammar | Working |
| `ast_to_ir_indexed.fmpl` | AST -> Indexed RPN (flat instructions) | Broken (state-threading bug) |
| `ir_to_execution_tape.fmpl` | IR -> execution_tape Rust source (text) | Incomplete, wrong approach |
| `ir_to_execution_tape_indexed.fmpl` | IR -> Indexed RPN asm (text) | Incomplete, wrong approach |
| `ir_to_rust.fmpl` | IR -> Rust source (text) | Incomplete |
| `ast_optimizer.fmpl` | Constant folding, algebraic simplification | Working |
| `grammar_optimizer.fmpl` | PEG grammar optimization passes | Working |
| `prelude.fmpl` | Helper functions for grammar actions | Working |

### What exists in Rust

| Component | Lines | Replaceable? |
|-----------|-------|-------------|
| Lexer (`lexer.rs`) | ~380 | Yes -- `fmpl_parser.fmpl` is scannerless |
| Parser (`parser.rs`) | ~2,100 | Yes -- `fmpl_parser.fmpl` covers full syntax |
| Compiler (`compiler.rs`) | ~4,500 | Partially -- `ast_to_ir.fmpl` + `ir::compile()` builtin |
| VM (`vm.rs`) | ~5,600 | No -- irreducible Rust kernel |
| Value types (`value.rs`) | ~1,800 | No -- memory management, type dispatch |
| Object system (`object.rs`) | ~180 | No -- ObjectDb, identity |
| I/O builtins | ~1,200 | No -- system interfaces |
| Grammar engine | ~2,000 | No -- PEG matching core |

### Image persistence gaps

| Component | Persisted? |
|-----------|-----------|
| Parse states | Yes (Fjall, tested) |
| Grammar memos | Partial (Fjall, integration incomplete) |
| Objects (ObjectDb) | No |
| Compiled bytecode | No |
| Grammar definitions | No |
| VM state | No |

## Architecture

### Compiler Pipeline

```
FMPL source
  |
  v
fmpl_parser.fmpl (PEG grammar)
  |
  v
AST tagged values (:Int, :Binary, :Lambda, ...)
  |
  v
ast_to_ir.fmpl (tree grammar)
  |
  v
IR tagged values (:LoadInt, :Add, :Call, ...)
  |
  v
ast_optimizer.fmpl (constant folding)
  |
  v
Optimized IR
  |                          |
  v                          v
ir::compile()           ir_to_mlir.fmpl
(Rust builtin)          (tree grammar, text emission)
  |                          |
  v                          v
execution_tape          MLIR C API (via FFI builtins)
bytecode                     |
  |                          v
  v                     fmpl_opt.fmpl
VM execution            (pass pipeline, lowering)
(interactive)                |
                             v
                        native code / execution_tape
                        (production)
```

Everything above `ir::compile()` is FMPL. The Rust builtin is the assembler for the interactive path. MLIR replaces it for production.

### MLIR Integration

FMPL calls MLIR through FFI builtins wrapping the MLIR C API. No C++ tool, no shelling out.

**FFI builtins to add:**

| Builtin | Purpose |
|---------|---------|
| `mlir::context.create()` | Create MLIR context |
| `mlir::module.parse(ctx, text)` | Parse MLIR text assembly |
| `mlir::module.to_string(module)` | Emit MLIR text |
| `mlir::pass_manager.create(ctx)` | Create pass manager |
| `mlir::pass_manager.add_pass(pm, pass)` | Add pass to pipeline |
| `mlir::pass_manager.run(pm, module)` | Run passes |
| `mlir::pass.from_lambda(fn)` | Register FMPL lambda as MLIR pass |
| `mlir::operation.*` | Query/create/modify operations |
| `mlir::type.*` | Type manipulation |
| `mlir::attribute.*` | Attribute manipulation |

**Dialect definitions via IRDL:**

MLIR's IR Definition Language (IRDL) allows defining dialects declaratively as MLIR operations, avoiding C++ TableGen. FMPL emits IRDL text.

```fmpl
-- fmpl_dialects.fmpl defines the dialect tower
let fmpl_high_irdl = "
  irdl.dialect @fmpl_high {
    irdl.operation @object_spawn { ... }
    irdl.operation @grammar_apply { ... }
    irdl.operation @facet_check { ... }
    irdl.operation @async_send { ... }
    irdl.operation @bcom_commit { ... }
  }
"
```

**Custom passes as FMPL lambdas:**

```fmpl
let lower_objects = mlir::pass.from_lambda(\module {
    -- Transform fmpl.high object ops to fmpl.low struct/vtable ops
    module @ { ... }
})
```

### MLIR Dialect Tower

```
fmpl.high   -- FMPL-specific operations
            -- object.spawn, grammar.apply, facet.as, async.send,
            -- match.dispatch, bcom.commit
                |
                v  (lowering passes, written in FMPL)
fmpl.low    -- Desugared operations
            -- closures -> struct + funcptr, objects -> vtable,
            -- grammars -> state machines, pattern match -> decision trees
                |
                v  (lowering passes, written in FMPL)
arith / func / scf / memref / llvm
            -- Standard MLIR dialects
                |
                v  (MLIR/LLVM built-in passes)
Native code or execution_tape bytecode
```

### Bootstrap Mechanism

**Stage 0 (seed creation):**
The current Rust compiler compiles the FMPL compiler pipeline (`fmpl_parser.fmpl` + `ast_to_ir.fmpl` + compiler driver) into bytecode. Serialize to disk as the seed artifact.

**Stage 1 (from seed):**
Load seed bytecode into a fresh VM. Feed the FMPL compiler's own source to itself. It produces new bytecode. Verify output matches seed (fixpoint check).

**Image workflow:**
The compiler lives as objects in the Fjall-persisted image. Develop interactively in the REPL. Periodically snapshot the seed. Process restarts reload from image; cold bootstrap loads from seed.

```
Normal operation:
  VM starts -> load image from Fjall -> compiler ready

Cold bootstrap:
  VM starts -> no image -> load seed bytecode -> run compiler
  -> compiler compiles itself -> populate image -> snapshot new seed
```

## Phasing

### Phase 1: Parser Cutover

**Goal**: `fmpl_parser.fmpl` replaces the Rust lexer + parser.

**Work:**
- Add a `parse_with_grammar` path in the compilation pipeline that uses `fmpl_parser.fmpl` instead of `Lexer` + `Parser`
- Run both parsers on the test suite, diff AST output
- Fix any mismatches (the FMPL parser produces tagged values, the Rust parser produces `ast::Expr` -- need a bridge or shared representation)
- Add a flag to select parser (default to FMPL parser once parity achieved)
- Retire Rust lexer/parser (keep as stage 0 fallback in fmpl-bootstrap)

**Acceptance criteria:**
- `fmpl_parser.fmpl` produces identical AST for all existing test cases
- REPL and web server use the FMPL parser by default
- `fmpl-bootstrap` retains the Rust parser for seed generation

### Phase 2: Compiler Cutover

**Goal**: `ast_to_ir.fmpl` + `ir::compile()` replaces the Rust compiler.

**Work:**
- Route AST tagged values through `ast_to_ir.fmpl` tree grammar
- Output feeds into existing `ir::compile()` Rust builtin
- Diff compiled bytecode against Rust compiler output
- Fix mismatches until parity
- Retire Rust compiler (keep as stage 0 fallback)

**Acceptance criteria:**
- FMPL compiler produces identical bytecode for all test cases
- `ir::compile()` builtin handles all IR tagged values from `ast_to_ir.fmpl`

### Phase 3: Image Persistence

**Goal**: Full compiler state survives process restarts.

**Work:**
- Add `ObjectDb` serialization to Fjall (objects already have Serialize/Deserialize derives)
- Add `CompiledCode` caching to Fjall (rkyv support exists but unused)
- Add `GrammarRegistry` persistence to Fjall
- Implement `Vm::snapshot()` / `Vm::restore()` for full state
- Test: start VM, compile code, restart, verify state preserved

**Acceptance criteria:**
- Objects, bytecode, grammars survive process restart
- REPL session state persists across restarts
- Web server recovers full image on restart

### Phase 4: Self-Compile and Seed

**Goal**: The FMPL compiler compiles itself. Seed bytecode is a build artifact.

**Work:**
- Write compiler driver in FMPL that orchestrates the pipeline
- Compile the compiler with itself (stage 1)
- Verify fixpoint: stage 1 output == stage 2 output
- Serialize stage 1 bytecode as the seed
- Add `fmpl-bootstrap` mode that loads seed and rebuilds

**Acceptance criteria:**
- `fmpl_compiler.fmpl` compiles itself and produces identical output
- Seed bytecode checked into repo
- Cold bootstrap from seed produces working compiler

### Phase 5: MLIR FFI Builtins

**Goal**: FMPL can call MLIR C API.

**Work:**
- Add `mlir-sys` (or raw C bindings) as optional dependency in fmpl-core
- Implement FFI builtins: `mlir::context`, `mlir::module`, `mlir::pass_manager`, `mlir::operation`, `mlir::type`, `mlir::attribute`
- Implement `mlir::pass.from_lambda()` for registering FMPL functions as MLIR passes
- Test: parse MLIR text, run a pass, emit result

**Acceptance criteria:**
- FMPL code can create MLIR modules, add operations, run passes
- FMPL lambdas can be registered as MLIR passes
- Round-trip: MLIR text -> parse -> pass -> emit matches expected output

### Phase 6: FMPL MLIR Backend

**Goal**: FMPL compiles to native code via MLIR.

**Work:**
- Write `ir_to_mlir.fmpl` -- tree grammar that emits fmpl.high MLIR
- Write `fmpl_dialects.fmpl` -- IRDL definitions for fmpl.high and fmpl.low
- Write `fmpl_opt.fmpl` -- optimizer driver that loads dialects, runs passes
- Write lowering passes: fmpl.high -> fmpl.low -> standard dialects
- Test: compile simple FMPL programs to native executables

**Acceptance criteria:**
- `1 + 2` compiles to native code and returns 3
- Lambda/closure compilation produces working native code
- fmpl-opt is an FMPL program, not a C++ tool

### Phase 7: MLIR Targets execution_tape

**Goal**: MLIR can emit execution_tape bytecode, making `ir::compile()` optional.

**Work:**
- Add execution_tape as an MLIR lowering target (custom dialect or direct emission)
- Verify bytecode matches `ir::compile()` output
- `ir::compile()` Rust builtin becomes optional -- MLIR path covers both native and VM

**Acceptance criteria:**
- MLIR backend produces valid execution_tape bytecode
- VM executes MLIR-generated bytecode correctly
- Full pipeline: FMPL source -> FMPL parser -> FMPL compiler -> MLIR -> execution_tape -> VM

## Dependencies

### External

| Dependency | Purpose | Phase |
|-----------|---------|-------|
| MLIR/LLVM C API | FFI bindings for optimization | Phase 5 |
| IRDL support in MLIR | Declarative dialect definitions | Phase 6 |
| ORC (optional) | JIT compilation of hot paths | Future |

### Internal (git-issue)

Phase 1 depends on:
- Parser completeness (any remaining syntax gaps in `fmpl_parser.fmpl`)
- AST bridge between tagged values and `ast::Expr`

Phase 3 depends on:
- ObjectDb serialization
- Fjall integration for bytecode/grammar storage

Phase 5 depends on:
- MLIR C API availability (build system integration)

## Irreducible Rust Kernel

These components stay in Rust permanently:

- **VM bytecode dispatch** (`vm.rs`) -- hot loop, needs native performance
- **Value types** (`value.rs`) -- memory layout, Arc/Mutex management
- **Async runtime** -- tokio integration
- **I/O builtins** -- curl, file, env (system interfaces)
- **Grammar PEG engine** -- core matching loop
- **Fjall persistence engine** -- storage layer
- **MLIR C API FFI bridge** -- thin wrapper over C functions

Everything else moves to FMPL.

## Related

- [fmpl-core.md](../../specs/fmpl-core.md) -- Core runtime spec
- [grammar-system.md](../../specs/grammar-system.md) -- PEG grammar spec
- [persistence.md](../../specs/persistence.md) -- Fjall persistence spec
- [type-system.md](../../specs/type-system.md) -- Type inference (benefits from MLIR's type system)
- Lattice project (`~/development/lattice`) -- reference for MLIR text emission + C++ backend pattern
