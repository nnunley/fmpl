# FMPL demo artifacts

Self-running demos of the FMPL coordination + persistence substrate.
Two complementary runners exist; both leave reproducible plain-text
transcripts and `script(1)` recordings.

## Demos

### 1. Three-hash audit (YAML-driven, multi-REPL)

The newer, more ambitious demo. Driven by `harness.py` from a YAML
scenario. Spawns named REPL processes, sequences them through a shared
on-disk SourceStore, captures hashes printed by one REPL and passes
them to another as `.fetch` arguments via `{{ var }}` substitution.

The canonical scenario at `scenarios/three_hashes.yaml`:

1. **Alice** spawns, opens an on-disk SourceStore, defines
   `square = \x x * x`, stores it under three content hashes:
   - source text (`.store-source`)
   - runtime Value (`.store-value`, serialized via serde_json)
   - compiled bytecode (`.store-bytecode`, the Lambda's `CompiledCode`)
2. **Alice exits** — releases fjall's single-writer lock.
3. **Bob** spawns as a fresh process, opens the same directory,
   fetches each hash, recovers the bytes.
4. The harness asserts Bob's recovered source equals Alice's typed
   source — byte-identical recovery across the process boundary.

Sequencing matters: fjall v3 takes a file lock on the keyspace, so two
processes cannot hold the same store open simultaneously. The demo
sequences them honestly through one shared on-disk store rather than
faking concurrent access.

### 2. Rusty Flagon walkthrough (Python-driven, single REPL)

The earlier demo. `run_full_demo.py` drives a single REPL through
`demo/tavern.fmpl` (tuplespace + faceted objects + grammars + pattern
dispatch) and then runs the source-handoff Rust example. Lower-risk,
no harness machinery.

## What's here

```
scenarios/
  three_hashes.yaml             — canonical YAML scenario
harness.py                      — YAML-driven multi-REPL harness
run_full_demo.py                — older Python-driven walkthrough
drive_tavern.py                 — REPL-only driver (used by run_full_demo)
tavern.fmpl                     — the Rusty Flagon end-to-end FMPL program
transcript-three_hashes.txt     — canonical three-hash demo transcript
transcript-three_hashes-recorded.txt
                                — paired transcript from a recorded run
typescript-three-hashes.bsd     — BSD script(1) recording of the three-hash demo
transcript-canonical.txt        — Rusty Flagon walkthrough transcript
transcript-recorded-fast.txt    — Rusty Flagon recorded run
```

## Reproduce

From the repo root:

```sh
# one-time: set up the venv
python3 -m venv .demo-venv
.demo-venv/bin/pip install pexpect pyyaml

# run the three-hash demo (writes a fresh timestamped transcript)
.demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml

# fast mode (no inter-action pauses)
.demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml --fast

# headless: only write the transcript, no stdout echo
.demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml --no-echo

# custom pacing (in seconds between actions)
.demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml --sleep 2.5

# no ANSI on stdout (transcript file is always plain regardless)
.demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml --no-color

# record a tty-fidelity session
/usr/bin/script -q demo/typescript-three-hashes.bsd \
    .demo-venv/bin/python demo/harness.py demo/scenarios/three_hashes.yaml --fast

# run the older Rusty Flagon walkthrough
.demo-venv/bin/python demo/run_full_demo.py
```

Both runners exit nonzero if any step fails — they double as smoke tests.

## YAML scenario format

```yaml
title: "scenario title"
description: |
  multi-line description shown at the top

defaults:
  # Default sleep between actions, in seconds. Per-action `sleep_after`
  # overrides this. CLI --fast overrides to 0.0; --sleep N overrides too.
  sleep_after: 0.6

actions:
  - type: comment
    title: "Reading guide"          # optional — renders as a section header
    body: |
      Multi-line annotation that prints each non-empty line as a
      narrate-style `# ...` comment in the transcript. Different from
      `narrate` (one paragraph) — group related sentences here when
      authoring a paginated walkthrough.

  - type: spawn
    name: alice
    sleep_after: 0                  # per-action override (skip the pause)
  - type: open_store
    in: alice
    path: "{{ tmpdir }}"          # `tmpdir` is pre-seeded by the harness
  - type: repl_eval
    in: alice
    expr: 'let square = \x x * x'
  - type: store_source
    in: alice
    var: square
    capture_hash_as: square_hash  # captured into {{ square_hash }}
  - type: close_session
    name: alice                   # releases fjall lock
  - type: spawn
    name: bob
  - type: open_store
    in: bob
    path: "{{ tmpdir }}"
  - type: fetch
    in: bob
    hash: "{{ square_hash }}"
    capture_source_as: bob_source
  - type: assert_equal
    label: "source round-trip"
    lhs: "{{ bob_source }}"
    rhs: "\\\\x x * x"
```

### Action types

| Type | Purpose |
|------|---------|
| `banner` / `section` / `narrate` | Transcript-only output |
| `comment` | Multi-line annotation block, optional `title:` |
| `shell` | Run a shell command; optional `capture_stdout_as` |
| `spawn` | Start a named REPL (`name:`) |
| `close_session` | `.quit` the REPL, releases fjall lock |
| `reset` | Send `.reset` to a session |
| `open_store` | `.open-store <path>` in a session |
| `repl_eval` | Send an FMPL expression; optional `capture_result_as` |
| `store_source` / `store_value` / `store_bytecode` | Send the corresponding `.store-*` command; `capture_hash_as:` captures the printed `hash: <64hex>` |
| `fetch` | Send `.fetch <hash>`; `capture_source_as:` captures the printed `source:` value |
| `assert_equal` | Compare two `{{ }}`-substituted values |
| `sleep` | Pause N seconds (e.g. for pacing in recorded demos) |

### `{{ var }}` substitution

Any string field gets `{{ name }}` placeholders substituted from the
captured-vars table before the action runs. Pre-seeded: `tmpdir`
(a fresh `mktemp -d` cleaned up at end of run).

## Per-action `sleep_after`

Every action can include `sleep_after: <seconds>` to override the
default pacing for that step. Useful when you want a long pause after a
key reveal (e.g. after `assert_equal`) but no pause around setup.

## Auto-detected REPL mode (script vs interactive)

`fmpl-cli` auto-detects via `std::io::stdin().is_terminal()` whether
it's running interactively or as a subprocess. Behavior:

| Mode | Stdin source | Prompts | Line editing | History | ANSI/bracketed-paste |
|------|--------------|---------|--------------|---------|----------------------|
| Interactive (TTY) | rustyline | `fmpl> ` + arrow keys | yes | `~/.fmpl_history` | yes (rustyline-emitted) |
| Script (pipe) | plain stdin | plain `fmpl> ` sync marker | no | not loaded | NONE |

The harness uses `pexpect.popen_spawn.PopenSpawn` (pipes, no pty), so
the child REPL automatically picks script mode and emits no terminal
control sequences — transcripts come out byte-clean.

## New REPL commands (fmpl-cli)

The harness depends on these dot-commands added in this session:

| Command | Purpose | Output |
|---------|---------|--------|
| `.open-store <path>` | Open a SourceStore at the path | `store: opened at <path>` |
| `.store-source <var>` | Hash+store source text of `let <var> = …` | `hash: <64hex>` + `kind: source` |
| `.store-value <var>` | Hash+store `serde_json(value)` | `hash: <64hex>` + `kind: value` |
| `.store-bytecode <var>` | Hash+store `serde_json(Lambda.code)` | `hash: <64hex>` + `kind: bytecode` |
| `.fetch <hash-hex>` | Load bytes from the store by hash | `loaded N bytes` + `source: "..."` |

Source tracking: when the user submits a top-level `let NAME = EXPR`,
the REPL captures `EXPR` into a private name→source map. `.store-source`
reads from that map. This is honest about what's stored — the original
typed text, not source recovered from the Lambda value (since
Lambda/Object/Grammar don't carry a source slot yet; that's
ITER-0005b-AST-SLOT).

## Status

- `fmpl-persistence`: 107 passing tests across 17 suites
- `fmpl-cli`: 15 passing tests (6 new for the `let`-parser)
- `cargo build --workspace`: clean
- Three-hash demo: 33 actions, 0 failures
- ITER-0005b: closed 2026-05-14

## Deferred to next session

- **`store::*` as FMPL FFI** — same operations callable from FMPL code
  rather than only as REPL dot-commands. Lets scenarios mix REPL
  commands with FMPL-level composition.
- **Disk-backed TupleSpace** — wire `TupleSpace` through the `Store`
  trait so coordination state survives process restart. The `Value`
  enum already serializes via serde; the remaining work is the tuple
  write path and the load-from-store path on open.
- **Multi-writer story** — fjall's single-writer lock means two REPLs
  must sequence through one store today. Broker process, read-replica,
  or alternative backend are the three honest paths.
