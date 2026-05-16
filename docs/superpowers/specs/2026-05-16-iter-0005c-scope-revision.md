# ITER-0005c — Scope (Round 4, post-R3 PAR)

**Date:** 2026-05-16
**Iteration:** ITER-0005c — Single-payload-class persistence: bytecode (proof case)
**Status:** Round 4 scope — applies all convergent R3 findings; proceeding to implementation.
**Predecessor history:** R1 + R2 returned REVISE with extensive findings (architectural). R3 returned REVISE with much smaller, textual findings — convergence on (a) `?Sized` sweep over-broad, (b) AC-narrowing loses session-to-session value, (c) several small textual fixes. R4 applies all of them.

## Honest re-baseline (unchanged from R3)

| Capability | Status | Evidence |
|---|---|---|
| `CompiledCode::save_to_store` | DONE | `fmpl-core/src/compiler.rs:721-750` |
| `CompiledCode::load_from_store` | DONE | `fmpl-core/src/compiler.rs:760-783` |
| Compile → save → load → execute (same store handle, fresh Vm) | DONE | `fmpl-persistence/tests/bytecode_persistence.rs:39-53` |
| `eval_persistent` | DONE | `fmpl-core/src/lib.rs:197-217` |
| `recover_and_rebind` | DONE | `fmpl-core/src/lib.rs:246-268` |
| Source-store integration via envelope `source_hash` | DONE | `fmpl-persistence/tests/bytecode_persistence.rs:143-180` |
| `nested: Vec<CompiledCode>` round-trip (same handle) | DONE | `fmpl-persistence/tests/bytecode_persistence.rs:102-113` |
| SCENARIO-0018 command registered in corpus | NOT DONE | `behavior-corpus.md:22` says `TBD` |
| **Drop+reopen-of-same-path** evidence (cold-load) | NOT DONE | No bytecode test drops `FjallStore` and reopens |
| `?Sized` on `CompiledCode::load_from_store` | NOT DONE | `compiler.rs:760` un-relaxed |
| STORY-0014 AC-1 / SCENARIO-0018 textual citations | INCORRECT | Multiple cross-doc mismatches |

## Revised scope (Round 4)

### Stories committed

**STORY-0014 — Persist compiled bytecode to Fjall.**

AC-1 **revised text** (calibrated to the integration evidence ITER-0005c proves; cross-process proof is left as an explicit carried gap):

> AC-1: Compiled bytecode is stored in Fjall under a key; after the store handle is closed and a fresh handle is opened at the same path in the same process, the bytecode can be loaded by key (without invoking the compiler — verified by the **current implementation** of `CompiledCode::load_from_store`, which neither imports nor calls `Compiler`/`Lexer`/`Parser`) and executed in a fresh VM. · impact:integration · seam:integration · scenario:SCENARIO-0018

Three changes from the original AC-1:
1. `impact:journey` → `impact:integration` (calibrate the claim to evidence — same-process drop+reopen, not subprocess). The cross-process proof STORY-0014's design sources reference (`bootstrap-design.md:223-235` "session-to-session" semantics) is **explicitly listed as a carried gap** owned by a future subprocess-sentinel iteration.
2. `scenario:SCENARIO-0004` → `scenario:SCENARIO-0018` (the dedicated bytecode-Fjall round-trip scenario whose seam is `integration` and whose owning story is exactly STORY-0014).
3. Wording change: "on process restart" → "after the store handle is closed and a fresh handle is opened at the same path in the same process". Plus an explicit parenthetical naming where the "without recompilation" property lives — in the current implementation body, not the type signature (per R3 A-3 / B-S1).

### Build order

1. **T0 — Documentation fixes (pure text, no code).**
   - `requirements/EPIC-003.md:37` — rewrite STORY-0014 AC-1 as above.
   - `roadmap.md` — grep for the literal `"Impacted scenarios: SCENARIO-0007"` in the ITER-0005c card body (the line number has drifted; R3 reviewers diverged on 2412/2417/2418, all rely on grep). Replace with `"Impacted scenarios: SCENARIO-0018"`.
   - `behavior-scenarios.md:506-533` (SCENARIO-0018):
     - Title: "Bytecode round-trip through Fjall persistence — drop+reopen" (drops the "compiler-free load" phrase since the compiler-free ratchet is out of scope — per R3 A-4).
     - Action: AUGMENT (not replace) existing Action to explicitly itemize: (a) open `FjallStore` + `SourceStore` at sibling subdirs of tempdir; (b) compile "1 + 2" to `CompiledCode`; (c) `save_to_store` with source bytes; (d) drop both stores; (e) reopen both stores at the SAME paths; (f) `CompiledCode::load_from_store(&store2 as &dyn Store, "k")`; (g) execute on a fresh `Vm`. (Per R3 B-M-1: this is an augmentation of correct prior text, not a fix of wrong text — the prior Action covered same-handle round-trip and remains correct for that observable.)
     - Expected observables: (i) result equals `Value::Int(3)`; (ii) the source bytes are recoverable from `source_store2.get(envelope.source_hash)` where `envelope.source_hash` is read from `store2`'s raw value at key "k" (proves the envelope's `source_hash` survives drop+reopen too — per R3 B-M-3).
     - Preconditions: keep existing + add a comment-style precondition that "fjall releases its single-writer lock on `Drop`, so re-opening at the same path in the same process succeeds" (per R3 B-M-2 — calls out the external dependency on fjall behavior).
   - `behavior-corpus.md:22`:
     - Title: "Bytecode round-trip across drop+reopen of FjallStore"
     - Command: `cargo test -p fmpl-persistence --features fjall-backend --test bytecode_persistence drop_and_reopen` (substring match runs both new sub-tests)
     - (populated **after** T2's tests land in T3)

2. **T1 — `?Sized` relaxation, bytecode only.**
   - `fmpl-core/src/compiler.rs:760` — `<S: Store>` → `<S: Store + ?Sized>` for `CompiledCode::load_from_store`.
   - **First-consumer:** T2's drop+reopen test calls `CompiledCode::load_from_store(&store2 as &dyn Store, "k")` — a concrete consumer of the relaxation.
   - **Scope narrowing from R3:** `ObjectDb::load_from_store` (`object.rs:223`) and `ParseState::load_from_store` (`incremental.rs:167`) **stay un-relaxed**. Per R3 R-A-2 / B-C-1: relaxing them would manufacture a consumer via mirror compile-only tests, violating `feedback_ship_infrastructure_with_first_consumer.md`. When ITER-0005d wires a real `&dyn Store` consumer through ObjectDb or ParseState, that iteration relaxes them with their first consumer. Surfaced as a **carried gap** (below) so the question doesn't get lost.
   - **Mirror test:** the T2 drop+reopen test IS the relaxation's first-consumer ratchet — no separate mirror needed.

3. **T2 — Drop+reopen tests (the genuinely-new evidence).**
   - Location: `fmpl-persistence/tests/bytecode_persistence.rs` (extends the existing file).
   - **Test 1: `bytecode_survives_drop_and_reopen`.**
     - Compile "1 + 2"; save with source bytes; **drop store + source_store**; **open fresh stores at same paths**; `CompiledCode::load_from_store(&store2 as &dyn Store, "k")` — exercises T1's relaxation; execute on fresh `Vm`; assert `Value::Int(3)`.
     - Additional assertion: read raw envelope bytes from `store2.get(b"k")`, decode header with `EnvelopeHeader::ref_from_prefix`, extract `source_hash`, call `source_store2.get(Hash::from_bytes(source_hash))`, assert the original source bytes are recoverable. (Mirrors the assertion shape from `save_with_source_stamps_envelope_and_populates_source_store` at `bytecode_persistence.rs:143-180`.)
   - **Test 2: `nested_code_survives_drop_and_reopen`.**
     - Same drop+reopen shape, source = `"let f = \\x x + 1; f(41)"` (matches `nested_code_survives` at line 102). Asserts `Value::Int(42)`. Covers `CompiledCode.nested: Vec<CompiledCode>` round-trip across drop+reopen — closes R2 B's finding on the nested-shape gap.

4. **T3 — Wrap artifacts.**
   - Mark STORY-0014 `done:ITER-0005c`.
   - SCENARIO-0018: `Automation status: automated`. Execution command populated per T0 plan.
   - Corpus row at `behavior-corpus.md:22`: Title + Command populated.
   - Roadmap: mark ITER-0005c `done`.
   - Iteration-log entry: include the honest re-baseline narrative + the four concrete deliverables (T0 doc fixes, T1 `?Sized` bytecode-only, T2 two new tests, T3 corpus) + the explicit "no compiler-free ratchet" decision + the explicit carried gaps.

### What this iteration explicitly does NOT include

| Item | Rationale |
|---|---|
| Subprocess sentinel | User chose drop+reopen template (per R3 B-S2: cross-process proof is now a carried gap, not silently dropped) |
| Compiler-free ratchet (counter or grep) | Structurally vacuous per PAR Round 2; not a proof-test gain |
| ObjectDb / ParseState `?Sized` relaxation | No real consumer in ITER-0005c's scope — would violate `feedback_ship_infrastructure_with_first_consumer.md` (per R3 R-A-2 / B-C-1) |
| `save_to_store` `?Sized` extension for ObjectDb/ParseState | Same first-consumer concern; whichever iteration adds a real consumer for these owns the relaxation |
| ITER-0005d/e/f walk-forward template commitment | ITER-0005d's scope card explicitly revisits drop+reopen vs subprocess as a per-payload-class decision (see "ITER-0005d note" below) |
| `MigrationEngine` revival | Per ITER-0005a.0 deferral |
| `TODO(ITER-0005a.4)` prefix-strip removal | ITER-0005a.4 owns it |

### Verification gates

- `cargo test -p fmpl-persistence --features fjall-backend --test bytecode_persistence` — all tests pass (existing + 2 new).
- `cargo test -p fmpl-core --features persistence --lib` — unchanged passing count (relaxation is additive).
- `cargo clippy --workspace --all-features -- -D warnings` clean.
- Sentinel sweep: 26 pass / 0 fail / 4 skip (unchanged from pre-iteration baseline).

### Carried gaps (explicit, with ownership)

1. **Cross-process bytecode load** ("session-to-session restart" per STORY-0014 design source `2026-03-03-self-hosting-bootstrap-design.md:223-235`) is NOT proven by ITER-0005c. ITER-0005c proves only same-process drop+reopen of `FjallStore` handles, calibrating AC-1's `impact` from `journey` to `integration`. Owner: a future subprocess-sentinel iteration (likely co-scoped with ITER-0005f feature-flag wiring or as a sibling of ITER-0005e VM-snapshot tests). Surfaced explicitly per R3 B-S2.
2. **`TODO(ITER-0005a.4)` manual prefix-strip** at `compiler.rs:755-759` — owned by ITER-0005a.4.
3. **ITER-0005e snapshot template question** (drop+reopen vs multi-keyspace atomic rename) — owned by ITER-0005e's scope card.
4. **ObjectDb / ParseState `?Sized` relaxation** — un-relaxed today; sweep when ITER-0005d wires a real `&dyn Store` consumer. Owner: ITER-0005d.
5. **Save-side `?Sized` asymmetry** in `ObjectDb` / `ParseState` `impl` blocks: `save_to_store` ends up symmetric with `load_from_store` (both unrelaxed) but asymmetric with `CompiledCode::save_to_store` (already `?Sized`). Acceptable today since neither has a `&dyn Store` save consumer. Surface for ITER-0005d to revisit (per R3 R-A-1 / B-S3).

### ITER-0005d note (explicit walk-forward)

When ITER-0005d (remaining payload classes) scopes, it MUST revisit the per-payload-class testing seam. Drop+reopen is the precedent ITER-0005c sets, but each payload class (ObjectDb, GrammarRegistry, grammar definitions with AST semantic actions, memo tables) may have its own preconditions that make subprocess testing the better choice. ITER-0005d does NOT silently inherit drop+reopen — its scope card revisits the decision per-payload (per R3 R-A-7 / B-S6).

## Trace: how R3 findings are closed

| Round | Finding | R4 address |
|---|---|---|
| R3 A-1 / B-S3 | T1 save/load asymmetry on ObjectDb/ParseState | T1 narrowed to bytecode only; save/load asymmetry on those two peers is now uniform (both unrelaxed) — carried gap #5 |
| R3 A-2 / B-C-1 | First-consumer principle violated for ObjectDb/ParseState | T1 narrowed to bytecode only — relaxation has real consumer (T2 test) — carried gap #4 |
| R3 A-3 / B-S1 | "Structurally enforced by signature" inaccurate | AC-1 text changed to "verified by the current implementation … which neither imports nor calls `Compiler`/`Lexer`/`Parser`" |
| R3 A-4 | SCENARIO-0018 retitle mentions dropped compiler-free property | Retitle changed to "drop+reopen" only (no "compiler-free load" phrase) |
| R3 A-5 | Mirror-test signatures don't match ObjectDb/ParseState shapes | Mirror tests dropped (T1 narrowed) |
| R3 A-6 / B-S5 | Hardcoded roadmap line numbers wrong | T0 instructs grep, not hardcoded line |
| R3 A-7 / B-S6 | Walk-forward boxing for ITER-0005d | Explicit ITER-0005d note added; carried gap #4 |
| R3 B-C-1 | First-consumer violation on the mirror tests | T1 narrowed; mirror tests dropped |
| R3 B-S2 | AC-narrowing loses session-to-session value | Carried gap #1 names the lost evidence explicitly with proposed owner |
| R3 B-M-1 | "Fix" framing of SCENARIO-0018 Action vs "augment" | Action is now augmented (existing text retained, drop+reopen added) |
| R3 B-M-2 | Missing fjall-behavior precondition comment | Added precondition note about fjall lock release on `Drop` |
| R3 B-M-3 | Source-recovery assertion under-specified | T2 explicitly reads envelope `source_hash` from `store2` post-reopen, proves envelope survives too |

R3 Critical and Serious findings are all addressed. R3 Minor findings are addressed where they have clear actions.

## Decision: proceed to implementation

R1 surfaced architecture/citation issues (closed in R3 scope).
R2 surfaced ratchet vacuity + sibling asymmetry + AC narrowing (closed in R3 scope).
R3 surfaced over-sweep + AC-narrowing acknowledgment + textual drift (closed in R4 scope).

R4 scope: T0 (text fixes), T1 (one line type-bound change with mirror test bundled into T2), T2 (two new tests), T3 (artifact wrap-up).
Total estimated change footprint: ~5 files modified, ~80 LOC added.
No new architectural commitments. All carried gaps explicit with named owners.

**Per `feedback_par_scope_revision_loop.md`: REVISE loops are bounded by convergence.** R3 → R4 has converged: the remaining R3 findings are all textual/scope-clarity, not structural. Proceeding to implementation under this scope. If implementation surfaces a new structural concern, it goes into the iteration-log + a new revision (not a new PAR round).
