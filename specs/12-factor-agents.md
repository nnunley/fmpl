# 12-Factor Agents in FMPL

Mapping the [12-Factor Agents](https://github.com/humanlayer/12-factor-agents) principles to FMPL's architecture, combined with [Recursive Language Model (RLM)](https://alexzhang13.github.io/blog/2025/rlm/) context management strategies.

**Location**: Cross-cutting concern across all FMPL crates

**Key Design Docs**:
- [unified-grammars-and-agents-design.md](../docs/plans/2026-01-19-unified-grammars-and-agents-design.md) — Grammar-based agent control flow
- [tuplespace-vat-actor-conversion.md](../docs/research/2025-12-27-tuplespace-vat-actor-conversion.md) — Tuple space for context sharing
- [fmpl-vs-agentic-comparison-final.md](../docs/analysis/2026-01-20-fmpl-vs-agentic-comparison-final.md) — Architecture comparison

---

## Overview

The 12-Factor Agents framework provides principles for building reliable LLM-powered agents. FMPL's streaming grammars, capability-based security, and tuple space coordination naturally support these principles. This spec maps each factor to FMPL implementation strategies.

**Core insight from unified-grammars design**: An agent's control flow is a grammar over message streams. Grammars provide explicit control flow (Factor 8), composability (Factor 10), and deterministic routing while semantic predicates (`&{ expr }`) handle context engineering (Factor 3).

---

## Factor 1: Natural Language to Tool Calls

> Convert user intent into structured function calls that deterministic code can execute reliably.

### FMPL Implementation

**Grammars as tool call parsers**: Use PEG grammars to extract structured tool calls from LLM output.

```fmpl
grammar ToolParser <: base::tree {
  tool_call = "TOOL:" word:name "(" json:args ")" => %{tool: name, args: args};
  text_chunk = (!tool_call .)+:text => %{text: text};
  output = (tool_call | text_chunk)*;
}

-- Parse LLM response into structured calls
llm_response @ ToolParser.output
```

**Status**:
- Grammar system for parsing tool calls
- `llm.parse_tool_call()` in `lib/llm-common.fmpl`
- Tool execution dispatch via pattern matching

**Gaps**:
- [ ] Tool registry for `llm.execute_tool()` dispatch
- [ ] Schema validation for tool arguments

---

## Factor 2: Own Your Prompts

> Maintain direct control over prompt engineering rather than relying on framework defaults.

### FMPL Implementation

**Prompts as first-class values**: Prompts are strings/templates managed in FMPL code, not hidden in framework internals.

```fmpl
-- Prompts are explicit, editable values
let system_prompt = "You are a coding assistant. When you need to execute code, use TOOL:run_code(...)"

-- Template composition
let prompt_with_context = system_prompt + "\n\nContext:\n" + context
```

**Status**:
- Prompts are plain strings in FMPL
- No hidden prompt templates in the runtime
- `lib/anthropic.fmpl` and `lib/ollama.fmpl` expose full prompt control

**Gaps**: None - FMPL is prompt-transparent by design.

---

## Factor 3: Own Your Context Window

> Deliberately manage what information enters the LLM's context to maximize relevance and minimize costs.

### FMPL Implementation

**RLM-inspired strategies**: Rather than stuffing context, use programmatic context management.

```fmpl
-- Partition + Map: Process large context in chunks
let chunks = split_context(large_document, 4000)
let summaries = chunks |> map(\chunk -> llm.summarize(chunk))
let final_context = join(summaries, "\n")

-- Grep: Narrow before processing
let relevant = grep_context(document, "error|exception|failed")
let analysis = llm.analyze(relevant)
```

**Tuple space for context sharing** (implemented):
```fmpl
-- Create a tuple space
let ts = tuplespace.new()

-- Agents write insights to tuple space
ts.out(:insight, %{topic: "auth", summary: auth_summary})

-- Other agents query what they need, not full context
let auth_context = ts.rd(:insight)  -- pattern matches type

-- Subscribe to changes reactively
let insight_stream = ts.subscribe(:insight)
insight_stream |> map(\t -> process_insight(t))
```

**Compaction detection** (`lib/compaction.fmpl`):
```fmpl
let check = should_compact(conversation_history, last_response)
if (check.should_compact)
  -- Trigger context compaction
  compact_context(conversation_history, check.reason)
```

**Status**:
- `lib/compaction.fmpl` detects off-track patterns (groveling, apologizing, condescending)
- `lib/compaction.fmpl` detects circular patterns (multiple short assistant responses)
- Pattern matching for context filtering
- Streaming grammars for incremental processing
- Tuple space implemented (`tuplespace.new()`, `out`, `in`, `rd`, `subscribe`)
- Tuple space facets for namespace isolation and read/write permissions

**Gaps**:
- [ ] Implement actual compaction (context elision)
- [ ] Token counting utilities
- [ ] Automatic context budget enforcement

---

## Factor 4: Tools Are Structured Outputs

> Treat tool definitions as specifications for LLM output formatting, not magical features.

### FMPL Implementation

**Grammar-defined tool schemas**: Tool calls are just patterns the grammar recognizes.

```fmpl
grammar ToolSchema {
  -- Define expected tool output format
  run_code = "run_code(" json:args ")"
    &{ has_key(args, "language") && has_key(args, "code") }
    => %{tool: :run_code, language: args.language, code: args.code};

  read_file = "read_file(" string:path ")" => %{tool: :read_file, path: path};

  tool = run_code | read_file;
}
```

**Tool definitions as data**:
```fmpl
let tools = [
  %{name: "run_code", params: %{language: :string, code: :string}},
  %{name: "read_file", params: %{path: :string}}
]

-- Generate prompt section from tool definitions
let tool_prompt = tools |> map(format_tool) |> join("\n")
```

**Status**:
- Grammars parse structured output
- Tool schemas expressible as FMPL data

**Gaps**:
- [ ] Tool schema validation helpers
- [ ] Auto-generate tool prompts from schemas

---

## Factor 5: Unify Execution and Business State

> Keep agent execution state synchronized with application data models.

### FMPL Implementation

**Single VM state**: All state lives in FMPL objects, no separate "agent state" vs "app state".

```fmpl
object TaskRunner {
  .#private
  tasks: []
  current_task: null

  .#public
  add_task(t): self.tasks = self.tasks + [t]
  run_next():
    let task = self.tasks[0]
    self.current_task = task
    <- execute(task)
  get_status(): %{pending: len(self.tasks), current: self.current_task}
}
```

**Fjall persistence**: State survives restarts via live image serialization.

**Status**:
- Unified object model (Goblins-inspired)
- Fjall persistence for durable state
- No separate agent/app state dichotomy

**Gaps**:
- [ ] Automatic transaction rollback on errors (Goblins `bcom`)
- [ ] State change subscriptions

---

## Factor 6: Launch/Pause/Resume with Simple APIs

> Enable agents to start, pause, and resume through stateless interfaces.

### FMPL Implementation

**Incremental parse state**: Grammar parsing can suspend and resume.

```fmpl
-- Start parsing
let state = parser.start("rule_name")

-- Suspend and serialize
let serialized = state.to_bytes()
fjall.put("parse_state", serialized)

-- Later: resume from saved state
let restored = ParseState.from_bytes(fjall.get("parse_state"))
let result = parser.resume(restored)
```

**Async operations with streams**:
```fmpl
-- Pause: stream is lazy, doesn't execute until pulled
let workflow = <- long_running_task() |> process |> save

-- Resume: continue pulling from stream
workflow |> next
```

**Status**:
- `ParseState` serialization in `grammar/incremental.rs`
- Fjall backing for durable parse states
- Lazy stream evaluation enables implicit pause

**Gaps**:
- [ ] High-level `agent.pause()` / `agent.resume()` API
- [ ] Checkpoint/restore for full VM state
- [ ] Human approval suspension (`<- human.approve(...)`)

---

## Factor 7: Contact Humans with Tool Calls

> Use the same tool-calling mechanism to request human approval or input.

### FMPL Implementation

**Human approval as async tool**:
```fmpl
grammar AgentWithApproval <: ToolParser {
  tool_call =
    | %{tool: t} &{ needs_approval(t) } => <- human.approve(t) @ approval_handler
    | <super.tool_call>;

  approval_handler =
    | %{approved: true} => execute_tool(t)
    | %{denied: reason} => %{error: "Denied: " + reason};
}
```

**Facet-gated capabilities**:
```fmpl
-- Tool requires elevated capability
let result = tool.as(:requires_approval).execute(args)
-- Facet check fails -> triggers human approval flow
```

**Status**:
- Facets provide capability gating
- Async operator (`<-`) supports blocking on external input
- Pattern matching routes approval responses

**Gaps**:
- [ ] `human.approve()` builtin with UI integration
- [ ] Approval timeout handling
- [ ] Approval audit trail

---

## Factor 8: Own Your Control Flow

> Explicitly define agent decision logic rather than delegating entirely to LLM reasoning.

### FMPL Implementation

**Grammars as control flow**: Agent behavior is a grammar, not an opaque LLM loop.

```fmpl
grammar TaskAgent <: base::tree {
  -- Explicit state machine as grammar rules
  turn =
    | %{state: :research} => research_phase
    | %{state: :plan} => plan_phase
    | %{state: :execute} => execute_phase
    | %{state: :review} => review_phase;

  research_phase = gather_context => %{state: :plan, context: _};
  plan_phase = create_plan => %{state: :execute, plan: _};
  execute_phase = run_plan => %{state: :review, result: _};
  review_phase = evaluate => done | %{state: :research};  -- loop or finish
}
```

**Deterministic routing**:
```fmpl
-- Pattern matching, not LLM decision
response @ {
  %{tool: "search", args: a} => execute_search(a)
  %{tool: "code", args: a} => execute_code(a)
  %{text: t} => emit(t)
  _ => handle_unknown()
}
```

**Status**:
- Grammars define explicit control flow
- Pattern matching for deterministic routing
- `llm.agent_loop()` in `lib/llm-common.fmpl`

**Gaps**: None - FMPL's grammar-based agents are inherently explicit.

---

## Factor 9: Compact Errors into Context Window

> Summarize failures efficiently so subsequent LLM calls learn from mistakes without token waste.

### FMPL Implementation

**Error compaction pattern**:
```fmpl
let error_summary = errors
  |> map(\e -> %{type: e.type, message: truncate(e.message, 100)})
  |> take(5)  -- Keep only recent errors

let retry_prompt = base_prompt + "\n\nPrevious errors (learn from these):\n" +
  format_errors(error_summary)
```

**Compaction detection triggers**:
```fmpl
-- Detect when errors are being repeated
let check = detect_circular(error_history)
if (check.detected)
  -- Compact error history before retry
  error_history = summarize_errors(error_history)
```

**Status**:
- `lib/compaction.fmpl` detects circular patterns
- Pattern matching for error classification

**Gaps**:
- [ ] Automatic error summarization
- [ ] Error budget tracking (max errors before escalation)
- [ ] Error categorization for smarter compaction

---

## Factor 10: Small, Focused Agents

> Design agents with narrow, specific responsibilities.

### FMPL Implementation

**Composable grammars**: Each agent is a focused grammar; compose via inheritance.

```fmpl
-- Small, focused agents
grammar CodeSearchAgent <: base::tree {
  query = search_request => execute_search;
}

grammar CodeEditAgent <: base::tree {
  edit = edit_request => apply_edit;
}

-- Compose into larger workflow
grammar CodingAgent <: base::tree {
  turn =
    | %{type: :search} => <- CodeSearchAgent.query
    | %{type: :edit} => <- CodeEditAgent.edit;
}
```

**Facets for agent capabilities**:
```fmpl
object Agent {
  .#facets
  searcher: [search]      -- Can only search
  editor: [edit]          -- Can only edit
  full: [search, edit]    -- Both capabilities
}
```

**Status**:
- Grammar inheritance for agent composition
- Facets restrict agent capabilities
- `spawn` creates isolated agent instances

**Gaps**:
- [ ] Agent registry for discovery
- [ ] Standard agent interfaces

---

## Factor 11: Trigger from Anywhere

> Enable agents to launch from diverse sources (webhooks, cron, user actions).

### FMPL Implementation

**HTTP triggers** (`fmpl-web`):
```fmpl
-- Axum route triggers agent
route("/agent/run", \req ->
  let task = req.body @ TaskParser.task
  <- spawn TaskAgent(task)
)
```

**Stream triggers**:
```fmpl
-- Tuple space subscription triggers agent
tuplespace.stream(%{type: :task, status: :pending})
  |> map(\task -> spawn TaskAgent(task))
```

**Timer triggers** (planned):
```fmpl
-- Cron-style scheduling
scheduler.every("0 * * * *", \_ -> spawn HealthCheckAgent())
```

**Status**:
- HTTP triggers via `fmpl-web` (Axum)
- Stream-based reactive triggers
- Async operator for event-driven activation

**Gaps**:
- [ ] Cron/scheduler integration
- [ ] Webhook endpoint helpers
- [ ] Event source connectors (file watch, etc.)

---

## Factor 12: Stateless Reducer

> Structure agents as pure functions that consume input and produce deterministic output.

### FMPL Implementation

**Grammars are pure**: Grammar rules are pure pattern-to-action mappings.

```fmpl
grammar PureAgent <: base::tree {
  -- Pure function: input -> output, no side effects in grammar
  process = input:i => transform(i);
}

-- Side effects isolated to semantic actions
let result = input @ PureAgent.process
-- Only after match: persist result
save(result)
```

**Immutable state updates** (Goblins `bcom` pattern):
```fmpl
object ^cell (bcom, val) {
  get(): val
  set(new): bcom(^cell(bcom, new))  -- Returns new cell, doesn't mutate
}
```

**Status**:
- Grammars are inherently pure (pattern matching)
- Semantic actions can be pure transformations
- Goblins-inspired `bcom` pattern designed (not implemented)

**Gaps**:
- [ ] Implement `bcom` for functional state updates
- [ ] Automatic transaction rollback on errors
- [ ] Effect isolation (pure core, effects at edges)

---

## RLM Integration

Recursive Language Model strategies complement the 12 factors:

### Peeking (Factor 3)

```fmpl
-- Examine structure before committing
let preview = take(context, 500)
let strategy = llm.decide_approach(preview)
```

### Grepping (Factor 3)

```fmpl
-- Regex narrowing before LLM processing
let relevant = context |> grep("error|warning|failed")
let analysis = llm.analyze(relevant)
```

### Partition + Map (Factor 10)

```fmpl
-- Spawn sub-agents for chunks
let chunks = partition(large_context, 4000)
let results = chunks |> map(\c -> <- spawn SummaryAgent(c))
let merged = merge_summaries(results)
```

### Summarization (Factor 9)

```fmpl
-- Compress for higher-level reasoning
let summary = llm.summarize(detailed_results)
let decision = llm.decide(summary)
```

---

## Implementation Priority

### Phase 1: Core Factors (Complete)
- [x] Factor 1: Grammar-based tool parsing
- [x] Factor 2: Prompt transparency
- [x] Factor 4: Tool schemas as data
- [x] Factor 8: Grammar-defined control flow
- [x] Factor 10: Composable focused agents (grammar inheritance)
- [x] Factor 12: Pure grammar rules
- [x] Tuple space (`tuplespace.new()`, `out`, `in`, `rd`, `subscribe`)
- [x] Tuple space facets (namespace, readonly, writeonly)

### Phase 2: Context Management (Implemented)

**Context compaction implemented** (Factor 3 + Factor 9):

The TUI now supports automatic detection and manual compaction:

1. **Off-track detection**: Detects groveling, apologizing, and condescending patterns
2. **Circular detection**: Identifies repeated short responses
3. **Compaction**: Truncates old messages while preserving recent context
4. **Undo support**: Original conversation preserved via compare_branch_id

**Usage in TUI**:
- Detection runs automatically after each LLM response
- Warning displayed when issues detected
- Press `Ctrl+C` to compact conversation
- `Ctrl+C` again on long conversations to force-compact

**Implementation**:
- Detection: Pure Rust in `check_compaction_needed()`
- Compaction: Pure Rust in `perform_compaction()`
- String methods added: `contains`, `starts_with`, `ends_with`, `slice`

**Remaining subtasks**:
- [ ] Factor 3: Token counting utilities (currently using chars/4 approximation)
- [ ] Factor 3: Context budget enforcement (max tokens per request)
- [ ] Factor 9: Error-specific compaction (summarize error chains)
- [ ] RLM: Partition + Map helpers for large contexts
- [ ] List methods: `filter`, `map`, `reduce` for FMPL-based compaction

### Phase 3: Human-in-the-Loop
- [ ] Factor 7: `human.approve()` builtin
- [ ] Factor 6: Full pause/resume API (ParseState exists, need VM state)
- [ ] Factor 7: Approval UI integration (TUI or web)

### Phase 4: Production Readiness
- [ ] Factor 5: `bcom` transaction rollback (Goblins pattern)
- [ ] Factor 11: Cron/webhook triggers
- [ ] Factor 6: Checkpoint/restore full VM state

---

---

## Quick Reference: 12 Factors Summary

From [unified-grammars-and-agents-design.md](../docs/plans/2026-01-19-unified-grammars-and-agents-design.md):

| Factor | FMPL Implementation |
|--------|---------------------|
| 1. NL → Tool Calls | Grammar parses intent → `%{tool: t, args: a}` |
| 2. Own Your Prompts | Grammars *are* prompts - you write the patterns |
| 3. Own Your Context Window | Predicates compute context: `&{ build_context(m) }` |
| 4. Tools Are Structured Outputs | `=> %{tool: name, args: args}` |
| 5. Unify Execution + Business State | Live image - everything is objects |
| 6. Launch/Pause/Resume | Continuations serialize to image |
| 7. Contact Humans with Tool Calls | `<- human.ask()` / `<- human.approve()` |
| 8. Own Your Control Flow | Grammar *is* control flow |
| 9. Compact Errors into Context | Predicates filter/summarize: `&{ compact_errors(ctx) }` |
| 10. Small Focused Agents | Grammar inheritance composes small grammars |
| 11. Trigger from Anywhere | Streams from HTTP, WS, tuple space, etc. |
| 12. Stateless Reducer | `bcom` pattern for functional state updates |

---

## Related Specs

- [grammar-system.md](./grammar-system.md) — OMeta-style grammars for tool parsing
- [pattern-matching.md](./pattern-matching.md) — Pattern matching for control flow
- [async-streams.md](./async-streams.md) — Async operations and streams
- [object-system.md](./object-system.md) — Goblins-inspired capabilities
- [lib.md](./lib.md) — Standard library (compaction detection, LLM clients)

---

## References

- [12-Factor Agents](https://github.com/humanlayer/12-factor-agents) — HumanLayer's principles
- [Recursive Language Models](https://alexzhang13.github.io/blog/2025/rlm/) — Context management strategies
- [Spritely Goblins](https://spritely.institute/goblins/) — Capability-based object model
