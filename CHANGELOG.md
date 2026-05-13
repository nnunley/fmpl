# Changelog

Notable changes to FMPL. Format loosely based on [Keep a Changelog](https://keepachangelog.com/);
versioning anchored to the FMPL bootstrap-stabilization iteration sequence
(`docs/superpowers/iterations/`) rather than semver until self-hosting lands.

## Unreleased — Persistence envelope rollout (ITER-0005 family in flight)

### On-disk wire-format breaks

**ITER-0005a.2 (2026-05-13) — fmpl-core persistence writes now use a 56-byte envelope header.**

Every persisted record written via `fmpl-core/src/`'s `save_to_fjall` methods
(or via `grammar/stream_input.rs`'s `spill_to_fjall` / `set_memo` paths) now
prefixes the serialized payload with a 56-byte `EnvelopeHeader` containing
magic bytes, format version, payload-kind discriminator, VM version triple,
schema version, payload length, source hash, and a blake3-truncated-to-32
checksum.

**Impact:** any Fjall keyspace data written before ITER-0005a.2 will fail
to load through fmpl-core's swept `load_from_fjall` paths. The loader's
transitional manual prefix-strip (to be replaced by `loader::decode` in
ITER-0005a.3) assumes every value starts with the 56-byte header.

**Acceptable because:** the `persistence` feature has no production
consumers at the time of this break. Pre-existing fjall databases are not
durable user data and are expected to be discarded across the upgrade.

**Affected payload classes** (all under `fmpl-core/src/`):

- `CompiledCode` (bytecode) — `PayloadKind::CompiledCode (0x03)`
- `ObjectDb` index + records — `PayloadKind::ObjectIndex (0x02)` + `ObjectRecord (0x01)`
- `ParseState` — `PayloadKind::ParseState (0x06)`
- Grammar memo entries — `PayloadKind::MemoTable (0x07)`
- Stream position spills — `PayloadKind::StreamPosition (0x09)`
  (added 2026-05-13 audit fix-up G2 to resolve a wire-tag collision
  with `ParseState`)

**Not affected (yet):** `fmpl-web` persistence (`continuations.rs`,
`image_store.rs`) — those use a parallel `SnapshotEnvelope` abstraction
and have NOT been swept through the fmpl-core envelope helper. See
deferred iteration `ITER-0005-WEB-PERSISTENCE` in
`docs/superpowers/iterations/roadmap.md` for the planned migration.

### References

- Iteration scope card: `docs/superpowers/iterations/roadmap.md` → `ITER-0005a.2`.
- Iteration-log entry: `docs/superpowers/iterations/iteration-log.md` → ITER-0005a.2.
- Story: `docs/superpowers/iterations/requirements/EPIC-003.md` → STORY-0099 AC-5.
- Invariant gate: `fmpl-core/tests/persistence_envelope_invariant.rs`.
- Writer→loader round-trip evidence: `fmpl-core/tests/scenario_0111_envelope_writer_roundtrip.rs`.
