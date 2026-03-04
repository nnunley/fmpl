#!/usr/bin/env python3
"""PreToolUse hook: enforce ralph loop state machine tool restrictions.

Reads .claude/.ralph-state.json for current state.
Blocks tool calls not allowed in the current state.
Auto-transitions TRIAGE → IMPLEMENT when Write/Edit is attempted.

States: HEALTH_CHECK → PICK_TASK → TRIAGE → IMPLEMENT → VERIFY → REVIEW → COMMIT
"""
import json
import os
import sys
import re

STATE_FILE = os.path.join(os.path.dirname(__file__), "..", ".ralph-state.json")

def load_state():
    try:
        with open(STATE_FILE) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return None

def save_state(state):
    with open(STATE_FILE, "w") as f:
        json.dump(state, f, indent=2)

def block(reason):
    print(reason, file=sys.stderr)
    sys.exit(2)

def allow():
    sys.exit(0)

def is_cargo_test(cmd):
    return bool(re.search(r'cargo\s+test', cmd))

def is_cargo_clippy(cmd):
    return bool(re.search(r'cargo\s+clippy', cmd))

def is_cargo_cmd(cmd):
    return bool(re.search(r'cargo\s+(test|clippy|build|check)', cmd))

def is_cargo_filtered(cmd):
    """Check if cargo output is piped through a filter (grep, head, tail)."""
    return bool(re.search(r'\|\s*(grep|head|tail)', cmd))

def is_jj_issue(cmd):
    return bool(re.search(r'jj\s+issue', cmd))

def is_jj_issue_ready(cmd):
    return bool(re.search(r'jj\s+issue\s+ready', cmd))

def is_jj_commit(cmd):
    return bool(re.search(r'jj\s+(describe|new|split|squash)', cmd))

# Tools always allowed regardless of state (non-destructive introspection)
ALWAYS_ALLOWED = {"Read", "Glob", "Grep"}

# Tools for research/exploration subagents
RESEARCH_TOOLS = {"Task"}

def pid_alive(pid):
    """Check if a process is still running."""
    try:
        os.kill(pid, 0)
        return True
    except (OSError, ProcessLookupError):
        return False

def main():
    state = load_state()
    if state is None:
        allow()

    # If the owning ralph.sh process is dead, state is stale — ignore it
    owner_pid = state.get("pid")
    if owner_pid and not pid_alive(owner_pid):
        allow()

    hook_input = json.loads(sys.stdin.read())
    tool_name = hook_input.get("tool_name", "")
    tool_input = hook_input.get("tool_input", {})

    current = state.get("state", "IMPLEMENT")
    cmd = ""
    if tool_name == "Bash":
        cmd = tool_input.get("command", "")

    # Always allow non-destructive reads
    if tool_name in ALWAYS_ALLOWED:
        allow()

    # Protect files from being overwritten (user's manual edits)
    protected = state.get("protected_files", [])
    if protected and tool_name in ("Write", "Edit"):
        target = tool_input.get("file_path", "")
        for pf in protected:
            if target.endswith(pf) or target.endswith("/" + pf):
                block(
                    f"BLOCKED: {pf} is protected (contains manual edits from "
                    f"outside the loop). Do not overwrite or revert it."
                )

    # --- HEALTH_CHECK ---
    if current == "HEALTH_CHECK":
        if tool_name == "Bash" and is_cargo_test(cmd):
            allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        block(
            f"BLOCKED [state=HEALTH_CHECK]: Only `cargo test` is allowed. "
            f"Run the health check first. Tried: {tool_name}"
        )

    # --- PICK_TASK ---
    if current == "PICK_TASK":
        if tool_name == "Bash" and is_jj_issue(cmd):
            allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        block(
            f"BLOCKED [state=PICK_TASK]: Pick a task with `jj issue ready` "
            f"then `jj issue show <id>`. Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- TRIAGE ---
    if current == "TRIAGE":
        if tool_name == "Bash":
            # Enforce close-and-pick limit
            if re.search(r'jj\s+issue\s+close', cmd) and state.get("close_count", 0) >= 3:
                block(
                    "BLOCKED [state=TRIAGE]: 3 close-and-pick loops exhausted. "
                    "You must implement the current task or output CLOSED/BLOCKED."
                )
            if is_jj_issue(cmd) or is_cargo_test(cmd):
                allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        # Auto-transition: Write/Edit attempt moves to IMPLEMENT
        if tool_name in ("Write", "Edit"):
            state["state"] = "IMPLEMENT"
            state["has_written_code"] = True
            save_state(state)
            allow()
        block(
            f"BLOCKED [state=TRIAGE]: Check if task is done, then start coding. "
            f"Use jj issue close, cargo test, or Write/Edit to begin. "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- IMPLEMENT ---
    if current == "IMPLEMENT":
        if tool_name == "Bash" and is_jj_issue_ready(cmd):
            block(
                "BLOCKED [state=IMPLEMENT]: You are committed to the current "
                "task. `jj issue ready` is not allowed during implementation. "
                "Finish the task, verify, and commit."
            )
        # Enforce cargo output filtering to limit context growth
        if tool_name == "Bash" and is_cargo_cmd(cmd) and not is_cargo_filtered(cmd):
            block(
                "BLOCKED [state=IMPLEMENT]: Cargo output must be filtered to "
                "limit context growth. Pipe through grep and head:\n"
                "  cargo test ... 2>&1 | grep -E '^(test |test result:|error\\[|thread.*panicked)'\n"
                "  cargo clippy ... 2>&1 | grep -E '^(error|warning:)' | head -30"
            )
        if tool_name in ("Write", "Edit"):
            # Require docs/codebase/ check before first write
            if not state.get("has_read_codebase_docs"):
                block(
                    "BLOCKED [state=IMPLEMENT]: Read docs/codebase/ before coding. "
                    "These contain pre-digested patterns that save research time. "
                    "Use: Glob('docs/codebase/*.md') then Read the relevant files."
                )
            if not state.get("has_written_code"):
                state["has_written_code"] = True
                save_state(state)
        allow()

    # --- RESEARCH ---
    if current == "RESEARCH":
        # Only allow subagents (Explore, codebase-analyzer) and reads
        if tool_name in RESEARCH_TOOLS:
            allow()
        if tool_name in ALWAYS_ALLOWED:
            allow()
        if tool_name == "Bash" and is_jj_issue(cmd):
            allow()
        # Write/Edit transitions to IMPLEMENT (agent decided to code instead)
        if tool_name in ("Write", "Edit"):
            state["state"] = "IMPLEMENT"
            state["has_written_code"] = True
            save_state(state)
            allow()
        block(
            f"BLOCKED [state=RESEARCH]: Use Explore/codebase-analyzer subagents "
            f"for research. Do not use direct Bash/Grep — dispatch a subagent. "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- DOCUMENT ---
    if current == "DOCUMENT":
        # Allow writing docs and creating subtasks
        if tool_name in ("Write", "Edit"):
            target = tool_input.get("file_path", "")
            if "docs/codebase" in target or "AGENTS.md" in target:
                allow()
            block(
                "BLOCKED [state=DOCUMENT]: Only write to docs/codebase/ and AGENTS.md. "
                f"Tried to write: {target}"
            )
        if tool_name == "Bash" and (is_jj_issue(cmd) or is_jj_commit(cmd)):
            allow()
        if tool_name in ALWAYS_ALLOWED:
            allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        block(
            f"BLOCKED [state=DOCUMENT]: Write findings to docs/codebase/, "
            f"update AGENTS.md, create subtasks with jj issue create. "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- VERIFY ---
    if current == "VERIFY":
        if tool_name == "Bash":
            # Enforce cargo output filtering
            if is_cargo_cmd(cmd) and not is_cargo_filtered(cmd):
                block(
                    "BLOCKED [state=VERIFY]: Cargo output must be filtered. "
                    "Pipe through grep and head:\n"
                    "  cargo test ... 2>&1 | grep -E '^(test |test result:|error\\[|thread.*panicked)'\n"
                    "  cargo clippy ... 2>&1 | grep -E '^(error|warning:)' | head -30"
                )
            if is_cargo_test(cmd) or is_cargo_clippy(cmd):
                allow()
            if is_jj_commit(cmd) or is_jj_issue(cmd):
                allow()
            if "grep" in cmd or "head" in cmd or "jj " in cmd:
                allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        if tool_name in ("Write", "Edit"):
            if state.get("verify_failed"):
                state["state"] = "IMPLEMENT"
                state["verify_failed"] = False
                save_state(state)
                allow()
            block(
                "BLOCKED [state=VERIFY]: Tests must pass before more edits. "
                "Run cargo test. If tests fail, you'll return to IMPLEMENT."
            )
        block(
            f"BLOCKED [state=VERIFY]: Run cargo test and cargo clippy. "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- REVIEW ---
    if current == "REVIEW":
        # Allow review tools: Skill (codereview-*), Task (review subagents),
        # Bash (jj diff, jj log), but NOT Write/Edit
        if tool_name == "Skill":
            allow()
        if tool_name in RESEARCH_TOOLS:
            allow()
        if tool_name == "Bash":
            if re.search(r'jj\s+(diff|log|show)', cmd):
                allow()
            if is_jj_commit(cmd) or is_jj_issue(cmd):
                allow()
            if re.search(r'(jj|git)\s', cmd):
                allow()
        if tool_name in ("Write", "Edit"):
            # Review found issues — go back to IMPLEMENT
            state["state"] = "IMPLEMENT"
            state["review_requested_changes"] = True
            state["tests_passed"] = False
            state["clippy_passed"] = False
            state["health_check_passed"] = False
            save_state(state)
            allow()
        block(
            f"BLOCKED [state=REVIEW]: Run code review skills before committing. "
            f"Use Skill(codereview-reviewing) or Skill(superpowers:requesting-code-review). "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- COMMIT ---
    if current == "COMMIT":
        if tool_name == "Bash":
            # Enforce cargo output filtering
            if is_cargo_cmd(cmd) and not is_cargo_filtered(cmd):
                block(
                    "BLOCKED [state=COMMIT]: Cargo output must be filtered. "
                    "Pipe through grep and head:\n"
                    "  cargo test ... 2>&1 | grep -E '^(test |test result:|error\\[|thread.*panicked)'\n"
                    "  cargo clippy ... 2>&1 | grep -E '^(error|warning:)' | head -30"
                )
            # Block jj describe until full health check passes
            if is_jj_commit(cmd):
                if not state.get("health_check_passed"):
                    block(
                        "BLOCKED [state=COMMIT]: Run full `cargo test` health check "
                        "before committing. The VERIFY step may have used filtered "
                        "tests — run the full suite now."
                    )
                allow()
            if is_jj_issue(cmd):
                allow()
            # Allow cargo test for the pre-commit health check
            if is_cargo_test(cmd) or is_cargo_clippy(cmd):
                allow()
            if re.search(r'(jj|git)\s', cmd):
                allow()
        block(
            f"BLOCKED [state=COMMIT]: Run `cargo test` then commit with `jj describe`. "
            f"Tried: {tool_name}"
            + (f" ({cmd[:60]})" if cmd else "")
        )

    # --- DONE or unknown ---
    allow()

if __name__ == "__main__":
    main()
