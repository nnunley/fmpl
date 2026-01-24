# Parser Limitations and Known Issues

**Status**: Documenting
**Date**: 2026-01-24
**Type**: Known Issues

---

## Summary

This document tracks known limitations in the FMPL parser that affect language usability. These are not bugs, but features that are not yet implemented.

---

## Known Issues

### Issue 1: Assignment Syntax (`=`) - PARTIALLY IMPLEMENTED ✅

**Status**: IMPLEMENTED (2026-01-24)
**Impact**: HIGH - Variable mutation now supported

**Description**: The assignment operator `=` is now implemented for simple variable mutation.

**What works**:
```fmpl
-- Simple variable mutation
let x = 10
x = 20  -- x is now 20

-- Right-associative chaining
let a = 1
let b = 2
let c = 3
a = b = c  -- All become 3

-- Assignment returns the assigned value
let result = b = c  -- result is 3, b is now 3
```

**Limitations**:
- Only simple identifiers are supported as assignment targets
- Property assignment (e.g., `obj.prop = value`) is NOT yet supported
- Qualified name assignment (e.g., `module::var = value`) is NOT yet supported

**What doesn't work yet**:
```fmpl
-- Property assignment (not supported)
obj.prop = 10

-- Qualified name assignment (not supported)
module::var = 20

-- Complex patterns as targets (not supported)
%{key: val} = some_map
```

---

### Issue 2: Qualified Names Starting with `::` Don't Parse

**Status**: FIXED (2026-01-24)
**Impact**: MEDIUM → LOW - Global namespace access now supported

**Description**: Qualified names starting with `::` (global namespace) are now supported.

**What works**:
```fmpl
-- Global qualified name
let x = ::__builtin_curl  -- Now parses correctly!

-- Method call on global qualified name
let result = ::__builtin_curl.get("https://example.com")

-- Multi-part global qualified names
let y = ::foo::bar::baz
```

**How it works**: The parser now recognizes `::` as the start of a global qualified name. The qualified name is represented with an empty string as the first part, e.g., `::__builtin_curl` becomes `["", "__builtin_curl"]`.

**Previous Workaround**: No longer needed - you can now use `::` prefix directly for accessing global builtins.

---

### Issue 3: Comment Syntax Uses `--`, Not `#`

**Status**: FIXED - Documentation updated
**Impact**: LOW - Affects library code documentation

**Description**: FMPL uses `--` for comments (like Haskell), not `#` (like Python/Shell). The lexer only recognizes `--` and `//` as comment starters.

**Example**:
```fmpl
-- This is a valid comment
// This is also a valid comment
# This is NOT a comment - causes "unexpected character: #" error
```

---

### Issue 4: Complex Pattern Matching Cases

**Status**: PARTIAL - Basic patterns work, complex cases may fail
**Impact**: MEDIUM - Affects tool dispatch and data extraction

**Description**: While basic `@` pattern matching works, certain complex patterns may fail at parse time. This is an area of active development.

**What works**:
```fmpl
-- Simple tool dispatch
@ tool_call {
  %{tool: "curl.get", args: %{url: url}} => curl.get(url)
  _ => %{error: "unknown_tool"}
}
```

**What may fail**:
```fmpl
-- Multiple patterns with same keys in different order
@ tool_call {
  %{tool: "curl.get", args: _} => ...
  %{tool: other, args: _} => ...  -- May cause parse errors
}
```

---

## Migration Guide

### For Library Code Authors

1. **Use `--` for comments**, not `#`
2. **Assignment syntax (`=`)** is now supported for simple variable mutation
3. **Test basic patterns first** - simple `@ { %{key: val} => ... }` patterns work reliably
4. **Global qualified names** (`::foo::bar`) are now supported - you can use `::__builtin_curl.get` directly

### For Tool Execution

Use pattern matching for dispatch:

```fmpl
-- Define tool dispatch using @ pattern matching
let execute_tool = \tool_call
  @ tool_call {
    %{tool: "curl.get", args: %{url: url}} => curl.get(url)
    %{tool: "curl.post", args: %{url: url, body: body}} => curl.post(url, body)
    %{tool: t, args: _} => %{error: "unknown_tool", tool: t}
    _ => %{error: "invalid_format"}
  }

-- Use it
let result = execute_tool(%{tool: "curl.get", args: %{url: "https://example.com"}})
```

---

## References

- `AGENTS.md` - Overall project documentation and known limitations
- `specs/pattern-matching.md` - Pattern matching implementation details
- `specs/llm-tool-calling.md` - Tool calling specification
- `fmpl-core/src/lexer.rs` - Token definitions and comment syntax
- `fmpl-core/src/parser.rs` - Expression parsing logic
