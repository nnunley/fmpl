# Design note: Lambda carries its original AST/CST in a slot

**Captured:** 2026-05-14 EDT during ITER-0005b planning (post-R2 PAR).
**Status:** captured for a future iteration; deferred out of ITER-0005b for sequencing reasons.
**Origin:** user direction during the ITER-0005b scope-card discussion, after the implementer discovered Lambda holds only bytecode (no AST), which made the proposed constructor synthesizer structurally unworkable.

## Statement

`Lambda` (and likely other runtime artifacts: `Grammar`, `Object`, `Partial`) should carry its **originating source AST/CST in a slot** alongside the bytecode/compiled form. Once a runtime value can answer "what AST produced me?", every downstream concern that needs to recover or re-synthesize source becomes mechanical pretty-printing rather than reverse-engineering.

Today's shape (post-ITER-0004):

```rust
pub struct Lambda {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,    // bytecode only
    pub captures: HashMap<SmolStr, Value>,
}
```

Proposed shape:

```rust
pub struct Lambda {
    pub params: Vec<SmolStr>,
    pub code: Arc<CompiledCode>,
    pub captures: HashMap<SmolStr, Value>,
    /// Originating AST/CST. `Some(ast)` when the lambda was created
    /// via `eval()` or any source-rooted path; `None` only for
    /// runtime-synthesized lambdas (rare; possibly impossible after
    /// this iteration ships). Pretty-printing this slot reproduces
    /// re-parseable source — the foundation for the constructor
    /// synthesizer that STORY-0100 AC-4 + AC-5 require.
    pub source_ast: Option<Arc<Expr>>,
}
```

(Whether the slot holds `Expr` (AST) or some richer CST that preserves comments + whitespace is a design call for that iteration. AST is the minimum-viable; CST gives prettier output and survives source-text fidelity.)

## Why this matters

STORY-0100 AC-4 + AC-5 (constructor synthesis for sourceless artifacts) cannot ship without it. Today's Lambda → bytecode → "pretty-print bytecode" is information-lossy: operator precedence collapses into stack ordering; sub-expression structure is implicit; let-bindings dissolve into Bind/NameRef ops. There's no way to recover the syntactic form once compilation has happened.

With the slot, the synthesizer becomes "pretty-print the AST already stored in the value." That's the framing the original ITER-0005b R1 plan assumed; the implementation discovery that Lambda has no AST is what blocked the original plan. Adding the slot unblocks it.

## What this iteration needs

A future iteration (call it **ITER-0005b-AST-SLOT** or whatever its scope card gets named) would:

1. **T1 — Lambda struct change.** Add `source_ast: Option<Arc<Expr>>`. All Lambda constructors updated. Likely-affected sites (sampling from a quick grep):
   - `fmpl-core/src/compiler.rs` — `compile_lambda` (the main constructor)
   - `fmpl-core/src/vm.rs` — runtime lambda creation paths
   - `fmpl-core/src/builtins/` — any builtin that synthesizes a Lambda value
   - `fmpl-core/src/eval.rs` or wherever the top-level eval lives — already has the AST; threads it down
   - Tests that construct synthetic Lambdas

2. **T2 — eval() threads AST down.** When user source compiles to a Lambda value, the Lambda's `source_ast` slot gets the original AST.

3. **T3 — Other artifacts get the slot too.** Grammar (already AST-shaped in body), Object (the spawn-expression that created it), Partial (probably not — derived from a Lambda; can reach back through `func`).

4. **T4 — AST-to-FMPL-source pretty-printer.** Walks `Expr` and emits a re-parseable FMPL source string. Per DESIGN-002 the AST is list-shaped `[:Tag, child, ...]` so the printer is uniform: numbers → digit text; symbols → `:name`; lists → `[item, item, ...]`; tagged forms → `[:Tag, args...]` or sugar where applicable.

5. **T5 — Three constructor-frame wrappers:** `synthesize_lambda(lam)`, `synthesize_object(obj)`, `synthesize_grammar(g)`. Each delegates to the pretty-printer for the inner pieces and wraps in the right outer syntactic form.

6. **T6 — Alpha-equivalence checker for tests** (the R-J-S-1 finding from ITER-0005b R1 PAR is back in scope here). Walk both ASTs, rename bound vars to canonical `_0, _1, _2...`, compare. Requires distinguishing binder positions from reference positions — non-trivial AST analysis. Could borrow from any existing FMPL transformer pass that does scope walking.

7. **T7 — SCENARIO-0101 evidence.** Spawn object → persist → fetch source → re-eval → check structural equivalence.

## What it touches outside the synthesizer

- **Serialization:** Lambda's serde derive must round-trip the new slot. AST is `Expr`-typed; check whether `Expr` already derives Serialize/Deserialize. If not, add it.
- **Memory:** every Lambda now holds its AST. A 100-line lambda body is ~100 lines of parsed AST in memory. Worth measuring; likely fine for FMPL's working-memory scale but worth profiling if it shows up in benches.
- **Persistence:** when CompiledCode is persisted with `source: Some(bytes)` (ITER-0005b T3), the source is the *outer* source string. But Lambda's `source_ast` is the *inner* AST shape. The two are related but not the same — the outer source produces the AST which produces the Lambda. ITER-0005b's source store stores the OUTER source text; the AST slot stores the parsed form. Both are needed for different use cases (recovery from incompatible bytecode vs. synthesizing sourceless artifacts).

## Sequencing rationale

Captured as a design note rather than absorbed into ITER-0005b because:

1. **ITER-0005b's recovery path (AC-6 / SCENARIO-0102) is independent.** `recover_incompatible` only needs the OUTER source string, which is already plumbed via the `source: Option<&[u8]>` param to `save_to_store`. Recovery ships without the AST slot.
2. **The AST slot is a real architectural change.** Lambda is in `value.rs`, used everywhere. Changing its shape ripples to every site that constructs a Lambda. That's a ~2-3 hour iteration on its own with its own PAR cycle.
3. **Discord bot demo timing.** The user wants Discord-bot work started today. ITER-0005b's 2hr scope unblocks the bot's crash-recovery path; the AST slot iteration can land after the demo.
4. **PAR discipline.** Folding a major Lambda struct change into ITER-0005b right after R2 approved the smaller scope would require R3 (and likely an R4 catching cascading concerns the smaller scope didn't expose). One iteration = one cohesive change set.

## When to schedule

Probably right after ITER-0005b ships and before ITER-0005c (bytecode proof case). The synthesizer + AST slot would naturally compose with 0005c's "real round-trip" test surface — a recovered Lambda whose AST slot survives a process restart is a stronger proof than a Lambda recovered via outer-source recompilation alone.

Tentative naming: **ITER-0005b-AST-SLOT** (lands between 0005b and 0005c). Add to roadmap.md when 0005b closes.

## Open questions for the future iteration

- Does `Expr` already implement `Serialize` + `Deserialize`? (Check at iteration entry — probably yes given FMPL's existing list-form-as-Value architecture.)
- Does `Arc<Expr>` clone cheaply enough to avoid making Lambda construction expensive? (Yes; Arc is one refcount bump.)
- For Object: does `Object` (not `ObjectDb`) have an obvious `source_ast` analog? The Object's spawn-expression AST? Verify by reading `object.rs`.
- For Grammar: Grammar bodies are already AST-shaped (DESIGN-002 list form). Does the slot duplicate the body? Or is the slot the OUTER `grammar { ... }` shell + the body is reachable via the existing rules? Likely the latter.
- Memory budget: a `Vm` with 1000 lambdas now holds 1000 ASTs. What's the typical lambda body size in FMPL programs? Probably small (~10-50 nodes); the cost is real but probably negligible. Measure during the iteration.

## Cross-references

- `docs/superpowers/iterations/requirements/EPIC-003.md` — STORY-0100 AC-4 + AC-5 (the consumers of this slot).
- `docs/superpowers/iterations/behavior-scenarios.md` — SCENARIO-0101 (the test surface this slot makes possible).
- `docs/superpowers/specs/2026-05-14-iter-0005b-plan.md` — the current iteration that explicitly defers this.
- `docs/design-principles.md` DESIGN-002 — list-shaped canonical AST that makes the synthesizer mechanical once the slot exists.
- `fmpl-core/src/value.rs:223` — current `Lambda` struct definition.
