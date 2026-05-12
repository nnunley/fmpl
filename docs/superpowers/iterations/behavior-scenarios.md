# Behavior Scenarios

## Surface Scenarios

## SCENARIO-0001 — Parser parity: FMPL parser produces identical AST to Rust parser

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0001, STORY-0003

**Preconditions:**
- fmpl_parser.fmpl is loaded and operational
- Rust lexer + parser is available
- A test suite of FMPL source files exists

**Action:**
- Parse each test source with the Rust lexer + parser
- Parse each test source with fmpl_parser.fmpl
- Compare the two AST representations for each test source

**Expected observables:**
- Each source produces an ast::Expr AST
- Each source produces AST tagged values
- All AST outputs are structurally identical after bridging
- Zero mismatches between Rust parser and FMPL parser for all test cases

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:196-202`

## SCENARIO-0002 — REPL uses FMPL parser by default

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0001, STORY-0002

**Preconditions:**
- FMPL parser has achieved parity with Rust parser
- Parser selection flag defaults to FMPL

**Action:**
- Launch the REPL (fmpl-cli)
- Enter a valid FMPL expression
- Enter a complex FMPL program with grammars, objects, and patterns

**Expected observables:**
- REPL starts successfully
- Expression is parsed by fmpl_parser.fmpl and evaluates correctly
- Program parses and executes correctly using the FMPL parser
- All REPL interactions use fmpl_parser.fmpl without explicit configuration

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:198-203`

## SCENARIO-0003 — Compiler parity: FMPL compiler produces identical bytecode to Rust compiler

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0005, STORY-0006

**Preconditions:**
- ast_to_ir.fmpl is loaded and operational
- ir::compile() builtin handles all IR tagged values
- Rust compiler is available for comparison

**Action:**
- Compile each test source with the Rust compiler
- Compile each test source through ast_to_ir.fmpl + ir::compile()
- Compare bytecode outputs

**Expected observables:**
- Each source produces execution_tape bytecode
- Each source produces execution_tape bytecode
- Bytecode is identical for each test case
- Zero mismatches between Rust compiler and FMPL compiler for all test cases

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:210-219`

## SCENARIO-0004 — Full image persistence survives process restart

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0017, STORY-0013, STORY-0014, STORY-0015, STORY-0016

**Preconditions:**
- Fjall persistence is configured
- ObjectDb, CompiledCode, and GrammarRegistry persistence are implemented

**Action:**
- Start the VM and compile code, create objects, and define grammars
- Shut down the process
- Start the VM again
- Access previously created objects, compiled code, and grammars

**Expected observables:**
- All state is created in memory and persisted to Fjall
- Process terminates cleanly
- VM loads image from Fjall
- All state is present and functional without recompilation or redefinition
- Objects created before restart are accessible after restart
- Compiled bytecode is available without recompilation
- Grammar definitions are loaded and usable

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:221-236`

## SCENARIO-0005 — REPL session state persists across restarts

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0017

**Preconditions:**
- Image persistence is fully implemented
- REPL is running

**Action:**
- In the REPL, define a variable and a function
- Shut down and restart the REPL
- Reference the previously defined variable and function

**Expected observables:**
- Definitions are stored in the image
- REPL loads from Fjall image
- Values are available and produce correct results
- REPL session state is fully preserved across process restarts

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:234`

## SCENARIO-0006 — Web server recovers full image on restart

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0017

**Preconditions:**
- Image persistence is fully implemented
- Web server (fmpl-web) is running with compiled state

**Action:**
- Interact with the web server creating session state
- Shut down and restart the web server
- Access the web server

**Expected observables:**
- State is persisted to Fjall
- Server starts and loads image from Fjall
- Full image is recovered and operational
- Web server recovers full image on restart without manual intervention

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:235`

## SCENARIO-0007 — Self-compile fixpoint: stage 1 output equals stage 2 output

**Kind:** contract
**Proof seam:** process-level
**Owning stories:** STORY-0022

**Preconditions:**
- FMPL compiler (parser + ast_to_ir + driver) is fully self-hosted
- Seed bytecode exists from Stage 0

**Action:**
- Load seed bytecode into a fresh VM (Stage 0 output)
- Feed the FMPL compiler's own source to itself (Stage 1)
- Feed the FMPL compiler's source to the Stage 1 compiler (Stage 2)
- Compare Stage 1 and Stage 2 bytecode

**Expected observables:**
- VM starts with the compiler loaded from seed
- Compiler produces new bytecode
- Compiler produces bytecode
- Bytecode is identical (fixpoint reached)
- Stage 1 bytecode == Stage 2 bytecode, proving the compiler is a fixpoint

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:173-174`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:243-249`

## SCENARIO-0008 — Cold bootstrap from seed produces working compiler

**Kind:** surface
**Proof seam:** process-level
**Owning stories:** STORY-0023, STORY-0018

**Preconditions:**
- Seed bytecode is on disk
- No Fjall image exists (clean state)

**Action:**
- Start VM with no existing Fjall image
- VM executes seed bytecode (compiler)
- Compiler compiles itself from source
- Snapshot new seed to disk
- Use the compiler to compile and run a test program

**Expected observables:**
- VM detects no image and loads seed bytecode
- Compiler is operational
- Compiler self-compiles and populates the Fjall image
- New seed bytecode is written
- Test program executes correctly
- A working compiler is available after cold bootstrap
- Fjall image is populated
- New seed bytecode is on disk

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:179-186`

## SCENARIO-0009 — Normal startup loads compiler from Fjall image

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0018

**Preconditions:**
- Fjall image exists with persisted compiler state

**Action:**
- Start the VM
- Compile and run a FMPL program

**Expected observables:**
- VM detects existing Fjall image and loads it
- Program compiles and runs correctly without any recompilation of the compiler itself
- Compiler is ready immediately from persisted image without seed loading or recompilation

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:180-181`

## SCENARIO-0010 — MLIR round-trip: parse, pass, emit

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0029, STORY-0030

**Preconditions:**
- MLIR FFI builtins are implemented
- mlir-sys or C bindings are available

**Action:**
- Create an MLIR context with mlir::context.create()
- Parse MLIR text assembly with mlir::module.parse(ctx, text)
- Create a pass manager and add a pass
- Run the pass pipeline on the module
- Emit the result with mlir::module.to_string(module)

**Expected observables:**
- Context is created
- Module is created from text
- Pass manager is configured
- Module is transformed
- Emitted text matches expected output
- MLIR text -> parse -> pass -> emit produces expected output

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:261-266`

## SCENARIO-0011 — FMPL lambda registered as MLIR pass transforms operations

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0031

**Preconditions:**
- MLIR FFI builtins are implemented
- mlir::pass.from_lambda() is available

**Action:**
- Define a FMPL lambda that transforms MLIR operations
- Register the lambda as an MLIR pass with mlir::pass.from_lambda(fn)
- Add the pass to a pass manager and run it on a module
- Emit the transformed module

**Expected observables:**
- Lambda is defined
- Pass is registered
- Module is transformed according to the lambda's logic
- Output reflects the transformations applied by the FMPL lambda
- FMPL lambda successfully executes as an MLIR pass, transforming the module

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:139-146`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:265`

## SCENARIO-0012 — Simple expression compiles to native code via MLIR and executes

**Kind:** surface
**Proof seam:** e2e
**Owning stories:** STORY-0033, STORY-0034, STORY-0036

**Preconditions:**
- ir_to_mlir.fmpl tree grammar is implemented
- fmpl.high and fmpl.low IRDL dialects are defined
- Lowering passes from fmpl.high to standard MLIR dialects exist
- MLIR can emit native code

**Action:**
- Parse '1 + 2' with fmpl_parser.fmpl
- Transform AST to IR with ast_to_ir.fmpl
- Emit fmpl.high MLIR with ir_to_mlir.fmpl
- Lower through fmpl.high -> fmpl.low -> standard dialects -> LLVM
- Compile to native and execute

**Expected observables:**
- AST tagged values produced
- IR tagged values produced
- Valid fmpl.high MLIR text produced
- Valid LLVM IR produced
- Returns 3
- Native executable from FMPL source '1 + 2' returns 3

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:280`

## SCENARIO-0013 — Full MLIR-to-execution_tape pipeline produces valid bytecode

**Kind:** contract
**Proof seam:** e2e
**Owning stories:** STORY-0037

**Preconditions:**
- MLIR backend execution_tape lowering target is implemented
- ir::compile() is available for comparison

**Action:**
- Compile a FMPL program through the MLIR pipeline to execution_tape bytecode
- Compile the same program through ir::compile()
- Execute the MLIR-generated bytecode in the VM
- Compare results of MLIR-generated and ir::compile()-generated bytecode

**Expected observables:**
- Valid execution_tape bytecode is produced
- Reference bytecode is produced
- Program runs correctly
- Results are identical
- MLIR-generated execution_tape bytecode executes correctly in the VM
- Full pipeline: FMPL source -> FMPL parser -> FMPL compiler -> MLIR -> execution_tape -> VM works end-to-end

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:284-297`

## SCENARIO-0014 — Seed creation from Rust compiler produces loadable bytecode

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0024

**Preconditions:**
- Rust compiler is operational
- fmpl_parser.fmpl, ast_to_ir.fmpl, and compiler driver source files exist

**Action:**
- Use the Rust compiler to compile the FMPL compiler pipeline into bytecode
- Serialize the bytecode to disk as the seed artifact
- Load the seed bytecode into a fresh VM
- Execute the loaded compiler on a test input

**Expected observables:**
- Bytecode is produced
- Seed file is written to disk
- VM loads the seed successfully
- Compiler processes the input correctly
- Seed bytecode from Rust compiler is loadable and produces a functional FMPL compiler in the VM

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:170-172`

## SCENARIO-0015 — REPL uses generated parser by default

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0038

**Preconditions:**
- FMPL_USE_LEGACY_PARSER is not set
- fmpl-cli binary is built

**Action:**
- Launch fmpl-cli REPL
- Enter '1 + 2 * 3'

**Expected observables:**
- REPL starts without error
- Output is 7
- Generated parser was used (no legacy parser env var needed)
- REPL correctly evaluates arithmetic using the generated parser

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:33-44`

## SCENARIO-0016 — ast_to_ir.fmpl parity for core constructs

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0007, STORY-0008

**Preconditions:**
- lib/core/prelude.fmpl is loadable
- lib/core/ast_to_ir.fmpl is loadable

**Action:**
- Compile '42' via Rust compiler (eval) and FMPL pipeline (ast::parse -> ast_to_ir.expr -> ir::compile -> code::eval)
- Compile '1 + 2 * 3' via both paths
- Compile '"hello"' via both paths
- Compile 'let (x = 42) x + 1' via both paths
- Compile 'if true then 1 else 2' via both paths
- Compile lambda and call via both paths
- Compile '[1, 2, 3]' via both paths
- Compile '%{a: 1, b: 2}' via both paths

**Expected observables:**
- Both produce identical Value
- Both produce Value::Int(7)
- Both produce identical Value::String
- Both produce Value::Int(43)
- Both produce Value::Int(1)
- Both produce Value::Int(42)
- Both produce identical list value
- Both produce identical map value
- All parity tests pass, confirming ast_to_ir.fmpl matches Rust compiler output for core constructs

**Automation status:** automated (ITER-0004c, sentinel)
**Execution command:** `cargo test -p fmpl-core --test ast_to_ir_parity`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:63-192`
- ITER-0004c kept SCENARIO-0016 as the optimizer-disabled parity gate; SCENARIO-0103 is the sibling optimizer-enabled gate. This separation prevents silent degradation if a future optimizer change folds away inputs that the ast_to_ir.fmpl rules need to be exercised against.

## SCENARIO-0017 — ObjectDb round-trip through Fjall persistence

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0013

**Preconditions:**
- Fjall keyspace is available via tempdir
- ObjectDb supports save_to_fjall and load_from_fjall

**Action:**
- Create an ObjectDb, create an object, set property 'name' to 'test'
- Call save_to_fjall on the ObjectDb with a Fjall keyspace
- Create a new ObjectDb and call load_from_fjall with the same keyspace
- Query get_property on the restored object for 'name'

**Expected observables:**
- Object is created with property set
- Save succeeds without error
- Load succeeds without error
- Returns Value::String('test')
- Object properties survive save/restore cycle through Fjall

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:220-246`

## SCENARIO-0018 — Bytecode round-trip through Fjall persistence

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0014

**Preconditions:**
- Fjall keyspace is available via tempdir
- Bytecode save/load functions exist

**Action:**
- Compile '1 + 2' to CompiledCode
- Save bytecode to Fjall with key 'test_key'
- Load bytecode from Fjall with key 'test_key'
- Execute restored bytecode in a VM

**Expected observables:**
- Compilation succeeds
- Save succeeds
- Load succeeds, returns CompiledCode
- Produces Value::Int(3)
- Compiled bytecode survives save/restore and executes correctly

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:265-288`

## SCENARIO-0019 — VM snapshot and restore preserves variable bindings

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0020

**Preconditions:**
- ObjectDb, bytecode, and GrammarRegistry persistence are implemented

**Action:**
- Create a VM and evaluate 'let x = 42'
- Call vm.snapshot(dir)
- Create a fresh VM and call vm2.restore(dir)
- Evaluate 'x' in the restored VM

**Expected observables:**
- Variable x is bound to 42
- Snapshot writes scope, ObjectDb, GrammarRegistry, and code cache to dir
- Restore succeeds
- Returns Value::Int(42)
- VM state including variable bindings is fully recoverable from a snapshot

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:329-348`

## SCENARIO-0020 — Seed snapshot round-trip with fmpl-bootstrap

**Kind:** surface
**Proof seam:** e2e
**Owning stories:** STORY-0025

**Preconditions:**
- Vm::snapshot and Vm::restore are implemented
- fmpl-bootstrap binary is built

**Action:**
- Run fmpl-bootstrap --snapshot bootstrap/seed.bin
- Run fmpl-bootstrap --from-seed bootstrap/seed.bin -e '1 + 2'

**Expected observables:**
- Seed file is created containing VM state with loaded compiler pipeline
- Output is 3
- FMPL compiler pipeline is usable from a seed snapshot without recompilation

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:371-392`

## SCENARIO-0021 — Self-compilation fixpoint verification

**Kind:** contract
**Proof seam:** e2e
**Owning stories:** STORY-0026

**Preconditions:**
- Seed snapshot exists
- FMPL compiler pipeline is complete
- ast_to_ir.fmpl handles all required constructs

**Action:**
- Stage 0: Rust compiler compiles parser_generator.fmpl to Rust source
- Stage 1: Load seed, run parser_generator.fmpl through FMPL pipeline
- Compare stage0_output and stage1_output

**Expected observables:**
- Produces stage0_output
- Produces stage1_output
- Outputs are identical (fixpoint achieved)
- FMPL compiler produces identical output whether compiled by Rust or by itself

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:400-431`

## SCENARIO-0022 — MLIR module parse and emit round-trip

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0040

**Preconditions:**
- mlir feature flag is enabled
- mlir-sys dependency is available

**Action:**
- Create MLIR context via mlir::context.create()
- Parse 'module {}' via mlir::module.parse(ctx, 'module {}')
- Convert module to string via mlir::module.to_string(m)

**Expected observables:**
- Returns a valid MLIR context value
- Returns an MLIR module value
- Returns Value::String containing valid MLIR text
- MLIR modules can be created, parsed, and emitted from FMPL

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:476-488`

## SCENARIO-0023 — IR to MLIR text emission for simple arithmetic

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0042

**Preconditions:**
- mlir feature flag is enabled
- ir_to_mlir.fmpl exists
- ast_to_ir.fmpl handles arithmetic

**Action:**
- Parse '1 + 2' via ast::parse, transform via ast_to_ir.expr, then apply ir_to_mlir.expr
- Inspect the MLIR text output

**Expected observables:**
- Returns Value::String containing MLIR text
- Contains arith.constant operations for 1 and 2
- Contains arith.addi operation
- FMPL IR is correctly lowered to MLIR text representation

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:530-548`

## SCENARIO-0024 — IR-compiled integer literal matches Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0043

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged value :LoadInt(42) via ir::compile and evaluate with code::eval
- Evaluate '42' via the Rust compiler

**Expected observables:**
- Both results are equal (integer 42)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:26`

## SCENARIO-0025 — IR-compiled boolean literals match Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0043

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged value :LoadBool(true) via ir::compile and evaluate
- Compile IR tagged value :LoadBool(false) via ir::compile and evaluate
- Evaluate 'true' and 'false' via the Rust compiler

**Expected observables:**
- IR true matches Rust true
- IR false matches Rust false

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:26`

## SCENARIO-0026 — IR-compiled arithmetic operations match Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0044

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged values for addition, subtraction, multiplication, division, modulo, and negation via ir::compile
- Evaluate equivalent expressions via the Rust compiler

**Expected observables:**
- Each IR arithmetic result equals the corresponding Rust compiler result

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:27`

## SCENARIO-0027 — IR-compiled comparison and logical operations match Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0045

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged values for ==, !=, <, >, <=, >= comparisons via ir::compile
- Compile IR tagged values for and, or, not logical operators via ir::compile
- Evaluate equivalent expressions via the Rust compiler

**Expected observables:**
- Each IR comparison/logical result equals the corresponding Rust compiler result

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:28-29`

## SCENARIO-0028 — IR-compiled control flow and let bindings match Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0046

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged values for if-true and if-false branches via ir::compile
- Compile IR tagged values for simple let and let-with-arithmetic via ir::compile
- Evaluate equivalent expressions via the Rust compiler

**Expected observables:**
- If-true selects the then branch
- If-false selects the else branch
- Let bindings produce correct values

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:30-31`

## SCENARIO-0029 — IR-compiled data structures and lambda match Rust compiler output

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0047

**Preconditions:**
- A fresh VM is available

**Action:**
- Compile IR tagged values for empty list, list of ints, empty map, and map literal via ir::compile
- Compile IR tagged value for lambda call via ir::compile
- Evaluate equivalent expressions via the Rust compiler

**Expected observables:**
- Each data structure IR result equals the Rust compiler result
- Lambda call IR result equals the Rust compiler result

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:32-33`

## SCENARIO-0030 — Full pipeline integer parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run '42' through full FMPL pipeline: ast::parse -> ast_to_ir.expr -> ir::compile -> code::eval
- Evaluate '42' via the Rust compiler

**Expected observables:**
- Both produce integer 42

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:45`

## SCENARIO-0031 — Full pipeline arithmetic with operator precedence parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run '1 + 2 * 3' through full FMPL pipeline
- Evaluate '1 + 2 * 3' via the Rust compiler

**Expected observables:**
- Both produce integer 7 (multiplication binds tighter than addition)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:46`

## SCENARIO-0032 — Full pipeline string parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run '"hello"' through full FMPL pipeline
- Evaluate '"hello"' via the Rust compiler

**Expected observables:**
- Both produce string "hello"

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:47`

## SCENARIO-0033 — Full pipeline let binding parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run 'let (x = 42) x + 1' through full FMPL pipeline
- Evaluate 'let (x = 42) x + 1' via the Rust compiler

**Expected observables:**
- Both produce integer 43

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:48`

## SCENARIO-0034 — Full pipeline if expression parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run 'if true then 1 else 2' through full FMPL pipeline
- Evaluate 'if true then 1 else 2' via the Rust compiler

**Expected observables:**
- Both produce integer 1

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:49`

## SCENARIO-0035 — Full pipeline lambda parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run 'let (f = \x x + 1) f(41)' through full FMPL pipeline
- Evaluate 'let (f = \x x + 1) f(41)' via the Rust compiler

**Expected observables:**
- Both produce integer 42

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:50`

## SCENARIO-0036 — Full pipeline list parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run '[1, 2, 3]' through full FMPL pipeline
- Evaluate '[1, 2, 3]' via the Rust compiler

**Expected observables:**
- Both produce a list containing integers 1, 2, 3

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:51`

## SCENARIO-0037 — Full pipeline map parity

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- VM with prelude and ast_to_ir loaded

**Action:**
- Run '%{a: 1, b: 2}' through full FMPL pipeline
- Evaluate '%{a: 1, b: 2}' via the Rust compiler

**Expected observables:**
- Both produce a map with keys a->1 and b->2

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:52`

## SCENARIO-0038 — Pipeline setup loads prelude and ast_to_ir successfully

**Kind:** positive
**Proof seam:** integration
**Owning stories:** STORY-0048

**Preconditions:**
- Workspace root contains lib/core/prelude.fmpl and lib/core/ast_to_ir.fmpl

**Action:**
- Create a new VM
- Load prelude via io::load("lib/core/prelude.fmpl")
- Load ast_to_ir via io::load("lib/core/ast_to_ir.fmpl")

**Expected observables:**
- Both loads succeed without error
- VM is ready for pipeline tests

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/ast_to_ir_parity_tests.md:35-41`

## SCENARIO-0039 — Tree grammar transforms AST constant-folding addition

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0057, STORY-0054, STORY-0053

**Preconditions:**
- A grammar `ast::optimizer` extends `base::tree`
- The grammar has an `add` rule matching `[:add a:const, b:const]` with action `a + b`
  where the rule body uses list-pattern syntax with a leading symbol head
- The grammar has a `const` rule matching `[:int, n]` returning `n`

**Action:**
- Apply the `add` rule to the list-shaped node `[:add, [:int, 1], [:int, 2]]`

**Expected observables:**
- The grammar descends into the list-shaped node
- SymbolMatch matches the `:add` head
- The `const` rule matches the two `[:int, n]` children, binding `a=1`, `b=2`
- The semantic action computes `1 + 2`
- The result is the integer value `3`

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:554-559`
- `docs/design-principles.md` (DESIGN-002: single canonical form)

**Note:** Rewritten 2026-05-12 (ITER-0004d.1 T17) — the original phrasing used `:int(n)` value-constructor syntax which was removed per DESIGN-002. The scenario contract is unchanged; only the surface syntax of the example grammar is migrated to the canonical list-pattern form.

## SCENARIO-0040 — Child grammar inherits parent rules and overrides specific rules

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0051

**Preconditions:**
- A parent grammar has rules `digit`, `letter`, and `word`
- A child grammar declared with `<: parent` overrides `word` to match `letter (letter | digit)*`

**Action:**
- Parse `abc` using the child grammar's `digit` rule
- Parse `abc123` using the child grammar's `word` rule

**Expected observables:**
- The inherited `digit` rule is used since the child does not override it
- The child's overridden `word` rule matches alphanumeric sequences
- Inherited rules work unchanged
- Overridden rules use the child's definition

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:241-248`

## SCENARIO-0041 — Super call invokes parent rule from child grammar

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0051

**Preconditions:**
- A parent grammar has a rule `value = digit+`
- A child grammar overrides `value` with `<super.value> | letter+`

**Action:**
- Parse `123` using the child grammar's `value` rule
- Parse `abc` using the child grammar's `value` rule

**Expected observables:**
- The super call invokes the parent's `value` rule
- Digits are matched via the parent rule
- The parent's rule fails for letters
- The child's alternative `letter+` matches
- Super calls correctly delegate to parent rule
- Child alternatives work when parent fails

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:246-248`

## SCENARIO-0042 — Binding and semantic action produce structured output

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0053

**Preconditions:**
- A grammar has rule `pair = ident:key "=" expr:val => %{k: key, v: val}`

**Action:**
- Parse `name=42` using the `pair` rule

**Expected observables:**
- ident matches `name` and binds to `key`
- expr matches `42` and binds to `val`
- Action constructs map `{k: "name", v: 42}`
- Result is a map with keys `k` and `v` containing the bound values

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:234-238`

## SCENARIO-0043 — Semantic predicate gates rule matching

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0053

**Preconditions:**
- A grammar has rule `verb = word:v &{ valid_verb(v) } => v`
- `valid_verb` returns true for known verbs and false otherwise

**Action:**
- Parse `take` where `valid_verb("take")` returns true
- Parse `xyz` where `valid_verb("xyz")` returns false

**Expected observables:**
- word matches `take`
- Predicate evaluates to truthy
- Rule succeeds with value `take`
- word matches `xyz`
- Predicate evaluates to falsy
- Rule fails
- Only inputs passing the predicate are matched

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:201-204`

## SCENARIO-0044 — Ordered choice backtracks on first alternative failure

**Kind:** surface
**Proof seam:** unit
**Owning stories:** STORY-0054, STORY-0055

**Preconditions:**
- A grammar has rule `value = string | number | boolean`

**Action:**
- Parse `42` using the `value` rule

**Expected observables:**
- string alternative fails
- Position is restored via checkpoint
- number alternative succeeds
- Input position is correctly restored between alternatives
- The matched alternative's result is returned

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:345-361`

## SCENARIO-0045 — Star repetition with zero-length guard prevents infinite loop

**Kind:** failure-recovery
**Proof seam:** integration
**Owning stories:** STORY-0055, STORY-0061

**Preconditions:**
- A grammar has a Star pattern wrapping a pattern that can match zero-length input

**Action:**
- Apply the Star pattern to input

**Expected observables:**
- The pattern matches successfully on first iteration
- On a subsequent iteration where position does not advance, the zero-length guard triggers
- The loop terminates
- Star terminates even with zero-length matching sub-patterns
- Results collected before the guard triggered are returned

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:334-344`

## SCENARIO-0046 — GrammarRegistry auto-registers built-in grammars

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0062

**Preconditions:**
- A new GrammarRegistry is constructed

**Action:**
- Query the registry for `base::parser`
- Query the registry for `base::binary`
- Query the registry for `base::tree`

**Expected observables:**
- Returns a grammar with rules: any, digit, letter, space, spaces, word, integer, eof, end
- Returns a grammar with rules: any, byte, uint8, uint16be, uint16le, uint32be, uint32le, end
- Returns a grammar with rules: any, null, bool, int, float, string, symbol, list, map, end
- All three built-in grammars are available without explicit registration

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:407-455`
- `specs/grammar-system.md:509-517`

## SCENARIO-0047 — Incremental parse suspends on NeedInput and resumes to Match

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0064

**Preconditions:**
- A streaming input source with partial data available
- A grammar rule that requires more input than initially available

**Action:**
- Call start(rule_name) to begin parsing
- Call resume(state) with insufficient input
- Add more input data and call resume(state) again

**Expected observables:**
- Returns initial ParseState
- Returns NeedInput(state) with serializable state
- Returns Match(value) with the parsed result
- Parse completes correctly across suspension points
- No data is lost between suspend and resume

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:121-141`

## SCENARIO-0048 — Grammar application via @ operator on text input

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0066

**Preconditions:**
- A grammar `mud::commands` is registered with a `command` rule
- The `command` rule matches `"take" spaces noun:obj` with a semantic action

**Action:**
- Evaluate `"take sword" @ mud::commands.command`

**Expected observables:**
- The string is parsed using the command rule
- The semantic action produces a structured result with action and target
- The @ operator returns the semantic action result from grammar application

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:200-208`

## SCENARIO-0049 — Anonymous grammar extension does not mutate base

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0052

**Preconditions:**
- A base grammar `g` has rules `a` and `b`
- An extension is created via `g <: { c = pattern }`

**Action:**
- Query the extended grammar for rule `a`
- Query the extended grammar for rule `c`
- Query the original base grammar `g` for rule `c`

**Expected observables:**
- Rule `a` is available (inherited from base)
- Rule `c` is available (added by extension)
- Rule `c` is NOT available (base was not mutated)
- Extension creates a new grammar without modifying the original

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:250-256`

## SCENARIO-0050 — Binary grammar parses multi-byte integers from byte stream

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0050

**Preconditions:**
- A grammar `png::header` extends `base::binary`
- The grammar has a `chunk` rule matching uint32be for length, type, data, and CRC

**Action:**
- Apply the `chunk` rule to a byte buffer containing a valid PNG chunk

**Expected observables:**
- uint32be matches 4-byte big-endian length
- uint32be matches 4-byte chunk type
- bytes(len) consumes exactly `len` bytes of data
- uint32be matches 4-byte CRC
- All fields are correctly extracted from the binary stream with bindings

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:545-549`

## SCENARIO-0051 — Memoization returns cached result on repeated rule application

**Kind:** surface
**Proof seam:** unit
**Owning stories:** STORY-0059

**Preconditions:**
- A grammar rule `expr` is applied at position 0
- The result is memoized in the per-position memo table

**Action:**
- Apply rule `expr` at position 0 a second time (e.g., during backtracking)

**Expected observables:**
- The memo table is consulted
- The cached result is returned without re-executing the rule body
- The second application returns the same result as the first
- No redundant pattern matching occurs

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:23`
- `specs/grammar-system.md:99-102`

## SCENARIO-0052 — apply_grammar_to_value handles polymorphic input types

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0067

**Preconditions:**
- A grammar with a `word` rule that matches letter sequences
- apply_grammar_to_value function is available

**Action:**
- Call apply_grammar_to_value with Value::String("hello") and rule "word"

**Expected observables:**
- The string is parsed as text input
- The word rule matches "hello"
- The function returns the matched value regardless of input type

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/grammar-system.md:577-579`

## SCENARIO-0053 — Arithmetic expression compiles and evaluates via indexed RPN

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0070

**Preconditions:**
- VM is initialized with Vm::new()

**Action:**
- Compile the expression (3 + 4) * 5
- Run the compiled code via vm.run()

**Expected observables:**
- Bytecode contains LoadInt(3) at index 0, LoadInt(4) at index 1, Add(0,1) at index 2, LoadInt(5) at index 3, Mul(2,3) at index 4
- values[0] = 3, values[1] = 4, values[2] = 7, values[3] = 5, values[4] = 35
- vm.run() returns Value::Int(35)
- The result is 35

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:179-188`

## SCENARIO-0054 — Variable binding resolved at compile time via NameRef

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0074

**Preconditions:**
- VM is initialized
- Code contains let x = 10; x + 5

**Action:**
- Compile and run resolve_names on code with LoadInt(10), Bind(x,0), LoadVar(x), LoadInt(5), Add(2,3)
- Run the compiled code

**Expected observables:**
- LoadVar(x) at index 2 is replaced with NameRef(bind: 1)
- NameRef reads from the Bind instruction's stored value
- Result is Value::Int(15)
- Variable x resolves to 10 without runtime scope lookup, expression evaluates to 15

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:192-208`

## SCENARIO-0055 — Function call creates isolated frame and returns value

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0076

**Preconditions:**
- VM is initialized
- A lambda add(a, b) = a + b is defined

**Action:**
- Compile add(3, 4) as LoadVar(add), LoadInt(3), LoadInt(4), Call(func:0, args:[1,2])
- Execute the lambda body in the new frame
- Return from the lambda

**Expected observables:**
- Call creates a new Frame for the lambda body
- Parameters a and b are bound to 3 and 4 in the new frame
- The body computes a + b = 7
- The new frame is popped
- Value 7 is stored at the Call instruction's position in the caller's values array
- vm.run() returns Value::Int(7)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:210-218`

## SCENARIO-0056 — Method call binds self to receiver object

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0079

**Preconditions:**
- VM is initialized
- An object counter with value:0 and increment method is defined

**Action:**
- Call counter.increment() via MethodCall instruction
- The method body accesses self.value

**Expected observables:**
- A new frame is created with frame.this set to counter's ObjectId
- LoadSelf returns the counter ObjectId
- self resolves to the counter object, self.value returns 0
- Method executes with self correctly bound to the receiver

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:220-249`

## SCENARIO-0057 — Try/catch catches division by zero

**Kind:** failure-recovery
**Proof seam:** integration
**Owning stories:** STORY-0086

**Preconditions:**
- VM is initialized

**Action:**
- Compile and run: try { 1 / 0 } catch (e) { 99 }

**Expected observables:**
- PushHandler registers catch target before try body
- Division by zero triggers Throw
- VM unwinds to handler depth and jumps to catch body
- Error value is bound to e in the catch block
- Expression evaluates to Value::Int(99)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:416-428`

## SCENARIO-0058 — Conditional jump selects correct branch

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0082

**Preconditions:**
- VM is initialized

**Action:**
- Compile if true then 1 else 2 with JumpIfFalse on the condition

**Expected observables:**
- Condition is truthy, so JumpIfFalse does NOT jump
- Execution continues to the then-branch which produces 1
- Expression evaluates to Value::Int(1)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:96-99`

## SCENARIO-0059 — Lambda captures values from enclosing scope

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0077

**Preconditions:**
- VM is initialized

**Action:**
- Compile: let x = 10; let f = lambda() x + 1; f()

**Expected observables:**
- MakeLambda captures x's value (10) via InstrIndex reference
- When f() is called, the captured value is available in the new frame
- f() returns Value::Int(11)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:133`

## SCENARIO-0060 — Pipe operator applies function to argument

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0073

**Preconditions:**
- VM is initialized
- A function double(x) = x * 2 is defined

**Action:**
- Compile and run: 5 |> double

**Expected observables:**
- Pipe instruction calls double with 5 as the argument
- Result is 10
- Expression evaluates to Value::Int(10)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:136`
- `specs/vm.md:257`

## SCENARIO-0061 — MakeList and MakeMap construct compound values

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0085

**Preconditions:**
- VM is initialized

**Action:**
- Compile and run: [1, 2, 3]
- Compile and run: %{a: 1, b: 2}

**Expected observables:**
- MakeList collects values from LoadInt instructions at indices 0, 1, 2
- Result is a List containing [1, 2, 3]
- MakeMap collects key-value pairs from indexed positions
- Result is a Map with keys a and b
- List contains three integers
- Map contains two key-value pairs

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:116-120`

## SCENARIO-0062 — Block scoping prevents variable leakage

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0075

**Preconditions:**
- VM is initialized

**Action:**
- Compile and run code with a let binding inside a block: { let x = 10; x } followed by a reference to x outside the block

**Expected observables:**
- BlockStart opens a new scope
- Bind introduces x within the scope
- x is accessible inside the block and evaluates to 10
- BlockEnd closes the scope
- Reference to x outside the block fails or resolves to a different binding
- Variable x is not visible after BlockEnd

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:122-127`

## SCENARIO-0063 — GrammarApply parses string input with named rule

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0087

**Preconditions:**
- VM is initialized
- A grammar with a rule named 'digit' is loaded

**Action:**
- Execute GrammarApply with a string input and the digit rule

**Expected observables:**
- The grammar engine parses the string using the named rule
- The parse result is stored at values[ip]
- GrammarApply returns the parsed value

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:432-444`

## SCENARIO-0064 — Copy converges if/else branch results

**Kind:** surface
**Proof seam:** unit
**Owning stories:** STORY-0084

**Preconditions:**
- Compiled code has an if/else with results at different instruction indices

**Action:**
- Execute if true then 42 else 0, where each branch stores its result at a different index, followed by Copy from the taken branch

**Expected observables:**
- The taken branch (then) stores 42 at its index
- Copy reads from the taken branch's index and stores 42 at the convergence point
- The convergence instruction holds Value::Int(42)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/vm.md:168`

## SCENARIO-0065 — Full evaluation pipeline: source to result

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0089, STORY-0092, STORY-0093, STORY-0091, STORY-0094

**Preconditions:**
- ObjectDb is created
- Vm is initialized with ObjectDb

**Action:**
- Call eval(vm, '1 + 2 * 3')

**Expected observables:**
- Source is lexed into tokens
- Tokens are parsed into AST
- AST is compiled to indexed RPN bytecode
- Bytecode is executed by VM
- Result is Ok(Int(7))
- Operator precedence is respected (multiplication before addition)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:158-172`

## SCENARIO-0066 — Tagged-shape list nodes carry head symbol and children

**Kind:** contract
**Proof seam:** unit
**Owning stories:** STORY-0095

**Preconditions:**
- A list-shaped node value is constructed: `Value::List([Value::Symbol("Expr"), Value::Int(1), Value::Int(2)])`
- Per DESIGN-002, structured data uses the single canonical form `[:Tag, child1, child2, ...]` — there is no separate `Value::Tagged` type

**Action:**
- Call `Value::as_node()` on the value (the canonical introspection helper)

**Expected observables:**
- `as_node()` returns `Some((tag, children))` because the list starts with a Symbol
- The returned `tag` is the SmolStr-interned symbol `Expr`
- The returned `children` slice equals `[Value::Int(1), Value::Int(2)]`
- The same list round-trips through compile → bytecode → eval and re-emerges as a structurally-equal list node (head + tail preserved)
- A list whose first element is not a Symbol returns `None` from `as_node()` (the introspection helper correctly identifies non-node lists)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:98` (original, pre-pivot — referenced `Value::Tagged`)
- `docs/design-principles.md` (DESIGN-002: list-shaped canonical form; DESIGN-003: symbols for tags)

**Note:** Rewritten 2026-05-12 (ITER-0004d.1 T17) — the original phrasing asserted on `Value::Tagged { tag, children }`, a Rust type deleted in ITER-0004b. The contract is now framed in the post-pivot canonical form: a tagged-shape value IS a list whose first element is a symbol; the `as_node()` helper is the canonical way to introspect it.

## SCENARIO-0067 — Grammar as first-class value

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0095, STORY-0097, STORY-0096

**Preconditions:**
- A Grammar is defined and registered

**Action:**
- Store grammar in a variable as a Value
- Pass grammar value to a stream parse operation

**Expected observables:**
- Grammar is wrapped as Value::Grammar(Arc<Grammar>)
- StreamOp::Parse accepts the grammar value and a rule name
- Grammars can be stored, passed, and used as first-class runtime values

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:99`
- `specs/fmpl-core.md:114-115`

## SCENARIO-0068 — Stream parse vs async parse operation modes

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0096

**Preconditions:**
- An async stream is producing elements
- A grammar with a named rule is available

**Action:**
- Apply StreamOp::Parse with grammar and rule name
- Apply StreamOp::AsyncParse with grammar and rule name

**Expected observables:**
- Stream is parsed in blocking mode using the grammar rule
- Stream is parsed incrementally, allowing suspension and resumption
- Blocking parse completes when stream is fully consumed
- Async parse supports incremental consumption without blocking

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:114-116`

## SCENARIO-0069 — Fjall persistence enables durable parse state

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0069, STORY-0097

**Preconditions:**
- fmpl-core is compiled with fjall-persistence feature enabled
- A parse operation is in progress on a stream

**Action:**
- Suspend parse via ParseState serialization
- Resume parse from serialized ParseState

**Expected observables:**
- ParseState is serialized to Fjall storage
- Memoization table is restored from persistence
- Parse continues from the suspension point
- Parse produces the same result as an unsuspended parse would
- Memo table entries survive the suspension/resume cycle

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:129-136`

## SCENARIO-0070 — Public API exports are all accessible

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0090

**Preconditions:**
- fmpl-core is added as a dependency

**Action:**
- Import all documented public types: eval, Expr, CompiledCode, Compiler, Grammar, GrammarRegistry, Pattern, Rule, Lexer, Token, Object, ObjectDb, ObjectId, Parser, Value, Vm

**Expected observables:**
- All imports resolve without errors
- Each type is usable in downstream code
- All types from the public API section are accessible from fmpl_core

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:74-88`

## SCENARIO-0071 — Evaluation failure returns descriptive error

**Kind:** failure-recovery
**Proof seam:** integration
**Owning stories:** STORY-0089

**Preconditions:**
- Vm is initialized

**Action:**
- Call eval(vm, 'invalid syntax @@@')
- Call eval(vm, 'undefined_var')

**Expected observables:**
- Result is Err with a parse error describing the failure location
- Result is Err with a runtime error describing the undefined variable
- Parse errors and runtime errors are distinct and descriptive
- Error messages include position or context information

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `specs/fmpl-core.md:77`
- `specs/fmpl-core.md:44`

## SCENARIO-0072 — fmpl.high closure lowered to struct + funcptr in fmpl.low

**Kind:** contract
**Proof seam:** integration
**Owning stories:**

**Preconditions:**
- fmpl.high and fmpl.low dialects are defined
- Lowering passes are implemented

**Action:**
- Create fmpl.high MLIR with a closure operation
- Run lowering pass from fmpl.high to fmpl.low

**Expected observables:**
- Valid fmpl.high module
- Closure is replaced with struct + funcptr operations
- fmpl.low output contains no closure ops, only struct and function pointer ops

**Automation status:** pending
**Execution command:** TBD

**Sources:**

## SCENARIO-0073 — Legacy parser opt-in via FMPL_USE_LEGACY_PARSER=1

**Kind:** surface
**Proof seam:** integration
**Owning stories:**

**Preconditions:**
- Both parsers are compiled in

**Action:**
- Set FMPL_USE_LEGACY_PARSER=1 and evaluate '1 + 2'

**Expected observables:**
- Uses Rust parser and returns 3
- Legacy parser is accessible via environment variable

**Automation status:** pending
**Execution command:** TBD

**Sources:**

## SCENARIO-0074 — FMPL_USE_LEGACY_PARSER=1 causes fallback to Rust parser

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0038

**Preconditions:**
- FMPL_USE_LEGACY_PARSER=1 is set in environment
- fmpl-cli binary is built

**Action:**
- Launch fmpl-cli REPL with FMPL_USE_LEGACY_PARSER=1
- Enter '1 + 2 * 3'

**Expected observables:**
- REPL starts using the Rust lexer+parser
- Output is 7, parsed via Rust parser
- Legacy parser fallback works correctly when env var is set

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:17-23`

## SCENARIO-0075 — fmpl-bootstrap uses Rust parser for seed generation

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0001

**Preconditions:**
- fmpl-bootstrap binary is built
- FMPL parser is the default in fmpl-core

**Action:**
- Run fmpl-bootstrap to generate a seed
- Verify the seed compiles correctly

**Expected observables:**
- Seed generation uses the Rust parser, not fmpl_parser.fmpl
- Seed bytecode is valid and loadable
- fmpl-bootstrap always uses the Rust parser for seed generation regardless of fmpl-core defaults

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:199`

## SCENARIO-0076 — Progressive replacement: full test suite passes after parser cutover

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0001

**Preconditions:**
- FMPL parser is set as default
- Full test suite exists (900+ tests)

**Action:**
- Switch the default parser from Rust to FMPL (fmpl_parser.fmpl)
- Run full test suite (cargo test)
- Run cargo clippy

**Expected observables:**
- Parser default is changed
- All 900+ tests pass with the FMPL parser as default
- Zero warnings
- System is fully working after parser cutover with zero test regressions

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:3`

## SCENARIO-0103 — Full parity corpus passes with optimizer enabled

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0010

**Preconditions:**
- VM with prelude, ast_to_ir, and ast_optimizer loaded
- ast_optimizer.fmpl is wired between `ast::parse` and `ast_to_ir.expr` in `eval_via_fmpl_pipeline`

**Action:**
- Run all 55 inputs from the ast_to_ir parity corpus through `eval_via_fmpl_pipeline` (which now includes the optimizer)
- For each input, also evaluate via `eval_via_native` (Rust compiler, no FMPL optimizer involved)

**Expected observables:**
- All 55 results from the optimizer-enabled FMPL pipeline equal the corresponding Rust compiler results
- At least one parity input demonstrably produces folded IR (verifiable by inspecting the IR before `ir::compile`) — proves the optimizer is wired into the actual pipeline, not silently bypassed
- No panics or compile-time errors from `INT_MIN` overflow or division-by-zero folds (corpus must include cases that exercise these guards)

**Automation status:** automated (ITER-0004c)
**Execution command:** `cargo test -p fmpl-core --test scenario_0103_optimizer_pipeline`

**Sources:**
- `docs/superpowers/iterations/requirements/EPIC-002.md` (STORY-0010)
- ITER-0004b PAR scope review (2026-05-08)
- ITER-0004c implementation (2026-05-10) — `fmpl-core/tests/scenario_0103_optimizer_pipeline.rs` provides 4 observables (parity, slot, fold-fires, guards) across 32 passing tests + 1 ignored (INT_MIN deferred to ITER-0004g per lexer limitation).

## SCENARIO-0099 — Loader skips records with incompatible VM version

**Kind:** failure-recovery
**Proof seam:** integration
**Owning stories:** STORY-0099

**Preconditions:**
- A Fjall keyspace contains three persisted records:
  - record A: written by current VM version, schema known
  - record B: written with a `vm_version` major one ahead of current
  - record C: written with a known `vm_version` but an unknown `payload_kind`
- A fourth record D has its CRC32 deliberately corrupted

**Action:**
- Iterate the keyspace through the envelope-aware loader

**Expected observables:**
- Record A loads successfully
- Records B, C, D are skipped without raising an error
- Loader stats report `loaded=1`, `skipped_incompatible=1` (B), `skipped_unknown_kind=1` (C), `skipped_corrupt=1` (D)
- Each skip uses `payload_len + source_len` from the header to advance past the record without parsing the payload
- Iteration completes; no record after a skipped record is missed

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/superpowers/iterations/requirements/EPIC-003.md` (STORY-0099)

## SCENARIO-0100 — Compiled bytecode persists with a content-addressed source reference

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0100

**Preconditions:**
- Fjall keyspace is available via tempdir
- Source store partition is initialized

**Action:**
- Call `eval(vm, "1 + 2")` and persist the resulting `CompiledCode`
- Call `eval(vm, "1 + 2")` again with identical source
- Inspect the source store partition

**Expected observables:**
- Both `CompiledCode` envelopes carry the same `source_hash` (blake3 of `"1 + 2"`)
- The source store contains exactly one entry under that hash
- Reading the source store at that hash returns `"1 + 2"` byte-for-byte
- Identical sources are deduplicated (no double-write)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/superpowers/iterations/requirements/EPIC-003.md` (STORY-0100)

## SCENARIO-0101 — Sourceless artifact gets a synthesized constructor expression

**Kind:** surface
**Proof seam:** integration
**Owning stories:** STORY-0100

**Preconditions:**
- VM is initialized with ObjectDb
- Source store partition is available

**Action:**
- Spawn an object at runtime with two facets and three property bindings (no originating FMPL text)
- Persist the object via the envelope writer
- Read back the source-hash from the envelope, fetch from source store
- Evaluate the fetched source text in a fresh VM

**Expected observables:**
- The fetched source text parses as a valid `spawn(...)` constructor expression
- The text references the same facet names and property keys as the original
- Evaluating the text produces an object with structurally equivalent facets and properties (same names, same values)

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/superpowers/iterations/requirements/EPIC-003.md` (STORY-0100)

## SCENARIO-0102 — Loader recovers from incompatible payload via source recompilation

**Kind:** failure-recovery
**Proof seam:** integration
**Owning stories:** STORY-0100

**Preconditions:**
- A keyspace contains a `CompiledCode` record whose envelope has a known magic but a `schema_version` the current loader does not understand
- The envelope's `source_hash` resolves to `"1 + 2"` in the source store

**Action:**
- Load the keyspace via the envelope-aware loader

**Expected observables:**
- The payload decode fails (incompatible schema)
- The loader detects the present `source_hash` and attempts recovery
- The recovery path resolves the hash, fetches `"1 + 2"`, recompiles via current `eval()`
- A new `CompiledCode` is bound under the original record's key
- Loader stats report `loaded=0`, `recovered_from_source=1`
- Executing the recovered code returns `Value::Int(3)`

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/superpowers/iterations/requirements/EPIC-003.md` (STORY-0100)

## SCENARIO-0077 — Web server POST to /eval uses generated parser

**Kind:** surface
**Proof seam:** app-level
**Owning stories:** STORY-0038

**Preconditions:**
- fmpl-web binary is built
- Generated parser is the default

**Action:**
- Launch fmpl-web server
- POST '1 + 2 * 3' to /eval endpoint

**Expected observables:**
- Server starts on port 3000
- Response is 7
- Web server correctly evaluates expressions using the generated parser via POST /eval

**Automation status:** pending
**Execution command:** TBD

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:43-44`

## SCENARIO-0104 — Parser rejects `:Tag(args)` value-constructor syntax

**Kind:** invariant
**Proof seam:** unit
**Owning stories:** STORY-0010 (EPIC-002), STORY-0095 (EPIC-032)
**Action type:** `parse_rejection`

**Preconditions:**
- The Rust FMPL parser (`parser.rs`) is the entry point under test
- The fallback grammar/parser DSL parser (`grammar/parser.rs`) is also under test (both parsers describe the same language per DESIGN-001)
- DESIGN-002 requires structured data use only the canonical list-shape `[:Tag, ...]` form

**Action:**
- For each parser, attempt to parse a source string containing a value-position `:Tag(args)` construction. Inputs to exercise:
  - `:Foo(1)` — single-argument
  - `:Bar(1, 2, 3)` — multi-argument
  - `let x = :Pair(1, 2)` — in let-binding rhs (a context where the rejection must fire even though `:Pair` could otherwise be a symbol)

**Expected observables:**
- Each parse attempt produces a structured parser error (not a panic, not a successful AST)
- The error message names the unsupported form and points the user to the canonical alternative — specifically, the error contains the phrase `use [:Tag] or [:Tag, ...] instead`
- The error carries a source location (line/column) pointing at the offending `(` token following the tag symbol
- No `Expr::Tagged` AST node is produced (the variant is deleted; the only outcome is the structured error)

**Cases:**
- source: `:Foo(1)`
- source: `:Bar(1, 2, 3)`
- source: `let (x = :Pair(1, 2)) x`
- action: `parse_success`
  source: `:Foo`
- action: `parse_success`
  source: `[:Foo, 1, 2]`
- source: `:Foo(1)`
  expect_error_contains:
    - `[:`
    - `instead`

**Automation status:** implemented (ITER-0004d.1 T19)
**Execution command:** `cargo test -p fmpl-core --test structural_invariants scenario_0104`

**Sources:**
- `docs/design-principles.md` (DESIGN-001 metacircular bootstrap, DESIGN-002 single canonical form)
- `fmpl-core/src/parser.rs:619` (T6 rejection site in expression position)
- `fmpl-core/src/grammar/parser.rs:874, 1099, 1316` (F1 rejection sites in grammar DSL)
- `fmpl-core/tests/structural_invariants.rs` (evidence tests, T19)

**Note:** Added 2026-05-12 (ITER-0004d.1 T17). The rejection behavior is already implemented; this scenario writes the behavior contract that T19 will turn into a passing evidence test.

## SCENARIO-0105 — Parser rejects `:Tag(p1, p2)` pattern-position syntax

**Kind:** invariant
**Proof seam:** unit
**Owning stories:** STORY-0010 (EPIC-002), STORY-0095 (EPIC-032)
**Action type:** `parse_rejection`

**Preconditions:**
- The Rust pattern parser (`parser.rs::parse_pattern`) is under test
- The grammar DSL parser (`grammar/parser.rs`) is also under test for pattern positions
- DESIGN-002 requires patterns over tagged shapes use the canonical list-pattern `[:Tag, p1, p2]` form

**Action:**
- For each parser, attempt to parse a source string containing a pattern-position `:Tag(p1, p2)` construction. Inputs to exercise:
  - `match x { :Pair(a, b) => ... }` — in a match arm pattern position
  - `let (:Pair(a, b) = expr)` — in a let-binding destructuring pattern (the pre-F2 rejection-sensitive site)
  - A grammar rule body like `:add(p1, p2)` — pattern position in the grammar DSL

**Expected observables:**
- Each parse attempt produces a structured parser error (not a panic, not a successful pattern)
- The error message identifies the unsupported pattern syntax and points to the canonical alternative — specifically, the error contains `use [:Tag] or [:Tag, ...] instead`
- The error carries a source location pointing at the `(` token following the tag symbol in pattern position
- No `ast::Pattern::Constructor`, `pattern::Pattern::Tagged`, or `pattern::Pattern::TagMatch` value is produced (those variants are deleted)

**Cases:**
- source: `match x { :Pair(a, b) => 1 }`
- source: `let (:Pair(a, b) = pair_value) a + b`
- action: `parse_success`
  source: `match x { [:Pair, a, b] => 1 }`
- source: `match x { :Foo(a) => 1 }`
  expect_error_contains:
    - `[:`
    - `instead`

**Automation status:** implemented (ITER-0004d.1 T19)
**Execution command:** `cargo test -p fmpl-core --test structural_invariants scenario_0105`

**Sources:**
- `docs/design-principles.md` (DESIGN-001 metacircular bootstrap, DESIGN-002 single canonical form)
- `fmpl-core/src/parser.rs:1839` (F2 rejection site in pattern position)
- `fmpl-core/src/grammar/parser.rs:874, 1099, 1316` (F1 rejection sites; grammar DSL patterns and grammar value-constructors share the rejection path)
- `fmpl-core/tests/structural_invariants.rs` (evidence tests, T19)

**Note:** Added 2026-05-12 (ITER-0004d.1 T17). Distinct from SCENARIO-0104 in two ways: (1) pattern-position parsing goes through a separate AST path (`parse_pattern` vs `parse_expr`) so it must be exercised independently; (2) three distinct AST/pattern variants were deleted to enforce this rejection (Constructor, Tagged, TagMatch) versus one for value position (Expr::Tagged).

## SCENARIO-0106 — Rust-side greppable invariant: deleted variants stay deleted

**Kind:** invariant
**Proof seam:** unit
**Owning stories:** STORY-0010 (EPIC-002), STORY-0095 (EPIC-032)
**Action type:** `expect_absent`

**Preconditions:**
- A run of `cargo test -p fmpl-core` is available
- The repo root is the working directory
- The legacy-syntax scanner (`fmpl-core/tests/diagnostics_fmpl_source_scan.rs`) is built
- DESIGN-002 requires the deleted variants (`Value::Tagged`, `Expr::Tagged`, `ast::Pattern::Constructor`, `pattern::Pattern::Tagged`, `pattern::Pattern::TagMatch`) remain deleted

**Action:**
- Run seven structural greps over the `fmpl-core/src/` tree (excluding `tests/`, `target/`, and historical scans). For each grep, count the number of matches.

**Expected observables:**
For all seven greps, the count is **zero** outside the strictly-allowed sites:

1. `\bValue::Tagged\b` — must NOT appear in `src/` (deleted in ITER-0004b)
2. `\bExpr::Tagged\b` — must NOT appear in `src/` (deleted in T9)
3. `\bPattern::Constructor\b` where `Pattern` resolves to `ast::Pattern` — must NOT appear in `src/` (deleted in T11). Synthetic enum names like `MyPattern::Constructor` in tests are allowed; the gate must distinguish.
4. `\bPattern::Tagged\b` where `Pattern` resolves to `pattern::Pattern` — must NOT appear in `src/` (deleted in T12)
5. `\bPattern::TagMatch\b` — must NOT appear in `src/` (deleted in T14)
6. `Instruction::MakeListNode` as a qualified reference (construction or pattern) — must NOT appear in `src/compiler.rs` (the emit was deleted as part of ITER-0004d.1 T9; the bare `MakeListNode` token inside the `Instruction` enum definition at `compiler.rs` is allowed because the test only flags the qualified `Instruction::MakeListNode` form). Surviving references in `src/vm.rs` (runtime dispatch handler) and `src/builtins/ir.rs` (IR-node handler) are explicitly out of scope for this grep (it is scoped to `compiler.rs` only). Needle was renamed from `\bInstruction::MakeTagged\b` to `Instruction::MakeListNode` by ITER-0004d.2 T6 to track the opcode rename; the semantic invariant (no qualified emit reference in `compiler.rs`) is unchanged.
7. `ExtractListChild` is the canonical replacement for the deleted-variant pattern-extraction path — it MUST appear at least once in `src/compiler.rs` (positive invariant: the replacement exists). The three live emit sites are in the `UP::ListMatch` arm and related list-pattern lowering. Needle was renamed from `\bExtractTaggedChild\b` to `ExtractListChild` by ITER-0004d.2 T6; the semantic invariant (≥1 live reference in `compiler.rs`) is unchanged.

**Expected observables (summary):**
- All seven greps produce the expected count (six at zero, one at ≥1)
- A diagnostic-style report names each grep and its count, so a regression points directly at the violating file:line
- Running this scenario before-and-after ITER-0004d.1 shows a strict drop in counts 1-6 and a strict rise in count 7

**Cases:**
- needle: `Value::Tagged`
  scope: `fmpl-core/src/`
- needle: `Expr::Tagged`
  scope: `fmpl-core/src/`
- needle: `Pattern::Constructor`
  scope: `fmpl-core/src/`
- needle: `Pattern::Tagged`
  scope: `fmpl-core/src/`
- needle: `Pattern::TagMatch`
  scope: `fmpl-core/src/`
- needle: `Instruction::MakeListNode`
  scope: `fmpl-core/src/compiler.rs`
- action: `expect_present`
  needle: `ExtractListChild`
  scope: `fmpl-core/src/compiler.rs`
  min_count: 1
- action: `expect_present`
  needle: `LegacyTagCtor`
  scope: `lib/core/fmpl_parser.fmpl`
- action: `expect_present`
  needle: `LegacyTagCtor`
  scope: `fmpl-core/src/builtins/ir_to_rust.rs`
- action: `expect_present`
  needle: `PatternLegacyTagCtor`
  scope: `lib/core/fmpl_parser.fmpl`
- action: `expect_present`
  needle: `PatternLegacyTagCtor`
  scope: `fmpl-core/src/builtins/ir_to_rust.rs`
- needle: `Type::Tagged`
  scope: `fmpl-core/src/`

**Automation status:** implemented (ITER-0004d.1 T19)
**Execution command:** `cargo test -p fmpl-core --test structural_invariants scenario_0106`

**Sources:**
- `docs/design-principles.md` (DESIGN-001 metacircular bootstrap, DESIGN-002 single canonical form)
- `fmpl-core/tests/structural_invariants.rs` (the evidence tests; ITER-0004d.1 T19 — the implementation uses a small in-test src-tree walker rather than the `diagnostics_fmpl_source_scan` helper because the greps target Rust type names, not FMPL `:Tag(args)` syntax, so the existing diagnostics scanner doesn't apply)
- F19 / round-6 PAR correction (this scenario was a finding in pre-T-task review that the parser-rejection scenarios alone don't prove the underlying Rust types stayed deleted)

**Note:** Added 2026-05-12 (ITER-0004d.1 T17). The role of this scenario is distinct from SCENARIO-0104/0105 (which exercise the parser surface). 0106 is the structural guard that ensures a future contributor doesn't reintroduce the deleted variants by name even if the parser surface still rejects the syntax — i.e., it catches the case where someone adds a new producer for the old variants from a non-parser surface (FFI, deserialization, builtin) and assumes the variants exist again. This is a higher-confidence invariant than syntactic gates because it's typed against the canonical Rust names.

## SCENARIO-0108 — Canonical-pipeline parity with source-tree parser

**Kind:** contract
**Proof seam:** integration
**Owning stories:** STORY-0010 (EPIC-002), DESIGN-001 (metacircular bootstrap)

**Preconditions:**
- The source-tree Rust parser (`Parser::with_source(...).parse()`) is callable.
- The canonical FMPL-generated parser (`parser::generated_parse(...)`) is callable. The build script regenerated it on the current source (no `FMPL_SKIP_PARSER_GEN=1`, no `FMPL_BOOTSTRAP_PHASE=1`).
- The fmpl-bootstrap binary was rebuilt from current source so its embedded postlude (`ir_to_rust.rs::value_to_expr`) is up-to-date.

**Action:**
- For each representative input, invoke both parsers on the same source string. Compare results.

**Expected observables (two equivalence classes):**

1. **Rejection equivalence.** For legacy `:Tag(args)` inputs (SCENARIO-0104 / SCENARIO-0105 carve-outs):
   - Both parsers MUST return `Err`.
   - Both error messages MUST contain the canonical-form hint substring `use [:`.
   - Specifically tested: `:Foo(1)`, `:Bar(1, 2, 3)`, `match x { :Pair(a, b) => 1 }`.

2. **AST equivalence.** For representative successful inputs:
   - Both parsers MUST return `Ok(ast)` with structurally-equal `Expr` trees under `PartialEq`.
   - Inputs covered: `42` (int literal), `1 + 2 * 3` (arithmetic precedence), `:Foo` (bare symbol — the SCENARIO-0104 carve-out), `[:Foo, 1, 2]` (canonical list form).

**Automation status:** implemented (ITER-0004d.3 T7a)
**Execution command:** `cargo test -p fmpl-core --test canonical_pipeline_parity` (run with `FMPL_SKIP_PARSER_GEN` and `FMPL_BOOTSTRAP_PHASE` unset to exercise the canonical pipeline)

**Sources:**
- `docs/design-principles.md` (DESIGN-001 metacircular bootstrap — Rust and FMPL parsers describe the same language)
- `fmpl-core/tests/canonical_pipeline_parity.rs` (the evidence tests; ITER-0004d.3 T7a)
- `lib/core/fmpl_parser.fmpl` (the FMPL grammar; ITER-0004d.3 T7b added the `legacy_tagged_ctor` rejection rule)
- `fmpl-core/src/builtins/ir_to_rust.rs` (postlude `value_to_expr` `"LegacyTagCtor"` arm — emits the rejection error during the value-to-Expr lowering, the only point where parse-action failures can survive the `ParseChoice` closure)

**Note:** Added 2026-05-12 (ITER-0004d.3 T7a). Two PAR scope reviewers independently flagged the absence of a sentinel test routing through `parser::generated_parse` — every other sentinel uses either `Parser::with_source` (source-tree) or `eval_via_legacy_parser`. The "all sentinels pass with canonical pipeline" claim from earlier iterations was weaker than implied because no sentinel exercised the generated parser. SCENARIO-0108 closes that gap. Its first run (before T7b) caught a real divergence: the source-tree parser rejected `:Foo(1)` per ITER-0004d.1 but the FMPL stdlib parser silently accepted it as `Call(Symbol("Foo"), [...])` — the metacircular pipeline was weaker than the source-tree parser. T7b added the `legacy_tagged_ctor` rejection to `lib/core/fmpl_parser.fmpl` (using a poison-AST-node pattern because the FMPL grammar runtime lacks a `fail()` primitive — that limitation is a documented follow-up).

## SCENARIO-0107 — Bytecode opcode rename invariant (post-ITER-0004d.2)

**Kind:** invariant
**Proof seam:** unit
**Owning stories:** STORY-0010 (EPIC-002) AC-11

**Preconditions:**
- ITER-0004d.2 renamed four `Instruction` enum variants to reflect post-ITER-0004d.1 list-node semantics: `MakeTagged` → `MakeListNode`, `ExtractTaggedChild` → `ExtractListChild`, `MatchTagged` → `MatchListNode`, `MatchTaggedWithBindings` → `MatchListNodeWithBindings`. `MatchTag` is PRESERVED unchanged (it backs `Pattern::Symbol` matching per AC-9).
- Wire-format compatibility preserved via `#[serde(rename = "...")]` attributes (Option B).
- `MatchListNode` and `MatchListNodeWithBindings` have ZERO live emit sites in current source (their compiler.rs emits were deleted in ITER-0004d.1). Their VM handlers exist but are unreachable from the sentinel suite without direct bytecode construction.

**Action:**
- Run the seven evidence tests in `fmpl-core/tests/opcode_rename_evidence.rs` plus the updated SCENARIO-0106 greps #6 and #7 in `fmpl-core/tests/structural_invariants.rs`.

**Expected observables:**

*Structural (greps):*
- Grep `Instruction::(MakeTagged|MatchTagged|MatchTaggedWithBindings|ExtractTaggedChild)\b` in `fmpl-core/src/` (non-comment code) returns 0 matches (all renamed). The `#[serde(rename = "...")]` attribute strings are allowed (they're the wire-format compatibility surface).
- Grep `Instruction::(MakeListNode|MatchListNode|MatchListNodeWithBindings|ExtractListChild)\b` in `fmpl-core/src/` returns matches (new names present).
- `Instruction::MatchTag\b` in `compiler.rs` returns matches at variant definition + 4 emit sites (PRESERVED).

*Variant reachability (proves the rename landed):*
- Each renamed variant is constructible from a Rust test crate: `Instruction::MakeListNode { tag, args }`, `Instruction::ExtractListChild { source, index }`, `Instruction::MatchListNode { tag_idx, patterns }`, `Instruction::MatchListNodeWithBindings { tag_idx, bindings }` all compile and execute their no-op constructors.
- `Instruction::MatchTag { value, tag, fail_target, expected_arity }` (PRESERVED) also constructible.

*Wire-format round-trip (catches missing/misspelled `serde(rename)`):*
- `MakeListNode` serializes via `serde_json` to a string containing `"MakeTagged"` (NOT `"MakeListNode"`).
- `ExtractListChild` serializes to `"ExtractTaggedChild"`.
- `MatchListNode` serializes to `"MatchTagged"`.
- `MatchListNodeWithBindings` serializes to `"MatchTaggedWithBindings"`.
- `MatchTag` (PRESERVED) serializes to `"MatchTag"` (no `serde(rename)` attribute).
- Each renamed variant deserializes back into its Rust-side new name (round-trip property).

*Behavioral assurance:*
- SCENARIO-0103 (full parity corpus with optimizer) still passes — exercises the live-emit opcodes `MakeListNode` (via FMPL pipeline list construction) and `ExtractListChild` (via Rust-side `Pattern::Symbol` matching paths in compiler.rs).
- SCENARIO-0016 (parity contract) still passes.
- SCENARIO-0108 (canonical-pipeline parity) still passes.
- `cargo test --workspace` passes; clippy clean.

**Automation status:** implemented (ITER-0004d.2 T7)
**Execution command:** `cargo test -p fmpl-core --test opcode_rename_evidence --test structural_invariants`

**Sources:**
- `fmpl-core/src/compiler.rs:260,364,507,513` — renamed variant definitions with `#[serde(rename)]` attributes
- `fmpl-core/src/vm.rs:877,1182,2521,2567` — renamed VM handler arms (line 1204 `MatchTag` preserved)
- `fmpl-core/src/builtins/ir.rs:336,344,983` — renamed FMPL-IR dispatcher arm keys + construction sites
- `fmpl-core/tests/opcode_rename_evidence.rs` — 7 evidence tests (T7)
- `fmpl-core/tests/structural_invariants.rs` — SCENARIO-0106 greps #6/#7 with new-name needles (T6)
- `docs/superpowers/iterations/requirements/EPIC-002.md` STORY-0010 AC-11

**Note:** Added 2026-05-12 (ITER-0004d.2 T7). The PAR scope review caught two real findings that shaped this scenario card: (1) `MatchListNode` and `MatchListNodeWithBindings` are dead-code post-ITER-0004d.1 (no live emit sites) — sentinel-pass alone doesn't prove their handlers work, so direct variant construction is the proof; (2) `bytecode_persistence.rs` doesn't exercise these opcodes — a missing or misspelled `serde(rename)` would silently ship a wire-format regression, so explicit round-trip tests close that gap.
