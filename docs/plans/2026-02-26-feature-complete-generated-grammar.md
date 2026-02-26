# Feature-Complete Generated Grammar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `lib/core/fmpl_parser.fmpl` a complete, self-contained scannerless parser for FMPL, achieving full parity with the legacy hand-written recursive descent parser.

**Architecture:** The grammar remains scannerless (character-level PEG). Each new language feature is added as grammar rules in `fmpl_parser.fmpl`, with corresponding `value_to_expr()` cases in both `value_to_ast.rs` (standalone) and the embedded template in `ir_to_rust.rs`. The grammar produces tagged values (`:If()`, `:Try()`, etc.) as IR, which `value_to_expr()` converts to typed `Expr` AST nodes.

**Tech Stack:** Rust workspace, PEG grammar in FMPL DSL, `fmpl-bootstrap` binary for code generation, `cargo test` for verification.

**Key constraint:** Grammar definitions must be parsed fully within the scannerless grammar (no delegation to runtime GrammarParser) to enable bootstrapping onto non-Rust VMs (execution_tape, ORC).

---

## How This Plan Works

### Two-File Value Mapping

Every new tagged value needs conversion cases in TWO places:
1. **`fmpl-core/src/value_to_ast.rs`** -- standalone Rust module used at runtime
2. **`fmpl-core/src/builtins/ir_to_rust.rs`** lines 1171-1446 -- embedded template in generated parser code

Both must handle the same tagged values. `value_to_ast.rs` is the canonical implementation; `ir_to_rust.rs` gets a copy of each new case.

### Test Strategy

Each batch adds test cases to:
- **`fmpl-core/tests/parser_equivalence.rs`** -- `TEST_CASES` array: both parsers produce identical ASTs
- **`fmpl-core/tests/generated_parser_correctness.rs`** -- `eval_generated()`: generated parser produces correct runtime values

### Build & Verify Cycle

After each batch:
```bash
cargo test -p fmpl-core                      # All tests pass (uses fallback parser)
```

After ALL batches, when grammar is feature-complete:
```bash
FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap  # Build bootstrap binary
cargo build -p fmpl-core                                # Regenerates parser
cargo test -p fmpl-core                                 # Full verification
```

### Grammar Helpers Available in `lib/core/prelude.fmpl`

- `symbol(s)` -- string to symbol
- `join(list)` -- join list of strings
- `reduce(f, init, list)` -- left fold
- `fold_binary(first, rest)` -- fold operator/operand pairs into `:Binary()` tree
- `prepend(item, list)` -- prepend item to list
- `fold_postfix(base, ops)` -- fold postfix ops into nested AST nodes
- `map_list(f, list)` -- transform list elements

### Keywords in Grammar

The grammar's `keyword` rule (line 61) must be extended as new keywords are added. Currently:
```
keyword = ("if" | "then" | "else" | "let" | "true" | "false" | "null" | "lambda") ~ident_rest
```

---

## Task 1: Extend keyword list in grammar

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl:61`

**Step 1: Update the keyword rule**

Add all FMPL keywords to prevent them from being parsed as identifiers:

```fmpl
keyword = ("if" | "then" | "else" | "let" | "true" | "false" | "null" | "lambda"
          | "return" | "yield" | "throw" | "try" | "catch" | "while" | "do" | "for" | "in"
          | "match" | "when" | "as" | "spawn" | "object" | "grammar" | "stream"
          | "self" | "parent" | "caller" | "user" | "args"
          | "fold" | "foldr" | "map" | "filter") ~ident_rest
```

**Step 2: Run tests**

Run: `cargo test -p fmpl-core parser_equivalence`
Expected: PASS (keywords don't affect existing tests since the grammar doesn't use them yet)

Run: `cargo test -p fmpl-core generated_parser_correctness`
Expected: PASS

**Step 3: Commit**

```bash
git add lib/core/fmpl_parser.fmpl
git commit -m "feat(grammar): extend keyword list for full language parity"
```

---

## Task 2: Batch 1 -- Semicolons, Sequences, and Statement Forms

This batch adds the statement-level features that allow multi-line programs.

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/src/value_to_ast.rs`
- Modify: `fmpl-core/src/builtins/ir_to_rust.rs`
- Modify: `fmpl-core/tests/parser_equivalence.rs`
- Modify: `fmpl-core/tests/generated_parser_correctness.rs`

### Step 1: Write equivalence test cases

Add to `TEST_CASES` in `fmpl-core/tests/parser_equivalence.rs`:

```rust
// Batch 1: Statements & Sequences
"let x = 42",
"let x = 1 + 2",
"return 42",
"return",
"throw \"error\"",
"yield 42",
"{1; 2; 3}",
"{let x = 1; x + 1}",
```

### Step 2: Write correctness test cases

Add to `fmpl-core/tests/generated_parser_correctness.rs`:

```rust
// =============================================================================
// STATEMENT FORM TESTS
// =============================================================================

#[test]
fn test_let_stmt() {
    // let x = 42 binds to current scope, returns value
    assert_evals_to("let x = 42", Value::Int(42));
}

#[test]
fn test_sequence_block() {
    // { expr; expr; expr } returns last
    assert_evals_to("{1; 2; 3}", Value::Int(3));
}

#[test]
fn test_sequence_with_let() {
    assert_evals_to("{let x = 1; let y = 2; x + y}", Value::Int(3));
}

#[test]
fn test_return_with_value() {
    assert_parses_and_compiles("return 42");
}

#[test]
fn test_throw_expression() {
    assert_parses_and_compiles(r#"throw "error""#);
}

#[test]
fn test_yield_expression() {
    assert_parses_and_compiles("yield 42");
}
```

### Step 3: Run tests to verify they fail

Run: `cargo test -p fmpl-core parser_equivalence`
Expected: FAIL (generated parser doesn't support `let x = 42`, sequences, etc.)

### Step 4: Add grammar rules to `fmpl_parser.fmpl`

Replace the `expr` and `code` rules at the bottom of the grammar with:

```fmpl
    -- Statement-level let: let name = expr (no parens, no body)
    let_stmt = "let" sp sym_ident:name "=" sp or_expr:value => :LetSimple(:Binding(name, value))

    -- Return, yield, throw
    return_expr = "return" sp or_expr:value => :Return(value)
                | "return" ~ident_rest sp => :ReturnVoid()
    yield_expr = "yield" sp or_expr:value => :Yield(value)
    throw_expr = "throw" sp or_expr:value => :Throw(value)

    -- Sequence block: { expr; expr; ... }
    seq_semi = ";" sp
    seq_item = expr
    seq_rest = seq_semi seq_item:e => e
    sequence = "{" sp seq_item:first seq_rest*:rest "}" sp => :Sequence(prepend(first, rest))
             | "{" sp "}" sp => :Sequence([])

    -- Assignment: postfix = expr (right-associative)
    assign = or_expr:target "=" sp expr:value => :Assign(target, value)

    -- Expression is the top level that includes if/let/lambdas/statements
    expr = if_expr | let_expr | let_stmt | short_lambda | full_lambda
         | return_expr | yield_expr | throw_expr | assign | or_expr

    -- Top-level code: semicolon-separated statements
    stmt = expr
    stmt_rest = (";" sp | sp) stmt:s => s
    stmts = stmt:first stmt_rest*:rest => prepend(first, rest)
    code = sp stmts:ss sp => if length(ss) == 1 then ss[0] else :Do(ss)
```

Also add `sequence` to the `primary` rule:

```fmpl
    primary = paren | sequence | list_lit | map_lit | float_lit | int | bool | null_lit | string_lit | tagged | symbol | var
```

### Step 5: Add `value_to_expr` cases

In `fmpl-core/src/value_to_ast.rs`, add these cases before the `_ =>` catch-all in the `match tag.as_str()` block:

```rust
"Return" => {
    if !children.is_empty() {
        let value = value_to_expr(&children[0])?;
        Ok(Expr::Return(Some(Box::new(value))))
    } else {
        Ok(Expr::Return(None))
    }
}
"ReturnVoid" => Ok(Expr::Return(None)),
"Throw" => {
    if !children.is_empty() {
        let value = value_to_expr(&children[0])?;
        Ok(Expr::Throw(Box::new(value)))
    } else {
        Err(Error::Runtime("Invalid Throw node".to_string()))
    }
}
"Assign" => {
    if children.len() >= 2 {
        let target = value_to_expr(&children[0])?;
        let value = value_to_expr(&children[1])?;
        Ok(Expr::Assignment(Box::new(target), Box::new(value)))
    } else {
        Err(Error::Runtime("Invalid Assign node".to_string()))
    }
}
"Sequence" => {
    if let Some(Value::List(items)) = children.first() {
        let exprs: Result<Vec<Expr>> = items.iter().map(value_to_expr).collect();
        Ok(Expr::Sequence(exprs?))
    } else {
        Err(Error::Runtime("Invalid Sequence node".to_string()))
    }
}
```

Note: `LetSimple`, `Do`, `Fold`, `Foldr`, `Yield` already exist in `value_to_ast.rs`.

Add the same cases to the embedded template in `fmpl-core/src/builtins/ir_to_rust.rs` (inside the `value_to_expr` function string, before the `_ =>` catch-all around line 1444).

### Step 6: Run tests

Run: `cargo test -p fmpl-core`
Expected: PASS

### Step 7: Commit

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/src/value_to_ast.rs fmpl-core/src/builtins/ir_to_rust.rs fmpl-core/tests/parser_equivalence.rs fmpl-core/tests/generated_parser_correctness.rs
git commit -m "feat(grammar): add statements, sequences, semicolons (batch 1)"
```

---

## Task 3: Batch 2 -- Control Flow

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/src/value_to_ast.rs`
- Modify: `fmpl-core/src/builtins/ir_to_rust.rs`
- Modify: `fmpl-core/tests/parser_equivalence.rs`
- Modify: `fmpl-core/tests/generated_parser_correctness.rs`

### Step 1: Write equivalence test cases

Add to `TEST_CASES`:

```rust
// Batch 2: Control Flow
"try { 42 } catch e { 0 }",
"try { 1 + 2 } catch err { err }",
"while true do 1",
"do 1 while true",
"match x { _ => 0 }",
```

### Step 2: Write correctness tests

```rust
// =============================================================================
// CONTROL FLOW TESTS
// =============================================================================

#[test]
fn test_try_catch_success() {
    assert_evals_to("try { 42 } catch e { 0 }", Value::Int(42));
}

#[test]
fn test_try_catch_failure() {
    assert_evals_to(r#"try { throw "err" } catch e { 99 }"#, Value::Int(99));
}

#[test]
fn test_while_loop() {
    assert_parses_and_compiles("while false do 1");
}

#[test]
fn test_do_while_loop() {
    assert_parses_and_compiles("do 1 while false");
}

#[test]
fn test_match_basic() {
    assert_evals_to("match 42 { x => x + 1 }", Value::Int(43));
}

#[test]
fn test_match_wildcard() {
    assert_evals_to("match 42 { _ => 0 }", Value::Int(0));
}
```

### Step 3: Add grammar rules

Add to `fmpl_parser.fmpl`:

```fmpl
    -- Try/catch
    try_body_item = expr
    try_body_rest = seq_semi try_body_item:e => e
    try_body = try_body_item:first try_body_rest*:rest => if length(rest) == 0 then first else :Sequence(prepend(first, rest))
    catch_body = try_body
    try_catch = "try" sp "{" sp try_body:body "}" sp "catch" sp sym_ident:binding "{" sp catch_body:handler "}" sp
              => :Try(body, binding, handler)

    -- While loop
    while_loop = "while" sp expr:cond "do" sp expr:body => :While(cond, body)

    -- Do-while loop
    do_while = "do" sp expr:body "while" sp expr:cond => :DoWhile(body, cond)

    -- For loop
    for_loop = "for" sp pattern:pat "in" sp expr:iter "do" sp expr:body => :For(pat, iter, body)
             | "for" sp pattern:pat "in" sp expr:iter "{" sp expr:body "}" sp => :For(pat, iter, body)

    -- Match expression
    match_case_guard = "when" sp or_expr:guard "=>" sp expr:body => :MatchCaseGuard(guard, body)
    match_case_body = "=>" sp expr:body => :MatchCaseSimple(body)
    match_case = pattern:pat (match_case_guard | match_case_body):action => :MatchCase(pat, action)
    match_case_rest = (seq_semi | sp) match_case:c => c
    match_cases = match_case:first match_case_rest*:rest => prepend(first, rest)
    match_expr = "match" sp expr:scrutinee "{" sp match_cases:cases "}" sp => :Match(scrutinee, cases)
```

Add these to the `expr` rule before `or_expr`:

```fmpl
    expr = if_expr | let_expr | let_stmt | short_lambda | full_lambda
         | return_expr | yield_expr | throw_expr | try_catch | while_loop | do_while | for_loop | match_expr
         | assign | or_expr
```

### Step 4: Add pattern rules

```fmpl
    -- Patterns for match/for/let destructuring
    pat_wildcard = "_" sp => :PatternWildcard()
    pat_var = ~keyword sym_ident:name => :PatternVar(name)
    pat_int = digit_val+:ds ~ident_rest sp => :PatternLiteral(:Int(reduce(\acc \d acc * 10 + d, 0, ds)))
    pat_string = "\"" string_char*:cs "\"" sp => :PatternLiteral(:String(cs))
    pat_symbol = ":" tag_name:tag "(" sp pat_args:pats ")" sp => :PatternTagged(tag, pats)
               | ":" sym_name:name sp => :PatternLiteral(:Symbol(name))
    pat_arg_rest = "," sp pattern:p => p
    pat_args = pattern:first pat_arg_rest*:rest => prepend(first, rest)
             | => []
    pat_list = "[" sp pat_args:pats "]" sp => :PatternList(pats)
    pat_map_entry = ident:k ":" sp pattern:v => [k, v]
    pat_map_entry_rest = "," sp pat_map_entry:e => e
    pat_map_entries = pat_map_entry:first pat_map_entry_rest*:rest => prepend(first, rest)
    pat_map = "%" "{" sp pat_map_entries:entries "}" sp => :PatternMap(entries)
            | "%" "{" sp "}" sp => :PatternMap([])
    pat_primary = pat_wildcard | pat_symbol | pat_int | pat_string | pat_list | pat_map | pat_var
    pat_as = pat_primary:p "as" sp sym_ident:name => :PatternAs(p, name)
    pattern = pat_as | pat_primary
```

### Step 5: Add `value_to_expr` cases

In `value_to_ast.rs`:

```rust
"Try" => {
    if children.len() >= 3 {
        let body = value_to_expr(&children[0])?;
        let binding = if let Value::Symbol(s) = &children[1] {
            s.clone()
        } else {
            return Err(Error::Runtime("Invalid Try binding".to_string()));
        };
        let catch_body = value_to_expr(&children[2])?;
        Ok(Expr::TryCatch {
            body: Box::new(body),
            error_binding: binding,
            catch_body: Box::new(catch_body),
        })
    } else {
        Err(Error::Runtime("Invalid Try node".to_string()))
    }
}
"While" => {
    if children.len() >= 2 {
        let cond = value_to_expr(&children[0])?;
        let body = value_to_expr(&children[1])?;
        Ok(Expr::While(Box::new(cond), Box::new(body)))
    } else {
        Err(Error::Runtime("Invalid While node".to_string()))
    }
}
"DoWhile" => {
    if children.len() >= 2 {
        let body = value_to_expr(&children[0])?;
        let cond = value_to_expr(&children[1])?;
        Ok(Expr::DoWhile(Box::new(body), Box::new(cond)))
    } else {
        Err(Error::Runtime("Invalid DoWhile node".to_string()))
    }
}
"For" => {
    if children.len() >= 3 {
        let pattern = value_to_pattern(&children[0])?;
        let iterable = value_to_expr(&children[1])?;
        let body = value_to_expr(&children[2])?;
        Ok(Expr::For(pattern, Box::new(iterable), Box::new(body)))
    } else {
        Err(Error::Runtime("Invalid For node".to_string()))
    }
}
"Match" => {
    if children.len() >= 2 {
        let scrutinee = value_to_expr(&children[0])?;
        if let Value::List(cases) = &children[1] {
            let match_cases = cases.iter().map(|c| {
                let (tag, cs) = match c {
                    Value::Tagged(tag, children) => (tag.as_str(), &**children),
                    _ => return Err(Error::Runtime("Invalid MatchCase".to_string())),
                };
                if tag != "MatchCase" || cs.len() < 2 {
                    return Err(Error::Runtime("Invalid MatchCase".to_string()));
                }
                let pattern = value_to_pattern(&cs[0])?;
                let (guard, body) = value_to_pattern_action(&cs[1])?;
                Ok(MatchCase { pattern, guard, body: Box::new(body) })
            }).collect::<Result<Vec<_>>>()?;
            Ok(Expr::Match(Box::new(scrutinee), match_cases))
        } else {
            Err(Error::Runtime("Invalid Match cases".to_string()))
        }
    } else {
        Err(Error::Runtime("Invalid Match node".to_string()))
    }
}
```

Note: `value_to_pattern` and `value_to_pattern_action` already exist in `value_to_ast.rs`. Add the match-case action handler if using different tag names:

```rust
"MatchCaseSimple" => same as "PatternCaseSimple"
"MatchCaseGuard" => same as "PatternCaseGuard"
```

Add corresponding cases to `ir_to_rust.rs` embedded template.

### Step 6: Run tests

Run: `cargo test -p fmpl-core`
Expected: PASS

### Step 7: Commit

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/src/value_to_ast.rs fmpl-core/src/builtins/ir_to_rust.rs fmpl-core/tests/parser_equivalence.rs fmpl-core/tests/generated_parser_correctness.rs
git commit -m "feat(grammar): add control flow - try/catch, loops, match (batch 2)"
```

---

## Task 4: Batch 3 -- Operators & Collections

**Files:** Same as previous tasks.

### Step 1: Write test cases

Equivalence tests:
```rust
// Batch 3: Operators & Collections
"x |> f",
r#""hello" @ g.rule"#,  // May not be testable without grammar setup
"[1 | rest]",
```

Correctness tests:
```rust
#[test]
fn test_pipe_operator() {
    assert_evals_to(r#"let (f = \x x + 1) 1 |> f"#, Value::Int(2));
}

#[test]
fn test_placeholder() {
    assert_parses_and_compiles("_");
}
```

### Step 2: Add grammar rules

```fmpl
    -- Pipe operator: lower precedence than or_expr
    pipe_op = "|" ">" sp
    pipe_expr = or_expr:first (pipe_op or_expr)*:rest => fold_binary_pipe(first, rest)

    -- @ operator: grammar apply or inline pattern block
    at_grammar = "@" sp or_expr:grammar "." ident:rule sp => [:grammar_apply, grammar, rule]
    at_inline = "@" sp "{" sp match_cases:cases "}" sp => [:inline_block, cases]
    at_suffix = at_grammar | at_inline

    -- Async/sync calls in unary
    async_call = "<" "-" sp unary:e => :AsyncCall(e)
    sync_call = "-" ">" sp unary:e => :SyncCall(e)

    -- List cons: [head | tail]
    list_cons = "[" sp expr:head "|" sp expr:tail "]" sp => :ListCons(head, tail)

    -- Slice: expr[start..end]
    slice_suffix = "[" sp expr:start ".." sp expr:end "]" sp => [:slice, start, end]

    -- Placeholder
    placeholder = "_" ~ident_rest sp => :Placeholder()
```

Update `unary`:
```fmpl
    unary = "-" sp unary:e => :Unary(:-, e)
          | "!" sp unary:e => :Unary(:!, e)
          | "<" "-" sp unary:e => :AsyncCall(e)
          | postfix
```

Update `primary` to include `placeholder` (before `var`):
```fmpl
    primary = paren | sequence | list_cons | list_lit | map_lit | float_lit | int | bool | null_lit | string_lit | tagged | symbol | placeholder | var
```

Update `postfix_op` to include `slice_suffix`:
```fmpl
    postfix_op = slice_suffix | index_suffix | call_suffix | method_suffix | prop_suffix
```

Update the `fold_postfix` helper in `prelude.fmpl` to handle slice:
```fmpl
    -- Add to fold_postfix:
    -- [:slice, start, end] => :Slice(acc, start, end)
```

Replace `or_expr` usage in `expr` with `pipe_expr` and add `@` handling after pipe:
```fmpl
    -- Pipe sits above or_expr, below assignment
    pipe_expr = or_expr:left (pipe_op or_expr):right => :Binary(:"|>", left, right)
              | or_expr:left at_suffix:at_op => apply_at(left, at_op)
              | or_expr
```

### Step 3: Add `value_to_expr` cases

```rust
"AsyncCall" => {
    if !children.is_empty() {
        let expr = value_to_expr(&children[0])?;
        Ok(Expr::AsyncCall(Box::new(expr)))
    } else {
        Err(Error::Runtime("Invalid AsyncCall".to_string()))
    }
}
"SyncCall" => {
    if !children.is_empty() {
        let expr = value_to_expr(&children[0])?;
        Ok(Expr::SyncCall(Box::new(expr)))
    } else {
        Err(Error::Runtime("Invalid SyncCall".to_string()))
    }
}
"ListCons" => {
    if children.len() >= 2 {
        let head = value_to_expr(&children[0])?;
        let tail = value_to_expr(&children[1])?;
        Ok(Expr::ListCons(Box::new(head), Box::new(tail)))
    } else {
        Err(Error::Runtime("Invalid ListCons".to_string()))
    }
}
"Placeholder" => Ok(Expr::Placeholder),
```

Note: `Slice`, `AtInlineBlock`, `AtGrammarApply` already exist in `value_to_ast.rs`. Add `Pipe` handling by mapping `:"|>"` to `BinOp::Pipe` in the `Binary` case.

Add corresponding cases to `ir_to_rust.rs`.

### Step 4: Run tests, commit

Run: `cargo test -p fmpl-core`

```bash
git commit -m "feat(grammar): add pipe, @, async/sync, list cons, slice (batch 3)"
```

---

## Task 5: Batch 4 -- Object System Keywords

**Files:** Same as previous tasks.

### Step 1: Add grammar rules for object-related keywords

```fmpl
    -- Special keywords as primary expressions
    self_expr = "self" ~ident_rest sp => :Self()
    parent_expr = "parent" ~ident_rest sp => :Parent()
    caller_expr = "caller" ~ident_rest sp => :Caller()
    user_expr = "user" ~ident_rest sp => :User()
    args_expr = "args" ~ident_rest sp => :Args()

    -- Object tag: ^name
    obj_tag = "^" ident:name sp => :ObjTag(name)

    -- Spawn: spawn expr
    spawn_expr = "spawn" sp primary:constructor => :Spawn(constructor)
```

Add to `primary` (before `var`):
```fmpl
    primary = paren | sequence | list_cons | list_lit | map_lit | float_lit | int | bool | null_lit | string_lit | tagged | symbol
            | self_expr | parent_expr | caller_expr | user_expr | args_expr | obj_tag | spawn_expr | placeholder | var
```

Add facet access to `postfix_op`:
```fmpl
    facet_suffix = ".as" sp "(" sp symbol:facet ")" sp => [:facet, facet]
```

Update `fold_postfix` to handle `:facet`:
```fmpl
    -- [:facet, :Symbol(name)] => :FacetAccess(acc, name)
```

### Step 2: Add `value_to_expr` cases

```rust
"Self" => Ok(Expr::Self_),
"Parent" => Ok(Expr::Parent),
"Caller" => Ok(Expr::Caller),
"User" => Ok(Expr::User),
"Args" => Ok(Expr::Args),
"ObjTag" => {
    if let Some(Value::String(name)) = children.first() {
        Ok(Expr::ObjTag(name.clone()))
    } else {
        Err(Error::Runtime("Invalid ObjTag".to_string()))
    }
}
"Spawn" => {
    if !children.is_empty() {
        let constructor = value_to_expr(&children[0])?;
        Ok(Expr::Spawn(Box::new(constructor), Vec::new()))
    } else {
        Err(Error::Runtime("Invalid Spawn".to_string()))
    }
}
"FacetAccess" => {
    if children.len() >= 2 {
        let obj = value_to_expr(&children[0])?;
        if let Value::Symbol(facet) = &children[1] {
            Ok(Expr::FacetAccess(Box::new(obj), facet.clone()))
        } else {
            Err(Error::Runtime("Invalid FacetAccess facet".to_string()))
        }
    } else {
        Err(Error::Runtime("Invalid FacetAccess".to_string()))
    }
}
```

### Step 3: Tests, verify, commit

```bash
git commit -m "feat(grammar): add self/parent/caller/user/args, obj tag, spawn, facet access (batch 4)"
```

---

## Task 6: Batch 5 -- Object Definitions

**Files:** Same as previous tasks.

### Step 1: Add grammar rules for object definitions

```fmpl
    -- Object definition
    visibility_private = "." "#" "private" sp
    visibility_public = "." "#" "public" sp
    visibility_protected = "." "#" "protected" sp
    visibility_facets = "." "#" "facets" sp

    -- Object binding: name(params): expr or name: expr
    obj_param_rest = "," sp sym_ident:p => p
    obj_params = sym_ident:first obj_param_rest*:rest => prepend(first, rest)
    obj_binding_with_params = ident:name "(" sp obj_params:params ")" sp ":" sp expr:value
                            => :ObjBinding(name, params, value, true)
    obj_binding_simple = ident:name ":" sp expr:value
                       => :ObjBinding(name, [], value, false)
    obj_binding = obj_binding_with_params | obj_binding_simple

    -- Facet definition: name: [member1, member2]
    facet_member_rest = "," sp ident:m => m
    facet_members = ident:first facet_member_rest*:rest => prepend(first, rest)
    facet_terminal = "!" => true | => false
    facet_def = ident:name facet_terminal:term ":" sp "[" sp facet_members:members "]" sp
              => :FacetDef(name, members, term)

    -- Object body sections
    obj_section_private = visibility_private obj_body_items:items => :Section(:private, items)
    obj_section_public = visibility_public obj_body_items:items => :Section(:public, items)
    obj_section_protected = visibility_protected obj_body_items:items => :Section(:protected, items)
    obj_section_facets = visibility_facets facet_body_items:items => :FacetSection(items)

    -- Object body item lists
    obj_body_item = obj_binding
    obj_body_sep = (";" sp | sp)
    obj_body_items = (obj_body_item obj_body_sep)*:items => items
    facet_body_items = (facet_def obj_body_sep)*:items => items

    -- Complete object definition
    obj_name = ident
    obj_def = "object" sp obj_name:name "{" sp obj_body_content:content "}" sp
            => :Object(name, content)

    obj_body_content = (obj_section_private | obj_section_public | obj_section_protected | obj_section_facets | obj_body_item obj_body_sep)*:items => items
```

Add `obj_def` to the `expr` rule (or to a top-level rule):
```fmpl
    expr = obj_def | if_expr | let_expr | let_stmt | ...
```

### Step 2: Add `value_to_expr` for Object

This is the most complex conversion. The `Object` tagged value needs to be transformed into `Expr::ObjectDef(ObjectDef { ... })`:

```rust
"Object" => {
    // :Object(name, content_items)
    // content_items is a list of :Section, :FacetSection, :ObjBinding
    if children.len() >= 2 {
        let name = if let Value::String(s) = &children[0] {
            QualifiedName::simple(SmolStr::new(s.as_str()))
        } else {
            return Err(Error::Runtime("Invalid Object name".to_string()));
        };
        // Parse content items into bindings and facets
        // ... (detailed conversion logic)
        Ok(Expr::ObjectDef(ObjectDef {
            name,
            params: Vec::new(),
            parents: Vec::new(),
            bindings,
            facets,
        }))
    } else {
        Err(Error::Runtime("Invalid Object node".to_string()))
    }
}
```

### Step 3: Tests, verify, commit

```bash
git commit -m "feat(grammar): add object definitions with facets (batch 5)"
```

---

## Task 7: Batch 6 -- Grammar Definitions (Meta-grammar)

This is the most complex batch. The grammar must be able to parse grammar definitions **within itself**, without delegating to the runtime GrammarParser.

**Files:** Same as previous tasks.

### Step 1: Add grammar rules for grammar definitions

The PEG grammar patterns use a different syntax from expressions. We need separate rules prefixed with `peg_`:

```fmpl
    -- PEG grammar parsing (meta-grammar)
    -- These rules parse grammar definitions within FMPL source code

    peg_string_char = "\\" escape_char:c => c | [^"\\]:c => c
    peg_string = "\"" peg_string_char*:cs "\"" sp => :StringLit(join(cs))

    peg_char_range = [^\\]]:from "-" [^\\]]:to => :Range(from, to)
                   | "\\" .:c => :Char(c)
                   | [^\\]]:c => :Char(c)
    peg_char_class = "[" "^" peg_char_range*:ranges "]" sp => :NegatedClass(ranges)
                   | "[" peg_char_range*:ranges "]" sp => :Class(ranges)

    peg_rule_ref = ident:name sp => :RuleRef(name)
    peg_any = "." sp => :Any()

    peg_primary = peg_string | peg_char_class | peg_any
                | "(" sp peg_choice:p ")" sp => p
                | peg_rule_ref

    peg_suffix = peg_primary:p "*" sp => :Star(p)
               | peg_primary:p "+" sp => :Plus(p)
               | peg_primary:p "?" sp => :Optional(p)
               | peg_primary

    peg_prefix = "~" sp peg_suffix:p => :Not(p)
               | "&" sp peg_suffix:p => :Lookahead(p)
               | peg_suffix

    peg_bind = peg_prefix:p ":" sym_ident:name sp => :Bind(p, name)
             | peg_prefix:p ":?" sym_ident:name sp => :BindChoice(p, name)
             | peg_prefix

    peg_action = peg_seq:p "=>" sp expr:action => :Action(p, action)
               | peg_seq

    peg_seq_item = peg_bind
    peg_seq = peg_seq_item:first peg_seq_item+:rest => :Seq(prepend(first, rest))
            | peg_seq_item

    peg_choice_rest = "|" sp peg_action:alt => alt
    peg_choice = peg_action:first peg_choice_rest+:rest => :Choice(prepend(first, rest))
               | peg_action

    -- Grammar rule definition
    grammar_rule = ident:name sp "=" sp peg_choice:pattern sp => :Rule(name, pattern, false)

    -- Grammar rule separator
    grammar_rule_sep = sp

    -- Grammar body
    grammar_rules = (grammar_rule grammar_rule_sep)*:rules => rules

    -- Named grammar: grammar Name { rules }
    named_grammar = "grammar" sp ident:name sp "{" sp grammar_rules:rules "}" sp
                  => :GrammarDef(name, null, rules)

    -- Let grammar: let name = grammar Name { rules }
    let_grammar = "let" sp sym_ident:name "=" sp named_grammar:g => :LetGrammar(name, g)

    -- Grammar extension: base <: { rules }
    grammar_extend = or_expr:base "<" ":" sp "{" sp grammar_rules:rules "}" sp
                   => :GrammarExtend(base, rules)
```

Add `named_grammar` to `primary` or `expr`:
```fmpl
    -- Grammar literal in expression position
    grammar_literal = "grammar" sp "{" sp grammar_rules:rules "}" sp
                    => :GrammarDef("", null, rules)
```

### Step 2: Add `value_to_expr` for grammar definitions

`GrammarDef` already exists in `value_to_ast.rs` and handles conversion to `Expr::GrammarLiteral(Grammar { ... })` by converting PEG pattern tagged values to `GrammarPattern` structs.

The `value_to_grammar_pattern` function already handles: `Any`, `StringLit`, `CharLit`, `Class`, `NegatedClass`, `RuleRef`, `Super`, `Lookahead`, `Not`, `Star`, `Plus`, `Optional`, `Bind`, `BindChoice`, `Guard`, `Seq`, `Choice`, `Action`, `Predicate`.

Add `LetGrammar` handling:
```rust
"LetGrammar" => {
    if children.len() >= 2 {
        let name = if let Value::Symbol(s) = &children[0] {
            s.clone()
        } else {
            return Err(Error::Runtime("Invalid LetGrammar name".to_string()));
        };
        // The grammar value needs to be a GrammarDef
        let grammar_expr = value_to_expr(&children[1])?;
        // Wrap as LetStmt(name, grammar_expr)
        Ok(Expr::LetStmt(name, Box::new(grammar_expr)))
    } else {
        Err(Error::Runtime("Invalid LetGrammar".to_string()))
    }
}
"GrammarExtend" => {
    if children.len() >= 2 {
        let base = value_to_expr(&children[0])?;
        // Build grammar from rules
        let mut grammar = Grammar::new(SmolStr::new(""));
        if let Value::List(rules) = &children[1] {
            for rule_val in rules.iter() {
                // Same rule parsing as GrammarDef
                // ...
            }
        }
        Ok(Expr::GrammarExtend { base: Box::new(base), rules: grammar })
    } else {
        Err(Error::Runtime("Invalid GrammarExtend".to_string()))
    }
}
```

### Step 3: Tests, verify, commit

```bash
git commit -m "feat(grammar): add grammar definitions - meta-grammar (batch 6)"
```

---

## Task 8: Batch 7 -- Advanced Iteration

**Files:** Same as previous tasks.

### Step 1: Add grammar rules

```fmpl
    -- fold: fold func, init, iter
    fold_expr = "fold" sp expr:func "," sp expr:init "," sp expr:iter => :Fold(func, init, iter)

    -- foldr: foldr func, init, iter
    foldr_expr = "foldr" sp expr:func "," sp expr:init "," sp expr:iter => :Foldr(func, init, iter)

    -- map: map elem in iter { body }
    map_expr = "map" sp sym_ident:var "in" sp expr:iter "{" sp expr:body "}" sp => :MapEach(var, iter, body)

    -- filter: filter elem in iter { body }
    filter_expr = "filter" sp sym_ident:var "in" sp expr:iter "{" sp expr:body "}" sp => :FilterExpr(var, iter, body)
```

Add to `expr`:
```fmpl
    expr = obj_def | if_expr | let_expr | let_stmt | short_lambda | full_lambda
         | return_expr | yield_expr | throw_expr | try_catch | while_loop | do_while | for_loop | match_expr
         | fold_expr | foldr_expr | map_expr | filter_expr
         | assign | or_expr
```

### Step 2: Add `value_to_expr` cases

`Fold` and `Foldr` already exist in `value_to_ast.rs`. Add:

```rust
"MapEach" => {
    if children.len() >= 3 {
        let var = if let Value::Symbol(s) = &children[0] {
            s.clone()
        } else {
            return Err(Error::Runtime("Invalid MapEach var".to_string()));
        };
        let iterable = value_to_expr(&children[1])?;
        let body = value_to_expr(&children[2])?;
        Ok(Expr::MapEach { elem_var: var, iterable: Box::new(iterable), body: Box::new(body) })
    } else {
        Err(Error::Runtime("Invalid MapEach".to_string()))
    }
}
"FilterExpr" => {
    if children.len() >= 3 {
        let var = if let Value::Symbol(s) = &children[0] {
            s.clone()
        } else {
            return Err(Error::Runtime("Invalid FilterExpr var".to_string()));
        };
        let iterable = value_to_expr(&children[1])?;
        let body = value_to_expr(&children[2])?;
        Ok(Expr::Filter { elem_var: var, iterable: Box::new(iterable), body: Box::new(body) })
    } else {
        Err(Error::Runtime("Invalid FilterExpr".to_string()))
    }
}
```

### Step 3: Tests, verify, commit

```bash
git commit -m "feat(grammar): add fold, foldr, map, filter (batch 7)"
```

---

## Task 9: Sync ir_to_rust.rs Embedded Template

The embedded `value_to_expr` template in `ir_to_rust.rs` (lines 1156-1446) must have ALL the same cases as `value_to_ast.rs`. After all batches, sync it.

**Files:**
- Modify: `fmpl-core/src/builtins/ir_to_rust.rs:1156-1446`

### Step 1: Compare the two value_to_expr implementations

Read both and identify missing cases in the embedded template.

### Step 2: Add all missing cases

Copy each new case from `value_to_ast.rs` into the template string in `ir_to_rust.rs`. The embedded template currently handles: Int, Float, Bool, Null, String, Symbol, Var, List, Map, Lambda, ShortLambda, Binary, Unary, If, Let, Tagged, QualifiedName, Call, Index, MethodCall, PropAccess.

Missing (add from value_to_ast.rs): LetSimple, Do, Fold, Foldr, Yield, Slice, AtInlineBlock, AtGrammarApply, GrammarDef, Return, ReturnVoid, Throw, Assign, Sequence, Try, While, DoWhile, For, Match, AsyncCall, SyncCall, ListCons, Placeholder, Self, Parent, Caller, User, Args, ObjTag, Spawn, FacetAccess, Object, LetGrammar, GrammarExtend, MapEach, FilterExpr.

### Step 3: Run tests, commit

```bash
cargo test -p fmpl-core
git commit -m "feat(grammar): sync ir_to_rust.rs embedded template with value_to_ast.rs"
```

---

## Task 10: Bootstrap Verification

Build `fmpl-bootstrap` and verify the generated parser passes all tests.

### Step 1: Build bootstrap binary

```bash
FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap
```

### Step 2: Rebuild fmpl-core with generated parser

```bash
cargo build -p fmpl-core
```

This triggers `build.rs` to run `fmpl-bootstrap`, which generates `generated_parser.rs` from the grammar.

### Step 3: Run all tests

```bash
cargo test -p fmpl-core
```

Expected: ALL tests pass with the generated parser (not the fallback stub).

### Step 4: Verify no regressions

```bash
cargo test
```

All workspace tests pass.

### Step 5: Commit

```bash
git add -A
git commit -m "feat(grammar): bootstrap verification - full parity achieved"
```

---

## Task 11: Update Grammar Header Comment

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl:1-20`

Update the header comment to reflect full feature coverage:

```fmpl
-- Scannerless FMPL parser - produces AST tagged values
-- Requires: lib/core/prelude.fmpl loaded first
--
-- This grammar achieves full parity with the legacy hand-written parser.
-- It can parse all valid FMPL programs, producing equivalent ASTs.
--
-- Supported features:
-- - Literals: int, float, bool, null, string
-- - Collections: lists [], maps %{}, list cons [h|t]
-- - Variables, qualified names (foo::bar), placeholders (_)
-- - Tagged values: :Tag(args...)
-- - Symbols: :name, :==, etc.
-- - Arithmetic: +, -, *, /, %
-- - Comparisons: ==, !=, <, >, <=, >=
-- - Logical: &&, ||
-- - Unary: -, !
-- - Pipe: |>
-- - Control flow: if/then/else, while/do, do/while, for/in, match
-- - Functions: lambdas (\x expr, lambda(x, y) expr), calls f(args)
-- - Statements: let x = expr, return, yield, throw, assignment
-- - Exception handling: try/catch
-- - Postfix: indexing [idx], property .name, method .name(args), slice [s..e]
-- - Object system: object def, self/parent/caller/user/args, ^tag, spawn, .as(:facet)
-- - Grammar definitions: grammar Name { rules }, grammar extension (<:)
-- - @ operator: grammar apply (expr @ g.rule), inline patterns (expr @ { ... })
-- - Async/sync: <- expr
-- - Iteration: fold, foldr, map, filter
-- - Comments: --, //, /* */
-- - Sequences: { expr; expr; ... }, semicolon separation
```

### Commit

```bash
git commit -m "docs(grammar): update header to reflect full parity"
```

---

## Task 12: Update Design Doc Status

**Files:**
- Modify: `docs/plans/2026-02-26-feature-complete-generated-grammar-design.md`

Update the Implementation Status section:

```markdown
## Implementation Status

- **Complete**: All batches (1-7) -- full parity with legacy parser
- **Verified**: Bootstrap builds and all tests pass with generated parser
```

### Commit

```bash
git commit -m "docs: mark feature-complete grammar design as complete"
```

---

## Verification Checklist

After all tasks:

```bash
# Unit tests
cargo test -p fmpl-core

# Parser equivalence
cargo test -p fmpl-core parser_equivalence

# Generated parser correctness
cargo test -p fmpl-core generated_parser_correctness

# Bootstrap
FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap
cargo build -p fmpl-core
cargo test -p fmpl-core

# Full workspace
cargo test

# Tavern demo still works
cargo test -p fmpl-core tavern_demo
```
