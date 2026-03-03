# fmpl-cli

Command-line REPL for FMPL.

**Crate**: `fmpl-cli`
**Binary**: `fmpl`
**Location**: [fmpl-cli/](../fmpl-cli/)

---

## Overview

A terminal-based REPL (Read-Eval-Print Loop) for interactive language exploration. Built with [rustyline](https://github.com/kkawakam/rustyline) for readline-style editing.

---

## Usage

```bash
# Start the REPL
cargo run --bin fmpl

# Or after installation
fmpl
```

```
FMPL v0.1.0
Type :help for commands, :quit to exit

fmpl> 1 + 2 * 3
=> 7
fmpl> let (x = 42) x + 1
=> 43
fmpl> :quit
Bye!
```

---

## REPL Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `:help` | `:h`, `:?` | Show help and quick reference |
| `:quit` | `:q`, `:exit` | Exit the REPL |
| `:clear` | — | Clear the screen |
| `:reset` | — | Reset VM state |
| `:objects` | — | List named objects |

---

## Features

### History

Commands are saved to `~/.fmpl_history` and restored on startup.

### Line Editing

Standard readline keybindings via rustyline:

- `Ctrl-A` / `Ctrl-E` — Beginning / end of line
- `Ctrl-R` — Reverse search history
- `Ctrl-L` — Clear screen
- `Up` / `Down` — Navigate history

---

## Quick Reference

Displayed via `:help`:

FMPL Quick Reference:

```fmpl
  let (x = 42) x + 1       Let binding
  lambda (x, y) x + y      Lambda expression
  \x x + 1                 Short lambda
  if cond then a else b    Conditional
  [1, 2, 3]                List literal
  %{foo: 1, bar: 2}        Map literal
  x |> f |> g              Pipe operator
  obj.method(args)         Method call
  obj.property             Property access
```

```fmpl
  object foo {
    prop: 10
    method(a,b): a + b
  }

  grammar bar {
    rule:n => action
  }
```

---

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `fmpl-core` | Language runtime |
| `rustyline` | Readline-style line editing |
| `dirs` | Home directory detection for history |

---

## Future Enhancements

### AC-CLI-1: Multi-line input mode

**File**: `fmpl-cli/src/main.rs`
**Test**: manual

- When a line ends with `{`, `(`, `[`, or `\`, prompt changes to `....>` and accumulates lines
- Use `fmpl_core::is_complete(buffer)` to detect when expression is complete
- Submit accumulated buffer on completion
- `Ctrl-C` cancels multi-line input and returns to `fmpl>`

### AC-CLI-2: Syntax highlighting

**File**: `fmpl-cli/src/main.rs` (or new `highlighter.rs`)
**Dependency**: `rustyline` `Highlighter` trait

- Implement `rustyline::highlight::Highlighter` for FMPL syntax
- Keywords (`let`, `if`, `then`, `else`, `while`, `do`, `object`, `grammar`) in bold/blue
- Strings in green, numbers in cyan, symbols (`:name`) in magenta
- Comments (`--`) in gray

### AC-CLI-3: Tab completion

**File**: `fmpl-cli/src/main.rs` (or new `completer.rs`)
**Dependency**: `rustyline` `Completer` trait

- Complete REPL commands (`:help`, `:quit`, `:reset`, `:objects`, `:clear`)
- Complete variable names from current VM scope
- Complete object property/method names after `.`
- Complete `io::load("` with filesystem paths

### AC-CLI-4: Object inspection (`:inspect`)

**File**: `fmpl-cli/src/main.rs`
**Test**: manual

- `:inspect obj` shows full object state: id, parent, properties, methods, facets
- `:inspect obj.method` shows method source if available
- Format matches the inspector view from [image-model.md](./object-system/image-model.md)

### AC-CLI-5: Grammar debugging (`:trace`)

**File**: `fmpl-cli/src/main.rs`, `fmpl-core/src/grammar/runtime.rs`
**Test**: manual

- `:trace grammar.rule` enables tracing for a specific rule
- While tracing, each rule entry/exit prints: `> rule_name at pos N` / `< rule_name: success|fail`
- `:trace off` disables all tracing
- Requires a `trace_enabled: HashSet<SmolStr>` on the grammar runtime

### AC-CLI-6: Load/save sessions

**File**: `fmpl-cli/src/main.rs`
**Test**: manual

- `:save path.fmpl` dumps all current variable bindings as `let` statements to a file
- `:load path.fmpl` is sugar for `io::load("path.fmpl")`
- Named objects are included in the dump

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [fmpl-web.md](./fmpl-web.md) — Web-based REPL alternative
