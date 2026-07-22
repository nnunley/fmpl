# Specifications

Design documentation for FMPL, a streaming-first DSL for building AI agents with grammars, capabilities, and durable state.

> **Documentation Standards**: See [docs/STANDARDS.md](../docs/STANDARDS.md) for standard structure and formatting guidelines for design documents, implementation plans, and specifications.

> **Roadmap**: See [ROADMAP.md](./ROADMAP.md) for where FMPL is and what's next.

## Crate Structure

| Spec | Code | Purpose |
|------|------|---------|
| [fmpl-core.md](./fmpl-core.md) | [fmpl-core/](../fmpl-core/) | Core runtime: lexer, parser, compiler, VM, grammars |
| [fmpl-cli.md](./fmpl-cli.md) | [fmpl-cli/](../fmpl-cli/) | Command-line REPL |
| [fmpl-web.md](./fmpl-web.md) | [fmpl-web/](../fmpl-web/) | Web-based REPL with Axum + HTMX |

## Core Systems

| Spec | Code | Purpose |
|------|------|---------|
| [grammar-system.md](./grammar-system.md) | [fmpl-core/src/grammar/](../fmpl-core/src/grammar/) | OMeta-style PEG grammars with inheritance and streaming support |
| [grammar-optimizer.md](./grammar-optimizer.md) | `fmpl-core/src/grammar/optimizer.rs` (planned) | Prefix trie, skip-to-literal fusion, Aho-Corasick multi-pattern |
| [object-system.md](./object-system.md) | [fmpl-core/src/object.rs](../fmpl-core/src/object.rs) | Goblins-inspired objects with spawn, facets |
| [vm.md](./vm.md) | [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs) | Indexed RPN bytecode VM with async support |
| [persistence.md](./persistence.md) | [fmpl-core/](../fmpl-core/), [fmpl-web/](../fmpl-web/) | Fjall-backed live image and memo persistence |

## Language Features

| Spec | Code | Purpose |
|------|------|---------|
| [language-guide.md](../docs/design/language-guide.md) | — | DSL syntax and concepts overview |
| [async-streams.md](./async-streams.md) | [fmpl-core/src/stream.rs](../fmpl-core/src/stream.rs) | Async streams with pipe operator |
| [parse-stream.md](./parse-stream.md) | [fmpl-core/src/parse_stream.rs](../fmpl-core/src/parse_stream.rs) | ParseStream with combinators and packrat memoization |
| [pattern-matching.md](./pattern-matching.md) | [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs) | Pattern matching with `@` operator |
| [in-operator.md](./in-operator.md) | [fmpl-core/src/vm.rs](../fmpl-core/src/vm.rs) | Membership testing with `in` operator |
| [type-system.md](./type-system.md) | — (not yet implemented) | Layered type inference: success typing, row polymorphism, occurrence typing, algebraic structures, SMT |

## Agent Framework

| Spec | Code | Purpose |
|------|------|---------|
| [12-factor-agents.md](./12-factor-agents.md) | Cross-cutting | 12-Factor Agents + RLM context management mapped to FMPL |
| [llm-tool-calling.md](./llm-tool-calling.md) | [fmpl-core/tests/tool_calling.rs](../fmpl-core/tests/tool_calling.rs) | LLM tool calling with `@` operator (Complete v4) |

## Backtracking & CSP

| Spec | Code | Purpose |
|------|------|---------|
| [backtracking-csp.md](./backtracking-csp.md) | [fmpl-core/src/grammar/runtime.rs](../fmpl-core/src/grammar/runtime.rs) | Prolog-style backtracking for grammar ambiguity |
| [backtracking-opt-in-marker.md](./backtracking-opt-in-marker.md) | — | Explicit `?` marker for opt-in backtracking |
| [csp-solving-status.md](./csp-solving-status.md) | — | CSP solving implementation status |

## Standard Library

| Spec | Code | Purpose |
|------|------|---------|
| [lib.md](./lib.md) | [lib/](../lib/) | Standard library modules (LLM clients, compaction detection) |

## Implementation Details

| Spec | Code | Purpose |
|------|------|---------|
| [indexed-rpn-conversion.md](./indexed-rpn-conversion.md) | [fmpl-core/src/compiler.rs](../fmpl-core/src/compiler.rs) | Indexed RPN bytecode design and conversion |
| [parser-limitations.md](./parser-limitations.md) | [fmpl-core/src/parser.rs](../fmpl-core/src/parser.rs) | Known parser limitations and workarounds |
| [tuplespace.md](./tuplespace.md) | [fmpl-core/src/tuplespace/](../fmpl-core/src/tuplespace/) | Linda-style tuple space coordination |

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
| [type-inference-duck-typed-systems.md](../docs/research/2026-02-25-type-inference-duck-typed-systems.md) | 12 type inference approaches for duck-typed languages (layered recommendation) |
| [category-theoretic-type-system.md](../docs/research/2026-02-25-category-theoretic-type-system.md) | Category theory, coalgebraic semantics, algebraic law inference for FMPL types |
| [lattice-salt-analysis.md](../docs/research/2026-02-25-lattice-salt-analysis.md) | Salt/Lattice: Z3 verification, coroutines, capability tokens |
| [oxiz-smt-solver-analysis.md](../docs/research/2026-02-25-oxiz-smt-solver-analysis.md) | OxiZ pure-Rust SMT solver for exhaustiveness checking |
| [multi-user-architecture-synthesis.md](../docs/research/2026-02-25-multi-user-architecture-synthesis.md) | Multi-user architecture synthesis (mooR, ColdMUD, Salt) |
| [moor-echo-analysis.md](../docs/research/2026-02-25-moor-echo-analysis.md) | mooR/Echo analysis for multi-user patterns |
| [coldmud-architecture.md](../docs/research/2026-02-25-coldmud-architecture.md) | ColdMUD architecture analysis |
| [tuplespace-vat-actor-conversion.md](../docs/research/2025-12-27-tuplespace-vat-actor-conversion.md) | Tuple space coordination (future) |
| [lindaspaces-book.md](../docs/research/lindaspaces-book.md) | Linda tuple space reference |

## Implementation Plans

| Plan | Status | Purpose |
|------|--------|---------|
| [streaming-grammar-push-model-implementation.md](../docs/plans/2026-01-20-streaming-grammar-push-model-implementation-plan.md) | Complete | Incremental parse with Fjall backing |
| [async-await-spawn-implementation.md](../docs/plans/2026-01-20-async-await-spawn-implementation.md) | Complete | Async operators implementation |
| [apply-operator-implementation.md](../docs/plans/2026-01-19-apply-operator-implementation-plan.md) | Complete | Grammar application (`@`) |
| [tuplespace-implementation-plan.md](../docs/plans/2026-01-23-tuplespace-implementation-plan.md) | Draft | Linda-style tuple space for coordination |

## External References

- [Spritely Goblins](https://spritely.institute/goblins/) — Distributed, transactional programming
- [OMeta](https://tinlizzie.org/ometa/) — Extensible PEG parsing
- [12 Factor Agents](https://www.humanlayer.dev/blog/12-factor-agents) — Patterns for reliable LLM agents
- [Recursive Language Models](https://alexzhang13.github.io/blog/2025/rlm/) — Context management strategies
