# FMPL TUI - Agentic Development Environment

## Features

### LLM Chat Integration ✅ NEW
- **Ctrl+L**: Toggle LLM chat mode
- **Ctrl+P**: Switch LLM provider (Ollama ↔ Anthropic)
- **Automatic bootstrapping**: Loads `lib/llm-common.fmpl`, `lib/ollama.fmpl`, `lib/anthropic.fmpl` on startup
- **Async response handling**: Automatically waits for LLM responses
- **Provider support**:
  - **Ollama**: Local LLM at `localhost:11434` (requires `ollama serve`)
  - **Anthropic**: Claude API (requires `ANTHROPIC_API_KEY` env var)

### Multi-line Code Editor
- **Arrow keys**: Navigate up/down/left/right
- **Enter**: Insert new line (in EDIT MODE)
- **Esc + Enter**: Execute code or send to LLM (switch to EXECUTE MODE, then Enter)
- **Tab**: Insert 4 spaces (indentation)
- **Backspace**: Delete character, merge lines if at start
- **Delete**: Delete character at cursor
- **Home/End**: Jump to start/end of line
- **Line numbers**: Displayed on left side
- **Cursor**: Yellow highlight on current character
- **Scrolling**: Automatic when cursor moves beyond visible area

### Mode Switching
- **EDIT MODE** (default): Enter inserts new lines
- **EXECUTE MODE**: Press Esc to toggle, then Enter to execute FMPL code
- **LLM CHAT MODE**: Press Ctrl+L to toggle, then Enter to send prompt to LLM

### Three-Panel Layout
1. **Research View** - Problem space analysis
2. **Planning View** - Collaborative scope definition
3. **Execution View** - Split into:
   - Code Editor (left)
   - Execution Output (right) - Shows LLM responses or FMPL results

## Usage

### Running the TUI
```bash
cargo run --bin fmpl-tui
```

### LLM Chat Example

1. **Start Ollama** (if using Ollama provider):
   ```bash
   ollama serve  # In another terminal
   ```

2. **Set API key** (if using Anthropic):
   ```bash
   export ANTHROPIC_API_KEY="sk-ant-..."
   ```

3. **Start chat**:
   - Press `Ctrl+L` to enter LLM chat mode
   - Type your prompt: "What is 2+2?"
   - Press `Esc` then `Enter` to send
   - Response appears in the output panel

4. **Switch providers**:
   - Press `Ctrl+P` to toggle between Ollama and Anthropic

### FMPL Code Execution

Enter the following multi-line FMPL code (Esc+Enter to execute):

```
let add = \x \y x + y
let result = add(10, 20)
result
```

Expected output: `30`

### Agentic Workflows

Use the `llm.agent_loop` function for Research→Plan→Execute→Review workflows:

```fmpl
# Load the agentic loop library
io.load("lib/llm-common.fmpl")

# Define a simple chat function
let my_chat = llm.agent_loop("Solve: 2+2", ollama.chat)

# Run the agentic loop
my_chat
```

This will:
1. **Research**: Understand the problem
2. **Plan**: Create a step-by-step plan
3. **Execute**: Run the plan
4. **Review**: Evaluate and suggest improvements

### Key Bindings Reference

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Esc` | Toggle EDIT/EXECUTE mode |
| `Ctrl+L` | Toggle LLM chat mode |
| `Ctrl+P` | Switch LLM provider (Ollama ↔ Anthropic) |
| `Enter` | New line (EDIT), Execute (EXECUTE), or Send to LLM (CHAT) |
| `↑↓←→` | Navigate |
| `Home/End` | Jump to line start/end |
| `Tab` | Insert 4 spaces |
| `Backspace` | Delete backward |
| `Delete` | Delete forward |

## Architecture

### Layer 1: Input Layer ✅ COMPLETE
- ✅ Three-panel layout (Research, Planning, Execution)
- ✅ Multi-line code editor with cursor management
- ✅ Real-time FMPL execution
- ✅ **LLM integration with provider switching**

### Layer 3: Agent Description/Dataflow ✅ COMPLETE
- ✅ FMPL language integration
- ✅ Grammar-based agent control (via `llm.agent_loop`)
- ✅ LLM→Tool→LLM loops (via pattern matching and builtins)

### Next Steps (Future Work)

**Layer 2: Contextual Layer**
- [ ] Revision history with VCS-style branching
- [ ] Automated backtrack detection
- [ ] Context compaction and elision

**Layer 4: Tooling Layer**
- [ ] Tool management interface
- [ ] External tool integration (MCP/ACP)
- [ ] Tool registry via `@` operator patterns

**Advanced LLM Features**
- [ ] Streaming response display (SSE parsing)
- [ ] Multi-turn conversation history
- [ ] Message buffer and context accumulation
- [ ] Tool calling workflow (parse LLM JSON → execute tools → feed back)
