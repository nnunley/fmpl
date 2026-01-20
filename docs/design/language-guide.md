# Language Guide

A streaming-first DSL for building AI agents with grammars, capabilities, and durable state.

---

## Core Concepts at a Glance

```fmpl
-- Objects with constructors and capabilities
object ^agent (bcom, state) {
  .#private
  state: state

  .#public
  process(msg): <- llm_complete(msg) |> self.parse_output

  .#facets
  viewer: [status]    -- restricted view
}

-- Spawn, call sync/async, pipe streams
let bot = spawn ^agent(%{history: []})
$ bot.status()                          -- sync call
<- bot.process("hello") |> handler      -- async + pipe
```

---

## 1. Objects and Capabilities

### Constructors and spawn

```fmpl
object ^cell (bcom, val) {
  get(): val
  set(new): bcom(^cell(bcom, new))      -- functional update
}

let c = spawn ^cell(42)
$ c.get()                               -- 42
$ c.set(100)                            -- returns new cell
```

- `^name` is a constructor
- `spawn ^constructor(args)` creates instances
- `bcom` enables immutable-style state updates (become pattern)

### Visibility Markers

```fmpl
object foo {
  .#private     -- internal only
  secret: 42

  .#public      -- callable by others
  greet(): "hi"

  .#facets      -- restricted views
  viewer: [greet]
  viewer!: [greet]   -- terminal (non-delegatable)
}
```

### Facets (Capabilities)

```fmpl
treasury.as(:auditor).view_balance()    -- allowed
treasury.as(:auditor).withdraw(100)     -- denied: not on facet
```

---

## 2. Sync vs Async

```fmpl
$ obj.method()      -- synchronous, same-vat
<- obj.method()     -- asynchronous, returns stream
```

Async calls return **streams** that can be piped and parsed.

---

## 3. Maps and Data

```fmpl
%{key: val, other: 42}           -- map literal
%{get_key() => computed}         -- computed key
%{}                              -- empty map

map | %{extra: 1}                -- merge (right wins)
map.key                          -- access
```

---

## 4. Pipe Operator

Chain transformations left-to-right:

```fmpl
input |> parse() |> validate() |> save()

-- Equivalent to:
save(validate(parse(input)))
```

Streams flow through pipes naturally:

```fmpl
llm_stream |> parser.tool_call |> execute_tool |> results
```

---

## 5. Pattern Matching (`@`)

Apply grammars or match patterns:

```fmpl
-- Grammar application
"take sword" @ mud::parser.command

-- Pattern matching on values
result @ {
  %{tool: t, args: a} => execute(t, a)
  %{done: r}          => r
  %{error: e}         => handle(e)
}

-- List destructuring
[head | tail] => process(head, tail)
```

---

## 6. Grammars (OMeta-Style)

Declarative parsing with inheritance:

```fmpl
grammar ToolParser <: base::tree {
  -- Parse tool calls from LLM output
  output =
    | "TOOL:" name:word "(" args:json ")" => %{tool: name, args: args}
    | chunk+                               => %{text: chunks}

  -- Semantic predicate: run code mid-match
  command = verb:v &{ valid_verb(v) } noun:n => %{v: v, n: n}
}

-- Apply to stream
llm_stream |> ToolParser.output |> handler
```

### Key Grammar Features

- **Inheritance**: `grammar Child <: Parent { ... }`
- **Alternatives**: `|` separates cases
- **Binding**: `name:pattern` binds match to variable
- **Semantic predicates**: `&{ code }` runs code, must return truthy
- **Super calls**: `<super.rule>` invokes parent rule
- **Negation**: `!pattern` succeeds if pattern fails

---

## 7. Async Streams

```fmpl
-- Create stream from async source
let stream = <- http.get(url)

-- Transform streams
stream
  |> map(|chunk| parse(chunk))
  |> filter(|x| x.valid)
  |> collect

-- Await all parallel results
tasks |> map(|t| spawn(process, t)) |> await_all
```

---

## 8. Agent as Grammar

Agent control flow expressed as grammar rules:

```fmpl
grammar TaskAgent <: base::tree {
  -- Main loop: process messages
  turn = message:m => <- llm_complete(m) |> tool_output

  -- Handle LLM output stream
  tool_output =
    | %{tool: t, args: a} => <- execute(t, a) |> turn   -- recurse
    | %{done: result}     => result                      -- done
    | %{text: t}          => emit(t); <tool_output>      -- stream text

  -- Human approval gate
  tool_output =
    | %{tool: t} &{ needs_approval(t) } => {
        let decision = <- human.approve(t)
        decision @ { %{approved: true} => ... }
      }
    | <super.tool_output>
}
```

**Why grammars for agents:**
- Declarative pattern matching over message streams
- Natural backtracking and retry semantics
- Composable via inheritance
- Inspectable (rules are data)

---

## 9. Durable Suspension

Agents can pause and resume across restarts:

```fmpl
-- Checkpoint saves continuation to Fjall
checkpoint("stage_name", data)

-- Human approval suspends until response
let decision = <- human.approve(request)   -- durable wait

-- Resume from saved state
resume_from(saved_checkpoint)
```

---

## 10. Currying and Partials

```fmpl
add(a, b, c): a + b + c

add(1)(2)(3)        -- 6
add(1, 2)(3)        -- 6
add(_, 5, _)        -- partial: \a \c add(a, 5, c)
```

---

## Quick Reference

| Syntax | Meaning |
|--------|---------|
| `spawn ^ctor(args)` | Create instance |
| `$ obj.method()` | Sync call |
| `<- obj.method()` | Async call (returns stream) |
| `a \|> b \|> c` | Pipe chain |
| `x @ grammar.rule` | Apply grammar |
| `x @ { pat => expr }` | Pattern match |
| `%{k: v}` | Map literal |
| `&{ code }` | Semantic predicate |
| `\x expr` | Lambda |
| `obj.as(:facet)` | Get restricted view |
| `.#private/.#public/.#facets` | Visibility markers |
| `bcom(^ctor(...))` | Functional state update |
| `grammar G <: P { }` | Grammar with inheritance |

---

## Putting It Together

A complete tool-calling agent:

```fmpl
grammar CodeAgent <: ToolAgent {
  -- Build context before LLM call
  turn = message:m => {
    let ctx = %{
      history: last_messages(10),
      codebase: <- search_code(m.text)
    }
    <- llm_complete(m, context: ctx) |> tool_output
  }

  -- Gate dangerous operations
  tool_output =
    | %{tool: t} &{ t in [:delete, :deploy] } => {
        <- human.approve(%{tool: t, reason: "Dangerous operation"})
          |> approval_handler
      }
    | <super.tool_output>

  approval_handler =
    | %{approved: true}  => <super.tool_output>
    | %{denied: reason}  => %{error: "Denied: " + reason}
}

-- Run: pipe user input through agent
user_messages |> CodeAgent.turn |> responses
```

This agent:
1. Builds focused context (history + code search)
2. Streams LLM output through grammar parser
3. Extracts tool calls, executes them
4. Gates dangerous tools behind human approval
5. Recursively processes tool results
6. All state durable via Fjall persistence
