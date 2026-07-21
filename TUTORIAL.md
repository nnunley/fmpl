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
git clone https://github.com/nnunley/fmpl.git
cd fmpl

# Build with the canonical FMPL-generated parser (two-step bootstrap)
just build

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
1 != 2         -- true
5 < 10         -- true
5 <= 5         -- true
10 > 5         -- true
10 >= 10       -- true

-- Logical operators
true && false  -- false
true || false  -- true
!true          -- false
```

**Note**: Exponentiation (`**`) is planned but not yet implemented. String
concatenation uses `+` and requires both operands to be strings — a mixed
`"n = " + 42` is a type mismatch and evaluates to `null`.

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

**Note**: Lists are immutable data structures. Higher-order methods work —
`[1, 2, 3].map(\x x * 2)` returns `[2, 4, 6]`, and `.fold()` is available.
Index accessors like `.length()` and `.get()` are planned; use pattern
matching with the `@` operator or recursive functions for element access.

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
  increment(): self.count + 1
  value(): self.count
}
```

**Note**: Objects must be named in the current implementation. Use `self` to
reference the receiver inside methods — there is no `this`. Anonymous object
literals and constructors (`^name`) are planned features.

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

**Map Pattern Matching** extracts values via bindings. Arms are separated
with `;`, and the OMeta-style binding syntax is `_:name`:

```fmpl
-- Extract values using bindings, with a wildcard fallback arm
let response = %{status: 200, body: "ok"}

response @ { %{status: _:code, body: _:msg} => msg; _ => "other" }
-- Returns: "ok"

-- Nested map patterns work too
%{outer: %{inner: "value"}} @ { %{outer: %{inner: _:i}} => i }
-- Returns: "value"
```

**Guards**: To match on specific values, bind and guard with `when` (or its
alias `if`):

```fmpl
%{status: 200, body: "ok"} @ {
  %{status: _:s} when s == 200 => "success";
  %{status: _:s} => "failed"
}
-- Returns: "success"

%{code: 404} @ { %{code: _:c} when c == 404 => "not_found"; _ => "found" }
-- Returns: "not_found"
```

**Note**: Literal values directly inside map patterns (`%{status: 200} => ...`)
are not yet supported — the compiler rejects them. Use a binding plus a guard,
as above.

**List Pattern Matching** is also supported:

```fmpl
-- Match a list and extract elements
[1, 2, 3] @ { [ _:x, _:y, _:z ] => [x, y, z] }
-- Returns: [1, 2, 3]

-- Length must match: this arm does not match a 3-element list
[1, 2, 3] @ { [ _:x, _:y ] => "two"; _ => "not two" }
-- Returns: "not two"

-- Empty list pattern
[] @ { [] => "empty" }
-- Returns: "empty"
```

**Note**: Rest patterns (`[first | rest]`) are planned but not yet
implemented. In the REPL, write `@ { ... }` match blocks on a single line
with `;` between arms — multi-line `@ {` blocks are routed to the grammar
engine. Multi-line works fine with the `match` keyword form:

```fmpl
match 5 { n if n > 3 => "big"; _ => "small" }
-- Returns: "big"
```

For direct map field access, you can still use:
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

Grammar rules match input and run semantic actions. Capture matched text
with a `:binding` suffix on a pattern element:

```fmpl
-- Define a grammar: capture the digits, return them from the action
let g = grammar { num = [0-9]+:d => d }

"42" @ g.num
-- Returns: "42"

-- Actions are arbitrary expressions
let shout = grammar { word = [a-z]+:w => w + "!" }
"hello" @ shout.word
-- Returns: "hello!"
```

A full JSON parser written this way ships with the repo — see
`lib/json.fmpl`. The metacircular FMPL parser itself
(`lib/core/fmpl_parser.fmpl`) is the largest grammar in the tree.

### Applying Grammars

```fmpl
-- Apply built-in base grammar rules to input
"12345" @ base::parser.integer   -- Returns: "12345"
"hello" @ base::parser.word      -- Returns: "hello"
```

### Grammar Inheritance

Grammar inheritance (`<:` with `<super.rule>` overrides) is a designed
feature that is deliberately deferred — see DESIGN-005 in
`docs/design-principles.md`. Compose grammars by referencing shared rules
for now.

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
-- Sum numbers recursively (lambda bound at top level)
let sum_range = \start, end if start > end then 0 else start + sum_range(start + 1, end)

sum_range(1, 10)
-- Returns: 55

-- Factorial via recursion
let factorial = \n if n <= 1 then 1 else n * factorial(n - 1)

factorial(5)
-- Returns: 120
```

**Note**: Recursion works through top-level (statement-style) `let` bindings —
the lambda body resolves the name at call time. Scoped `let (f = ...)`
expression bindings cannot see themselves recursively yet (recursive let is a
known limitation; see `docs/known-gaps.md`).

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

Functions are lambdas bound to names with `let`:

```fmpl
-- Bind a lambda to a name
let add = \a, b a + b
add(1, 2)           -- 3

-- The lambda keyword form is equivalent
let inc = lambda (n) n + 1
inc(41)             -- 42
```

**Note**: The `name(args): body` definition syntax only exists *inside*
`object` blocks (as method definitions) — it is not a top-level function
form. Functions must be defined before they're called.

### Lambdas (Anonymous Functions)

```fmpl
-- Lambda syntax: single param, multi-param (comma-separated), curried
\x x + 1
\x, y x + y
\x \y x + y

-- Apply lambda immediately
(\x x * 2)(5)       -- 10

-- Store and call later
let doubler = \x x * 2
doubler(7)          -- 14

-- Curried application
let addc = \x \y x + y
addc(3)(4)          -- 7
```

### Higher-Order Functions

```fmpl
-- Functions can take other functions as arguments
let apply_twice = \f, x f(f(x))
let add_one = \x x + 1

apply_twice(add_one, 5)
-- Returns: 7

-- Built-in higher-order list methods
[1, 2, 3].map(\x x * 2)
-- Returns: [2, 4, 6]
```

**Note**: `.map()` and `.fold()` are implemented as list methods; see the
Data Structures section for what's still planned.

---

## Objects and Methods

### Basic Object Usage

```fmpl
-- Define object
object counter {
  count: 0
  increment(): self.count + 1
  value(): self.count
}

-- Access methods via the object name
counter.value()       -- 0
counter.increment()   -- 1
```

### Special Variables

Objects have access to special variables (magical variables) in method context:

- `self` - Reference to current object (the receiver of the method call)
- `parent` - Reference to parent object (for prototype chain lookup)
- `caller` - Reference to the object that called this method
- `user` - Reference to the current user context
- `args` - The list of all arguments passed to the method

```fmpl
object greeter {
  name: "world"
  show(): "Hello, " + self.name
}

greeter.show()
-- Returns: "Hello, world"
```

These variables are always available within method bodies. Note that the
receiver is `self` (as in Python/Smalltalk) — there is no `this`, and using
`this` silently breaks the enclosing object definition.

---

## Metaprogramming

FMPL supports **first-class AST and IR values**, enabling you to write compilers, DSLs, and code generators entirely in FMPL.

### Tagged Values (Canonical List Form)

FMPL supports algebraic data types via tagged values, written as lists whose
first element is a symbol (DESIGN-002, the single canonical list form):

```fmpl
-- Create tagged values
[:Int, 42]
[:Binary, :+, [:Int, 1], [:Int, 2]]
[:User, "alice", %{active: true}]

-- Pattern match on tagged values (bare identifiers bind in tagged patterns)
let value = [:Binary, :+, [:Int, 1], [:Int, 2]]
value @ {
  [:Binary, :+, a, b] => "addition";
  [:Binary, :-, a, b] => "subtraction";
  [:Int, n] => "just a number"
}
-- Returns: "addition"
```

**Note**: Operator symbols like `:+`, `:-`, `:*` are ordinary symbols and can
appear in tagged values and patterns. The legacy constructor syntax
`:Int(42)` is rejected with a hint: use `[:Int, 42]` instead.

### Parsing Source to AST

Use `ast::parse` to convert FMPL source code into a tagged AST:

```fmpl
let ast = ast::parse("1 + 2")
-- Returns: [:Binary, :+, [:Int, 1], [:Int, 2]]

let ast2 = ast::parse("if true then 1 else 2")
-- Returns: [:If, [:Bool, true], [:Int, 1], [:Int, 2]]

let ast3 = ast::parse("[1, 2, 3]")
-- Returns: [:List, [[:Int, 1], [:Int, 2], [:Int, 3]]]
```

### Compiling IR to Bytecode

Use `ir::compile` to transform IR (intermediate representation) into executable bytecode:

```fmpl
-- Direct IR construction
let code = ir::compile([:Add, [:LoadInt, 1], [:LoadInt, 2]])
-- Returns: <code> (first-class bytecode)

-- With let bindings
let code2 = ir::compile([:Let, :x, [:LoadInt, 42], [:Var, :x]])
```

Supported IR nodes: `LoadNull`, `LoadBool`, `LoadInt`, `LoadFloat`, `LoadString`, `LoadVar`, `Var`, `Add`, `Sub`, `Mul`, `Div`, `Mod`, `Neg`, `Not`, `Eq`, `NotEq`, `Lt`, `Gt`, `LtEq`, `GtEq`, `Let`, `Seq`, `If`, `Return`, `MakeList`, `MakeTagged`.

### Executing Bytecode

Use `code::eval` to execute compiled bytecode:

```fmpl
let code = ir::compile([:Add, [:LoadInt, 1], [:LoadInt, 2]])
code::eval(code)
-- Returns: 3
```

### Full Metaprogramming Pipeline

Combine all three to write compilers in FMPL:

```fmpl
-- Parse source, transform AST to IR, compile, execute
let ast = ast::parse("1 + 2")

let ir = ast @ {
  [:Binary, :+, [:Int, a], [:Int, b]] => [:Add, [:LoadInt, a], [:LoadInt, b]];
  [:Binary, :-, [:Int, a], [:Int, b]] => [:Sub, [:LoadInt, a], [:LoadInt, b]];
  [:Binary, :*, [:Int, a], [:Int, b]] => [:Mul, [:LoadInt, a], [:LoadInt, b]];
  [:Binary, :/, [:Int, a], [:Int, b]] => [:Div, [:LoadInt, a], [:LoadInt, b]]
}

let code = ir::compile(ir)
code::eval(code)
-- Returns: 3
```

This is exactly the shape of the real bootstrap pipeline: `lib/core/ast_to_ir.fmpl`
transforms full ASTs to IR the same way, and `ast::parse` itself is backed by
the FMPL-written parser in `lib/core/fmpl_parser.fmpl` (DESIGN-001, the
metacircular bootstrap).

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
-- Returns: %{age: 30, name: "Alice"}

-- Validate structure
parsed @ {
  %{name: _:n, age: _:a} when a >= 18 => "Adult: " + n;
  %{name: _:n, age: _:a} => "Minor: " + n;
  _ => "Invalid structure"
}
-- Returns: "Adult: Alice"
```

### Example 2: HTTP Requests with Tool Calling

```fmpl
-- Make HTTP GET request using the curl builtin (requires network)
let response = curl.get("https://api.example.com/data")

-- Parse JSON response
let data = json::parse(response)

-- Extract specific fields
data @ {
  %{status: _:s, results: _:r} when s == "ok" => r;
  %{error: _:e} => "Error: " + e;
  _ => "Unknown response"
}
```

### Example 3: Building a Simple Agent Loop

```fmpl
-- Dispatch tool calls by binding the tool name and guarding on it
let handle_tool_call = \tc tc @ {
  %{tool: _:t, args: _:a} when t == "curl.get" => curl.get(a.url);
  %{tool: _:t, args: _:a} when t == "curl.post" => curl.post(a.url, a.body);
  %{text: _:txt} => txt;
  _ => "Error: Unrecognized response"
}

-- Simulate LLM response
let llm_output = "{\"tool\": \"curl.get\", \"args\": {\"url\": \"https://example.com\"}}"
let parsed = json::parse(llm_output)

-- Handle the tool call (performs the HTTP request)
handle_tool_call(parsed)
```

---

## Tool Calling and Agent Workflows

FMPL is designed for **agentic AI workflows**—closing the loop between LLMs, tools, and human oversight.

Real LLM clients ship in the standard library: `lib/anthropic.fmpl` (Claude
API, requires `ANTHROPIC_API_KEY`) and `lib/ollama.fmpl` (local models), with
shared plumbing in `lib/llm-common.fmpl`.

### The Agent Loop

```fmpl
-- 1. User sends message
let user_message = "What's the weather in NYC?"

-- 2. LLM responds (simulated here with a map)
let llm_response = %{tool: "curl.get", args: %{url: "https://wttr.in/NYC?format=j1"}}

-- 3. Parse and execute the tool call, feed result to the next turn
llm_response @ { %{tool: _:t, args: _:a} when t == "curl.get" => curl.get(a.url) }
```

### Multi-Turn Tool Calling

The same shape extends to a recursive agent loop (sketch — `llm_complete` and
`execute_tool` stand in for your LLM client and tool registry):

```fmpl
let agent_turn = \input, history
  llm_complete(%{history: history, input: input}) @ {
    %{tool: _:t, args: _:a} => agent_turn(execute_tool(t, a), history);
    %{answer: _:ans} => ans;
    _ => "Error: Unexpected LLM output"
  }

let result = agent_turn("Search for latest Rust version", [])
```

---

## Advanced Topics

### Grammar-Based Agents

FMPL's unique design direction: **express agent control flow as grammars**.
This is the project's north star, not yet a working feature — the sketch
below shows the intended shape (see
`docs/plans/2026-01-19-unified-grammars-and-agents-design.md`):

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

FMPL's engine supports **durable state** via Fjall (embedded key-value
store) — see `specs/persistence.md`. Language-level checkpoint/resume
builtins are designed but not yet exposed:

```fmpl
-- Design sketch (not yet implemented as builtins)
checkpoint("stage_name", data)
resume_from(saved_checkpoint)
```

This is intended to enable:
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
=> 3
fmpl> "hello" @ { [a-z]+ => "word" }
=> "word"
fmpl> let x = 42
=> 42
fmpl> x * 2
=> 84
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

**Currently Implemented** (as of 2026-07-20):
- ✅ Core parser and lexer — canonical parser is FMPL-generated (metacircular bootstrap, DESIGN-001)
- ✅ Expression evaluation (arithmetic: `+`, `-`, `*`, `/`, `%`; checked — overflow is a clean error)
- ✅ Comparisons (`==`, `!=`, `<`, `>`, `<=`, `>=`)
- ✅ Logical operators (`&&`, `||`, `!`)
- ✅ Pattern matching with `@` operator (regex, wildcard, map/list/tagged-value patterns with `_:name` bindings)
- ✅ Guards on match arms (`when`, with `if` accepted as an alias)
- ✅ `match` expression form
- ✅ Grammar system (OMeta-style PEG) with `:binding` captures in rules
- ✅ Named object definitions and method calls (`self` receiver)
- ✅ Lambdas (`\x`, `\x, y`, curried `\x \y`, and the `lambda (x)` keyword form)
- ✅ Recursion through top-level `let`-bound lambdas
- ✅ If-then-else conditionals
- ✅ Let-bindings (statement and expression forms)
- ✅ Lists (`[]`) and maps (`%{}`); `.map()` / `.fold()` list methods
- ✅ Type predicates on all values (`is_int`, `is_string`, …, `type_name()`)
- ✅ Tagged values in canonical list form (`[:Tag, args...]`)
- ✅ Operator symbol literals (`:+`, `:-`, `:*`, etc.)
- ✅ Metaprogramming (`ast::parse`, `ir::compile`, `code::eval`)
- ✅ JSON parsing (`json::parse` builtin)
- ✅ HTTP tools (`curl.get` / `curl.post`)
- ✅ Indexed RPN bytecode VM
- ✅ Streaming grammar support (push model)
- ✅ Persistence engine (Fjall-backed)

**Partially Implemented**:
- ⚠️ Async operators (`<-`, `spawn`, `|>`) - syntax exists, runtime in progress
- ⚠️ Object constructors (`^name`) - syntax designed, implementation evolving
- ⚠️ Bootstrap pipeline (`ast_to_ir.fmpl`) - core expressions work; several AST node types still produce incorrect IR (see `docs/known-gaps.md`)

**Not Yet Implemented**:
- ❌ Exponentiation (`**`)
- ❌ String interpolation (`"{x}"` is literal text; concatenate with `+`)
- ❌ Literal values inside map patterns (`%{status: 200}` — bind and guard instead)
- ❌ Rest patterns (`[first | rest]`)
- ❌ Recursive scoped `let (f = ...)` bindings (top-level `let` recursion works)
- ❌ List accessors `.length()` / `.get()`
- ❌ Grammar inheritance (`<:`) — deliberately deferred (DESIGN-005)
- ❌ Anonymous object literals
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

**Last Updated**: 2026-07-20 (swept against a fresh REPL)
**Version**: 0.1.0 (experimental)
