#!/usr/bin/env python3
"""Ralph loop pre-flight: runs health check, detects uncommitted state,
and initializes the state machine with the right starting state.

Outputs:
  - .claude/.ralph-state.json (state machine file)
  - stdout: context message for the claude invocation (user prompt)

Usage:
  python3 ralph-preflight.py          # run pre-flight, emit context
  python3 ralph-preflight.py --clear  # remove state file
"""
import json
import os
import subprocess
import sys

STATE_FILE = os.path.join(os.path.dirname(__file__), "..", ".ralph-state.json")

def run(cmd, timeout=60):
    """Run a shell command and return (returncode, stdout)."""
    try:
        result = subprocess.run(
            cmd, shell=True, capture_output=True, text=True, timeout=timeout
        )
        return result.returncode, result.stdout
    except subprocess.TimeoutExpired:
        return 1, ""

def pid_alive(pid):
    """Check if a process is still running."""
    try:
        os.kill(pid, 0)
        return True
    except (OSError, ProcessLookupError):
        return False

def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--clear":
        # Check if owning process is still running
        try:
            with open(STATE_FILE) as f:
                state = json.load(f)
            owner_pid = state.get("pid")
            if owner_pid and pid_alive(owner_pid):
                print(f"WARNING: ralph run still active (PID {owner_pid}). "
                      f"State file NOT cleared.", file=sys.stderr)
                sys.exit(1)
        except (FileNotFoundError, json.JSONDecodeError):
            pass
        try:
            os.remove(STATE_FILE)
        except FileNotFoundError:
            pass
        print("Ralph state machine disabled", file=sys.stderr)
        sys.exit(0)

    # --- Pre-flight checks ---
    context_parts = []

    # 1. Health check: cargo test
    _, test_output = run(
        "cargo test -p fmpl-core 2>&1 | grep -E '^(test |test result:|FAILED|error\\[|thread.*panicked)' | head -20"
    )

    # Parse test results
    tests_failing = False
    test_summary = ""
    for line in test_output.strip().split("\n"):
        if "FAILED" in line:
            tests_failing = True
        if "test result:" in line:
            test_summary += line.strip() + "\n"
            # Check for N failed where N > 0
            import re
            m = re.search(r'(\d+)\s+failed', line)
            if m and int(m.group(1)) > 0:
                tests_failing = True

    # 2. Health check: cargo clippy
    _, clippy_output = run(
        "cargo clippy -p fmpl-core 2>&1 | grep -v objfs | grep -E '^warning:' "
        "| grep -v 'fmpl-core@' | grep -v 'generated.*warnings'",
        timeout=120,
    )
    clippy_warnings = [
        line.strip() for line in clippy_output.strip().split("\n") if line.strip()
    ]
    has_clippy_warnings = len(clippy_warnings) > 0

    # 3. Uncommitted changes: jj diff --stat
    _, diff_output = run("jj diff --stat 2>&1")
    has_uncommitted = bool(diff_output.strip()) and "0 files changed" not in diff_output

    # Parse which files are modified
    modified_files = []
    protected_files = []
    if has_uncommitted:
        for line in diff_output.strip().split("\n"):
            line = line.strip()
            if "|" in line:
                fname = line.split("|")[0].strip()
                if fname:
                    modified_files.append(fname)
                    # Protect user-edited files (PROMPT.md, AGENTS.md, settings)
                    if fname in ("PROMPT.md", "AGENTS.md", ".claude/settings.json",
                                 ".claude/settings.local.json"):
                        protected_files.append(fname)

    # 4. Determine starting state
    if tests_failing:
        start_state = "IMPLEMENT"
        health_fix = True
        context_parts.append("## Health Check: FAILED\n")
        context_parts.append("Tests are failing. Fix them before picking a new task.\n")
        context_parts.append("```\n" + test_output.strip() + "\n```\n")
    else:
        start_state = "PICK_TASK"
        health_fix = False
        context_parts.append("## Health Check: PASSED\n")
        context_parts.append(test_summary.strip() + "\n")

    if has_clippy_warnings:
        # Clippy warnings are a health check failure — fix before new tasks
        if not tests_failing:
            start_state = "IMPLEMENT"
            health_fix = True
        context_parts.append(f"\n## Clippy: FAILED — {len(clippy_warnings)} warnings\n")
        context_parts.append(
            "Zero warnings required. Fix all clippy warnings before picking a new task.\n"
            "Run `cargo clippy --fix -p fmpl-core` first, then fix remaining manually.\n"
        )
        context_parts.append("```\n" + "\n".join(clippy_warnings[:15]) + "\n```\n")
    else:
        context_parts.append("\n## Clippy: CLEAN\n")

    if has_uncommitted:
        context_parts.append("\n## Uncommitted Changes\n")
        context_parts.append("The following files have uncommitted changes:\n")
        context_parts.append("```\n" + diff_output.strip() + "\n```\n")
        if protected_files:
            context_parts.append(
                "\n**PROTECTED FILES** (do not revert or overwrite these — "
                "they contain manual edits):\n"
            )
            for f in protected_files:
                context_parts.append(f"- `{f}`\n")
        context_parts.append(
            "\nThese changes were made outside the loop. Do NOT revert them. "
            "If they conflict with your task, work around them.\n"
        )

    context_parts.append(
        "\nExecute the task loop. Output COMPLETED:<id>, BLOCKED:<id>, "
        "or CLOSED:<id> when done.\n"
    )

    # Progressive reveal: inject instructions for the starting state
    steps_dir = os.path.join(os.path.dirname(__file__), "steps")
    step_file = os.path.join(steps_dir, f"{start_state}.md")
    try:
        with open(step_file) as f:
            context_parts.append(f"\n## Current Step: {start_state}\n\n")
            context_parts.append(f.read())
    except FileNotFoundError:
        pass

    # --- Write state file ---
    # Store parent PID (ralph.sh) so hooks can detect active runs
    ralph_pid = os.getppid()
    state = {
        "state": start_state,
        "pid": ralph_pid,
        "task_id": None,
        "close_count": 0,
        "has_written_code": False,
        "health_fix": health_fix,
        "decomposed": False,
        "tests_passed": False,
        "clippy_passed": False,
        "clippy_warning_count_at_start": len(clippy_warnings),
        "verify_failed": False,
        "protected_files": protected_files,
        "uncommitted_files": modified_files,
    }

    with open(STATE_FILE, "w") as f:
        json.dump(state, f, indent=2)

    print(f"Ralph state: {start_state} "
          f"(uncommitted: {len(modified_files)}, protected: {len(protected_files)})",
          file=sys.stderr)

    # --- Output context message for claude ---
    print("".join(context_parts))

if __name__ == "__main__":
    main()
