# EPIC-003 — Image Persistence

**Summary:** Image Persistence
**Stories:** STORY-0013, STORY-0014, STORY-0015, STORY-0016, STORY-0017, STORY-0018, STORY-0019, STORY-0020, STORY-0021, STORY-0099, STORY-0100
**Primary sources:** `docs/plans/2026-03-03-self-hosting-bootstrap-design.md`, `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md`
**Status:** 0/11 done

## STORY-0013

**Epic:** EPIC-003 — Image Persistence
**Title:** Persist ObjectDb to Fjall

**As a** FMPL developer
**I want** ObjectDb objects to be serialized and persisted in Fjall
**So that** object state survives process restarts

**Acceptance criteria:**
- AC-1: Objects created in the ObjectDb are persisted to Fjall storage and survive process restart · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0004`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:221-236`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:55-56`
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:214-257`

**Status:** pending

## STORY-0014

**Epic:** EPIC-003 — Image Persistence
**Title:** Persist compiled bytecode to Fjall

**As a** FMPL developer
**I want** compiled bytecode (CompiledCode) to be cached in Fjall
**So that** previously compiled code does not need recompilation after restart

**Acceptance criteria:**
- AC-1: Compiled bytecode is stored in Fjall and can be loaded on process restart without recompilation · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0004`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:227-228`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:56-57`
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:259-297`

**Status:** pending

## STORY-0015

**Epic:** EPIC-003 — Image Persistence
**Title:** Persist grammar definitions to Fjall

**As a** FMPL developer
**I want** grammar definitions (GrammarRegistry) to be persisted in Fjall
**So that** grammar state survives process restarts

**Acceptance criteria:**
- AC-1: Grammar definitions stored in the GrammarRegistry are persisted to Fjall and restored on process restart · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0004`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:228-229`
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:57`

**Status:** pending

## STORY-0016

**Epic:** EPIC-003 — Image Persistence
**Title:** Implement VM snapshot and restore

**As a** FMPL developer
**I want** Vm::snapshot() and Vm::restore() to capture and restore full VM state
**So that** the full compiler state including VM can survive process restarts

**Acceptance criteria:**
- AC-1: Vm::snapshot() serializes full VM state to Fjall storage · impact:`local` · seam:`integration` · scenario:`SCENARIO-0004`
- AC-2: Vm::restore() loads VM state from Fjall storage and resumes execution · impact:`local` · seam:`integration` · scenario:`SCENARIO-0004`
- AC-3: After snapshot/restore cycle, compiling and running code produces identical results to pre-snapshot state · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0004`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:229-230`

**Status:** pending

## STORY-0017

**Epic:** EPIC-003 — Image Persistence
**Title:** Full image persistence across process restarts

**As a** FMPL developer
**I want** the complete compiler state (objects, bytecode, grammars) to persist across process restarts
**So that** I can develop the compiler interactively in the REPL without losing work on restart

**Acceptance criteria:**
- AC-1: Objects, bytecode, and grammar definitions survive process restart and are available immediately · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0004`
- AC-2: REPL session state persists across restarts - previously defined values and functions are available · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0004`
- AC-3: Web server recovers full image on restart without manual intervention · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0004`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:232-236`

**Status:** pending

## STORY-0018

**Epic:** EPIC-003 — Image Persistence
**Title:** Image-based normal operation startup

**As a** FMPL developer
**I want** the VM to load the compiler from the Fjall image on normal startup
**So that** the compiler is immediately ready without recompilation

**Acceptance criteria:**
- AC-1: On VM start with existing Fjall image, the compiler is loaded from image and ready for use without recompilation · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0008`
- AC-2: On VM start with no Fjall image, the system falls back to seed bytecode loading · impact:`journey` · seam:`app-level` · scenario:`SCENARIO-0008`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:176-186`

**Status:** pending

## STORY-0019

**Epic:** EPIC-003 — Image Persistence
**Title:** Persist GrammarRegistry to Fjall

**As a** FMPL runtime developer
**I want** GrammarRegistry to be serializable to and deserializable from Fjall storage
**So that** grammars defined in one session can be loaded and used in another

**Acceptance criteria:**
- AC-1: GrammarRegistry save/load methods exist and round-trip grammar definitions through Fjall · impact:`local` · seam:`integration`
- AC-2: Grammar semantic actions containing AST expressions are correctly serialized and deserialized · impact:`local` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:299-319`

**Status:** pending

## STORY-0020

**Epic:** EPIC-003 — Image Persistence
**Title:** Implement Vm::snapshot() and Vm::restore()

**As a** FMPL runtime developer
**I want** the VM to support snapshotting its entire state to disk and restoring from it
**So that** a complete VM state including variables, objects, grammars, and compiled code can be persisted and resumed

**Acceptance criteria:**
- AC-1: Vm::snapshot(dir) saves scope (variable bindings), ObjectDb, GrammarRegistry, and compiled code cache to the given directory · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0019`
- AC-2: Vm::restore(dir) loads all state from a previous snapshot into a fresh VM · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0019`
- AC-3: A variable defined with 'let x = 42' in one VM, snapshotted, then restored into a new VM, is accessible and evaluates to 42 · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0019`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-implementation.md:321-358`

**Status:** pending

## STORY-0021

**Epic:** EPIC-003 — Image Persistence
**Title:** Complete grammar memo persistence in Fjall

**As a** FMPL developer
**I want** grammar memoization caches to be fully persisted in Fjall
**So that** grammar parsing performance is preserved across process restarts

**Acceptance criteria:**
- AC-1: Grammar memo caches (currently partially integrated with Fjall) are fully serialized and deserialized on process restart · impact:`journey` · seam:`integration`

**Sources:**
- `docs/plans/2026-03-03-self-hosting-bootstrap-design.md:53`

**Status:** pending

## STORY-0068

**Epic:** EPIC-021 — Persistence
**Title:** Fjall overflow for streaming positions and memoization

**As a** streaming grammar runtime
**I want** positions and memo tables to spill to Fjall when memory is limited
**So that** long-running streams do not exhaust memory

**Acceptance criteria:**
- AC-1: When memory_limit is set and exceeded, StreamSource::Async spills positions to Fjall overflow · impact:`local` · seam:`integration`
- AC-2: ParseState can be saved to and loaded from Fjall for durable parse suspension · impact:`local` · seam:`integration`
- AC-3: Memo tables can optionally use Fjall backing for persistent memoization across restarts · impact:`local` · seam:`integration`

**Sources:**
- `specs/grammar-system.md:143-164`
- `specs/grammar-system.md:70-72`

**Status:** pending


## STORY-0069

**Epic:** EPIC-021 — Persistence
**Title:** Enable Fjall-backed persistence via feature flag

**As a** FMPL deployment
**I want** to enable fjall-persistence feature for durable storage
**So that** stream positions, memo tables, and parse state survive process restarts

**Acceptance criteria:**
- AC-1: When fjall-persistence is enabled, large stream buffers spill to disk via Fjall · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0069`
- AC-2: When fjall-persistence is enabled, memoization tables persist across suspension/resume · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0069`
- AC-3: When fjall-persistence is enabled, ParseState can be serialized for durable parse suspension · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0069`
- AC-4: By default (no feature flags), no optional features are enabled · impact:`none` · seam:`unit`

**Sources:**
- `specs/fmpl-core.md:122-140`

**Status:** pending

## STORY-0099

**Epic:** EPIC-003 — Image Persistence
**Title:** Versioned persistence envelope with skip-on-incompatible

**As a** FMPL VM operator upgrading across versions
**I want** every persisted artifact (object, bytecode, grammar, parse state, lambda) wrapped in a self-describing envelope
**So that** the loader can identify, version-check, and skip incompatible records without crashing or silently corrupting state

**Acceptance criteria:**
- AC-1: Every persisted record begins with a fixed-layout envelope header containing magic bytes, envelope-format version, payload kind discriminator, VM version triple (major, minor, patch), schema version per payload kind, payload byte length, source byte length, flags, and a CRC32 over (header + payload) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0099`
- AC-2: A loader encountering a record with a known magic but a `vm_version` major mismatch logs the record key + reason, skips the entire record using `payload_len + source_len + envelope_size`, and continues iteration without raising an error · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0099`
- AC-3: A loader encountering a record with a known magic but an unknown `payload_kind` or unknown `schema_version` for a known kind also skips the record without aborting · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0099`
- AC-4: A loader encountering a record whose CRC32 fails skips the record, logs the corruption, and continues · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0099`
- AC-5: The envelope wraps `Object`, `CompiledCode`, `Grammar`, `ParseState`, `Lambda`, and full-VM-snapshot payloads — all current `save_to_fjall` callers route through the envelope helper, no caller writes raw `serde_json` bytes to a Fjall keyspace · impact:`cross-surface` · seam:`integration`
- AC-6: VM version is derived from `env!("CARGO_PKG_VERSION")` at build time and embedded automatically; payload-kind-specific schema versions live in a single `persistence::schema` module so additions are tracked centrally · impact:`local` · seam:`unit`
- AC-7: Loader exposes per-keyspace statistics (`loaded`, `skipped_incompatible`, `skipped_corrupt`, `skipped_unknown_kind`) so operators can detect silent data loss after an upgrade · impact:`local` · seam:`integration`

**Sources:**
- This conversation: design discussion 2026-05-08
- `docs/codebase/fjall-persistence-patterns.md` (current raw-serde pattern being replaced)

**Status:** pending

## STORY-0100

**Epic:** EPIC-003 — Image Persistence
**Title:** Content-addressed source store with constructor synthesis fallback

**As a** FMPL VM operator recovering from a schema-incompatible snapshot
**I want** every persisted artifact carry a recoverable source — original FMPL text where available, synthesized constructor expressions otherwise
**So that** an artifact whose payload is unloadable can still be rebuilt by recompiling its source

**Acceptance criteria:**
- AC-1: A `sources` Fjall partition stores source bytes keyed by their blake3 hash; envelope records reference source via the hash, not inline bytes, so duplicate sources (e.g., shared prelude code) are stored once · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0100`
- AC-2: When `eval()` compiles user source, the resulting `CompiledCode` is persisted with a `source_hash` pointing to the original source bytes in the source store · impact:`journey` · seam:`integration` · scenario:`SCENARIO-0100`
- AC-3: When a `Grammar` is registered from a source string, its persisted record carries the `source_hash` for the defining source · impact:`local` · seam:`integration` · scenario:`SCENARIO-0100`
- AC-4: For artifacts created at runtime without an originating source — spawned objects, anonymous lambdas without a syntactic anchor, grammars built programmatically — the persistence layer synthesizes a constructor expression (e.g., `spawn(facets: [...], properties: %{...})` for an object, `λ(x) { ... }` text for an anonymous lambda) and stores that synthesized text in the source store · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0101`
- AC-5: The synthesized constructor expression is round-trippable: evaluating it via `eval()` produces a value structurally equivalent to the original artifact (same facets, same properties, same lambda body up to alpha-equivalence) · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0101`
- AC-6: When the loader encounters a payload it cannot decode (incompatible schema per STORY-0099) but the envelope's `source_hash` resolves in the source store, it logs the recovery attempt, recompiles the source via the current `eval()` path, and binds the resulting value under the original key · impact:`cross-surface` · seam:`integration` · scenario:`SCENARIO-0102`
- AC-7: The source store is garbage-collected on demand (e.g., via a `compact()` API) — sources unreferenced by any envelope are removed; reference counting is via a full scan of envelope records, not a separate refcount table · impact:`local` · seam:`integration`

**Sources:**
- This conversation: design discussion 2026-05-08
- `Cargo.toml:42` (blake3 already a workspace dependency)

**Status:** pending
