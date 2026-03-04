# Roadmap

## Walking skeleton (ITER-0000)

**Intent:** Establish the bootstrap parity test harness and sentinel corpus, verifying the currently-passing subset of the FMPL compilation pipeline (source → ast::parse → ast_to_ir.fmpl → ir::compile → code::eval) produces correct results for basic expressions.

**Design rationale:** Phase 1 (parser cutover) is already complete. ITER-0000 formalizes the test harness, confirms the passing tests as the sentinel corpus baseline, and establishes the regression gate for all subsequent iterations.

**Journey scenario:** SCENARIO-0016 (parity contract)

**Stories committed:**
- STORY-0007 (EPIC-002)

**Status:** done

---

## Iteration list

### Completed iterations

### ITER-0001 — Parity: Core Expression Coverage

**Stories:** STORY-0043, STORY-0044, STORY-0045, STORY-0046, STORY-0047, STORY-0048
**Status:** done
**Result:** 36/55 parity tests passing. IR compilation layer fully verified.

### ITER-0002 — Parity: Control Flow and Bindings

**Stories:** STORY-0006, STORY-0008, STORY-0049a
**Status:** done
**Result:** Fixed grammar engine binding scoping bug (sub-runtime rule_depth). Unblocked arithmetic, string, let, if, sequence parity.

### ITER-0003 — Parity: Advanced Language Features + ir::compile Gaps

**Stories:** STORY-0049b, STORY-0009
**Status:** done
**Result:** Fixed Star-in-TagMatch list unwrapping, `args` keyword collision, map pair binding, vacuous all() check, added ir::compile handlers for While/For/Block/Pipe/Slice/Match/TryCatch, added tagged value introspection, added InlinePatternBlock→Match in ast::parse. **55/55 parity tests passing, 0 ignored.**

---

## Remaining iterations (critical path to self-hosting)

### ITER-0004 — Optimizer Integration and Compiler Retirement

**Stories:** STORY-0010, STORY-0012, STORY-0011, STORY-0005
**Rationale:** Integrate ast_optimizer.fmpl (constant folding, algebraic simplification) into the bootstrap compilation pipeline between ast_to_ir.fmpl and ir::compile. Verify parity is preserved with optimization enabled. Then retire the Rust compiler from the main compilation path — the FMPL pipeline becomes the default, with the Rust compiler retained only in fmpl-bootstrap as the stage 0 fallback. This is the **compiler cutover milestone**.
**Status:** done (compiler cutover wired; optimizer integration deferred — ast_optimizer.fmpl uses list-based patterns and needs the AST refactor first)
**Impacted scenarios:** SCENARIO-0003, SCENARIO-0016
**Depends on:** ITER-0003 (complete)
**Look-ahead check:** Completes Phase 2. Unblocks self-compile (ITER-0006). Persistence (ITER-0005) must follow ITER-0004b for representation stability.

**Delivered:**
- `eval_via_fmpl_pipeline()` — runs source through ast::parse → ast_to_ir.fmpl → ir::compile → code::eval
- `eval_via_rust_compiler()` — explicit Rust-compiler path (formerly the only `eval`)
- `FMPL_USE_FMPL_COMPILER=1` opt-in flag for FMPL pipeline as default
- 11 E2E tests (`fmpl_pipeline_compiler.rs`) verify identical results: integer, arithmetic with precedence, string, let, if, lambda, list, nested arith, boolean logic, comparison, plus bootstrap caching
- Rust compiler is now an explicit fallback, not the only path

**Deferred to ITER-0004b:**
- STORY-0010 (ast_optimizer.fmpl integration — needs list-based AST refactor; STORY-0012 consolidated into STORY-0010 as duplicate)
- Removing `FMPL_USE_FMPL_COMPILER` opt-in and making FMPL pipeline the default everywhere

### ITER-0004b — Lists-Everywhere AST Refactor and Optimizer Integration

**Stories:** STORY-0010 (STORY-0012 consolidated into STORY-0010 — duplicate scope)
**Rationale:** The "lists everywhere" goal was never actually landed despite earlier auto-memory notes suggesting it was. The current system uses `Value::Tagged` end-to-end (`expr_to_value`, `ast_to_ir.fmpl`, `ast_optimizer.fmpl` all use Tagged). This iteration completes the move: `expr_to_value` flips to emit lists in one shot; all consumers (Rust + FMPL) update to match; the optimizer rewrites to its original list-pattern form; everything wires into the FMPL pipeline. **This iteration MUST land before ITER-0005** — see "Why before persistence" below.
**Status:** pending
**Impacted scenarios:** SCENARIO-0003, SCENARIO-0016, SCENARIO-0039, SCENARIO-0103 (NEW)
**Depends on:** ITER-0004 (compiler cutover wired)
**Look-ahead check:** Locks in `Value::List` as the canonical AST/IR representation BEFORE ITER-0005 starts persisting things — snapshots get one shape, not two.

**Scope (revised after second PAR review on actual codebase state):**

1. **Flip `expr_to_value` to emit lists.** `fmpl-core/src/builtins/ast.rs` rewritten so every `Value::Tagged("Tag", [...])` becomes `Value::List([Value::Symbol("Tag"), ...])`. Tag becomes the first list element instead of a separate enum arm. The `ast_node!` macro at `fmpl-core/src/macros.rs:17` already does exactly this; use it consistently.

2. **Update `ir::compile_node` to read lists.** `fmpl-core/src/builtins/ir.rs` currently has both Tagged and list dispatch paths. Remove the Tagged path; list-head dispatch becomes the only path. Use the `ast_match!` macro at `fmpl-core/src/macros.rs:54`.

3. **Rewrite `lib/core/ast_to_ir.fmpl` to list patterns.** `:Binary(:+, expr:l, expr:r) => :Add(l, r)` becomes `[:Binary, :+, expr:l, expr:r] => [:Add, l, r]`. The pattern shape `[head, ...children]` matches the list representation directly.

4. **Rewrite `lib/core/ast_optimizer.fmpl` to list patterns.** Restore the original list-pattern intent (`[:Binary, :+, [:Int, a], [:Int, b]] => [:Int, a+b]`). Keep the `&{ b != 0 }` div-by-zero guard and the INT_MIN guard from the previous attempt.

5. **Wire optimizer into `eval_via_fmpl_pipeline`** at the correct slot: `ast::parse → ast_optimizer.optimize → ast_to_ir.expr → ir::compile → code::eval`.

6. **Add SCENARIO-0103: full parity corpus passes with optimizer enabled.**

7. **Verify all existing tests pass.** Hundreds of test failures expected initially (every consumer of `ast::parse` output sees a different shape). All must pass before commit.

8. **Document optimizer coverage gap** (TODO in `ast_optimizer.fmpl`): Lambda bodies, Let, Match, Call, List, Map, Block fall through unchanged — constants inside them don't fold. Tracked for a future iteration.

**Explicitly OUT OF SCOPE:**

- **Removing `Value::Tagged` enum arm.** 231+ call sites across grammar runtime, builtins, generated parser. Keep the arm in place (unused by ast::parse path post-refactor) and let a future iteration remove it. The persistence concern (ITER-0005) is addressed by STORY-0099 envelope versioning — schemas can evolve without requiring enum cleanup first.
- **Removing `FMPL_USE_FMPL_COMPILER` opt-in flag.** Default `eval()` still uses `eval_via_native`. AC-2 scope is "for the parity test corpus through the FMPL pipeline," not "for the full 1160-test default suite." Promotion to default is a separate iteration.

**Why before persistence:**
ITER-0005 will serialize `ObjectDb`, `CompiledCode`, `GrammarRegistry`, and the full VM image — all of which transitively contain `Value`. With `expr_to_value` emitting Tagged today, snapshots taken now would be locked to a shape the user wants to abandon. Landing the list refactor first means ITER-0005 persists `Value::List`, the canonical shape going forward.

### ITER-0005 — Image Persistence (Consolidated)

**Stories:** STORY-0099, STORY-0100, STORY-0013, STORY-0014, STORY-0015, STORY-0019, STORY-0021, STORY-0069, STORY-0016, STORY-0017, STORY-0018, STORY-0020
**Rationale:** Consolidated from old ITER-0007/0008/0009. Persist all compiler state to Fjall in one iteration: ObjectDb (objects already derive Serialize/Deserialize), compiled bytecode (rkyv support exists), grammar definitions and memo tables (hardest — semantic actions contain AST expressions), and full VM snapshot/restore. Enable fjall-persistence feature flag. Verify full image survives process restart.
**Status:** pending
**Impacted scenarios:** SCENARIO-0007, SCENARIO-0008, SCENARIO-0009, SCENARIO-0010, SCENARIO-0011, SCENARIO-0099, SCENARIO-0100, SCENARIO-0101, SCENARIO-0102
**Depends on:** ITER-0004b (representation stability — see ITER-0004b "Why before persistence")
**Look-ahead check:** Unblocks self-compile seed creation (ITER-0006).

**Build order within iteration (STORY-0099 and STORY-0100 are foundational):**
1. **STORY-0099 first** — versioned envelope is the schema all other persistence callers will write through. Land it before any single payload writer is plumbed in, so no caller is ever rewritten away from raw `serde_json::to_vec`.
2. **STORY-0100 second** — content-addressed source store. The envelope (STORY-0099) carries a `source_hash` field; nothing populates it until this story lands. Constructor synthesis for sourceless artifacts (objects, anonymous lambdas, runtime grammars) is the hard part — implement and test in isolation before the per-payload stories depend on it.
3. **STORY-0013/0014/0015/0019/0021** — per-payload writers (objects, bytecode, grammars, memo tables) all built on the STORY-0099 envelope and STORY-0100 source store. None should bypass the envelope.
4. **STORY-0016/0017/0018/0020** — VM snapshot, full-image roundtrip, normal-startup loading. Depend on the per-payload writers above.
5. **STORY-0069** — feature flag wiring; ship last so the default-disabled path is well-defined.

### ITER-0006 — Self-Compile and Seed

**Stories:** STORY-0024, STORY-0025, STORY-0027, STORY-0028
**Rationale:** Create seed snapshot from current Rust compiler (Stage 0). Add --snapshot and --from-seed flags to fmpl-bootstrap. Write fmpl_compiler.fmpl — the FMPL compiler driver that orchestrates the full pipeline (fmpl_parser.fmpl → ast_to_ir.fmpl → ast_optimizer.fmpl → ir::compile). Verify round-trip: snapshot → restore → compile "1 + 2" → get 3.
**Status:** pending
**Impacted scenarios:** SCENARIO-0020
**Depends on:** ITER-0004 (compiler cutover), ITER-0004b (representation stability), and ITER-0005 (persistence)
**Look-ahead check:** Unblocks fixpoint verification.

### ITER-0007 — Fixpoint Verification

**Stories:** STORY-0022, STORY-0023, STORY-0026
**Rationale:** The capstone. Stage 0: Rust compiler compiles FMPL compiler pipeline into bytecode seed. Stage 1: load seed, feed FMPL compiler source to itself, produce new bytecode. Verify fixpoint: Stage 1 output == Stage 0 output (byte-identical or semantically equivalent). Verify cold bootstrap from seed produces a working compiler. After this, the bootstrap is stable.
**Status:** pending
**Impacted scenarios:** SCENARIO-0021
**Depends on:** ITER-0006
**Look-ahead check:** After this, self-hosting is achieved.

---

## Deferred

### Parser Cutover Completion (was ITER-0012)

**Stories:** STORY-0001, STORY-0002, STORY-0003, STORY-0004, STORY-0038, STORY-0089
**Rationale:** Phase 1 parser cutover is functionally complete and verified by the 900+ test suite. Formal evidence gathering for AST bridge, parse_with_grammar path, and legacy parser retirement is nice-to-have but not on the critical path.
**Status:** deferred

### Grammar/VM Verification (was ITER-0005/0006)

**Stories:** STORY-0050, STORY-0053, STORY-0057, STORY-0066, STORY-0062, STORY-0070, STORY-0071, STORY-0076, STORY-0077, STORY-0082, STORY-0085, STORY-0086
**Rationale:** Absorbed by ITER-0001 through ITER-0003. The 55/55 parity tests provide stronger evidence that the grammar engine and VM work correctly than the planned formalization stories. These were evidence-gathering, not implementation.
**Status:** absorbed

### Grammar Advanced Features (was ITER-0013)

**Stories:** STORY-0051, STORY-0052, STORY-0054, STORY-0055, STORY-0058, STORY-0059, STORY-0060
**Rationale:** Grammar inheritance, anonymous grammars, PEG combinators, backtracking, memoization, trampolining. Not on the bootstrap critical path. Pursue if stability issues arise.
**Status:** deferred

### VM Advanced Features (was ITER-0014)

**Stories:** STORY-0073, STORY-0074, STORY-0075, STORY-0078, STORY-0079, STORY-0080, STORY-0083, STORY-0084, STORY-0087, STORY-0088
**Rationale:** Pipe operator, name resolution, scoping, nested compiled bodies, object properties, async. Not on the bootstrap critical path. Pursue if needed during self-compile.
**Status:** deferred

### Supporting Infrastructure (was ITER-0015)

**Stories:** STORY-0056, STORY-0061, STORY-0063, STORY-0064, STORY-0065, STORY-0067, STORY-0068, STORY-0081, STORY-0090, STORY-0091, STORY-0092, STORY-0093, STORY-0094, STORY-0095, STORY-0096, STORY-0097, STORY-0098
**Status:** pruned

### MLIR Backend (was ITER-FUTURE)

**Stories:** STORY-0029 through STORY-0042
**Rationale:** Post-self-hosting initiative. Not in scope for bootstrap stabilization.
**Status:** deferred
