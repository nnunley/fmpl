# FMPL Demo

This document demonstrates the current state of FMPL with working examples.

## Working Features

### 1. Basic Arithmetic and Variables
```fmpl
let a = 10
let b = 20
a + b
```
Result: `30`

### 2. Lists and Indexing
```fmpl
let numbers = [1, 2, 3, 4, 5]
numbers[0]  // => 1
numbers[4]  // => 5
```

### 3. List Methods
```fmpl
let numbers = [1, 2, 3]
numbers.len()    // => 3
numbers.push(4)  // => [1, 2, 3, 4]
```

### 4. Map/Object Literals
```fmpl
let person = %{name: "Alice", age: 30, active: true}
person.name  // => "Alice"
```

### 5. String Operations
```fmpl
let greeting = "Hello, "
let target = "FMPL!"
greeting + target  // => "Hello, FMPL!"
```

### 6. Cursor and Stream Observation
```fmpl
let data = [10, 20, 30]
let cursor = stream::observe(data)
cursor::current(cursor)  // => 10
```

### 7. Cursor Operations
```fmpl
let data = [10, 20, 30]
let cursor = stream::observe(data)

// Get current position
cursor::position(cursor)  // => 0 (Int)

// Advance cursor
let advanced = cursor::advance(cursor, 1)
cursor::current(advanced)  // => 20

// Rewind cursor
let rewound = cursor::rewind(advanced, 1)
cursor::current(rewound)  // => 10
```

### 8. Multiple Cursors (Copy-on-Write)
```fmpl
let data = [10, 20, 30]
let cursor1 = stream::observe(data)
let cursor2 = stream::observe(data)

cursor1::position(cursor1)  // => 0
cursor2::position(cursor2)  // => 0 (independent cursor)

let advanced = cursor::advance(cursor1, 1)
cursor1::position(advanced)   // => 1
cursor2::position(cursor2)   // => 0 (unchanged)
```

### 9. Grammar Application (Pattern Matching)
```fmpl
-- Pattern match on values, with a guard.
-- `if` and `when` are interchangeable guard keywords.
let value = 42
value @ {
  n if n > 0 => n * 2,
  _ => 0
}
// => 84
```

## Known Limitations

### 1. For Loop Mutation
For loops currently cannot mutate variables from outer scope due to scope boundary:

```fmpl
let sum = 0
for x in [1, 2, 3] {
  sum = sum + x  -- This creates a new 'sum' in inner scope
}
sum  // => 0 (outer sum unchanged)
```

**Workaround**: Use continuation-style or collect results:

```fmpl
let numbers = [1, 2, 3, 4, 5]
let doubled = numbers.map(\x x * 2)
doubled.fold(0, \acc, x acc + x)  // => 30
```

Note: multi-argument short lambdas separate parameters with commas
(`\acc, x ...`), not spaces.

### 2. Grammar Loading
Grammar files exist (`lib/json.fmpl`, `lib/yaml.fmpl`) but need to be explicitly loaded in the VM. The `@` operator works with inline grammars:

```fmpl
let value = 42
value @ {
  n if n > 0 => n,
  _ => 0
}
```

But accessing `json.value` requires the grammar to be loaded first.

## Example Session

```
fmpl> let greeting = "Hello, "
=> "Hello, "

fmpl> let target = "FMPL!"
=> "FMPL!"

fmpl> greeting + target
=> "Hello, FMPL!"

fmpl> let data = [10, 20, 30]
=> [10, 20, 30]

fmpl> let cursor = stream::observe(data)
=> <cursor branch:main pos:0>

fmpl> cursor::current(cursor)
=> 10

fmpl> cursor::advance(cursor, 1)
=> <cursor branch:main pos:1>

fmpl> cursor::current(cursor)
=> 20

fmpl> .quit
Bye!
```

## Status Summary

**Working**:
- Core VM and evaluation
- Variables and arithmetic
- Lists and list methods
- Map/object literals
- Strings and string operations
- Cursor/stream observation
- Cursor operations (advance, rewind, position)
- Pattern matching with `@` operator (inline grammars), guards (`when`/`if`)
- List higher-order methods (`map`, `fold`)
- Universal type predicates (`.is_number()`, `.is_string()`, `.type_name()`, …)

**Needs Work**:
- Grammar loading from .fmpl files
- For loop scope mutation (design decision needed)
- JSON/YAML parsing integration
