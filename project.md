# FMPL Project

## North Star

FMPL is a prototype-based object-oriented language with lambda-calculus constructs, designed as a **live, image-based multi-user programming environment**. The image is the source of truth: a persistent object graph plus compiled code and source blobs. The system is event-driven, responding primarily to IO streams (local and network), and it supports pretty-printing of internally represented code back into readable FMPL.

The system has a **system root** object that anchors global state and services, alongside **player roots** for users with authoring rights. Other objects may also act as roots (for example, world/region objects), enabling modular ownership and capability boundaries. Reflection is first-class: objects can be inspected, methods and properties enumerated, and source recovered from compiled code when available.

The long-term design borrows from VPRI FoNC's emphasis on live, inspectable systems and small-core semantics. The target is a stable, understandable core that supports interactive editing, persistence, and narrative-centric workflows while remaining general-purpose.

## Lineage

- **c. 1992**: The original FMPL ("of Accardi") at UC Berkeley's Experimental Computing Facility (XCF), interpreter written by Jon Blow. A MUD server language similar to ColdMUD and LambdaMOO.
- **Late 1990s**: Norman Nunley, Jr. extracts an EBNF grammar from the original FMPL sources.
- **2025-present**: Nunley builds this language from that grammar — a Rust implementation whose syntax is only lightly similar to the original, combining the MUD heritage with modern streaming, grammars, and agent capabilities.

## Inspirations

| System | What we take from it |
|--------|---------------------|
| **Self** | Prototype-based objects, image-based live environment, inspector as primary interface |
| **Smalltalk** | Live image, everything is an object, inspector-driven development |
| **Common Lisp** | Image persistence, interactive REPL, runtime reflection |
| **LambdaMOO** | Multi-user image, transparent persistence, in-world programming |
| **ColdMUD** | Parsing separated from dispatch, strict encapsulation, driver minimalism |
| **Spritely Goblins** | spawn/bcom, facets, VATs, capability security, promise pipelining |
| **E Language** | Capability security, VATs, promise pipelining |
| **OMeta** | PEG grammars with inheritance for parsing any stream |
| **VPRI FoNC** | Small-core semantics, live inspectable systems |

## Core Principles

1. **Image is truth** — Objects live in a persistent image. Source files are bootstrapping convenience; the canonical state is the image.
2. **Default private** — All slots private. External access only through facets (sealed views).
3. **Facets are capabilities** — Lightweight, sealed, non-extractable views on objects. The only way to interact with an object you don't own.
4. **Multi-principal** — Humans and LLM agents are both principals. Same capability model, same faceted access, same resource limits.
5. **Grammars are first-class** — PEG grammars with inheritance parse any stream. Command dispatch, protocol parsing, and data transformation all use grammars.
6. **`@` unifies** — Parsing, pattern matching, and stream processing through one operator.
7. **Success typing** — No explicit type annotations. Infer from usage, error only on guaranteed contradictions.
8. **Inspector over annotations** — Algebraic properties discovered by runtime, reported in inspector, not declared in syntax.
9. **Driver minimalism** — The Rust runtime provides: bytecode execution, grammar evaluation, tuple space, I/O, yield injection. Everything else is FMPL code.

## Architecture

```
Source → Lexer (logos) → Parser (recursive descent) → AST → Compiler → Indexed RPN bytecode → VM
                                                                                    |
                                                                              ObjectDb (image)
                                                                              TupleSpace
                                                                              Fjall (persistence)
```

**Rust workspace** with 4 crates:

| Crate | Purpose |
|-------|---------|
| `fmpl-core` | Lexer, parser, compiler, VM, object system, grammar engine |
| `fmpl-cli` | REPL with rustyline |
| `fmpl-web` | Axum server with HTMX frontend |
| `fmpl-tui` | Ratatui TUI for agentic LLM interaction |

## Type System Philosophy

No explicit typing. Types inferred from usage via success typing (Dialyzer-style). Operations are morphisms, not type-specific functions — `+` means "supports combine," not "is a number."

Facets can optionally declare arity and unification variables for cross-slot relationships:

```fmpl
-- Level 1: names only
auditor: [view_balance]

-- Level 2: with arity
container: [put(_), take()]

-- Level 3: with unification
combinable(T): [combine(T) -> T]
```

Laws are discovered by the runtime (QuickSpec-style), reported in the inspector, not declared in syntax. See [category-theoretic type system research](docs/research/2026-02-25-category-theoretic-type-system.md).

## Multi-User / Multi-Agent Model

Multi-user and multi-agent are the same problem: multiple principals with different capabilities on shared mutable state.

- **Identity**: `user` magical variable carries principal through call chain
- **Capability**: Facets (sealed views) — you can only do what your facets allow
- **Isolation**: VATs (per-principal event loops) with atomic turns
- **Resource limits**: Yield injection at loop back-edges
- **Coordination**: Tuple space with faceted access control
- **Command parsing**: Grammar dispatch per connection (ColdMUD pattern)

## Documentation Map

| Path | Contents |
|------|----------|
| `project.md` | This file — north star, principles, meta knowledge |
| `AGENTS.md` | Build/test commands, critical patterns, key files (LLM context) |
| `specs/` | Implementation specs — concise, grounded, with file/line targets |
| `docs/design/` | High-level design documents |
| `docs/plans/` | Implementation plans (dated) |
| `docs/research/` | Research notes with bibliographies |
| `docs/STANDARDS.md` | Documentation formatting standards |

### Key Specs

| Spec | Scope |
|------|-------|
| [object-system](specs/object-system.md) | Prototypes, facets, visibility, multi-principal, spawn/bcom |
| [vm](specs/vm.md) | Indexed RPN execution, instructions, magical variables |
| [grammar-system](specs/grammar-system.md) | PEG grammars, inheritance, streaming, memoization |
| [tuplespace](specs/tuplespace.md) | Linda-style coordination with faceted access |
| [persistence](specs/persistence.md) | Fjall-backed transparent persistence |
| [pattern-matching](specs/pattern-matching.md) | `@` operator for parsing and matching |
| [async-streams](specs/async-streams.md) | First-class async streams |

### Key Research

| Research | Scope |
|----------|-------|
| [type-inference](docs/research/2026-02-25-type-inference-duck-typed-systems.md) | 12 approaches to duck-typed inference |
| [category-theory](docs/research/2026-02-25-category-theoretic-type-system.md) | Facets as named categories, algebraic laws |
| [multi-user-synthesis](docs/research/2026-02-25-multi-user-architecture-synthesis.md) | Cross-project architecture synthesis |
| [coldmud](docs/research/2026-02-25-coldmud-architecture.md) | Parsing/dispatch separation |
| [moor-echo](docs/research/2026-02-25-moor-echo-analysis.md) | Capability declarations, colorless concurrency |
| [lattice-salt](docs/research/2026-02-25-lattice-salt-analysis.md) | Yield injection, Z3 contracts |
| [oxiz-smt](docs/research/2026-02-25-oxiz-smt-solver-analysis.md) | Pure-Rust SMT solver for constraints |
