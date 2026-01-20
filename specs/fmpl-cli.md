# fmpl-cli

Command-line REPL for [Project Name TBD].

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
| `:objects` | — | List named objects (TODO) |

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

```
FMPL Quick Reference:
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

---

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `fmpl-core` | Language runtime |
| `rustyline` | Readline-style line editing |
| `dirs` | Home directory detection for history |

---

## Future Enhancements

- [ ] Multi-line input mode
- [ ] Syntax highlighting
- [ ] Tab completion
- [ ] Object inspection (`:inspect obj`)
- [ ] Grammar debugging (`:trace grammar.rule`)
- [ ] Load/save sessions

---

## Related Specs

- [fmpl-core.md](./fmpl-core.md) — Core runtime
- [fmpl-web.md](./fmpl-web.md) — Web-based REPL alternative
