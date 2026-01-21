# FMPL — Technical Overview

---

## What This Project Is

A **Goblins-inspired runtime** with **OMeta-style streaming grammars** designed for building reliable AI agents.

**Core thesis**: Agent control flow is pattern matching over message streams. Grammars give us declarative, composable, inspectable agent logic with built-in backtracking and incremental parsing.

---

## Key Capabilities (Implemented)

### 1. Streaming Grammar Pipelines

Parse async streams (LLM output, HTTP chunks) incrementally with full backtracking:

```fmpl
llm_stream |> parser.tool_call |> execute_tool
```

- **Push-based parsing**: Values arrive asynchronously, grammar emits matches downstream
- **Unlimited backtracking**: Buffered positions with Fjall overflow for long streams
- **Packrat memoization**: External calls memoized as part of rule results
- **Incremental API**: `start()`/`resume()` for durable suspension

**Status**: Core implemented (Tasks 1-5 of streaming grammar plan complete)

### 2. Grammar-Based Agent Control Flow

Agent behavior expressed as grammar rules over message streams:

```fmpl
grammar TaskAgent <: Agent {
  turn =
    | message:m &{ needs_approval(m) } => <- human.ask(m) @ approval_handler
    | message:m => process(m) @ result_handler

  result_handler =
    | %{tool: t, args: a} => <- execute(t, a) @ result_handler
    | %{done: result} => yield(result)
}
```

**Why grammars for agents**:
- **Declarative**: Pattern matching, not imperative control flow
- **Composable**: Grammar inheritance (`<: BaseAgent`)
- **Inspectable**: Rules are data, can be pretty-printed
- **Backtracking**: Natural retry semantics
- **Semantic predicates**: `&{ needs_approval(m) }` for context-aware routing

### 3. Goblins-Inspired Object Model

```fmpl
-- Spawn creates object instances
let obj = spawn ^constructor(args)

-- Sync vs async
$ obj.method()    -- same-vat, synchronous
<- obj.method()   -- async, returns stream

-- Facet-based capabilities (object-bound)
treasury.as(:auditor).view_balance()   -- works
treasury.as(:auditor).withdraw(100)    -- denied: not on facet
```

**Implemented**:
- `spawn` operator for object creation
- `<-` async operator returning streams
- Facet-based capability system (terminal facets, member restrictions)

**Planned**:
- `bcom` for functional state updates
- Automatic transactions (error = rollback)
- Promise pipelining

### 4. Fjall Persistence Layer

- **Live image**: Object graph persists across restarts
- **Streaming position overflow**: Large buffers spill to disk
- **Memo table persistence**: Memoization survives suspension (implemented)
- **ParseState serialization**: Durable parse suspension (in progress)

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│         Single-VAT Event Loop Runtime           │
│  ┌─────────────────────────────────────────┐    │
│  │  Goblins Object Model                   │    │
│  │  - spawn/bcom (functional state)        │    │
│  │  - $ sync / <- async                    │    │
│  │  - Facet capabilities                   │    │
│  └─────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────┐    │
│  │  Streaming Grammar Engine               │    │
│  │  - OMeta-style PEG with inheritance     │    │
│  │  - Incremental parse (start/resume)     │    │
│  │  - ParseDriver for async pipelines      │    │
│  │  - Fjall-backed memoization             │    │
│  └─────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────┐    │
│  │  Async Runtime (Tokio)                  │    │
│  │  - Stream channels                      │    │
│  │  - HTTP client (curl)                   │    │
│  │  - spawn/await/select                   │    │
│  └─────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────┐
│         Fjall Persistence (LSM Store)           │
│  - Live image (object graph)                    │
│  - StreamPosition overflow                      │
│  - Memo table persistence                       │
│  - ParseState serialization                     │
└─────────────────────────────────────────────────┘
```

---

## Comparison with Collaborative Agentic System Design

The `architecture.md` document describes a multi-process coordinator system. Here's how this project relates:

| Aspect | This Project | Agentic System Design |
|--------|--------------|----------------------|
| **Execution model** | Single-VAT event loop | Multi-process coordinator + workers |
| **Coordination** | Grammar pipelines + streams | Tuple space + reactive subscriptions |
| **Agent model** | Grammar rules over messages | DSL scripts with tuple operations |
| **Object model** | Goblins (spawn/bcom/facets) | Not specified |
| **Security** | Facets (object-bound capabilities) | PASETO tokens (hierarchical) |
| **Multi-user** | Not designed (single VAT) | First-class (users, namespaces) |
| **Persistence** | Fjall (live image) | Fjall (tuple write-through) |
| **LLM integration** | Streaming grammar parsers | Centralized API client |

### What This Project Provides That Agentic System Needs

1. **Streaming grammar engine**: Parsing LLM output, tool call extraction, incremental state
2. **Agent-as-grammar pattern**: Declarative control flow with backtracking
3. **Goblins object model**: spawn/bcom, facet capabilities
4. **Durable suspension**: ParseState serialization for pause/resume

### What This Project Lacks (from Agentic System) — Future Work

1. **Multi-user isolation**: Single shared VAT (multi-VAT planned for later)
2. **Tuple space coordination**: Designed but not implemented
3. **PASETO tokens**: No distributed auth (not needed until multi-VAT)
4. **Worker sandboxing**: No cgroups/seccomp (not needed until multi-VAT)
5. **Reactive scheduler**: No tuple subscriptions (planned with tuple space)

---

## Roadmap: Single-VAT Now, Multi-VAT Later

### Current: Single-VAT Runtime

This project is currently a **single-VAT system** — one event loop, one VM instance, no horizontal scaling. This is intentional for the current phase:

- **Simpler**: No distributed coordination complexity
- **Faster iteration**: Focus on grammar engine and agent patterns
- **Sufficient for**: REPL, experiments, single-user agent workflows

### Future: Multi-VAT with Coordinator

When scaling is needed, the path forward is multi-VAT:

```
Coordinator (future)
  ├── Tuple Store + Reactive Scheduler
  ├── Auth (PASETO or similar)
  └── VAT Management
         │
         ▼
VATs (N separate processes)
  └── Each runs this project's runtime
      ├── Streaming grammar engine
      ├── Goblins object model
      ├── Facet capabilities
      └── Tuple space client (to coordinator)
```

**Not building this now** — focus is on completing the grammar engine and agent patterns first. Multi-VAT adds complexity (tuple space coordination, distributed auth, worker lifecycle) that isn't needed until we have proven the single-VAT model.

---

## Worked Examples: 12-Factor Agents with Streaming Grammars

The [12-Factor Agents](https://www.humanlayer.dev/blog/12-factor-agents) principles describe patterns for building reliable LLM agents. Here's how each maps to streaming grammar pipelines.

---

### Example 1: Basic Tool-Calling Agent

**Factors covered**: 1 (NL→Tool), 4 (Tools as Structured Output), 8 (Own Control Flow)

```fmpl
-- Define the agent as a grammar over message streams
grammar ToolAgent <: base::tree {

  -- Main entry: process user message
  turn = message:m => <- llm_complete(m) |> tool_output

  -- Parse LLM output stream, extract tool calls
  tool_output =
    | %{tool: t, args: a} => <- execute_tool(t, a) |> turn  -- recurse
    | %{done: result}     => result                          -- terminal
    | %{text: t}          => emit_to_user(t); <tool_output>  -- stream text, continue
}

-- Usage: pipe user messages through agent
user_messages |> ToolAgent.turn |> responses
```

**What's happening**:
1. User message arrives on `user_messages` stream
2. Grammar matches it, calls LLM
3. LLM response streams back, piped through `tool_output` rule
4. Tool calls get executed, results feed back into `turn`
5. Final result emits to `responses`

---

### Example 2: Context Engineering with Semantic Predicates

**Factors covered**: 3 (Own Context Window), 9 (Compact Errors)

```fmpl
grammar ContextAgent <: ToolAgent {

  -- Override turn to build context before LLM call
  turn = message:m &{ build_context(m) }:ctx => {
    <- llm_complete(m, context: ctx)
  } |> tool_output

  -- Semantic predicate: compute context mid-match
  build_context(m) = {
    let history = conversation_history(10)
    let retrieved = <- vector_search(m.text, limit: 5)
    let errors = recent_errors(3)

    history
      |> inject_retrieved(retrieved)
      |> compact_errors(errors)        -- Factor 9: summarize errors
      |> truncate_to_budget(8000)      -- stay within token limit
  }

  -- Error compaction: don't stuff raw stacktraces
  compact_errors(errors) = errors
    |> map(|e| %{type: e.type, message: e.message |> truncate(100)})
}
```

**What's happening**:
1. `&{ build_context(m) }` is a semantic predicate — runs mid-match
2. Context is computed *before* LLM call, bound to `ctx`
3. Errors are compacted, not raw (Factor 9)
4. Token budget is enforced (Factor 3)

---

### Example 3: Human-in-the-Loop Approval

**Factors covered**: 6 (Launch/Pause/Resume), 7 (Contact Humans)

```fmpl
grammar ApprovalAgent <: ToolAgent {

  -- Gate dangerous tools behind human approval
  tool_output =
    | %{tool: t, args: a} &{ needs_approval(t) } => {
        -- Pause for human (durable — survives restart)
        let decision = <- human.approve(%{
          tool: t,
          args: a,
          reason: "This action requires approval"
        })
        decision @ {
          %{approved: true}  => <- execute_tool(t, a) |> tool_output
          %{denied: r}       => %{error: "denied", reason: r}
        }
      }
    | <super.tool_output>  -- inherit other cases

  -- What needs approval?
  needs_approval(tool) = tool in [:delete_file, :send_email, :deploy]
}

-- The human.approve call:
-- 1. Serializes continuation to Fjall (durable)
-- 2. Creates approval request object
-- 3. Suspends agent
-- 4. When human clicks approve/deny, agent resumes exactly where it left off
```

**What's happening**:
1. Dangerous tools hit the `&{ needs_approval(t) }` predicate
2. `<- human.approve(...)` suspends the agent, creates durable request
3. Human responds hours/days later
4. Agent resumes from serialized state (Factor 6)
5. Decision routes to execute or deny

---

### Example 4: Composing Small Focused Agents

**Factors covered**: 10 (Small Focused Agents), 2 (Own Your Prompts)

```fmpl
-- Base agent: just handles errors
grammar BaseAgent <: base::tree {
  error_handler =
    | %{error: e} &{ retryable(e) } => log(e); <turn>  -- retry
    | %{error: e}                   => escalate(e)     -- give up
}

-- RAG agent: adds retrieval
grammar RAGAgent <: BaseAgent {
  context_for(m) = {
    let base = %{history: last_messages(10)}
    base | %{retrieved: <- vector_search(m.text)}
  }
}

-- Tool agent: adds tool execution
grammar ToolAgent <: RAGAgent {
  context_for(m) = {
    let base = <super.context_for>(m)
    base | %{tools: available_tools()}
  }

  tool_output =
    | %{tool: t, args: a} => <- execute_tool(t, a) |> tool_output
    | <super.error_handler>
    | %{done: r} => r
}

-- Code agent: specialized for coding tasks
grammar CodeAgent <: ToolAgent {
  context_for(m) = {
    let base = <super.context_for>(m)
    base | %{
      codebase: <- search_codebase(m.text),
      conventions: load_conventions()
    }
  }

  -- Override to add code-specific tools
  available_tools() = [:read_file, :write_file, :run_tests, :search_code]
}
```

**What's happening**:
1. Each grammar is small and focused (Factor 10)
2. Inheritance composes behaviors: `CodeAgent <: ToolAgent <: RAGAgent <: BaseAgent`
3. `<super.context_for>` calls parent, then extends
4. Prompts/patterns are explicit in code (Factor 2)

---

### Example 5: Multi-Step Workflow with Checkpoints

**Factors covered**: 5 (Unify Execution + Business State), 6 (Pause/Resume)

```fmpl
grammar FeatureAgent <: ToolAgent {

  -- Multi-step feature implementation
  implement_feature = task:t => {
    -- Step 1: Plan
    let plan = <- plan_step(t)
    checkpoint("planned", plan)

    -- Step 2: Implement each item
    plan.steps |> map(|step| {
      let result = <- implement_step(step)
      checkpoint("implemented", step, result)
      result
    }) |> collect:results

    -- Step 3: Test
    let test_result = <- run_tests()
    checkpoint("tested", test_result)

    -- Step 4: Human review
    <- human.review(%{plan: plan, results: results, tests: test_result})
  }

  -- Checkpoint: persist state, can resume from here
  checkpoint(stage, data*) = {
    persist(%{
      stage: stage,
      data: data,
      continuation: current_continuation()
    })
  }

  -- Resume from checkpoint (e.g., after crash)
  resume_from(checkpoint) = checkpoint.continuation.resume()
}
```

**What's happening**:
1. Workflow has explicit stages: plan → implement → test → review
2. `checkpoint()` persists continuation + state to Fjall
3. If system crashes, `resume_from()` picks up where it left off
4. Business state (plan, results) unified with execution state (Factor 5)

---

### Example 6: Streaming LLM Output Parsing

**Factors covered**: 11 (Trigger from Anywhere), 4 (Structured Output)

```fmpl
grammar LLMOutputParser <: base::tree {

  -- Parse streaming LLM tokens
  output =
    | "```" language:word code_block* "```" => emit_code(language, code_block)
    | "TOOL:" tool_call                      => tool_call
    | chunk+                                 => stream_to_user(chunks)

  -- Tool call: TOOL:name({"arg": "value"})
  tool_call = name:word "(" json:json ")" => %{tool: name, args: json}

  -- Code block content (until closing ```)
  code_block = !("```") any:c => c
}

-- Usage: LLM stream → parser → structured output
llm.stream(prompt)
  |> LLMOutputParser.output
  |> handle_output

-- handle_output receives:
-- - %{tool: "search", args: %{query: "..."}}
-- - %{code: "rust", content: "fn main() {...}"}
-- - %{text: "Here's what I found..."}
```

**What's happening**:
1. Raw LLM tokens stream in
2. Grammar parses incrementally, emits structured values
3. Tool calls become `%{tool, args}` maps (Factor 4)
4. Code blocks extracted with language tag
5. Plain text streamed through to user

---

### Example 7: Error Recovery with Backtracking

**Factors covered**: 9 (Compact Errors), 8 (Own Control Flow)

```fmpl
grammar ResilientAgent <: ToolAgent {

  -- Retry with error context
  tool_output =
    | %{tool: t, args: a} => {
        <- execute_tool(t, a) @ {
          %{error: e} &{ retryable(e) } => {
            -- Compact error, retry with context
            let context = compact_error(e)
            <- llm_complete("Tool failed: " + context + ". Try again.")
              |> tool_output
          }
          %{error: e} => escalate(e)
          result      => result |> tool_output
        }
      }
    | <super.tool_output>

  -- Retryable errors
  retryable(e) = e.type in [:timeout, :rate_limit, :transient]

  -- Compact error for context (don't stuff raw stacktrace)
  compact_error(e) = %{
    type: e.type,
    message: e.message |> truncate(200),
    suggestion: error_suggestion(e.type)
  }

  error_suggestion(type) = type @ {
    :timeout    => "Try a simpler approach"
    :rate_limit => "Wait and retry"
    :transient  => "Retry the same operation"
    _           => "Consider an alternative"
  }
}
```

**What's happening**:
1. Tool execution wrapped in error handler
2. Retryable errors trigger LLM retry with compacted context
3. Grammar backtracking handles the retry naturally
4. Non-retryable errors escalate

---

### Example 8: Recursive Language Model (RLM) Patterns

**Factors covered**: 3 (Context Window), 10 (Small Agents) + RLM strategies

The [RLM paper](https://alexzhang13.github.io/blog/2025/rlm/) describes strategies for avoiding context rot: instead of stuffing entire context into prompts, agents recursively decompose work and share knowledge through a coordination layer.

```fmpl
grammar RLMAgent <: ToolAgent {

  -- Main entry: decompose large tasks recursively
  turn = task:t => {
    -- Peek: sample to understand structure
    let structure = <- peek(t)

    structure @ {
      %{decomposable: true, subtasks: subs} => {
        -- Partition + Map: spawn child agents
        subs
          |> map(|sub| spawn(<turn>, sub))  -- recursive spawn
          |> await_all                       -- wait for children
          |> summarize                       -- aggregate results
      }
      _ => work_directly(t)  -- small enough, do it
    }
  }

  -- PEEK: Sample context to understand structure
  peek(task) = {
    let sample = task.files |> take(1) |> read_chunk(0, 100)
    <- llm_complete(
      "What's the structure? Is this decomposable?",
      context: sample
    )
  }

  -- GREP: Filter to relevant sections
  grep(task, pattern) = {
    task.files
      |> flat_map(|f| search_file(f, pattern))
      |> take(20)  -- limit context size
  }

  -- SUMMARIZE: Condense child results for parent
  summarize(results) = {
    let summary = <- llm_complete(
      "Summarize these results concisely",
      context: results |> map(|r| r.summary) |> join("\n")
    )
    %{summary: summary, details: results}
  }

  -- Direct work: when task is small enough
  work_directly(task) = {
    let context = grep(task, task.focus_pattern)
    <- llm_complete(task.prompt, context: context)
  }
}
```

**RLM strategies as grammar patterns:**

```fmpl
-- PEEKING: Sample before committing to full read
peek_file(path) = {
  let chunk = <- read_file(path, offset: 0, limit: 100)
  <- llm_complete("What's in this file?", context: chunk)
}

-- GREPPING: Filter to relevant sections
grep_codebase(pattern) = {
  <- search_code(pattern)
    |> take(10)
    |> map(|match| %{
        file: match.file,
        line: match.line,
        context: match.surrounding_lines(3)
      })
}

-- PARTITION + MAP: Parallel recursive decomposition
partition_and_map(items, process_fn) = {
  items
    |> chunk(10)                              -- partition into chunks
    |> map(|chunk| spawn(process_fn, chunk))  -- spawn child per chunk
    |> await_all                              -- wait for all children
    |> flat_map(|r| r.results)                -- merge results
}

-- SUMMARIZATION: Condense for parent context
summarize_for_parent(results) = {
  <- llm_complete(
    "Summarize key findings in 3 bullets",
    context: results
  ) |> parse_bullets
}
```

**Full RLM workflow example:**

```fmpl
-- Implement a feature across a large codebase
implement_feature(spec) = {
  -- 1. Peek at codebase structure
  let structure = <- peek_codebase()

  -- 2. Grep for relevant files
  let relevant = grep_codebase(spec.keywords)

  -- 3. Partition files, spawn child agents
  let results = relevant.files
    |> partition_and_map(|file| {
        -- Each child: focused context, no rot
        let context = <- read_file(file)
        let changes = <- llm_complete(
          "What changes needed for: " + spec.description,
          context: context
        )
        %{file: file, changes: changes}
      })

  -- 4. Summarize for human review
  let summary = <- summarize_for_parent(results)

  -- 5. Human approval before applying
  <- human.review(%{
    spec: spec,
    summary: summary,
    changes: results
  })
}
```

**Why this avoids context rot:**
- Each child agent gets **focused context** (just its file/chunk)
- Parent sees **summaries**, not raw content
- Recursive depth handled naturally by grammar + spawn
- Knowledge reuse: summaries can be cached/persisted

---

### Summary: 12 Factors + RLM → Grammar Patterns

| Factor | Grammar Pattern |
|--------|-----------------|
| 1. NL → Tool Calls | `%{tool: t, args: a}` pattern matching |
| 2. Own Your Prompts | Grammar rules ARE the prompts |
| 3. Own Context Window | `&{ build_context(m) }` semantic predicates |
| 4. Tools = Structured Output | `=> %{tool: name, args: args}` |
| 5. Unify State | Live image + `checkpoint()` |
| 6. Launch/Pause/Resume | `current_continuation()` + Fjall persistence |
| 7. Contact Humans | `<- human.approve()` / `<- human.ask()` |
| 8. Own Control Flow | Grammar inheritance + pattern matching |
| 9. Compact Errors | `compact_error()` + predicates |
| 10. Small Agents | Grammar inheritance (`<: BaseAgent`) |
| 11. Trigger Anywhere | Stream sources: HTTP, WS, files, etc. |
| 12. Stateless Reducer | `bcom` pattern (planned) |

| RLM Strategy | Grammar Pattern |
|--------------|-----------------|
| Peeking | `peek(task)` — sample before full read |
| Grepping | `grep(pattern)` — filter to relevant sections |
| Partition + Map | `items \|> map(\|i\| spawn(fn, i)) \|> await_all` |
| Summarization | `summarize_for_parent(results)` — condense for parent |
| Recursive Spawn | `spawn(<turn>, subtask)` — child agents with focused context |
| No Context Rot | Children get focused context, parent sees summaries |

---

## Current Implementation Status

### Complete
- [x] OMeta-style PEG grammars with inheritance
- [x] Grammar application (`@` operator)
- [x] `spawn` and `<-` async operators
- [x] Facet-based capability system
- [x] Fjall persistence (live image, streaming overflow)
- [x] Exception handling (cross-frame unwinding)
- [x] Stream pipelines (map, filter, parse, async-parse)
- [x] ParseState/ParseNext types for incremental parsing
- [x] Incremental parse API (start/resume)
- [x] ParseDriver for async pipelines
- [x] Fjall backing for memo tables
- [x] ParseState binary serialization
- [x] Integration tests for streaming pipeline
- [x] Streaming grammar and async operators documentation

### In Progress
- [ ] Pattern matching in `@` expressions (map/list patterns)
- [ ] Tuple space operations (`out`/`in`/`rd`)

### Planned
- [ ] `bcom` for functional state updates
- [ ] Automatic transactions (error rollback)
- [ ] Anonymous grammar blocks with full pattern support
- [ ] Multi-VAT coordination

---

## Project Structure

```
fmpl-core/           # Core runtime (lexer, parser, compiler, VM, grammars)
  src/
    grammar/
      runtime.rs     # PegRuntime with start/resume
      incremental.rs # ParseState, ParseNext
      driver.rs      # ParseDriver for async streams
      stream_input.rs # StreamPosition with Fjall backing
    object.rs        # Goblins-style objects with facets
    value.rs         # Runtime values (streams, grammars, etc.)
    vm.rs            # Stack-based bytecode VM

fmpl-cli/            # REPL interface
fmpl-web/            # HTTP server (Axum + HTMX)

docs/
  plans/             # Design and implementation plans
  research/          # Research notes (tuplespaces, lindaspaces)
  design/            # Vision documents
  analysis/          # Comparison documents
```

---

## References

### This Project
- [Unified Grammars and Agents Design](../plans/2026-01-19-unified-grammars-and-agents-design.md)
- [Streaming Grammar Push-Model Design](../plans/2026-01-20-streaming-grammar-push-model-design.md)
- [FMPL Revival Design](../plans/2025-12-19-fmpl-revival-design.md) (original, Goblins-inspired)
- [Tuple Space VAT Actor Conversion](../research/2025-12-27-tuplespace-vat-actor-conversion.md)

### Inspirations
- [Spritely Goblins](https://spritely.institute/goblins/) — Distributed, transactional programming
- [OMeta](https://tinlizzie.org/ometa/) — Extensible PEG parsing
- [12 Factor Agents](https://www.humanlayer.dev/blog/12-factor-agents) — Patterns for reliable LLM agents
- [maru](https://piumarta.com/software/maru/) — Polymorphic streams over strings and lists

---

## Name Candidates

The project needs a new name. Considerations:
- Reflects streaming grammars + agentic workflows
- Not tied to 1992 FMPL heritage
- Memorable, searchable

Some directions to explore:
- **Grammar-related**: parse, stream, pattern, rule
- **Agent-related**: agent, coord, flow, loop
- **Goblins-inspired**: vat, actor, cap, facet

