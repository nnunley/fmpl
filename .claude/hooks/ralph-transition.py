#!/usr/bin/env python3
"""PostToolUse hook: detect state transitions in the ralph loop state machine.

Reads tool output and updates .claude/.ralph-state.json accordingly.
Transitions are deterministic based on which tool was called and its output.

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

def emit(obj):
    """Print JSON for Claude to see as additional context."""
    print(json.dumps(obj))

STEPS_DIR = os.path.join(os.path.dirname(__file__), "steps")

def step_instructions(state_name):
    """Load step-specific instructions for progressive reveal."""
    path = os.path.join(STEPS_DIR, f"{state_name}.md")
    try:
        with open(path) as f:
            return f.read().strip()
    except FileNotFoundError:
        return ""

def context(msg):
    """Send a state transition message to Claude, with step instructions on transitions."""
    full_msg = msg
    # Auto-detect state from the transition message
    m = re.search(r'\[STATE -> (\w+)\]', msg)
    if m:
        instructions = step_instructions(m.group(1))
        if instructions:
            full_msg += "\n\n---\n" + instructions
    emit({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": full_msg
        }
    })

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
        sys.exit(0)

    # If the owning ralph.sh process is dead, state is stale — ignore it
    owner_pid = state.get("pid")
    if owner_pid and not pid_alive(owner_pid):
        sys.exit(0)

    hook_input = json.loads(sys.stdin.read())
    tool_name = hook_input.get("tool_name", "")
    tool_input = hook_input.get("tool_input", {})
    tool_response = hook_input.get("tool_response", "")

    current = state.get("state", "IMPLEMENT")
    cmd = ""
    if tool_name == "Bash":
        cmd = tool_input.get("command", "")

    # Normalize tool_response to string for pattern matching
    response_str = ""
    if isinstance(tool_response, str):
        response_str = tool_response
    elif isinstance(tool_response, dict):
        response_str = json.dumps(tool_response)

    def tests_failed(text):
        """Detect actual test failures (not '0 failed')."""
        # Match 'N failed' where N > 0
        for m in re.finditer(r'(\d+)\s+failed', text):
            if int(m.group(1)) > 0:
                return True
        # Also match the FAILED banner line from cargo test
        if re.search(r'^FAILED', text, re.MULTILINE):
            return True
        return False

    # --- HEALTH_CHECK ---
    if current == "HEALTH_CHECK":
        if tool_name == "Bash" and re.search(r'cargo\s+test', cmd):
            if tests_failed(response_str):
                state["state"] = "IMPLEMENT"
                state["health_fix"] = True
                save_state(state)
                context(
                    "[STATE -> IMPLEMENT] Tests are failing. Fix them before "
                    "picking a new task. Create an issue if needed."
                )
            else:
                state["state"] = "PICK_TASK"
                save_state(state)
                context(
                    "[STATE -> PICK_TASK] Health check passed. "
                    "Pick a task: `jj issue ready | head -2`"
                )

    # --- PICK_TASK ---
    elif current == "PICK_TASK":
        if tool_name == "Bash" and re.search(r'jj\s+issue\s+show', cmd):
            match = re.search(r'jj\s+issue\s+show\s+(\S+)', cmd)
            task_id = match.group(1) if match else "unknown"
            close_count = state.get("close_count", 0)
            state["state"] = "TRIAGE"
            state["task_id"] = task_id
            # Preserve close_count across close-and-pick loops
            save_state(state)
            context(
                f"[STATE -> TRIAGE] Task #{task_id} selected "
                f"({close_count}/3 close-and-pick loops used). "
                f"Check if already done or proceed to implement."
            )

    # --- TRIAGE ---
    elif current == "TRIAGE":
        if tool_name == "Bash" and re.search(r'jj\s+issue\s+close', cmd):
            close_count = state.get("close_count", 0) + 1
            state["close_count"] = close_count
            if close_count >= 3:
                state["state"] = "DONE"
                save_state(state)
                context(
                    "[STATE -> DONE] 3 close-and-pick loops used. "
                    "Output CLOSED:<id> for this iteration."
                )
            else:
                state["state"] = "PICK_TASK"
                save_state(state)
                context(
                    f"[STATE -> PICK_TASK] Issue closed ({close_count}/3 loops). "
                    f"Pick next task."
                )
        elif tool_name == "Bash" and re.search(r'jj\s+issue\s+comment', cmd):
            # Check if entering research arc
            if "RESEARCH:" in cmd or "RESEARCH" in response_str:
                state["state"] = "RESEARCH"
                state["research_subagents"] = 0
                save_state(state)
                context(
                    "[STATE -> RESEARCH] Research arc entered. "
                    "Dispatch Explore subagent(s) to gather findings. "
                    "Max 2 subagent dispatches, then write docs."
                )
            # Otherwise it's just a regular comment, stay in TRIAGE
        elif tool_name == "Bash" and re.search(r'jj\s+issue\s+create', cmd):
            state["state"] = "IMPLEMENT"
            state["decomposed"] = True
            save_state(state)
            context(
                "[STATE -> IMPLEMENT] Subtask created. Now implement it. "
                "You must write code before this iteration ends."
            )

    # --- RESEARCH ---
    elif current == "RESEARCH":
        if tool_name == "Task":
            count = state.get("research_subagents", 0) + 1
            state["research_subagents"] = count
            save_state(state)
            if count >= 2:
                state["state"] = "DOCUMENT"
                save_state(state)
                context(
                    "[STATE -> DOCUMENT] Research complete (2 subagents dispatched). "
                    "Write findings to docs/codebase/, update AGENTS.md, "
                    "create subtasks with jj issue create."
                )
            else:
                context(
                    f"[STATE: RESEARCH] Subagent {count}/2 complete. "
                    f"Dispatch another or transition to DOCUMENT by writing docs."
                )

        # Writing to docs/codebase/ transitions to DOCUMENT
        if tool_name in ("Write", "Edit"):
            target = tool_input.get("file_path", "")
            if "docs/codebase" in target:
                state["state"] = "DOCUMENT"
                save_state(state)
                context(
                    "[STATE -> DOCUMENT] Writing docs. "
                    "Update AGENTS.md and create subtasks."
                )

    # --- DOCUMENT ---
    elif current == "DOCUMENT":
        if tool_name == "Bash" and re.search(r'jj\s+issue\s+create', cmd):
            state["has_created_subtasks"] = True
            save_state(state)
            context(
                "[STATE: DOCUMENT] Subtask created. Create more or commit."
            )
        if tool_name == "Bash" and re.search(r'jj\s+describe', cmd):
            state["state"] = "DONE"
            save_state(state)
            context(
                "[STATE -> DONE] Research committed. Output your COMPLETED:<id> line."
            )

    # --- IMPLEMENT ---
    elif current == "IMPLEMENT":
        # Track when agent reads codebase discovery docs
        if tool_name in ("Read", "Glob"):
            target = tool_input.get("file_path", "") or tool_input.get("pattern", "")
            if "docs/codebase" in target:
                if not state.get("has_read_codebase_docs"):
                    state["has_read_codebase_docs"] = True
                    save_state(state)

        if tool_name in ("Write", "Edit"):
            if not state.get("has_written_code"):
                state["has_written_code"] = True
                save_state(state)

        if tool_name == "Bash" and re.search(r'cargo\s+test', cmd):
            if state.get("has_written_code"):
                if tests_failed(response_str):
                    state["verify_failed"] = True
                    save_state(state)
                    context(
                        "[STATE: IMPLEMENT] Tests failed. Fix the code and retry."
                    )
                else:
                    state["state"] = "VERIFY"
                    state["tests_passed"] = True
                    save_state(state)
                    context(
                        "[STATE -> VERIFY] Tests passed. "
                        "Now run cargo clippy."
                    )

    # --- VERIFY ---
    elif current == "VERIFY":
        if tool_name == "Bash" and re.search(r'cargo\s+test', cmd):
            if tests_failed(response_str):
                state["state"] = "IMPLEMENT"
                state["verify_failed"] = True
                state["tests_passed"] = False
                save_state(state)
                context(
                    "[STATE -> IMPLEMENT] Tests failed during verification. "
                    "Fix the code."
                )
            else:
                state["tests_passed"] = True
                save_state(state)

        if tool_name == "Bash" and re.search(r'cargo\s+clippy', cmd):
            # Check for actual errors (not just warnings in the grep output)
            has_errors = bool(re.search(r'^error\b', response_str, re.MULTILINE))
            if has_errors:
                state["state"] = "IMPLEMENT"
                state["verify_failed"] = True
                save_state(state)
                context("[STATE -> IMPLEMENT] Clippy errors found. Fix them.")
            else:
                state["clippy_passed"] = True
                if state.get("tests_passed"):
                    state["state"] = "REVIEW"
                    save_state(state)
                    context(
                        "[STATE -> REVIEW] Tests and clippy passed. "
                        "Run code review before committing. Use: "
                        "Skill(codereview-reviewing) or "
                        "Skill(superpowers:requesting-code-review)"
                    )
                else:
                    save_state(state)

    # --- REVIEW ---
    elif current == "REVIEW":
        # Transition to COMMIT after review skill or subagent completes
        if tool_name == "Skill":
            skill_name = tool_input.get("skill", "")
            if any(kw in skill_name for kw in [
                "codereview", "code-review", "requesting-code-review"
            ]):
                state["review_completed"] = True
                state["state"] = "COMMIT"
                save_state(state)
                context(
                    "[STATE -> COMMIT] Review complete. "
                    "Commit with `jj describe -m '...'`."
                )
        if tool_name == "Task":
            subagent = tool_input.get("subagent_type", "")
            if any(kw in subagent.lower() for kw in [
                "review", "code-reviewer", "superpowers:code-reviewer",
                "feature-dev:code-reviewer"
            ]):
                state["review_completed"] = True
                state["state"] = "COMMIT"
                save_state(state)
                context(
                    "[STATE -> COMMIT] Review complete. "
                    "Commit with `jj describe -m '...'`."
                )

    # --- COMMIT ---
    elif current == "COMMIT":
        if tool_name == "Bash" and re.search(r'cargo\s+test', cmd):
            if tests_failed(response_str):
                state["state"] = "IMPLEMENT"
                state["verify_failed"] = True
                state["health_check_passed"] = False
                save_state(state)
                context(
                    "[STATE -> IMPLEMENT] Pre-commit health check failed. "
                    "Fix the failing tests."
                )
            else:
                state["health_check_passed"] = True
                save_state(state)
                context(
                    "[STATE: COMMIT] Health check passed. "
                    "Commit with `jj describe -m '...'`."
                )

        if tool_name == "Bash" and re.search(r'jj\s+describe', cmd):
            # Auto-format before finalizing commit
            import subprocess
            subprocess.run("cargo fmt 2>/dev/null", shell=True, timeout=60)
            state["state"] = "DONE"
            save_state(state)
            context(
                "[STATE -> DONE] Committed (cargo fmt applied). "
                "Output your COMPLETED:<id> line."
            )

    sys.exit(0)

if __name__ == "__main__":
    main()
