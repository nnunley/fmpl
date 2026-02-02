# Standard Library

The `lib/` directory contains FMPL standard library modules that provide common functionality for building AI agents, working with LLM APIs, and managing agent state.

## Modules

| Module | Purpose |
|--------|---------|
| [llm-common.fmpl](#llm-commonfmpl) | Shared utilities for LLM integration (response parsing, agentic loops, tool calling) |
| [anthropic.fmpl](#anthropicfmpl) | Anthropic Claude API client (chat completion, streaming, multi-turn conversations) |
| [ollama.fmpl](#ollamafmpl) | Ollama local LLM client (localhost API, model management, streaming) |
| [compaction.fmpl](#compactionfmpl) | Layer 2 auto-detection for off-track and circular conversation patterns (core logic in Rust) |
| [rand](#rand-built-in) | Random number generation for jitter and stochastic behaviors |

## llm-common.fmpl

**Purpose**: Shared utilities for working with LLM APIs in FMPL.

**Exports**:

- `llm.extract_text(response)` — Extract text content from common LLM response formats (OpenAI/Ollama/Anthropic)
- `llm.is_error(response)` — Check if response is an error
- `llm.agent_loop(task, chat_fn)` — Research → Plan → Execute → Review loop (12-factor agentic workflow)
- `llm.multi_turn(initial_prompt, turns)` — Multi-turn conversation with context accumulation
- `llm.parse_tool_call(response_text)` — Parse tool call from LLM response (expects `{"tool": "...", "args": {...}}`)
- `llm.execute_tool(tool_call)` — Execute a tool call (placeholder, needs tool registry)
- `llm.tool_loop(prompt, chat_fn, max_iterations)` — Tool calling loop: LLM → Tool → Result → LLM
- `llm.collect_stream(stream)` — Collect stream chunks into a single string
- `llm.parse_sse(stream)` — Parse Server-Sent Events (implemented via `sse.parse()` builtin)
- `llm.safe_chat(chat_fn, prompt)` — Wrap a chat call with error handling
- `llm.retry_chat(chat_fn, prompt, max_retries)` — Retry with exponential backoff and jitter (using `rand::int()` for jitter)

**Usage**:

```fmpl
io.load("lib/llm-common.fmpl")

# Extract text from any LLM response
let text = llm.extract_text(ollama_response)

# Run agentic loop
let result = llm.agent_loop("Build a web scraper", ollama.chat)

# Tool calling loop
let final = llm.tool_loop("Check the weather", ollama.chat, 5)
```

## anthropic.fmpl

**Purpose**: Anthropic Claude API client for chat completion and streaming.

**Configuration**:

- `anthropic.endpoint` — API endpoint (default: `https://api.anthropic.com/v1/messages`)
- `anthropic.model` — Model name (default: `claude-sonnet-4-20250514`)
- `anthropic.version` — API version header (default: `2023-06-01`)

**Exports**:

- `anthropic.get_api_key()` — Get API key from `ANTHROPIC_API_KEY` environment variable
- `anthropic.chat(prompt)` — Send a chat completion request, returns response text
- `anthropic.parse_response(json_text)` — Parse Claude's JSON response to extract text
- `anthropic.chat_history(messages)` — Multi-turn conversation with message history (alias: `chat_with_history`)
- `anthropic.chat_stream(prompt)` — Stream completion with SSE parsing
- `anthropic.extract_deltas(events)` — Extract and concatenate all `delta.text` fields from SSE events

**Usage**:

```fmpl
io.load("lib/anthropic.fmpl")

# Simple chat
let response = anthropic.chat("What is 2+2?")
print(response)

# Multi-turn conversation
let messages = [
  %{role: "user", content: "Hello"},
  %{role: "assistant", content: "Hi there!"},
  %{role: "user", content: "How are you?"}
]
let response = anthropic.chat_history(messages)

# Streaming completion
let stream = anthropic.chat_stream("Tell me a story")
```

## ollama.fmpl

**Purpose**: Ollama local LLM client for running models locally.

**Configuration**:

- `ollama.endpoint` — API endpoint (default: `http://localhost:11434`)
- `ollama.model` — Model name (default: `llama3.2`)

**Exports**:

- `ollama.chat(prompt)` — Send a chat completion request, returns response text
- `ollama.parse_response(json_text)` — Parse Ollama's JSON response (`{"response": "...", "done": true}`)
- `ollama.chat_stream(prompt)` — Stream completion with SSE parsing
- `ollama.extract_responses(events)` — Extract and concatenate all `response` fields from SSE events
- `ollama.chat_with_history(messages)` — Multi-turn conversation with message history
- `ollama.build_context(messages)` — Build conversation context from messages list
- `ollama.list_models()` — List available models
- `ollama.show_model(model_name)` — Show model info

**Usage**:

```fmpl
io.load("lib/ollama.fmpl")

# Simple chat
let response = ollama.chat("What is 2+2?")
print(response)

# Multi-turn conversation
let messages = [
  %{role: "user", content: "Hello"},
  %{role: "assistant", content: "Hi there!"},
  %{role: "user", content: "How are you?"}
]
let response = ollama.chat_with_history(messages)

# List available models
let models = ollama.list_models()
print(models)
```

## compaction.fmpl

**Purpose**: Layer 2 auto-detection and context compaction for identifying when an agent conversation has gone off-track or entered a circular pattern, and actually compacting the context to reduce token usage.

**Exports**:

### Detection Functions

- `detect_off_track(msg)` — Detect off-track patterns (groveling, apologizing, condescending language)
  - Returns: `%{detected: bool, pattern: string, confidence: float, message: string}`
- `detect_circular(history)` — Detect repeated similar responses in conversation history
  - Returns: `%{detected: bool, cycle_count: int, message: string}`
- `should_compact(history, last_response)` — Combined detection check
  - Returns: `%{should_compact: bool, reason: string, confidence: float, detection_type: string}`

### Compaction Functions

- `compact_history(history, options)` — Main compaction function with full control
  - `history`: Array of `%{role: string, content: string}` messages
  - `options`: `%{keep_recent: int, max_tokens: int, summarize_old: bool, compact_tools: bool, remove_circular: bool}`
  - Returns: `%{history: [...], original_tokens: int, compacted_tokens: int, messages_removed: int, messages_summarized: int, savings_percent: int}`
- `quick_compact(history)` — Simplified compaction with sensible defaults
  - Returns: Compacted history array
- `summarize_message(msg, max_chars)` — Truncate a single message
- `compact_tool_output(msg)` — Compress verbose tool output
- `remove_circular(history)` — Remove duplicate/near-duplicate messages

### Token Estimation

- `estimate_tokens(text)` — Estimate token count for a string (~4 chars per token)
- `estimate_history_tokens(history)` — Estimate total tokens in conversation

### Utility Functions

- `get_assistant_msgs(history)` — Filter messages to assistant responses only
- `get_user_msgs(history)` — Filter messages to user messages only
- `summarize_message(msg, max_chars)` — Truncate a single message content
- `compact_tool_output(msg)` — Compress verbose tool output (code blocks > 500 chars)

**Usage**:

```fmpl
io.load("lib/compaction.fmpl")

# Check if conversation should be compacted
let check = should_compact(conversation_history, last_response)

if (check.should_compact)
  print("Compaction needed: " + check.reason)

  # Perform compaction
  let result = compact_history(conversation_history, %{
    keep_recent: 5,      # Keep last 5 messages intact
    max_tokens: 4000,    # Target token budget
    summarize_old: true, # Summarize old messages
    compact_tools: true  # Compress tool outputs
  })

  print("Saved " + result.savings_percent + "% tokens")
  print("Removed " + result.messages_removed + " messages")

  # Use compacted history for next LLM call
  let new_history = result.history
else
  print("Conversation healthy, confidence: " + check.confidence)

# Quick compaction with defaults
let compacted = quick_compact(conversation_history)

# Detect off-track patterns
let off_track = detect_off_track("You're absolutely right, I apologize...")
if (off_track.detected)
  print("Agent lost original goal: " + off_track.pattern)
```

## rand Built-in

**Purpose**: Random number generation for implementing jitter, randomized testing, and stochastic behaviors.

**Methods**:

- `rand::int(min, max)` — Generate random integer in range [min, max)
  - `min`: Minimum value (inclusive, integer)
  - `max`: Maximum value (exclusive, integer)
  - Returns: Random integer where min <= result < max
  - Note: Returns error if min >= max (empty range)

- `rand::float()` — Generate random float in range [0.0, 1.0)
  - Returns: Random float where 0.0 <= result < 1.0

**Usage**:

```fmpl
-- Random integer between 0 and 99
let n = rand::int(0, 100)

-- Random float
let f = rand::float()

-- Jitter for retry backoff: 100ms +/- 50ms
let base_delay = 100
let jitter = rand::int(0, 50)
time::sleep(base_delay + jitter)

-- Exponential backoff with jitter (as used in llm.retry_chat)
let backoff_ms = \attempt
  let base = 2 ^ attempt * 100
  let jitter = rand::int(0, base / 2)  -- up to 50% of base
  base + jitter
```

## Design Notes

### Agentic Loop Pattern

The `llm.agent_loop` function implements the core 12-factor agentic workflow:

1. **Research** — Understand the problem and gather context
2. **Plan** — Create a step-by-step execution plan
3. **Execute** — Run the plan (currently placeholder, needs tool execution)
4. **Review** — Evaluate the result and suggest improvements

### Tool Calling Workflow

The `llm.tool_loop` function implements the standard tool calling pattern:

1. LLM receives prompt and decides whether to call a tool
2. If tool call detected, execute tool and capture result
3. Feed result back to LLM for final response
4. Repeat until max_iterations reached or no tool call

### Compaction Detection

The `compaction.fmpl` module provides two detection strategies:

1. **Off-Track Detection** — Pattern-based detection of language indicating the agent has lost its original goal (groveling, excessive apologizing, condescending language)
2. **Circular Conversation Detection** — Detects repeated similar responses in conversation history (short, similar responses from assistant)

These are used by Layer 2 agent systems to automatically compact conversation context when it becomes inefficient.

## Future Work

- [x] ~~Implement `llm.retry_chat` with exponential backoff~~ — **DONE**: `rand::int()` and `time::sleep()` builtins implemented, `retry_chat` available with exponential backoff and jitter (note: full recursive version requires parser improvements for nested lambda pattern matching)
- [x] ~~Implement `llm.parse_sse` for streaming response parsing~~ — **DONE**: `sse.parse()` builtin implemented in `fmpl-core/src/builtins/sse.rs`
- [ ] Create tool registry for `llm.execute_tool` dispatch
- [ ] Add more sophisticated similarity metrics for `compaction.fmpl`
- [ ] Add OpenAI API client module
- [ ] Add generic embedding/API client utilities

## Implementation Notes

### time::sleep() Builtin

The `time::sleep(ms)` builtin is now available for implementing retry logic and rate limiting:

```fmpl
-- Sleep for 100 milliseconds
time::sleep(100)

-- Returns null after completion
let result = time::sleep(0)  -- result is null
```

**Usage in retry logic:**

```fmpl
let result = chat_fn(prompt)
result @ {
  %{error: _} =>
    time::sleep(1000)  -- Wait 1 second before retry
    -- ... retry logic
  response => response
}
```

**Limitations:** Due to current parser limitations with pattern matching in nested lambdas, the full recursive `llm.retry_chat` implementation is provided as a simplified single-retry version. For full retry loops with exponential backoff, inline the retry pattern directly in your code as documented in `lib/llm-common.fmpl`.
