# FMPL Tutorial for Experienced Programmers

A practical guide to FMPL ("of Accardi"), a prototype-based object-oriented programming language with pattern matching, grammar-based parsing, and agentic AI capabilities.

**Target Audience**: Experienced programmers who want to understand FMPL's syntax, semantics, and practical usage.

> **Important**: FMPL is a **purely functional language** with **immutable bindings**. All variables are immutable—once bound, they cannot be changed. Loops and iteration are implemented via recursion, not mutable counters.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Language Basics](#language-basics)
3. [Data Structures](#data-structures)
4. [Pattern Matching with `@`](#pattern-matching-with-)
5. [Grammars and Parsing](#grammars-and-parsing)
6. [Control Flow](#control-flow)
7. [Functions and Lambdas](#functions-and-lambdas)
8. [Objects and Methods](#objects-and-methods)
9. [Metaprogramming](#metaprogramming)
10. [Practical Examples](#practical-examples)
11. [Tool Calling and Agent Workflows](#tool-calling-and-agent-workflows)

---

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/ndn/fmpl.git
cd fmpl

# Run the REPL
cargo run -p fmpl-cli

# Or run the web UI
cargo run -p fmpl-web

# Or run the terminal UI (ratatui-based)
cargo run -p fmpl-tui
```

### Hello, World

```fmpl
"Hello, World!"   -- String literals evaluate to themselves
42                -- Numbers evaluate to themselves
true              -- Booleans evaluate to themselves
```

**FMPL is expression-oriented**: Every expression produces a value. Statements don't exist—only expressions.

---

## Language Basics

### Primitive Types

```fmpl
-- Numbers
42
3.14
-10

-- Strings
"Hello, World!"
"Line 1\nLine 2\tTabbed"   -- Escape sequences: \n \t \r \\ \" \' \0

-- Booleans
true
false

-- Null
null
```

### Comments

```fmpl
-- Single-line comments start with double dash

/*
   Multi-line comments
   are supported
*/
```

### Arithmetic and Logic

```fmpl
-- Arithmetic operators
1 + 2          -- 3
10 - 4         -- 6
3 * 4          -- 12
15 / 3         -- 5

-- Comparison operators
1 == 1         -- true
5 < 10         -- true
5 <= 5         -- true
10 > 5         -- true
10 >= 10       -- true
```

**Note**: Logical operators (`&&`, `||`) and exponentiation (`**`) are planned but not yet fully implemented in the current version.

---

## Data Structures

### Lists

```fmpl
-- List literals
[1, 2, 3]
["apple", "banana", "cherry"]
[1, "mixed", true]

-- Empty list
[]
```

**Note**: Lists are currently immutable data structures. Methods like `.length()` and `.get()` are planned. Use pattern matching with the `@` operator or recursive functions to process lists.

### Maps (Hash Tables)

```fmpl
-- Map literal
%{name: "Alice", age: 30, city: "NYC"}

-- Empty map
%{}

-- Map access
let person = %{name: "Bob", age: 25}
person.name              -- "Bob"
person.age               -- 25
```

### Objects

FMPL is **prototype-based** (not class-based). Objects are created with `object` expressions:

```fmpl
-- Basic object (must be named)
object counter {
  count: 0
  increment(): this.count + 1
  get(): this.count
}
```

**Note**: Objects must be named in the current implementation. Anonymous object literals and constructors (`^name`) are planned features.

---

## Pattern Matching with `@`

The `@` operator is FMPL's swiss-army knife for:

1. **Applying grammars** to parse text
2. **Matching patterns** against values
3. **Transforming data** via pattern-directed rules

### Basic Pattern Matching

```fmpl
-- Match strings against regex patterns
"hello" @ {
  [a-z]+ => "word"
}
-- Returns: "word" (matches [a-z]+)

"12345" @ {
  [0-9]+ => "number"
}
-- Returns: "number"
```

### Pattern Matching on Data Structures

**Note**: Map pattern matching (`%{tool: t} => ...`) and list pattern matching (`[x, y] => ...`) are planned features. Currently, pattern matching works best with regex patterns on strings.

For map access, use direct field access:
```fmpl
let response = %{
  tool: "curl.get",
  args: %{url: "https://example.com"}
}

response.tool
-- Returns: "curl.get"

response.args.url
-- Returns: "https://example.com"
```

---

## Grammars and Parsing

FMPL includes an **OMeta-style PEG grammar system** for parsing and transformation.

### Basic Grammar Rules

```fmpl
-- Define a grammar
let json_parser = grammar {
  value =
    | object
    | array
    | string
    | number
    | "true" => true
    | "false" => false
    | "null" => null

  number = [0-9]+ "."? [0-9]* => parse_float(matched)

  string = '"' ([^"]* | '\\"')* '"' => trim_quotes(matched)

  object = "{" pairs "}" => %{object: pairs}

  pairs =
    | string ":" value ("," string ":" value)* => build_map(matched)
    | => %{}  -- empty object
}
```

### Applying Grammars

```fmpl
-- Apply grammar rule to input
"12345" @ base::parser.integer   -- Returns: "12345"
"hello" @ base::parser.word      -- Returns: "hello"

-- Apply custom grammar
let result = "{\"name\": \"Alice\"}" @ json_parser.value
-- Returns: %{object: %{name: "Alice"}}
```

### Grammar Inheritance

```fmpl
-- Extend base grammar
let enhanced_json = json_parser <: {
  -- Override or add rules
  value = <super.value> | date | regex

  date = [0-9]{4}-[0-9]{2}-[0-9]{2} => parse_date(matched)
}

"2024-01-15" @ enhanced_json.value
-- Returns: parsed date object
```

---

## Control Flow

### Conditionals

```fmpl
-- if-then-else
if 15 > 10 then "big" else "small"
-- Returns: "big"

-- Nested with expressions
if 150 > 100 then
  "huge"
else if 15 > 10 then
  "big"
else
  "small"
-- Returns: "huge"

-- With let bindings
let (value = 42)
  if value > 10 then "big" else "small"
-- Returns: "big"
```

**Note**: FMPL uses `then`/`else` keywords (not braces).

### Loops and Recursion

**Important**: FMPL is a **purely functional language** with **immutable bindings**. There are no mutable variables or assignment statements. Loops are implemented via recursion.

```fmpl
-- Sum numbers recursively
let sum_range = {start, end}
  if start > end then
    0
  else
    start + sum_range(start + 1, end)

sum_range(1, 10)
-- Returns: 55

-- Factorial via recursion
let factorial = {n}
  if n <= 1 then
    1
  else
    n * factorial(n - 1)

factorial(5)
-- Returns: 120
```

The `while` and `do-while` syntax exists but requires careful implementation using recursive functions or streaming patterns, as immutable bindings prevent traditional loop counter patterns.

### Let-Bindings

```fmpl
-- let-in expression (scoped binding)
let (x = 42) x * 2
-- Returns: 84

-- Multiple bindings
let (x = 10, y = 20) x + y
-- Returns: 30

-- Statement-style let (binds to current scope)
let x = 42
let y = x * 2
y + 10
-- Returns: 94
```

---

## Functions and Lambdas

### Defining Functions

```fmpl
-- Named function
add(a, b): a + b

-- Function with multiple expressions
calculate(x, y): {
  let temp = x * 2
  temp + y
}

-- Call functions (after defining them)
add(1, 2)           -- 3
calculate(5, 3)     -- 13
```

**Note**: Functions must be defined before they're called in the current implementation.

### Lambdas (Anonymous Functions)

```fmpl
-- Lambda syntax
\x x + 1
\x \y x + y

-- Apply lambda immediately
(\x x * 2)(5)       -- 10

-- Store and call later
let doubler = \x x * 2
doubler(7)          -- 14
```

### Higher-Order Functions

```fmpl
-- Functions can take other functions as arguments
let apply_twice = {f, x}
  f(f(x))

let add_one = {x} x + 1
apply_twice(add_one, 5)
-- Returns: 7

-- List operations via recursion (map pattern)
let map_list = {f, list}
  -- Would use pattern matching on list structure here
  -- Full implementation requires list destructuring patterns
  list

-- Filter via recursion (filter pattern)
let filter_list = {pred, list}
  -- Would use pattern matching on list structure here
  -- Full implementation requires list destructuring patterns
  list
```

**Note**: Higher-order functions are supported, but list methods like `.map()` and `.filter()` require list destructuring patterns which are planned but not yet implemented.

---

## Objects and Methods

### Basic Object Usage

```fmpl
-- Define object
object counter {
  count: 0
  increment(): this.count + 1
  get(): this.count
}

-- Access methods via the object name
counter.get()
counter.increment()
```

### Special Variables

Objects have access to special variables (magical variables) in method context:

- `self` - Reference to current object (the receiver of the method call)
- `parent` - Reference to parent object (for prototype chain lookup)
- `caller` - Reference to the object that called this method
- `user` - Reference to the current user context
- `args` - The list of all arguments passed to the method

```fmpl
object {
  value: 42
  show(): "Value is: " + self.value
}
```

These variables are always available within method bodies, similar to `this` in JavaScript or `self` in Python/Smalltalk.

---

## Metaprogramming

FMPL supports **first-class AST and IR values**, enabling you to write compilers, DSLs, and code generators entirely in FMPL.

### Tagged Values (Constructor Values)

FMPL supports algebraic data types via tagged values:

```fmpl
-- Create tagged values (constructor syntax)
:Int(42)
:Binary(:+, :Int(1), :Int(2))
:User("alice", %{active: true})

-- Pattern match on tagged values
let value = :Binary(:+, :Int(1), :Int(2))
value @ {
  :Binary(:+, a, b) => "Addition: " ++ a ++ " + " ++ b
  :Binary(:-, a, b) => "Subtraction: " ++ a ++ " - " ++ b
  :Int(n) => "Just a number: " ++ n
}
-- Returns: "Addition: :Int(1) + :Int(2)"
```

**Note**: Operator symbols like `:+`, `:-`, `:*` can be used as tagged values and in patterns.

### Parsing Source to AST

Use `ast::parse` to convert FMPL source code into a tagged AST:

```fmpl
let ast = ast::parse("1 + 2")
-- Returns: :Binary(:+, :Int(1), :Int(2))

let ast2 = ast::parse("if true then 1 else 2")
-- Returns: :If(:Bool(true), :Int(1), :Int(2))

let ast3 = ast::parse("[1, 2, 3]")
-- Returns: :List([:Int(1), :Int(2), :Int(3)])
```

### Compiling IR to Bytecode

Use `ir::compile` to transform IR (intermediate representation) into executable bytecode:

```fmpl
-- Direct IR construction
let code = ir::compile(:Add(:LoadInt(1), :LoadInt(2)))
-- Returns: <code> (first-class bytecode)

-- With let bindings
let code2 = ir::compile(
  :Let(:x, :LoadInt(42),
    :Var(:x))
)
```

Supported IR nodes: `LoadNull`, `LoadBool`, `LoadInt`, `LoadFloat`, `LoadString`, `LoadVar`, `Var`, `Add`, `Sub`, `Mul`, `Div`, `Mod`, `Neg`, `Not`, `Eq`, `NotEq`, `Lt`, `Gt`, `LtEq`, `GtEq`, `Let`, `Seq`, `If`, `Return`, `MakeList`, `MakeTagged`.

### Executing Bytecode

Use `code::eval` to execute compiled bytecode:

```fmpl
let code = ir::compile(:Add(:LoadInt(1), :LoadInt(2)))
code::eval(code)
-- Returns: 3
```

### Full Metaprogramming Pipeline

Combine all three to write compilers in FMPL:

```fmpl
-- Parse source, transform AST to IR, compile, execute
let ast = ast::parse("1 + 2")

let ir = ast @ {
  :Binary(:+, :Int(a), :Int(b)) => :Add(:LoadInt(a), :LoadInt(b))
  :Binary(:-, :Int(a), :Int(b)) => :Sub(:LoadInt(a), :LoadInt(b))
  :Binary(:*, :Int(a), :Int(b)) => :Mul(:LoadInt(a), :LoadInt(b))
  :Binary(:/, :Int(a), :Int(b)) => :Div(:LoadInt(a), :LoadInt(b))
}

let code = ir::compile(ir)
code::eval(code)
-- Returns: 3
```

This enables:
- **Writing compilers** in FMPL (source → AST → IR → bytecode)
- **DSL implementations** (parse custom syntax, compile to FMPL bytecode)
- **Code transformation** (read code, modify it, write it back)
- **Macro systems** (syntactic abstraction via pattern matching)

---

## Practical Examples

### Example 1: JSON Parsing and Validation

```fmpl
-- Parse JSON string
let json_str = "{\"name\": \"Alice\", \"age\": 30}"

-- Use json::parse builtin
let parsed = json::parse(json_str)

-- Validate structure
parsed @ {
  %{name: n, age: a} when a >= 18 => "Adult: " + n
  %{name: n, age: a} => "Minor: " + n
  _ => "Invalid structure"
}
-- Returns: "Adult: Alice"
```

### Example 2: HTTP Requests with Tool Calling

```fmpl
-- Make HTTP GET request using curl builtin
let response = ::__builtin_curl.get("https://api.example.com/data")

-- Parse JSON response
let data = json::parse(response)

-- Extract specific fields
data @ {
  %{status: "ok", results: r} => r
  %{error: e} => "Error: " + e
  _ => "Unknown response"
}
```

### Example 3: Building a Simple Agent Loop

```fmpl
-- Define tool registry pattern
let handle_tool_call = {tool_response} {
  tool_response @ {
    -- Execute curl.get tool
    %{tool: "curl.get", args: %{url: u}} => {
      let result = ::__builtin_curl.get(u)
      "Result: " + result
    }

    -- Execute curl.post tool
    %{tool: "curl.post", args: %{url: u, body: b}} => {
      let result = ::__builtin_curl.post(u, b)
      "Result: " + result
    }

    -- No tool call, return text
    %{text: t} => t

    -- Unknown format
    _ => "Error: Unrecognized response"
  }
}

-- Simulate LLM response
let llm_output = "{\"tool\": \"curl.get\", \"args\": {\"url\": \"https://example.com\"}}"
let parsed = json::parse(llm_output)

-- Handle the tool call
handle_tool_call(parsed)
-- Returns: "Result: <HTTP response>"
```

---

## Tool Calling and Agent Workflows

FMPL is designed for **agentic AI workflows**—closing the loop between LLMs, tools, and human oversight.

### The Agent Loop

```fmpl
-- 1. User sends message
let user_message = "What's the weather in NYC?"

-- 2. LLM responds (simulated here with string)
let llm_response = %{
  tool: "curl.get",
  args: %{url: "https://wttr.in/NYC?format=j1"}
}

-- 3. Parse and execute tool call
llm_response @ {
  %{tool: t, args: a} => {
    let result = ::__builtin_curl.get(a.url)

    -- 4. Feed result back to LLM (next turn)
    "Weather data: " + result
  }
}
```

### Multi-Turn Tool Calling

```fmpl
-- Agent that processes multi-turn conversations
let agent_turn = {input, history} {
  let context = %{history: history, input: input}

  -- Simulate LLM decision-making
  let llm_output = ::llm_complete(context)

  -- Handle tool calls or return final answer
  llm_output @ {
    %{tool: t, args: a} => {
      let result = execute_tool(t, a)

      -- Recurse with tool result in history
      agent_turn(
        "Tool result: " + result,
        history + [llm_output, result]
      )
    }

    %{answer: a} => a  -- Final answer
    _ => "Error: Unexpected LLM output"
  }
}

-- Usage
let result = agent_turn("Search for latest Rust version", [])
```

---

## Advanced Topics

### Grammar-Based Agents

FMPL's unique feature: **express agent control flow as grammars**.

```fmpl
grammar ToolAgent <: base::tree {
  -- Main loop: process messages
  turn = message:m => {
    let ctx = %{history: get_history()}
    ::llm_complete(m, ctx) |> tool_output
  }

  -- Handle LLM output stream
  tool_output =
    | %{tool: t, args: a} => {
        let result = ::execute_tool(t, a)
        turn(result)  -- recurse with result
      }
    | %{done: r} => r  -- terminate
    | %{text: t} => t  -- stream text
}
```

This approach enables:
- **Declarative control flow** (grammar rules define behavior)
- **Natural backtracking** (try alternatives, retry on failure)
- **Inspectable state** (rules are data, not code)
- **Composition via inheritance** (share and override rules)

### Persistence and Durable Suspension

FMPL supports **durable state** via Fjall (embedded key-value store):

```fmpl
-- Checkpoint saves continuation
checkpoint("stage_name", data)

-- Resume from saved state
resume_from(saved_checkpoint)
```

This enables:
- Pause-and-resume workflows
- Human-in-the-loop approvals
- Crash recovery
- Long-running agent processes

---

## Further Reading

### Core Specifications

- **[vm.md](specs/vm.md)** - Virtual machine architecture, bytecode format, execution model
- **[grammar-system.md](specs/grammar-system.md)** - OMeta-style grammar parsing engine with streaming support
- **[object-system.md](specs/object-system.md)** - Prototype-based objects, capabilities, facets
- **[pattern-matching.md](specs/pattern-matching.md)** - Pattern matching syntax and semantics

### Implementation Guides

- **[llm-tool-calling.md](specs/llm-tool-calling.md)** - Tool calling integration patterns
- **[indexed-rpn-conversion.md](specs/indexed-rpn-conversion.md)** - Bytecode VM design
- **[12-layer-human-ai-architecture.md](docs/plans/12-layer-human-ai-architecture.md)** - System architecture

### Design Documents

- **[unified-grammars-and-agents-design.md](docs/plans/2026-01-19-unified-grammars-and-agents-design.md)** - Grammar-based agents
- **[language-guide.md](docs/design/language-guide.md)** - Language syntax overview (more aspirational)

---

## Running the Examples

### Using the CLI REPL

```bash
cargo run -p fmpl-cli
```

```
fmpl> 1 + 2
3
fmpl> "hello" @ { [a-z]+ => "word" }
"hello"
fmpl> let x = 42
fmpl> x * 2
84
```

### Using the Web UI

```bash
cargo run -p fmpl-web
# Visit http://localhost:3000
```

### Using the Terminal UI (TUI)

```bash
cargo run -p fmpl-tui
# Ratatui-based interface with panels for:
# - Research (problem space analysis)
# - Planning (collaborative scope definition)
# - Code Editor (FMPL code)
# - Execution Output (results)
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test -p fmpl-core tool_calling
cargo test -p fmpl-core apply_operator

# Run with output
cargo test -- --nocapture
```

---

## Status and Limitations

**Currently Implemented** (as of 2026-01-28):
- ✅ Core parser and lexer (EBNF grammar)
- ✅ Expression evaluation (arithmetic: `+`, `-`, `*`, `/`)
- ✅ Comparisons (`==`, `<`, `>`, `<=`, `>=`, `!=`)
- ✅ Pattern matching with `@` operator (regex, wildcard, and tagged value patterns)
- ✅ Grammar system (OMeta-style PEG)
- ✅ Named object definitions and method calls
- ✅ Named functions and lambdas
- ✅ If-then-else conditionals
- ✅ Let-bindings (statement and expression forms)
- ✅ Lists (`[]`) and maps (`%{}`)
- ✅ Tagged values (constructor syntax: `:Tag(args...)`)
- ✅ Operator symbol literals (`:+`, `:-`, `:*`, etc.)
- ✅ Metaprogramming (`ast::parse`, `ir::compile`, `code::eval`)
- ✅ JSON parsing (`json::parse` builtin)
- ✅ HTTP tools (`::__builtin_curl.get/post`)
- ✅ Indexed RPN bytecode VM
- ✅ Streaming grammar support (push model)
- ✅ Persistence (Fjall-backed)

**Partially Implemented**:
- ⚠️ Logical operators (`&&`, `||`) - syntax exists, returns `null` instead of boolean
- ⚠️ Async operators (`<-`, `spawn`, `|>`) - syntax exists, runtime in progress
- ⚠️ Object constructors (`^name`) - syntax designed, implementation evolving
- ⚠️ Pattern matching on maps/lists - spec complete, implementation pending

**Not Yet Implemented**:
- ❌ Exponentiation (`**`)
- ❌ Logical NOT (`!`)
- ❌ Inequality operator (`!=`)
- ❌ Anonymous object literals
- ❌ Map/list pattern matching in `@` blocks
- ❌ Tuple space coordination (Linda-style)
- ❌ Multi-user vat isolation
- ❌ Full async/await runtime
- ❌ Capability security policies
- ❌ Human-in-the-loop approvals (durable)

---

## Contributing

FMPL is a research project exploring **grammar-based agent control flow** and **prototype-based object-oriented programming** for agentic AI systems.

Areas of active development:
1. Async stream processing
2. Tool calling standard library
3. Object capabilities and facets
4. Persistence and checkpointing
5. TUI/CLI enhancements

See `docs/plans/` for implementation roadmap.

---

## License

MIT License - See [LICENSE](LICENSE) for details.

---

**Last Updated**: 2026-01-23
**Version**: 0.1.0 (experimental)
