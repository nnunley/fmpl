You MUST produce code in this step. Triage and decomposition are not deliverables.

**Before coding, check `docs/codebase/`** for existing pattern docs (Write/Edit blocked until you do).

Single-crate task: implement directly.
Multi-crate or large task: decompose into subtasks (`jj issue create`), pick the first, implement it.

**Context budget — every byte counts:**
- The issue description contains code snippets and file paths. Use them — don't re-read files already excerpted in the issue.
- Read ONLY the section you need: use `offset` and `limit` on files > 100 lines. Target the function/block you're editing.
- For broad searches (finding patterns, call sites), dispatch an Explore subagent — its context is isolated from yours.
- Cargo commands MUST be filtered: `2>&1 | grep -E '^(error|warning:|test )' | head -30`. Unfiltered output is blocked.
- Don't re-read files you just wrote or edited — you know what's in them.

**Checkpoints:** `jj status` before risky changes to snapshot. `jj undo` to roll back.

**Subagents:** Use Explore for code structure, codebase-analyzer for call chains, context7 for external API docs. Do NOT use subagents to write files you're also editing.

**Anti-avoidance:** You are committed. No task-switching, no decompose-without-code, no picking easier tasks.

When tests pass (cargo test), you'll advance to VERIFY.
