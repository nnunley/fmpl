# Specifications

Design documentation for [Project Name TBD], a streaming-first DSL for building AI agents with grammars, capabilities, and durable state.

## Crate Structure

| Spec | Code | Purpose |
|------|------|---------|
| [fmpl-core.md](./fmpl-core.md) | [fmpl-core/](../fmpl-core/) | Core runtime: lexer, parser, compiler, VM, grammars |
| [fmpl-cli.md](./fmpl-cli.md) | [fmpl-cli/](../fmpl-cli/) | Command-line REPL |
| [fmpl-web.md](./fmpl-web.md) | [fmpl-web/](../fmpl-web/) | Web-based REPL with Axum + HTMX |

## Core Systems

| Spec | Code | Purpose |
|------|------|---------|
| [grammar-system.md](./grammar-system.md) | [fmpl-core/src/grammar/](../fmpl-core/src/grammar/) | OMeta-style PEG grammars with inheritance |
| [streaming-grammar.md](./streaming-grammar.md) | [fmpl-core/src/grammar/](../fmpl-core/src/grammar/) | Push-based incremental parsing for async streams |
| [object-system.md](./object-system.md) | [fmpl-core/src/object.rs](../fmpl-core/src/object.rs) | Goblins-inspired objects with spawn, bcom, facets |
| [vm.md](./vm.md) | [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs) | Stack-based bytecode VM with async support |
| [persistence.md](./persistence.md) | [fmpl-core/](../fmpl-core/), [fmpl-web/](../fmpl-web/) | Fjall-backed live image and memo persistence |

## Language Features

| Spec | Code | Purpose |
|------|------|---------|
| [language-guide.md](../docs/design/language-guide.md) | — | DSL syntax and concepts overview |
| [async-streams.md](./async-streams.md) | [fmpl-core/src/stream.rs](../fmpl-core/src/stream.rs) | Async streams with pipe operator |
| [pattern-matching.md](./pattern-matching.md) | [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs) | Pattern matching with `@` operator |

## Design Documents

| Document | Purpose |
|----------|---------|
| [project-overview-draft.md](../docs/design/project-overview-draft.md) | Technical overview with worked examples |
| [fmpl-revival-design.md](../docs/plans/2025-12-19-fmpl-revival-design.md) | Original language design (Goblins-inspired) |
| [unified-grammars-and-agents-design.md](../docs/plans/2026-01-19-unified-grammars-and-agents-design.md) | Grammar-based agent control flow |
| [streaming-grammar-push-model-design.md](../docs/plans/2026-01-20-streaming-grammar-push-model-design.md) | Incremental parse API for streams |
| [async-await-spawn-design.md](../docs/plans/2026-01-20-async-await-spawn-design.md) | Async operators (`<-`, `spawn`) |

## Research

| Document | Purpose |
|----------|---------|
| [tuplespace-vat-actor-conversion.md](../docs/research/2025-12-27-tuplespace-vat-actor-conversion.md) | Tuple space coordination (future) |
| [lindaspaces-book.md](../docs/research/lindaspaces-book.md) | Linda tuple space reference |

## Implementation Plans

| Plan | Status | Purpose |
|------|--------|---------|
| [streaming-grammar-push-model-implementation.md](../docs/plans/2026-01-20-streaming-grammar-push-model-implementation-plan.md) | In Progress | Incremental parse with Fjall backing |
| [async-await-spawn-implementation.md](../docs/plans/2026-01-20-async-await-spawn-implementation.md) | Complete | Async operators implementation |
| [apply-operator-implementation.md](../docs/plans/2026-01-19-apply-operator-implementation-plan.md) | Complete | Grammar application (`@`) |

## External References

- [Spritely Goblins](https://spritely.institute/goblins/) — Distributed, transactional programming
- [OMeta](https://tinlizzie.org/ometa/) — Extensible PEG parsing
- [12 Factor Agents](https://www.humanlayer.dev/blog/12-factor-agents) — Patterns for reliable LLM agents
- [Recursive Language Models](https://alexzhang13.github.io/blog/2025/rlm/) — Context management strategies
