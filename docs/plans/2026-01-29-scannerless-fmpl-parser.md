# Scannerless FMPL Parser Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a scannerless PEG grammar written in FMPL that parses FMPL source code to AST tagged values, enabling a true self-interpreter.

**Architecture:**
- `lib/core/prelude.fmpl` - Helper functions (`to_int`, `join`, `reduce`, `fold_binary`)
- `lib/core/fmpl_parser.fmpl` - Scannerless grammar that produces AST tagged values
- Grammar actions use prelude helpers via direct function calls
- Load order: prelude first, then parser grammar

**Tech Stack:** FMPL grammar system (`base::parser`), FMPL helper functions, existing `@` operator

---

## Task 1: Create Prelude with `to_int` for Single Characters

**Files:**
- Create: `lib/core/prelude.fmpl`
- Create: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
// fmpl-core/tests/core_prelude.rs
use fmpl_core::{eval, Value, Vm};

#[test]
fn test_to_int_digit_0() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        to_int("0")
    "#).unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_to_int_digit_9() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        to_int("9")
    "#).unwrap();
    assert_eq!(result, Value::Int(9));
}

#[test]
fn test_to_int_digit_5() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        to_int("5")
    "#).unwrap();
    assert_eq!(result, Value::Int(5));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_to_int`
Expected: FAIL - file not found or function undefined

**Step 3: Write minimal implementation**

```fmpl
-- lib/core/prelude.fmpl
-- Core helper functions for FMPL grammar actions

-- Convert a single digit character to its integer value
-- to_int("0") => 0, to_int("9") => 9
let to_int = \c c @ {
    "0" => 0
    "1" => 1
    "2" => 2
    "3" => 3
    "4" => 4
    "5" => 5
    "6" => 6
    "7" => 7
    "8" => 8
    "9" => 9
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_to_int`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/prelude.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add prelude with to_int for digit characters"
```

---

## Task 2: Add `join` Function to Prelude

**Files:**
- Modify: `lib/core/prelude.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
// Add to fmpl-core/tests/core_prelude.rs
#[test]
fn test_join_empty_list() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        join([])
    "#).unwrap();
    assert_eq!(result, Value::String("".into()));
}

#[test]
fn test_join_single_char() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        join(["a"])
    "#).unwrap();
    assert_eq!(result, Value::String("a".into()));
}

#[test]
fn test_join_multiple_chars() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        join(["h", "e", "l", "l", "o"])
    "#).unwrap();
    assert_eq!(result, Value::String("hello".into()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_join`
Expected: FAIL - function undefined

**Step 3: Write minimal implementation**

```fmpl
-- Add to lib/core/prelude.fmpl

-- Join a list of strings into a single string
-- join(["a", "b", "c"]) => "abc"
let join = \list list.concat()
```

Note: This assumes `.concat()` exists on lists. If not, we need:

```fmpl
-- Alternative using reduce if concat doesn't exist
let join = \list reduce(\acc \s acc ++ s, "", list)
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_join`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/prelude.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add join function to prelude"
```

---

## Task 3: Add `reduce` Function to Prelude

**Files:**
- Modify: `lib/core/prelude.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_reduce_sum() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        reduce(\acc \x acc + x, 0, [1, 2, 3, 4])
    "#).unwrap();
    assert_eq!(result, Value::Int(10));
}

#[test]
fn test_reduce_empty() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        reduce(\acc \x acc + x, 0, [])
    "#).unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_reduce_digits_to_int() {
    let mut vm = Vm::new();
    // reduce((\acc \d acc * 10 + to_int(d)), 0, ["1", "2", "3"]) => 123
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        reduce(\acc \d acc * 10 + to_int(d), 0, ["1", "2", "3"])
    "#).unwrap();
    assert_eq!(result, Value::Int(123));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_reduce`
Expected: FAIL - function undefined

**Step 3: Write minimal implementation**

```fmpl
-- Add to lib/core/prelude.fmpl

-- Left fold: reduce(f, init, [a, b, c]) => f(f(f(init, a), b), c)
let reduce = \f \init \list
    if list.len() == 0
    then init
    else reduce(f, f(init, list[0]), list[1:])
```

Note: This uses list slicing `list[1:]`. If that doesn't exist, we need a different approach using indices.

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_reduce`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/prelude.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add reduce function to prelude"
```

---

## Task 4: Add `fold_binary` for Left-Associative Operators

**Files:**
- Modify: `lib/core/prelude.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fold_binary_empty_rest() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), []) => :Int(1)
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        fold_binary(:Int(1), [])
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(1));
    } else {
        panic!("expected Tagged");
    }
}

#[test]
fn test_fold_binary_single_op() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), [[:+, :Int(2)]]) => :Binary(:+, :Int(1), :Int(2))
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        fold_binary(:Int(1), [[:+, :Int(2)]])
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    } else {
        panic!("expected Tagged(:Binary, ...)");
    }
}

#[test]
fn test_fold_binary_multiple_ops() {
    let mut vm = Vm::new();
    // fold_binary(:Int(1), [[:+, :Int(2)], [:+, :Int(3)]])
    // => :Binary(:+, :Binary(:+, :Int(1), :Int(2)), :Int(3))
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        fold_binary(:Int(1), [[:+, :Int(2)], [:+, :Int(3)]])
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        // The left child should also be a Binary
        if let Value::Tagged(left_tag, _) = &children[1] {
            assert_eq!(left_tag.as_str(), "Binary");
        } else {
            panic!("expected nested Binary");
        }
    } else {
        panic!("expected Tagged(:Binary, ...)");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fold_binary`
Expected: FAIL - function undefined

**Step 3: Write minimal implementation**

```fmpl
-- Add to lib/core/prelude.fmpl

-- Fold operator/operand pairs into left-associative binary tree
-- fold_binary(first, [[op1, e1], [op2, e2]]) => :Binary(op2, :Binary(op1, first, e1), e2)
let fold_binary = \first \rest
    reduce(\acc \pair :Binary(pair[0], acc, pair[1]), first, rest)
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fold_binary`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/prelude.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add fold_binary for left-associative operators"
```

---

## Task 5: Create Minimal FMPL Parser Grammar (Integer Literals Only)

**Files:**
- Create: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_int_literal() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "42" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(42));
    } else {
        panic!("expected Tagged(:Int, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_int_literal_single_digit() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "7" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Int");
        assert_eq!(children[0], Value::Int(7));
    } else {
        panic!("expected Tagged(:Int, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_int`
Expected: FAIL - grammar not found

**Step 3: Write minimal implementation**

```fmpl
-- lib/core/fmpl_parser.fmpl
-- Scannerless FMPL parser - produces AST tagged values
-- Requires: lib/core/prelude.fmpl loaded first

let fmpl_parser = grammar fmpl_parser <: base::parser {
    -- Whitespace (consumed but not captured)
    ws = [ \t\n\r]*
    _ = ws

    -- Integer literal: produces :Int(n)
    -- Uses reduce to convert digit list to integer
    int = ds:digit+ _ => :Int(reduce(\acc \d acc * 10 + to_int(d), 0, ds))

    -- Entry point (for now, just integers)
    primary = int
    expr = primary
    code = _ e:expr _ => e
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_int`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add minimal fmpl_parser grammar with integer literals"
```

---

## Task 6: Add Boolean and Null Literals to Parser

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_bool_true() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "true" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Bool");
        assert_eq!(children[0], Value::Bool(true));
    } else {
        panic!("expected Tagged(:Bool, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_bool_false() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "false" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Bool");
        assert_eq!(children[0], Value::Bool(false));
    } else {
        panic!("expected Tagged(:Bool, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_null() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "null" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Null");
        assert!(children.is_empty());
    } else {
        panic!("expected Tagged(:Null, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_bool`
Expected: FAIL - grammar doesn't match

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl - add to grammar rules

    -- Boolean literals
    bool = "true" _ => :Bool(true)
         | "false" _ => :Bool(false)

    -- Null literal
    null_lit = "null" _ => :Null()

    -- Update primary to include new literals
    primary = int | bool | null_lit
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_bool`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add boolean and null literals to fmpl_parser"
```

---

## Task 7: Add Identifiers to Parser

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_identifier() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "foo" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Var");
        assert_eq!(children[0], Value::Symbol("foo".into()));
    } else {
        panic!("expected Tagged(:Var, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_identifier_with_underscore() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "my_var" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Var");
        assert_eq!(children[0], Value::Symbol("my_var".into()));
    } else {
        panic!("expected Tagged(:Var, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_keyword_not_identifier() {
    let mut vm = Vm::new();
    // "true" should parse as Bool, not Var
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "true" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, _) = result {
        assert_eq!(tag.as_str(), "Bool"); // Not "Var"
    } else {
        panic!("expected Tagged(:Bool, ...)");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_identifier`
Expected: FAIL - grammar doesn't match identifiers

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl

    -- Identifier characters
    ident_start = letter | "_"
    ident_char = letter | digit | "_"

    -- Keywords (must not be followed by ident_char)
    keyword = ("let" | "if" | "then" | "else" | "true" | "false" | "null") ~ident_char

    -- Identifier: not a keyword, starts with letter or underscore
    ident = ~keyword first:ident_start rest:ident_char* _ => join([first] ++ rest)

    -- Variable reference
    var = name:ident => :Var(:name)

    -- Update primary (put var last to prefer keywords)
    primary = int | bool | null_lit | var
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_identifier`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add identifier parsing to fmpl_parser"
```

---

## Task 8: Add Arithmetic Operators

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_addition() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "1 + 2" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_multiplication() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "3 * 4" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("*".into()));
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_precedence() {
    let mut vm = Vm::new();
    // 1 + 2 * 3 should parse as 1 + (2 * 3), not (1 + 2) * 3
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "1 + 2 * 3" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("+".into()));
        // Right child should be Binary(:*, ...)
        if let Value::Tagged(right_tag, _) = &children[2] {
            assert_eq!(right_tag.as_str(), "Binary");
        }
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_addition`
Expected: FAIL - grammar doesn't handle operators

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl

    -- Addition/subtraction operators
    add_op = "+" _ => :+
           | "-" _ => :-

    -- Multiplication/division/modulo operators
    mult_op = "*" _ => :*
            | "/" _ => :/
            | "%" _ => :%

    -- Unary operators
    unary_expr = "-" _ e:unary_expr => :Unary(:-, e)
               | "!" _ e:unary_expr => :Unary(:!, e)
               | primary

    -- Multiplication level (higher precedence)
    mult_expr = first:unary_expr rest:(op:mult_op e:unary_expr => [op, e])*
                => fold_binary(first, rest)

    -- Addition level (lower precedence)
    add_expr = first:mult_expr rest:(op:add_op e:mult_expr => [op, e])*
               => fold_binary(first, rest)

    -- Expression entry point
    expr = add_expr

    -- Parenthesized expressions
    primary = "(" _ e:expr ")" _ => e
            | int | bool | null_lit | var
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_addition`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add arithmetic operators to fmpl_parser"
```

---

## Task 9: Add Comparison Operators

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_comparison() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "1 < 2" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("<".into()));
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}

#[test]
fn test_fmpl_parser_equality() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "x == y" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Binary");
        assert_eq!(children[0], Value::Symbol("==".into()));
    } else {
        panic!("expected Tagged(:Binary, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_comparison`
Expected: FAIL

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl

    -- Comparison operators (must check longer ones first)
    cmp_op = "==" _ => :==
           | "!=" _ => :!=
           | "<=" _ => :<=
           | ">=" _ => :>=
           | "<" _ => :<
           | ">" _ => :>

    -- Comparison level (lower than arithmetic)
    cmp_expr = lhs:add_expr op:cmp_op rhs:add_expr => :Binary(op, lhs, rhs)
             | add_expr

    -- Update expr
    expr = cmp_expr
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_comparison`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add comparison operators to fmpl_parser"
```

---

## Task 10: Add If/Then/Else

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_if_then_else() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "if true then 1 else 2" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "If");
        assert_eq!(children.len(), 3);
    } else {
        panic!("expected Tagged(:If, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_if`
Expected: FAIL

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl

    -- If/then/else expression
    if_expr = "if" _ cond:expr "then" _ then_branch:expr "else" _ else_branch:expr
              => :If(cond, then_branch, else_branch)

    -- Update expr to include if
    expr = if_expr | cmp_expr
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_if`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add if/then/else to fmpl_parser"
```

---

## Task 11: Add Let Bindings

**Files:**
- Modify: `lib/core/fmpl_parser.fmpl`
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_fmpl_parser_let() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        "let (x = 1) x + 1" @ fmpl_parser.code
    "#).unwrap();
    if let Value::Tagged(tag, children) = result {
        assert_eq!(tag.as_str(), "Let");
        assert_eq!(children.len(), 2); // bindings list and body
    } else {
        panic!("expected Tagged(:Let, ...), got {:?}", result);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_let`
Expected: FAIL

**Step 3: Write minimal implementation**

```fmpl
-- Update lib/core/fmpl_parser.fmpl

    -- Let binding
    let_expr = "let" _ "(" _ name:ident "=" _ value:expr ")" _ body:expr
               => :Let([[:Binding(:name, value)]], body)

    -- Update expr
    expr = let_expr | if_expr | cmp_expr
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p fmpl-core --test core_prelude test_fmpl_parser_let`
Expected: PASS

**Step 5: Commit**

```bash
git add lib/core/fmpl_parser.fmpl fmpl-core/tests/core_prelude.rs
git commit -m "feat(core): add let bindings to fmpl_parser"
```

---

## Task 12: Integration Test - Full Self-Interpreter Pipeline

**Files:**
- Modify: `fmpl-core/tests/core_prelude.rs`

**Step 1: Write the integration test**

```rust
#[test]
fn test_self_interpreter_integer() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        let (ast = "42" @ fmpl_parser.code)
        let (ir = ast @ { :Int(n) => :LoadInt(n) })
        code::eval(ir::compile(ir))
    "#).unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_self_interpreter_addition() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        let (ast = "1 + 2" @ fmpl_parser.code)
        let (ir = ast @ {
            :Int(n) => :LoadInt(n)
            :Binary(:+, l, r) => :Add(l @ ir, r @ ir)
        })
        code::eval(ir::compile(ir))
    "#).unwrap();
    assert_eq!(result, Value::Int(3));
}

#[test]
fn test_self_interpreter_conditional() {
    let mut vm = Vm::new();
    let result = eval(&mut vm, r#"
        io::load("lib/core/prelude.fmpl")
        io::load("lib/core/fmpl_parser.fmpl")
        let (ast = "if true then 1 else 2" @ fmpl_parser.code)
        let (ir = ast @ {
            :Int(n) => :LoadInt(n)
            :Bool(b) => :LoadBool(b)
            :If(c, t, e) => :If(c @ ir, t @ ir, e @ ir)
        })
        code::eval(ir::compile(ir))
    "#).unwrap();
    assert_eq!(result, Value::Int(1));
}
```

**Step 2: Run tests**

Run: `cargo test -p fmpl-core --test core_prelude test_self_interpreter`
Expected: PASS (if all previous tasks completed)

**Step 3: Commit**

```bash
git add fmpl-core/tests/core_prelude.rs
git commit -m "test(core): add self-interpreter integration tests"
```

---

## Summary

This plan builds the scannerless FMPL parser incrementally:

1. **Tasks 1-4:** Prelude helpers (`to_int`, `join`, `reduce`, `fold_binary`)
2. **Tasks 5-11:** Grammar rules (literals, identifiers, operators, control flow)
3. **Task 12:** Integration test proving the self-interpreter pipeline works

Each task follows TDD: write failing test, implement, verify, commit.

The final result enables:
```fmpl
io::load("lib/core/prelude.fmpl")
io::load("lib/core/fmpl_parser.fmpl")
"1 + 2" @ fmpl_parser.code  -- Returns :Binary(:+, :Int(1), :Int(2))
```
