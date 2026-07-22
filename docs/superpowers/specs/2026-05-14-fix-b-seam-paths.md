# ITER-0005b-FIX-B — Evidence-seam path study (AC-2 + AC-6)

**Status:** spec / pre-iter-PAR input.
**Date:** 2026-05-14 EDT.
**Owner:** main thread (post sibling-project study).

## Why this doc exists

ITER-0005b shipped STORY-0100's `save_to_store` API surface and a standalone
`recover_incompatible` pass, but two of its ACs are evidence-shallow at the
seams their text declares:

| AC   | Impact         | Declared seam | What's missing                                                                                                                          |
|------|----------------|---------------|------------------------------------------------------------------------------------------------------------------------------------------|
| AC-2 | `journey`      | `integration` | No `eval()` → persist wire. The API exists; no scenario drives it through the evaluator.                                                |
| AC-6 | `cross-surface`| `integration` | `recover_incompatible` is invoked nowhere in production; the test uses a no-op closure that doesn't actually recompile or rebind.       |

FIX-B's job is to pick **where to relocate the evidence** before the iteration
opens scope. The decision matrix is path A (wire to the real seam) vs path B
(amend the AC to a smaller seam).

## Decision matrix

### AC-2

| Path | What it means                                                                                                          | Cost                                                                 |
|------|------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------|
| 2A   | Wire `eval()` → `save_to_store` directly. New scenario drives `eval("1 + 2")` → CompiledCode stamped with `source_hash`. | Touches the evaluator's main path. Cross-cutting; affects all evaluator callers. |
| 2B   | Amend AC-2 to "explicit `save_to_store` API contract". Downgrade impact `journey` → `local`. Evidence stays at the API. | Cheap to ship. Loses the journey contract; if the journey matters later, we'll re-open. |

### AC-6

| Path | What it means                                                                                                                                                | Cost                                                                                       |
|------|--------------------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------|
| 6A   | Loader auto-chains: `loader::decode` on incompatible payload internally consults `source_hash`, recompiles via real `eval()`, binds under the original key. | Significant: introduces a loader-eval coupling; loader gains a dependency on evaluator.    |
| 6B   | Keep `recover_incompatible` standalone. Amend AC-6 wording to match: "a standalone pass is available; caller-driven invocation." Downgrade impact.          | Cheap to ship. Loses auto-chain; caller must remember to invoke recovery. Loses cross-surface contract. |

## Sibling-project findings

### moor-echo (advocates Path B for both)

**Setting:** moor-echo is an evaluator with persistence. Its evaluator and
storage couple at the **object level**, not the per-compilation level.

**Key file evidence (`~/development/moor-echo/`):**

1. **No source_hash, no envelope, no recovery layer.** `crates/echo-core/src/storage/object_store.rs:184-206` shows `get()` / `store()` as direct bincode pass-through. Deserialization errors propagate without fallback.
2. **Source + compiled-form co-located in one artifact.** `VerbDefinition` (object_store.rs:69-78) stores both `code: String` and `ast: Vec<EchoAst>` together. The struct itself is the artifact; there's no cross-reference between source and compiled form via hash.
3. **TransformationRule exists but is passive.** `crates/echo-core/src/tracer/rules.rs:14-47` — it's a debugger tool, not a loader-recovery mechanism. The loader doesn't invoke it.
4. **`execute_verb` trusts the loaded AST without re-validation.** `crates/echo-core/src/evaluator/mod.rs:3024` runs `for stmt in &verb_def.ast` directly. No "AST stale, recompile from source" fallback.

**Recommendation:** Path 2B + Path 6B. The artifact-co-location pattern makes
AC-2's evidence the artifact itself (no separate source_hash bookkeeping
needed), and the lack of loader-recovery infrastructure shows that recovery
should be an opt-in tool, not loader machinery.

**Limitation:** moor-echo doesn't have STORY-0099's schema-versioning problem
at scale. It treats incompatible payloads as fatal errors and migrates by
manual data work. fmpl has explicitly committed to schema-evolution-tolerant
persistence, so the analogy is incomplete.

### cairn (advocates Path A for both)

**Setting:** cairn is a compiler pipeline with a strict linear stage discipline.

**Key file evidence (`~/development/cairn/`):**

1. **Pipeline driver is the integration seam.** `crates/interloko_compiler/src/session.rs:2419-2461` — `compile_file_with_host_registry()` chains `syntax → ast → sem → lower → tape → runtime` in one driver. Each stage is internal; there's exactly one public entry per artifact type. There's no equivalent of "use save_to_store directly."
2. **Recovery is a semantic construct lowered inline.** `crates/interloko_compiler/src/sem.rs:1326` (`Restart`) + `crates/interloko_compiler/src/lower.rs:1444-1567` (`lower_signal()`) — `signal`/`restart` is encoded in user source and lowered into conditional branch IR. There is no separate "recovery pass" the caller invokes; the recovery is _in the program text_.
3. **Recovery evidence is E2E from corpus.** `tools/interloko_corpus/src/main.rs:280-349` — recovery tests live in `corpus/pass/restarts/*.loko`, are compiled and run as full programs (`RuntimeMode::RunMain`), and assert behavior via `// expect:` directives. No isolated recovery-API test exists.

**Recommendation:** Path 2A + Path 6A. The cairn discipline is "no separate
test paths." Every contract is proven through the real entry point, and
recovery is structurally part of the main pipeline (not a side branch the
caller chooses).

**Limitation:** cairn's recovery mechanism is user-level `signal`/`restart`
(an exception system), not loader-level schema-mismatch recovery. The pattern
is "evidence at the journey seam"; the analogy to schema-recovery is by
**discipline**, not by direct implementation match.

## Synthesis — they disagree productively

The two siblings advocate opposite paths because they answer different
questions:

- **moor-echo:** "What does the simplest correct persistence look like?" → no
  source_hash, no recovery, artifacts co-locate everything. Path B.
- **cairn:** "What does evidence-at-the-real-seam look like for a compiler?" →
  one pipeline driver, recovery wired in, E2E corpus. Path A.

fmpl is closer to **cairn structurally** (we have a compiler pipeline, an
envelope-format persistence layer, a versioned schema with declared
recovery semantics) and closer to **moor-echo culturally** (we want simple,
low-coupling code).

The honest reading: **fmpl committed to cairn-shaped semantics when it wrote
STORY-0100's ACs** (impact: `journey` for AC-2, `cross-surface` for AC-6;
seam: `integration` for both). Path B would not satisfy those commitments;
it would amend them.

## Recommendation to pre-iter PAR (pre-PAR position — superseded by §"PAR aggregation" below)

Going into the FIX-B pre-iter PAR, the main thread's recommendation was:

- **AC-2: Path 2A** (wire `eval()` → persist). Evidence: a new scenario that
  drives `eval(source)` and asserts `CompiledCode.source_hash` resolves in the
  source store under the original source bytes. Seam stays `integration`.
- **AC-6: Path 6A** (loader auto-chain). Evidence: SCENARIO-0102 rebuilt to
  drive `loader::decode` on a real schema-incompatible record and assert the
  recompiled value is bound under the original key. Seam stays `integration`.

Two reasons to take cairn's side over moor-echo's:

1. **The ACs are already written cairn-shaped.** Amending wording to fit
   moor-echo's pattern is silent scope reduction; if we don't want
   journey/cross-surface ACs, we should change the spec deliberately, not
   change the ACs to match a smaller implementation.
2. **The mechanical defense gate from FIX-A** (sentinel-sweep with verbatim
   output) is itself a Path-A-discipline artifact — it proves the contract at
   the real seam. Aligning FIX-B with the same discipline is coherent; mixing
   disciplines is not.

PAR refined Path 2A from "modify `eval()` in place" to "ship a sibling entry
`eval_persistent(...)`." Seam stays `integration`; impact stays `journey`. The
12 call sites of `eval()` across cli/tui/web stay binary-compatible.

## Open questions for the pre-iter PAR (resolved — see §"PAR aggregation")

The PAR reviewers were asked to interrogate four open questions:

1. **Is the eval→persist wire optional or always-on?** If always-on, `eval()`
   gains a Store-dependency in its signature, which couples evaluator and
   persistence. If optional (e.g., behind a builder flag), the AC needs to
   say so.
2. **What's the eval-stub-for-recovery shape?** AC-6 path 6A requires the
   loader to invoke `eval()`. This is structurally a circular dependency at
   the module level (persistence depends on evaluator, but evaluator currently
   has no such dependency on persistence). The likely answer is a trait
   (`SourceCompiler { fn compile(&self, src: &[u8]) -> Result<Value> }`)
   injected at construction time. PAR should bless or reject the trait shape.
3. **How is the recovered value bound?** AC-6 says "binds the resulting value
   under the original key." Where does "the original key" come from? The
   envelope holds source_hash; does it also hold the binding key? If not, we
   need a (key → source_hash) lookup that survives the incompatible-payload
   case.
4. **Are 2A and 6A separable iterations?** AC-2 closure alone (without AC-6)
   delivers stamping. AC-6 closure alone (without AC-2) implies the source
   store was populated by some non-eval path. Splitting may be cleaner — or
   it may be entangled. PAR should decide.

## PAR aggregation (2026-05-14, paired reviewers A + B)

Two independent reviewers evaluated all four questions in parallel. PAR rules
applied: same-finding-both = high confidence; finding-from-one = still
actionable; severity disagreement = take worst.

### Q1 — eval→persist wire shape

**Both reviewers agree: opt-in, NOT always-on.** High confidence.

Evidence both cite:
- `fmpl-core/src/lib.rs:65` — `pub fn eval(vm: &mut Vm, source: &str) -> Result<Value>`.
- `fmpl-core/src/vm.rs:35-49` — `Vm` has no Store field; adding one breaks every test that constructs a Vm without a tempdir.
- 5–12 production call sites of `eval` across cli/tui/web (count differs by
  scope; Reviewer B's 12 is the more conservative total: `fmpl-cli/src/main.rs:129`,
  `fmpl-web/src/{main.rs:97,storylet.rs:32+390+397,image_store.rs:27}`,
  `fmpl-tui/src/main.rs:386+1142+2338+2563+2902+2949`).
- The internal recursive call at `fmpl-core/src/vm.rs:3468` (`crate::eval(vm, code)` inside the `io::load` builtin) makes always-on plumbing especially invasive.
- Dependency direction stays clean: `fmpl-core/Cargo.toml:26` declares
  `fmpl-persistence` as a regular dep; `fmpl-persistence/Cargo.toml:30-37`
  carries `fmpl-core` only as `[dev-dependencies]` with an explicit
  no-cycle comment. Adding a Store parameter does not invert this.

**Productive disagreement:** Reviewer A proposed a `Vm::with_persistence(...)`
builder with internal `if vm.persistence.is_some()` stamping; Reviewer B
proposed a sibling entry function `eval_persistent(...)`. The sibling-entry
shape wins because (a) it keeps `Vm`'s shape stable, (b) the persistence wire
is explicit at the call site rather than conditional on hidden internal
state, and (c) AC-2 wording becomes "when `eval_persistent()` compiles user
source" — naming a function the journey scenario can target unambiguously.

**Resolved shape:**

```rust
#[cfg(feature = "persistence")]
pub fn eval_persistent(
    vm: &mut Vm,
    source: &str,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
    key: &str,
) -> Result<Value>;
```

`eval()` stays unchanged at all 12 production call sites.

### Q2 — eval-stub-for-recovery shape

**Both reviewers reach opposite calls; Reviewer B's wins on PAR severity
escalation.** High confidence.

- **Reviewer A:** lift the closure to a `SourceCompiler` trait in
  fmpl-persistence with a blanket impl over `FnMut`, then ship a concrete
  `VmRecompiler` in fmpl-core.
- **Reviewer B:** **don't introduce the trait.** The closure
  `F: FnMut(&[u8], &[u8]) -> Result<(), RecoveryError>` at
  `fmpl-persistence/src/recovery.rs:155` *already* is the inversion-of-control
  seam. The orchestrator function belongs in fmpl-core, closing over
  `&mut Vm`.

Reviewer B flagged this as Serious: "no existing inverted-dep trait
precedent in fmpl-persistence; the project's pattern is closure parameters."
That's a real finding — fmpl already chose closures over traits at this
exact seam. Inventing a trait wraps existing infrastructure in ceremony.

**Resolved shape:** orchestrator in fmpl-core, no new trait:

```rust
#[cfg(feature = "persistence")]
pub fn recover_and_rebind(
    vm: &mut Vm,
    bytecode_store: &dyn fmpl_persistence::Store,
    source_store: &fmpl_persistence::SourceStore,
) -> Result<fmpl_persistence::RecoveryStats>;
```

Internally calls `recover_incompatible(...)` with a closure that converts
`&[u8]` key → `&str`, calls `eval_persistent`, and maps errors through
`RecoveryError::recompile(...)`.

### Q3 — "original key" provenance

**Both reviewers agree: it's the fjall iteration key, already plumbed.** High
confidence.

Evidence both cite:
- `fmpl-persistence/src/envelope.rs:84-129` — `EnvelopeHeader` carries
  magic, format/payload/flags/vm-version/schema-version/payload-len/
  source-hash/crc32. No binding key. The envelope is value-only.
- `fmpl-persistence/src/store.rs:50` — `StoreIterItem = Result<(Vec<u8>, Vec<u8>), StoreError>`.
  Keys are first-class in iteration.
- `fmpl-persistence/src/recovery.rs:155,183` — closure receives `(key, src)`
  already.
- `fmpl-persistence/tests/scenario_0102_recover_incompatible.rs:91` — the
  existing test asserts on `received_keys`, proving the key plumbing.

**Resolved shape:** no new data structure. `recover_and_rebind`'s closure
calls `std::str::from_utf8(key)` and passes it to `eval_persistent`'s `key`
parameter, which writes via `Store::insert` and overwrites the stale envelope
at the same fjall key.

**Subtlety both noted:** the `&[u8]`→`&str` conversion is infallible for keys
that came in via the existing `save_to_store(..., key: &str, ...)` API, but
should still surface `RecoveryError::recompile(...)` on UTF-8 failure for
defense against fjall keys written by future non-string-keyed callers.

### Q4 — separability of 2A and 6A

**Both reviewers agree on ordering (2A before 6A); slight phrasing
difference on bundling.** High confidence.

Both reviewers find:
- No production writer to source store today (grep over `source_store.put`
  lands only in tests, the closest production-path consumer is
  `fmpl-core/src/compiler.rs:734-735` inside `CompiledCode::save_to_store`).
- Without 2A, the production journey produces records with
  `source_hash == Hash::NONE`, and `recover_incompatible` (at
  `fmpl-persistence/src/recovery.rs:177-179`) classifies them as
  `unrecoverable_no_source` before any source-store query fires.
- 2A alone delivers AC-2 evidence in one new scenario; 6A's cross-surface
  evidence requires 2A's writer to be live.

Reviewer A recommended splitting into FIX-B.1 + FIX-B.2; Reviewer B
recommended one iteration with two ordered ACs.

**Resolved shape:** **one iteration, two ordered ACs.** Bundling is
appropriate because (a) 6A's first real consumer is exactly 2A's writer
output — that's the project's "ship infrastructure with first consumer"
discipline, not a violation of it, and (b) splitting would create an
artificial seam between two halves of one logical contract. The
reader/writer asymmetry lesson applies when each half has independent
downstream consumers; here, 6A has no downstream consumer other than 2A's
output.

### Additional findings flagged by reviewers

**Reviewer A — Serious findings:**

1. **`io::load` recursive eval at `fmpl-core/src/vm.rs:3468` would double-write
   under always-on persistence.** Under the resolved sibling-entry shape
   (Q1), this concern dissolves — `io::load` calls `crate::eval`, not
   `eval_persistent`, so the loaded file's compile is unpersisted by default.
   **Disposition: non-issue under sibling-entry path.**
2. **AC-6's "logs the recovery attempt" requirement is not satisfied by
   today's `recover_incompatible`** — the function returns stats but emits no
   logs. SCENARIO-0102 currently asserts stats only. **Disposition: real
   finding. FIX-B must either add a tracing/log hook OR amend the AC wording
   to "RecoveryStats reflect the recovery attempt." Decision deferred to
   iteration owner at T3.**

**Reviewer B — Minor finding:**

1. **AC-2 text reads "when `eval()` compiles user source"** — under
   sibling-entry shape, the AC should name `eval_persistent`. **This is a
   wording-fix, not a Path 2B downgrade.** Seam stays `integration`; impact
   stays `journey`. **Disposition: documented in FIX-B scope card as T5.**

## Resolved build sequence (input to FIX-B scope card)

One iteration with two ordered ACs:

1. **T0** — Add `eval_persistent(vm, source, store, source_store, key)` to
   fmpl-core under `#[cfg(feature = "persistence")]`. Sibling entry point;
   `eval()` unchanged. Open decision: does `eval_persistent` wrap `eval()`
   internally, or duplicate the dispatch through `eval_via_native` /
   `eval_via_legacy_parser`? PAR didn't have enough to call this — iteration
   owner decides.
2. **T1 (AC-2)** — Wire `eval_persistent` to call
   `code.save_to_store(store, source_store, key, Some(source.as_bytes()))`
   after compile. New SCENARIO-0101-eval-persist drives
   `eval_persistent("1 + 2")` end-to-end and asserts
   `source_store.get(envelope.source_hash) == Some(b"1 + 2".to_vec())`.
3. **T2** — Add `recover_and_rebind(vm, store, source_store)` in fmpl-core.
   Internally calls `recover_incompatible` with a closure that converts
   `&[u8]` key → `&str`, calls `eval_persistent`, returns
   `RecoveryError::recompile(...)` on UTF-8 or compile failure.
4. **T3 (AC-6, plus Reviewer A finding #2)** — Either (a) add a tracing hook
   in `recover_incompatible` per record so log-emission is observable, or
   (b) amend AC-6 wording to read "RecoveryStats reflect the recovery
   attempt." Iteration owner picks; both are defensible.
5. **T4 (AC-6 scenario rebuild)** — Rebuild SCENARIO-0102:
   - Open Vm with persistence
   - Call `eval_persistent("1 + 2", ..., key="answer")`
   - Tear down + reopen with a bumped VM major
   - Call `recover_and_rebind(...)`
   - Assert: `recovered_from_source == 1`, the rebound value at key
     `"answer"` is `Value::Int(3)` per AC-6 text.
6. **T5** — Update AC-2 + AC-6 wording in
   `docs/superpowers/iterations/requirements/EPIC-003.md` to name
   `eval_persistent` / `recover_and_rebind` as the contract entry points.
   Seam stays `integration`; impacts stay `journey` / `cross-surface`.
7. **T6** — Wrap: `roadmap.md`, `iteration-log.md`, `progress.md`. Sentinel
   sweep now includes SCENARIO-0101 + rebuilt SCENARIO-0102 at cadence
   `sentinel`.

## Open decisions remaining for the iteration owner

PAR didn't have enough to resolve these; pick at iteration time:

1. **T0 dispatch internals** — does `eval_persistent` wrap `eval()` (cleaner
   composition; potential perf overhead from double-dispatch on the
   `eval_via_*` family), or duplicate the dispatch (cleaner code path;
   maintenance burden when `eval_via_*` evolves)?
2. **T3 logging vs amend** — does FIX-B add tracing to `recover_incompatible`
   (preserves AC text but adds tracing dep), or amend AC-6 to drop the "logs"
   requirement (cheaper to ship but reduces AC text)? Both reviewers agree
   either is defensible.

## Files this spec touches (none yet)

This is a planning document. Iteration scope card lives in `roadmap.md` after
this spec is consumed.

## Cross-references

- `docs/superpowers/iterations/requirements/EPIC-003.md` — STORY-0100 ACs
  (lines 258, 262 for AC-2 / AC-6 text; lines 273, 277 for the re-open notes).
- `docs/superpowers/iterations/roadmap.md` — ITER-0005b-FIX-B scope card
  (to be authored by PAR).
- `docs/superpowers/iterations/iteration-log.md` — ITER-0005b-FIX-A closing
  entry (precedent for FIX-MECH discipline, which informs the Path-A bias).
- `~/development/moor-echo/` — Path B sibling.
- `~/development/cairn/` — Path A sibling.
