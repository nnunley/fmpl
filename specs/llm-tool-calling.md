# LLM Tool Calling with @ Operator

**Status**: Complete (v4)
**Author**: Spec Writer
**Date**: 2026-01-24
**Changes**:
- v2: Removed `execute()` builtin, use existing `call_builtin()` pattern with `__builtin_curl::get` syntax
- v3: Deferred AC-8/AC-9 (streaming) to Phase 2, Phase 1 scope is AC-1 through AC-7 (core tool calling only)
- v4: AC-7 (JSON parsing) implemented with both json::parse and json::stringify

---

## Implementation Status ✅ COMPLETE

All acceptance criteria AC-1 through AC-7 have been implemented:

- ✅ AC-1: Parse LLM Tool Call Responses - Uses `@` operator with pattern matching
- ✅ AC-2: Execute Tools via Built-ins - Leverages existing Symbol dispatch
- ✅ AC-3: Handle Tool Results - Pattern matching processes results
- ✅ AC-4: Multi-turn Tool Loop - Foundation established for agentic workflows
- ✅ AC-5: Error Handling - Built-in error handling for tool calls
- ✅ AC-6: Dynamic Tool Registry - Pattern matching enables flexible tool lookup
- ✅ AC-7: JSON Response Parsing - `json::parse()` and `json::stringify()` implemented

## Summary

Enable FMPL programs to parse LLM JSON responses, execute tool calls (curl, search, etc.), and feed results back to close the Research→Plan→Execute→Review agentic loop. The `@` operator pattern matching serves as the bridge between LLM text output and structured tool execution.

---

## Background

FMPL has pattern matching via the `@` operator (`specs/pattern-matching.md`), LLM integration via `curl.get/post`, and a streaming VM. What's missing is the **tool calling loop**: parsing LLM JSON responses into executable tool calls and feeding results back.

This spec implements the **agentic workflow** from `docs/plans/2026-01-19-unified-grammars-and-agents-design.md`:

```
Research → Plan → Execute → Review → Research → ...
    ↓        ↓        ↓        ↓
  LLM    LLM    Tools    LLM
          ↓              ↑
      @ operator    results
   pattern match
```

---

## Acceptance Criteria

### AC-1: Parse LLM Tool Call Responses

**Given** an LLM response containing a JSON tool call
**When** the response is applied to a pattern using `@`
**Then** the tool name and arguments are extracted into FMPL values

**Example**:
```fmpl
let llm_output = json::parse('{
  "tool": "curl.get",
  "args": {"url": "https://api.github.com/repos/anthropics/claude-code"}
}')

llm_output @ {
  %{tool: tool_name, args: tool_args} => {
    -- tool_name = "curl.get"
    -- tool_args = %{url: "https://..."}
  }
}
```

**Implementation**: Pattern matching uses existing `@` operator from `specs/pattern-matching.md`
JSON parsing uses `json::parse` builtin (see AC-7)

**Test**: T-1 - Extract tool name and args from LLM response

### AC-2: Execute Tools via Existing Built-ins

**Given** extracted tool name and arguments
**When** the tool is called using the existing Symbol dispatch mechanism
**Then** the corresponding FMPL builtin is executed with the arguments

**Example**:
```fmpl
-- Parse LLM response
let response = json::parse('{
  "tool": "curl.get",
  "args": {"url": "https://api.example.com/data"}
}')

-- Match pattern and dispatch to builtin
response @ {
  %{tool: tool_name, args: tool_args} => {
    -- tool_name = "curl.get"
    -- Convert to Symbol for method dispatch
    let builtin_symbol = __builtin_curl
    let method = "get"
    let args = [tool_args.url]

    -- Call via existing method dispatch
    builtin_symbol.(method)(args)
  }
}
```

**Alternative (direct Symbol call)**:
```fmpl
let tool_response = json::parse('{
  "tool": "curl.get",
  "args": {"url": "https://api.example.com/data"}
}')

tool_response @ {
  %{tool: "curl.get", args: %{url: url}} => {
    -- Direct call to __builtin_curl::get via Symbol
    __builtin_curl.get([url])
  }
  %{tool: "curl.post", args: %{url: url, body: body}} => {
    __builtin_curl.post([url, body])
  }
}
```

**Implementation**: Uses existing `call_builtin()` in `vm.rs:1025`
- Builtins are Symbols starting with `__builtin_`
- Method dispatch: `Symbol.(method_name)(args)` calls `call_builtin(object, method, args)`
- Existing builtins: `__builtin_curl::get`, `__builtin_curl::post` (see `vm.rs:1027-1050`)

**Test**: T-2 - Execute curl.get via Symbol method dispatch

### AC-3: Handle Tool Results

**Given** a tool execution result
**When** the result is returned
**Then** it can be fed back to the LLM for the next turn

**Example**:
```fmpl
-- Execute tool
let result = __builtin_curl.get(["https://api.example.com/data"])
-- result = %{status: 200, body: "{...}"}

-- Feed back to LLM via streaming
let next_prompt = "Tool result: " + result.body
let llm_response2 = <- llm.complete(next_prompt)
```

**Implementation**: Tool results return FMPL `Value` types (Map, String, Int, etc.)
- `curl.get` returns: `%{status: <int>, body: <string>}`
- Can be pattern-matched with `@` operator
- Passed to next LLM turn via `<-` async operator

**Test**: T-3 - Tool result is returned as FMPL Value

### AC-4: Multi-Turn Tool Calling Loop

**Given** an initial prompt
**When** the LLM responds with a tool call
**Then** execute the tool, feed result back, and continue until done

**Example**:
```fmpl
let agent = fn(prompt) {
  prompt @ {
    _ => {
      let response = <- llm.complete(prompt)
      let parsed = json::parse(response)

      parsed @ {
        %{tool: "curl.get", args: %{url: url}} => {
          let result = __builtin_curl.get([url])
          agent("Tool result: " + result.body)  -- recursive call
        }
        %{tool: "curl.post", args: %{url: url, body: body}} => {
          let result = __builtin_curl.post([url, body])
          agent("Tool result: " + result.body)
        }
        %{done: result} => result  -- terminal case
        %{text: t} => t  -- non-tool response
        _ => %{error: "unknown_response", data: parsed}
      }
    }
  }
}

agent("Search for recent Rust posts")
```

**Implementation**: Uses recursion with pattern matching
- Each turn: LLM → JSON parse → `@` pattern match → tool execution → recurse
- Terminates on `%{done: result}` pattern
- Errors fall through to `_` wildcard pattern

**Test**: T-4 - Multi-turn loop terminates on %{done: result}

### AC-5: Error Handling for Failed Tool Calls

**Given** a tool execution that fails (HTTP error, timeout, etc.)
**When** the error occurs
**Then** the error is returned as a structured value

**Example**:
```fmpl
-- Attempt to call invalid tool
let response = json::parse('{
  "tool": "curl.get",
  "args": {"url": "https://invalid.example"}
}')

response @ {
  %{tool: "curl.get", args: %{url: url}} => {
    __builtin_curl.get([url]) @ {
      %{error: err_type, message: msg} => {
        "Tool failed: " + msg
      }
      %{status: s, body: b} => {
        "Success: " + b
      }
    }
  }
}
```

**Implementation**: Errors are FMPL `Value::Map` with `error` and `message` fields
- HTTP errors return: `%{error: "http_error", status: <code>, message: <text>}`
- Network failures return: `%{error: "network_error", message: <text>}`
- Pattern match on result to handle both success and error cases

**Test**: T-5 - Tool execution errors return %{error: ..., message: ...}

### AC-6: Dynamic Tool Registry via Pattern Matching

**Given** a pattern with multiple tool branches
**When** an LLM response is matched
**Then** the correct branch executes the corresponding builtin

**Example**:
```fmpl
-- Pattern match serves as registry
let response = json::parse(llm_output)

response @ {
  %{tool: "curl.get", args: %{url: url}} => {
    __builtin_curl.get([url])
  }
  %{tool: "curl.post", args: %{url: url, body: body}} => {
    __builtin_curl.post([url, body])
  }
  %{tool: "search", args: %{query: q}} => {
    -- User-defined search function
    search_files(q)
  }
  %{tool: tool_name, args: a} => {
    -- Unknown tool
    %{error: "unknown_tool", name: tool_name}
  }
}
```

**Implementation**: Pattern matching `@` operator **IS** the registry
- Each pattern branch is a tool entry
- No separate registry data structure needed
- Leverage existing `@` operator from `specs/pattern-matching.md`
- User-defined tools coexist with builtins in the same pattern

**Test**: T-6 - Pattern matching dispatches to correct tool implementation

### AC-7: String to JSON Response Parsing ✅ IMPLEMENTED

**Given** raw LLM output as a string
**When** the string contains JSON
**Then** the JSON is parsed into FMPL values

**Example**:
```fmpl
let raw_response = "{\"tool\": \"curl.get\", \"args\": {\"url\": \"https://api.example.com\"}}"
let parsed = json::parse(raw_response)

-- Also supports serialization
let json_string = json::stringify(parsed)
```

**Implementation**:
- `json::parse()` converts JSON strings to FMPL values (Map, List, String, Int, Float, Bool, Null)
- `json::stringify()` converts FMPL values to JSON strings
- Error handling returns Map with `error` and `message` keys
- Both functions supported in compiler and VM

parsed @ {
  %{tool: "curl.get", args: %{url: url}} => {
    __builtin_curl.get([url])
  }
}
```

**Implementation**: `json::parse` builtin converts JSON strings to FMPL `Value` types:
- JSON objects → `Value::Map(HashMap<SmolStr, Value>)`
- JSON arrays → `Value::List(Vec<Value>)`
- JSON strings → `Value::String(SmolStr)`
- JSON numbers → `Value::Int(i64)` or `Value::Float(f64)`
- JSON booleans → `Value::Bool(bool)`
- JSON null → `Value::Null`

Uses existing `serde_json` dependency (already in `Cargo.toml`)

**Test**: T-7 - Parse JSON string to FMPL Value via json::parse

### AC-8: Streaming LLM Responses (DEFERRED to Phase 2)

**Given** an LLM streaming tokens via a StreamOp
**When** tokens arrive
**Then** accumulate and parse JSON when complete

**Status**: DEFERRED - `accumulate_json` StreamOp syntax undefined per `specs/async-streams.md`
**Phase 2**: Define `accumulate_json` StreamOp, implement token accumulation, add T-8 test

**Example**:
```fmpl
llm_stream |> accumulate_json() |> parse_response() |> handle_tool()
```

### AC-9: Tool Result Streaming (DEFERRED to Phase 2)

**Given** a tool that produces streaming output (e.g., `tail -f`)
**When** output arrives
**Then** stream chunks back to LLM or user

**Status**: DEFERRED - `stream::execute` syntax undefined per `specs/async-streams.md`
**Phase 2**: Define streaming tool execution protocol, add T-9 test

**Example**:
```fmpl
stream::execute("tail", %{"file": "/var/log/syslog"})
  |> stream_to_user()
```

### AC-10: Sandboxed Tool Execution (Future)

**Given** a tool execution request
**When** the tool is invoked
**Then** execute with capability-based security (cgroups, seccomp)

**Non-functional**: This is a placeholder for future capability security integration

**Test**: T-10 - Placeholder test for sandboxed execution

---

## Out of Scope

- [ ] Capability-based security (deferred to future spec)
- [ ] Human-in-the-loop approvals (deferred)
- [ ] Multi-user coordination (deferred)
- [ ] Tuple space for agent coordination (deferred)
- [ ] Durable pause/resume of agent loops (deferred)

---

## Migration Strategy

### Phase 1: Core Tool Calling ✅ COMPLETE
1. ✅ Implement `json::parse` builtin in `vm.rs:call_builtin()`
2. ✅ Add compiler support for `json::parse()` expressions
3. ✅ Add T-1 through T-7 tests (JSON parsing, pattern matching, curl integration)
4. ✅ No dispatcher needed - leverage existing pattern matching
5. ✅ **Scope**: AC-1 through AC-7 only (non-streaming tool calling)

### Phase 2: Streaming Support (Deferred)
1. Implement `accumulate_json` StreamOp in `fmpl-core/src/stream.rs`
2. Define streaming tool output protocol
3. Add T-8 through T-9 tests
4. **Prerequisites**: Define `accumulate_json` and `stream::execute` syntax per `specs/async-streams.md`

### Phase 3: Integration (1 week)
1. Build end-to-end agentic loop examples
2. Add documentation examples
3. Performance testing

---

## Implementation Notes

### Files to Modify

- **fmpl-core/src/vm.rs**: Add `json::parse` builtin instruction (LoadJsonParse or similar)
- **fmpl-core/src/compiler.rs**: Compile `json::parse()` expressions to VM instructions
- **fmpl-core/tests/tool_calling.rs**: New test file for T-1 through T-10
- **specs/vm.md**: Document `json::parse` builtin in builtins table

**No dispatcher needed**: Pattern matching `@` operator serves as the tool registry.

### Dependencies

Existing:
- `serde_json` for JSON parsing (already in `Cargo.toml`)
- `curl` builtins (`fmpl-core/src/builtins/curl.rs`) - already implemented
- StreamOps (`fmpl-core/src/stream.rs`) - already implemented
- Pattern matching `@` operator - already implemented

New:
- None required

### Error Conditions

| Condition | Return Value |
|-----------|--------------|
| Invalid JSON string | `%{error: "invalid_json", input: <string>}` |
| Unknown builtin tool | Pattern falls through to `_` wildcard branch |
| Missing required args | Builtin returns `%{error: "missing_args", expected: <fields>}` |
| Tool execution error | Builtin returns `%{error: "execution_failed", message: <reason>}` |

### Builtin Integration

Add to existing `call_builtin()` in `vm.rs:1025`:

```rust
fn call_builtin(&mut self, object: &str, method: &str, args: Vec<Value>) -> Result<Value> {
    match (object, method) {
        // Existing: curl, list, string methods...
        ("__builtin_json", "parse") => {
            let json_str = match args.first() {
                Some(Value::String(s)) => s.as_str(),
                _ => return Ok(Value::Map(Map::from([
                    ("error".into(), "invalid_args".into()),
                    ("message".into(), "json::parse requires string argument".into())
                ]))),
            };
            serde_json::from_str::<serde_json::Value>(json_str)
                .map(|v| convert_json_to_fmpl(v))
                .unwrap_or_else(|e| Value::Map(Map::from([
                    ("error".into(), "invalid_json".into()),
                    ("message".into(), e.to_string().into())
                ])))
        }
        // ... rest of existing builtins
    }
}
```

---

## Examples

### Example 1: Simple Tool Call

```fmpl
let prompt = "What's the latest Rust blog post?"

let response = <- llm.complete(prompt + " Respond with tool call only.")
let parsed = json::parse(response)

parsed @ {
  %{tool: "curl.get", args: %{url: url}} => {
    let result = __builtin_curl.get([url])
    "Content: " + result.body
  }
  %{text: t} => t
}
```

### Example 2: Research Agent Loop

```fmpl
let researcher = fn(question) {
  let response = <- llm.complete("Research: " + question)
  let parsed = json::parse(response)

  parsed @ {
    %{tool: "curl.get", args: %{url: url}} => {
      let result = __builtin_curl.get([url])
      researcher(question + "\n\nTool result: " + result.body)
    }
    %{answer: a} => a
    %{error: e} => "Research failed: " + e
  }
}

researcher("What's new in FMPL?")
```

### Example 3: Multi-Tool Agent

```fmpl
let agent = fn(task) {
  let plan = <- llm.complete("Plan: " + task)
  let parsed = json::parse(plan)

  parsed @ {
    %{tool: "curl.get", args: %{url: url}} => {
      let result = __builtin_curl.get([url])
      agent(task + "\n\nCompleted: curl.get\nResult: " + result.body)
    }
    %{tool: "curl.post", args: %{url: url, body: body}} => {
      let result = __builtin_curl.post([url, body])
      agent(task + "\n\nCompleted: curl.post\nResult: " + result.body)
    }
    %{tool: "search", args: %{query: q}} => {
      let result = grep_files(q)
      agent(task + "\n\nCompleted: search\nResult: " + result)
    }
    %{done: summary} => summary
  }
}

agent("Deploy FMPL to production")
```

---

## References

- Pattern matching spec: `specs/pattern-matching.md`
- Unified grammars design: `docs/plans/2026-01-19-unified-grammars-and-agents-design.md`
- 12-layer architecture: `docs/plans/12-layer-human-ai-architecture.md`
- curl builtins: `fmpl-core/src/builtins/curl.rs`
- StreamOps: `fmpl-core/src/stream.rs`

---

## Glossary

- **Tool**: A callable function (FMPL builtin or user-defined) that performs a specific action
- **Tool call**: A structured request `%{tool: <name>, args: <map>}` from an LLM
- **Dispatcher**: Logic that maps tool names to implementations
- **Agentic loop**: Research → Plan → Execute → Review cycle
- **@ operator**: Pattern matching and grammar application operator in FMPL
