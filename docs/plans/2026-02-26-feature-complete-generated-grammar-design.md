# Feature-Complete Generated Grammar

## Overview

Make `lib/core/fmpl_parser.fmpl` a complete, self-contained scannerless parser for FMPL, achieving full parity with the legacy hand-written recursive descent parser. This eliminates the legacy parser as a runtime dependency and enables bootstrapping onto other VMs (execution_tape, ORC) that lack the Rust parser.

## Core Problem

The generated grammar covers 42% of FMPL syntax (basic expressions). The remaining 58% -- object definitions, grammar definitions, pattern matching, control flow, exception handling, the @ operator -- exists only in the hand-written Rust parser. Any VM that cannot call Rust code cannot parse FMPL programs.

Full parity means: every program the legacy parser accepts, the generated grammar also accepts, producing an equivalent AST.

## Design

### Architecture

The grammar remains **scannerless** (operates on raw characters, no separate lexer). It produces **tagged values** (`:If()`, `:Try()`, `:Object()`, etc.) as intermediate representation. The `value_to_expr()` function in generated code converts these to typed `Expr` AST nodes.

Grammar definitions are parsed **fully within the scannerless grammar** -- no delegation to the runtime `GrammarParser`. This is necessary for bootstrap: a grammar that delegates to Rust code for grammar parsing cannot be ported to non-Rust VMs.

### Delivery: Test-Driven Batches

Each batch follows this cycle:
1. Add test cases to `parser_equivalence.rs` (both parsers must produce identical ASTs)
2. Add test cases to `generated_parser_correctness.rs` (generated parser produces correct runtime values)
3. Implement grammar rules in `fmpl_parser.fmpl`
4. Extend `value_to_expr()` for new tagged values
5. Build fmpl-bootstrap, regenerate, verify all tests pass
6. Verify no regressions in existing tests

## Key Components

### Batch 1: Statements & Sequences

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| Semicolons / sequences | `sequence = expr (";" sp expr)*` | `:Sequence(exprs)` |
| Let statement form | `let_stmt = "let" sp pattern "=" sp expr` | `:LetStmt(name, value)` |
| Assignment | `assign = postfix "=" sp expr` | `:Assign(target, value)` |
| Return | `return_expr = "return" sp expr` | `:Return(value)` |
| Yield | `yield_expr = "yield" sp expr` | `:Yield(value)` |
| Throw | `throw_expr = "throw" sp expr` | `:Throw(value)` |

### Batch 2: Control Flow

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| try/catch | `try_catch = "try" sp block "catch" sp ident block` | `:Try(body, binding, catch)` |
| while | `while_loop = "while" sp expr "do" sp expr` | `:While(cond, body)` |
| do-while | `do_while = "do" sp expr "while" sp expr` | `:DoWhile(body, cond)` |
| for | `for_loop = "for" sp pattern "in" sp expr "do" sp expr` | `:For(pattern, iter, body)` |
| match | `match_expr = "match" sp expr block_cases` | `:Match(scrutinee, cases)` |

### Batch 3: Operators & Collections

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| Pipe | `pipe_op = "\|>" sp` at precedence below `or_expr` | `:Pipe(left, right)` |
| @ grammar apply | `at_op = "@" sp qualified.rule` | `:GrammarApply(input, grammar, rule)` |
| @ inline patterns | `at_op = "@" sp "{" cases "}"` | `:InlinePatternBlock(input, cases)` |
| Async call | `"<-" sp expr` in unary | `:AsyncCall(expr)` |
| Sync call | `"->" sp expr` in unary | `:SyncCall(expr)` |
| List cons | `"[" expr "\|" expr "]"` | `:ListCons(head, tail)` |
| Slice | `expr "[" expr ".." expr "]"` | `:Slice(expr, start, end)` |
| List comprehension | `"[" expr "for" pattern "in" expr "]"` | `:ListComp(expr, pattern, iter, cond)` |

### Batch 4: Patterns

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| Wildcard | `"_"` | `:PatWildcard()` |
| Variable | `ident` | `:PatVar(name)` |
| Literal | int / string / symbol | `:PatInt(n)` etc. |
| Constructor | `":" tag "(" patterns ")"` | `:PatConstructor(tag, children)` |
| List pattern | `"[" patterns "]"` | `:PatList(patterns, rest)` |
| Map pattern | `"%{" entries "}"` | `:PatMap(entries)` |
| As pattern | `pattern "as" ident` | `:PatAs(pattern, name)` |
| When guard | `pattern "when" expr` | `:PatGuard(pattern, cond)` |

### Batch 5: Object System

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| Object def | `"object" name "{" body "}"` | `:Object(name, params, parents, bindings, facets)` |
| Facet def | `".#facets" entries` | `:Facet(name, members)` |
| self/parent/caller/user/args | keywords in primary | `:Self()`, `:Parent()`, etc. |
| Object tag | `"^" name` | `:ObjTag(name)` |
| Facet access | `.as(:symbol)` as postfix | `:FacetAccess(obj, facet)` |
| Spawn | `"spawn" expr` | `:Spawn(expr)` |

### Batch 6: Grammar Definitions (Meta-grammar)

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| Named grammar | `"grammar" name "{" rules "}"` | `:GrammarDef(name, rules)` |
| Let grammar | `"let" name "=" "grammar" ...` | `:LetGrammar(name, grammar)` |
| Rule def | `name "=" pattern ("=>" action)?` | `:RuleDef(name, pattern, action)` |
| Sequence | `pattern pattern` | `:GSeq(patterns)` |
| Choice | `pattern "\|" pattern` | `:GChoice(patterns)` |
| Repetition | `pattern "*"` / `"+"` / `"?"` | `:GStar(p)`, `:GPlus(p)`, `:GOpt(p)` |
| Char class | `"[" ranges "]"` | `:GCharClass(ranges)` |
| Negation | `"~" pattern` | `:GNot(pattern)` |
| Any | `"."` | `:GAny()` |
| Binding | `pattern ":" name` | `:GBind(pattern, name)` |
| Action | `pattern "=>" expr` | `:GAction(pattern, expr)` |
| Grammar extension | `grammar G <: Base { ... }` | `:GrammarExtend(name, base, rules)` |
| String literal | `"text"` | `:GLiteral(text)` |

### Batch 7: Advanced Iteration

| Feature | Grammar Rule | Tagged Value |
|---------|-------------|--------------|
| fold | `"fold" "(" fn "," init "," iter ")"` | `:Fold(fn, init, iter)` |
| foldr | `"foldr" "(" fn "," init "," iter ")"` | `:Foldr(fn, init, iter)` |
| map keyword | `"map" "(" fn "," iter ")"` | `:MapKw(fn, iter)` |
| filter keyword | `"filter" "(" fn "," iter ")"` | `:FilterKw(fn, iter)` |
| Placeholder _ | `"_"` in call args | `:Placeholder()` |

### value_to_expr() Extensions

Each batch adds conversion cases to the generated `value_to_expr()` function. This function lives in `ir_to_rust.rs` as part of the generated Rust code. New tagged values map to existing `Expr` variants:

- `:Sequence(exprs)` -> `Expr::Sequence(vec)`
- `:LetStmt(name, value)` -> `Expr::Let(LetBinding { ... })`
- `:Assign(target, value)` -> `Expr::Assign(target, value)`
- `:Try(body, binding, catch)` -> `Expr::TryCatch(body, binding, catch)`
- `:Object(...)` -> `Expr::ObjectDef(ObjectDef { ... })`
- `:GrammarDef(...)` -> `Expr::NamedGrammar(name, Grammar { ... })`
- etc.

## Bootstrap Significance

With a feature-complete grammar:
1. `fmpl-bootstrap` uses the legacy parser to generate Rust code from the grammar
2. The generated Rust code can parse all FMPL, replacing the legacy parser
3. The same grammar can be loaded by any VM that implements the PEG engine
4. execution_tape and ORC can parse FMPL by loading `fmpl_parser.fmpl` as data

The grammar becomes a **portable parser specification**, decoupled from Rust.

## Implementation Status

- **Complete**: Basic expressions (42% -- literals, arithmetic, let expressions, lambdas, if/then/else, collections, comments)
- **In Progress**: Batch 1
- **Planned**: Batches 2-7

## References

- `lib/core/fmpl_parser.fmpl` -- the grammar
- `lib/core/parser_generator.fmpl` -- the generator pipeline
- `fmpl-core/src/builtins/ir_to_rust.rs` -- IR to Rust transpilation
- `fmpl-core/tests/parser_equivalence.rs` -- equivalence tests
- `fmpl-core/tests/generated_parser_correctness.rs` -- correctness tests
- `docs/plans/2026-01-19-unified-grammars-and-agents-design.md` -- @ operator design
