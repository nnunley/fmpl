# `in` Operator Implementation

## Overview

Adds membership testing operator `x in list` to FMPL for CSP solving and general collection membership checks.

## Syntax

### Membership Test

```fmpl
x in list          # Returns true if x is an element of list
!(x in list)       # Negation: true if x is NOT in list
```

### Supported Collection Types

1. **Lists**: Check if element is in list
   ```fmpl
   3 in [1, 2, 3, 4]      # true
   5 in [1, 2, 3, 4]      # false
   !(5 in [1, 2, 3, 4])   # true
   ```

2. **Strings**: Check if substring exists
   ```fmpl
   "ll" in "hello"          # true
   "x" in "hello"           # false
   !("x" in "hello")        # true
   ```

3. **Maps**: Check if key exists
   ```fmpl
   "name" in %{name: "Bob"}  # true
   "age" in %{name: "Bob"}    # false
   !("age" in %{name: "Bob"}) # true
   ```

## Implementation

### AST Changes

**File: `src/ast.rs`**

```rust
pub enum BinOp {
    // ... existing operators ...
    In,  // 'x in list' membership test
}
```

### Compiler Changes

**File: `src/compiler.rs`**

1. Add `In` instruction to `Instruction` enum (after comparison operators):
```rust
In { lhs: InstrIndex, rhs: InstrIndex },  // Membership test: lhs in rhs
```

2. Add `BinOp::In` case in `compile_binary()`:
```rust
BinOp::In => self.code.emit(Instruction::In { lhs, rhs }),
```

### VM Changes

**File: `src/vm.rs`**

Add `In` instruction handler in the main execution loop (after `GtEq`):
```rust
Instruction::In { lhs, rhs } => {
    let elem = frame.get(lhs);
    let collection = frame.get(rhs);
    let result = match collection {
        Value::List(items) => items.contains(&elem),
        Value::String(s) => {
            match elem {
                Value::String(elem_str) => s.contains(elem_str.as_str()),
                _ => false,
            }
        }
        Value::Map(map) => {
            match elem {
                Value::String(key) => map.contains_key(key.as_str()),
                Value::Symbol(key) => map.contains_key(key.as_str()),
                _ => false,
            }
        }
        _ => false,
    };
    frame.set_current(Value::Bool(result));
}
```

### Parser Changes

**File: `src/parser.rs`**

Add `Token::In` case in `parse_comparison()` function:
```rust
fn parse_comparison(&mut self) -> Result<Expr> {
    let mut left = self.parse_term()?;

    loop {
        let op = if self.check(&Token::Lt) {
            BinOp::Lt
        } else if self.check(&Token::Gt) {
            BinOp::Gt
        } else if self.check(&Token::LtEq) {
            BinOp::LtEq
        } else if self.check(&Token::GtEq) {
            BinOp::GtEq
        } else if self.check(&Token::In) {
            BinOp::In
        } else {
            break;
        };
        self.advance();
        let right = self.parse_term()?;
        left = Expr::Binary(Box::new(left), op, Box::new(right));
    }

    Ok(left)
}
```

Note: The `Token::In` already exists in the lexer (`src/lexer.rs`).

### Display/Repr Changes

**File: `src/repr.rs`**

Add display formatting for `BinOp::In`:
```rust
BinOp::In => write!(f, " in "),
```

**File: `src/builtins/ast.rs`**

Add string conversion for `BinOp::In`:
```rust
BinOp::In => " in ",
```

## Operator Precedence

The `in` operator is at the **comparison level**, same precedence as `<`, `>`, `<=`, `>=`:

```fmpl
# Higher precedence (tighter binding)
x + y * z

# Comparison level (in is here)
x < y
x in list
!(x in list)     # Unary ! binds tighter than in

# Lower precedence (looser binding)
x && y
x || y
```

## Examples

### CSP Usage

```fmpl
# Check if digit is already used
?digit:d when !(d in [s, e, n])

# Multiple membership checks
!(x in [1, 2, 3]) && !(y in [4, 5, 6])
```

### String Substring Search

```fmpl
"hello" in "hello world"     # true
"world" in "hello world"     # true
!("x" in "hello world")      # true (negated)
```

### Map Key Lookup

```fmpl
let person = %{name: "Alice", age: 30}
"name" in person              # true
!("email" in person)          # true (email key doesn't exist)
```

## Testing

```fmpl
# Test list membership
assert(3 in [1, 2, 3, 4])

# Test string membership
assert("ll" in "hello")

# Test negation
assert(!(5 in [1, 2, 3]))

# Test CSP-style constraints
let s = 9
let e = 5
assert(!(e in [s]))
```
