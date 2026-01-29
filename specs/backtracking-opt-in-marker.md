# Backtracking Opt-in Marker Specification

## Overview

Add a `?` marker to explicitly opt-in to backtracking for specific grammar rules or pattern alternatives. This makes backtracking **explicit** rather than global, avoiding performance costs for unambiguous grammars while enabling it where needed.

## Syntax

### Rule-Level Marker

```fmpl
grammar BacktrackingExample {
    # This rule uses backtracking (explore all alternatives)
    ?ambiguous = "a" | [a-z]

    # This rule uses traditional PEG semantics (first match wins)
    specific = "x" | "y" | "z"
}
```

### Alternative-Level Marker

```fmpl
grammar MixedExample {
    # Only specific alternatives participate in backtracking
    choice =
        ?"a" |          # Backtrack: try both "a" and [a-z]
        "b" |           # Traditional: stop at first match
        ?[0-9]          # Backtrack: try all digits
}
```

### Semantics

1. **Rule with `?` prefix**: ALL alternatives in that rule use backtracking
2. **Alternative with `?` prefix**: Only that alternative participates in backtracking
3. **No `?` marker**: Traditional PEG semantics (ordered choice, first match wins)

## Implementation Plan

### Phase 1: Grammar AST Changes

**File: `src/grammar/mod.rs`**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub name: SmolStr,
    pub pattern: Pattern,
    pub action: Option<crate::ast::Expr>,
    pub backtracking: bool,  // NEW: whether this rule uses backtracking
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    // Existing variants...

    /// Choice with optional backtracking marker per alternative
    Choice {
        alternatives: Vec<(Pattern, bool)>,  // (pattern, uses_backtracking)
    },
}
```

### Phase 2: Parser Changes

**File: `src/grammar/parser.rs`**

```rust
// Parse rule with optional `?` marker
fn parse_rule(&mut self) -> Result<Rule> {
    let backtracking = if self.peek_char() == Some('?') {
        self.advance();
        true
    } else {
        false
    };

    let name = self.parse_ident()?;
    // ... rest of rule parsing

    Ok(Rule { name, pattern, action, backtracking })
}

// Parse choice with optional `?` markers on alternatives
fn parse_choice(&mut self) -> Result<Pattern> {
    self.expect_char('|')?;
    let mut alternatives = Vec::new();

    loop {
        // Check for `?` marker on this alternative
        let uses_backtracking = if self.peek_char() == Some('?') {
            self.advance();
            true
        } else {
            false
        };

        let pattern = self.parse_pattern()?;  // or parse_sequence
        alternatives.push((pattern, uses_backtracking));

        match self.peek_char() {
            Some('|') => {
                self.advance();
                continue;
            }
            _ => break,
        }
    }

    Ok(Pattern::Choice { alternatives })
}
```

### Phase 3: Compiler Changes

**File: `src/compiler.rs`**

When compiling Choice patterns:
1. Check if any alternative has `?` marker
2. If any alternative marked, collect all successful alternatives
3. Push choice point onto backtracking stack

```rust
GP::Choice(alternatives) => {
    let any_marked = alternatives.iter().any(|(_, uses_bt)| *uses_bt);

    if any_marked {
        // Try ALL marked alternatives, collect successes
        for (alt, uses_bt) in alternatives {
            if *uses_bt {
                // Parse and add to successful_alternatives
            } else {
                // Traditional: return first match immediately
                match self.match_pattern(alt)? {
                    ParseResult::Success(v, new_index) => {
                        return Ok(ParseResult::Success(v, new_index));
                    }
                    ParseResult::Failure => continue,
                }
            }
        }
        // Create choice point on backtracking stack
    } else {
        // Traditional PEG: try alternatives in order
        for (alt, _) in alternatives {
            match self.match_pattern(alt)? {
                ParseResult::Success(v, new_index) => {
                    return Ok(ParseResult::Success(v, new_index));
                }
                ParseResult::Failure => continue,
            }
        }
    }
}
```

### Phase 4: Runtime Changes

**File: `src/grammar/runtime.rs`**

```rust
Pattern::Choice(alternatives) => {
    let any_marked = alternatives.iter().any(|(_, uses_bt)| *uses_bt);

    if !any_marked {
        // Traditional PEG: no backtracking needed
        for (alt, _) in alternatives {
            match self.match_pattern(alt, pos.clone())? {
                ParseResult::Success(v, new_index) => {
                    return Ok(ParseResult::Success(v, new_index));
                }
                ParseResult::Failure => continue,
            }
        }
        return Ok(ParseResult::Failure);
    }

    // Backtracking mode: try all MARKED alternatives
    let mut successful_alternatives: Vec<(Value, usize)> = Vec::new();

    for (alt, uses_bt) in alternatives {
        if !uses_bt {
            // Unmarked alternatives: traditional semantics
            match self.match_pattern(alt, pos.clone())? {
                ParseResult::Success(v, new_index) => {
                    return Ok(ParseResult::Success(v, new_index));
                }
                ParseResult::Failure => continue,
            }
        } else {
            // Marked alternatives: collect for backtracking
            match self.match_pattern(alt, pos.clone())? {
                ParseResult::Success(v, new_index) => {
                    successful_alternatives.push((v, new_index));
                }
                ParseResult::Failure => continue,
            }
        }
    }

    // Create choice point if multiple marked alternatives succeeded
    if successful_alternatives.len() > 1 {
        let choice_entry = BacktrackEntry::Choice {
            position: start_index,
            alternatives: successful_alternatives.clone(),
            next_index: 1,
        };
        self.backtracking_stack.push(choice_entry);

        // Return first alternative
        let (value, end_pos) = successful_alternatives.into_iter().next().unwrap();
        Ok(ParseResult::Success(value, end_pos))
    } else if successful_alternatives.len() == 1 {
        let (value, end_pos) = successful_alternatives.into_iter().next().unwrap();
        Ok(ParseResult::Success(value, end_pos))
    } else {
        Ok(ParseResult::Failure)
    }
}
```

## Examples

### Example 1: CSP Digit Generation

```fmpl
grammar CSP {
    # Generate digits with backtracking
    ?digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
}
```

When parsing "1", this will create a choice point with 10 alternatives (all digits), enabling backtracking search.

### Example 2: Mixed Semantics

```fmpl
grammar Mixed {
    # Ambiguous: both match "a"
    ?ambig = "a" | [a-z]

    # Traditional: stops at first match
    ordered = "x" | "y" | [a-z]
}
```

- `ambig` creates choice points (2 ways to match "a")
- `ordered` uses traditional PEG (returns "x", never tries "y" or `[a-z]`)

### Example 3: Alternative-Level Control

```fmpl
grammar Selective {
    choice =
        ?"a" |    # Explore this branch
        "b" |     # Stop here if matches (traditional)
        ?[c-e]    # Explore this branch too
}
```

For input "a": tries both `"a"` and `[c-e]` (backtracking)
For input "b": returns immediately (traditional, no backtracking)

## Benefits

1. **Explicit control**: Users choose where backtracking happens
2. **Zero cost by default**: Unmarked rules use fast PEG semantics
3. **Fine-grained control**: Mark specific alternatives within choices
4. **No global mode**: Don't need `with_backtracking_mode()`
5. **Backward compatible**: Existing grammars work unchanged

## Migration Path

- Existing grammars without `?` work exactly as before (PEG semantics)
- Add `?` to rules that need CSP/logic programming behavior
- No breaking changes to existing code

## Edge Cases

1. **All alternatives marked**: Same as full backtracking mode
2. **No alternatives marked**: Traditional PEG (current behavior)
3. **Mix of marked/unmarked**: Marked alternatives are collected, unmarked return immediately
4. **Nested choices**: Each choice has its own marking semantics
