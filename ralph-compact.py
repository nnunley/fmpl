#!/usr/bin/env python3
"""Compact a ralph segment log into a continuation message for the next segment.

Reads a stream-json log file and .ralph-state.json, extracts what was done,
and produces a concise summary that can serve as the user message for a fresh
claude invocation — discarding the raw tool result history.

Usage:
    python3 ralph-compact.py <segment-log.jsonl>
    python3 ralph-compact.py <segment-log.jsonl> --state /path/to/.ralph-state.json

Output (stdout): Markdown continuation message for the next segment.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

DEFAULT_STATE = os.path.join(
    os.path.dirname(__file__), ".claude", ".ralph-state.json"
)


def parse_events(path):
    """Parse stream-json log into events."""
    events = []
    for line in open(path):
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return events


def extract_actions(events):
    """Extract a concise list of actions from assistant turns and tool calls."""
    actions = []
    files_read = set()
    files_written = set()
    commands = []

    for ev in events:
        if ev.get("type") != "assistant":
            continue
        msg = ev.get("message", {})
        for c in msg.get("content", []):
            ct = c.get("type", "")
            if ct == "tool_use":
                name = c.get("name", "")
                inp = c.get("input", {})
                if name == "Read":
                    files_read.add(inp.get("file_path", ""))
                elif name in ("Edit", "Write"):
                    files_written.add(inp.get("file_path", ""))
                elif name == "Bash":
                    cmd = inp.get("command", "")[:120]
                    commands.append(cmd)
                elif name == "Task":
                    desc = inp.get("description", "")[:80]
                    actions.append(f"- Dispatched agent: {desc}")
                elif name == "Skill":
                    actions.append(f"- Invoked skill: {inp.get('skill', '?')}")

    if files_read:
        actions.append(f"- Read {len(files_read)} files: {', '.join(sorted(files_read)[-5:])}")
    if files_written:
        actions.append(f"- Modified {len(files_written)} files: {', '.join(sorted(files_written))}")
    if commands:
        # Summarize commands by category
        test_cmds = [c for c in commands if "cargo test" in c]
        build_cmds = [c for c in commands if "cargo build" in c or "cargo clippy" in c]
        jj_cmds = [c for c in commands if c.startswith("jj ")]
        other = len(commands) - len(test_cmds) - len(build_cmds) - len(jj_cmds)
        parts = []
        if test_cmds:
            parts.append(f"{len(test_cmds)} test runs")
        if build_cmds:
            parts.append(f"{len(build_cmds)} build/clippy")
        if jj_cmds:
            parts.append(f"{len(jj_cmds)} jj commands")
        if other > 0:
            parts.append(f"{other} other")
        actions.append(f"- Ran {len(commands)} commands ({', '.join(parts)})")

    return "\n".join(actions) if actions else "No actions recorded."


def extract_errors(events):
    """Extract errors from tool results and assistant text."""
    errors = []

    for ev in events:
        if ev.get("type") != "user":
            continue
        msg = ev.get("message", {})
        for c in msg.get("content", []):
            if c.get("type") == "tool_result" and c.get("is_error"):
                content = c.get("content", "")
                if isinstance(content, list):
                    text = " ".join(
                        item.get("text", str(item))[:200] for item in content
                    )
                else:
                    text = str(content)[:300]
                errors.append(text)

    return errors


def extract_assistant_narrative(events):
    """Extract a combined narrative from ALL assistant text blocks.

    The last few text blocks are most important — they contain what
    the model was about to do when the segment ended. We keep them
    in full. Earlier blocks are summarized to first lines only.
    """
    all_texts = []
    for ev in events:
        if ev.get("type") != "assistant":
            continue
        msg = ev.get("message", {})
        for c in msg.get("content", []):
            if c.get("type") == "text":
                text = c.get("text", "").strip()
                if text:
                    all_texts.append(text)

    if not all_texts:
        return ""

    # Keep last 3 text blocks in full (these have the plan/next steps)
    # Summarize earlier blocks to first line only
    parts = []
    for i, text in enumerate(all_texts):
        if i >= len(all_texts) - 3:
            # Last 3: keep in full, but cap each at 400 chars
            if len(text) > 400:
                text = text[:400] + "..."
            parts.append(text)
        else:
            # Earlier: first line only
            first_line = text.split("\n")[0][:120]
            parts.append(f"- {first_line}")

    return "\n\n".join(parts)


def extract_edited_file_summaries(events):
    """Extract summaries of Edit/Write operations so the next segment
    knows what was already written without re-reading files.

    Returns a list of (file_path, summary) tuples.
    """
    summaries = []
    seen = set()

    for ev in events:
        if ev.get("type") != "assistant":
            continue
        msg = ev.get("message", {})
        for c in msg.get("content", []):
            if c.get("type") != "tool_use":
                continue
            name = c.get("name", "")
            inp = c.get("input", {})

            if name == "Edit":
                fp = inp.get("file_path", "")
                old = inp.get("old_string", "")[:80]
                new = inp.get("new_string", "")[:200]
                if fp and fp not in seen:
                    seen.add(fp)
                    summaries.append((fp, f"Edited: replaced `{old}...` with `{new}...`"))
            elif name == "Write":
                fp = inp.get("file_path", "")
                content = inp.get("content", "")
                if fp and fp not in seen:
                    seen.add(fp)
                    # First 5 lines of written content
                    preview = "\n".join(content.split("\n")[:5])
                    summaries.append((fp, f"Wrote new file ({len(content)} chars): {preview}..."))

    return summaries


def extract_result(events):
    """Extract the result event if present."""
    for ev in events:
        if ev.get("type") == "result":
            return {
                "success": ev.get("subtype") == "success",
                "num_turns": ev.get("num_turns", 0),
                "cost_usd": ev.get("total_cost_usd", 0),
                "is_error": ev.get("is_error", False),
                "subtype": ev.get("subtype", "unknown"),
            }
    return None


def load_state(state_path):
    """Load ralph state machine file."""
    try:
        with open(state_path) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return {}


def get_diff_stat():
    """Get jj diff --stat output."""
    try:
        result = subprocess.run(
            "jj diff --stat 2>&1",
            shell=True, capture_output=True, text=True, timeout=30,
        )
        return result.stdout.strip()
    except (subprocess.TimeoutExpired, Exception):
        return "(could not get diff stat)"


def get_step_instructions(state):
    """Load progressive-reveal step instructions for current state."""
    current_state = state.get("state", "")
    if not current_state:
        return ""
    steps_dir = os.path.join(os.path.dirname(__file__), ".claude", "hooks", "steps")
    step_file = os.path.join(steps_dir, f"{current_state}.md")
    try:
        with open(step_file) as f:
            return f.read()
    except FileNotFoundError:
        return ""


def compact(log_path, state_path=DEFAULT_STATE):
    """Produce a compacted context message from a segment log."""
    events = parse_events(log_path)
    state = load_state(state_path)
    result = extract_result(events)

    actions = extract_actions(events)
    errors = extract_errors(events)
    narrative = extract_assistant_narrative(events)
    edits = extract_edited_file_summaries(events)
    diff_stat = get_diff_stat()
    step_instructions = get_step_instructions(state)

    parts = []
    parts.append("## Continuation — You are resuming mid-task\n")
    parts.append(
        "This is a continuation of the same iteration. The previous segment "
        "ran out of budget. You MUST continue the same task — do NOT pick a "
        "new task or re-read files you already read. The work below was "
        "already done by you in the previous segment."
    )
    parts.append("")

    # Task and state info
    task_id = state.get("task_id", "unknown")
    current_state = state.get("state", "unknown")
    parts.append(f"Task: #{task_id}")
    parts.append(f"State: {current_state}")

    # State machine flags
    flags = []
    if state.get("has_written_code"):
        flags.append("has_written_code")
    if state.get("tests_passed"):
        flags.append("tests_passed")
    if state.get("clippy_passed"):
        flags.append("clippy_passed")
    if state.get("health_fix"):
        flags.append("health_fix")
    if state.get("decomposed"):
        flags.append("decomposed")
    if flags:
        parts.append(f"Flags: {', '.join(flags)}")
    parts.append("")

    # Segment result
    if result:
        budget_note = ""
        if result.get("subtype") == "error_max_budget_usd":
            budget_note = " (hit budget limit — this is expected, continue working)"
        parts.append(f"Previous segment: {result['num_turns']} turns, ${result['cost_usd']:.3f}{budget_note}")
        parts.append("")

    # What was done
    parts.append("### What was already done (do NOT repeat)")
    parts.append(actions)
    parts.append("")

    # Edits already made — so the model doesn't re-read these files
    if edits:
        parts.append("### Edits already made to files")
        for fp, summary in edits:
            parts.append(f"- `{fp}`: {summary}")
        parts.append("")

    # Narrative — the model's own reasoning from the previous segment
    if narrative:
        parts.append("### Your reasoning and plan (from previous segment)")
        parts.append(narrative)
        parts.append("")

    # Errors
    if errors:
        parts.append("### Errors encountered")
        for err in errors[-3:]:  # Last 3 errors only
            parts.append(f"- {err}")
        parts.append("")

    # Files changed on disk
    if diff_stat:
        parts.append("### Files changed on disk (jj diff --stat)")
        parts.append("```")
        parts.append(diff_stat)
        parts.append("```")
        parts.append("")

    # Protected files from state
    protected = state.get("protected_files", [])
    if protected:
        parts.append("**PROTECTED FILES** (do not revert or overwrite):")
        for f in protected:
            parts.append(f"- `{f}`")
        parts.append("")

    # Progressive reveal: inject step instructions for work states only.
    # Skip PICK_TASK — if we're compacting mid-iteration, the task was
    # already picked. Injecting PICK_TASK instructions would confuse the
    # model into picking a new task instead of continuing.
    if step_instructions and current_state not in ("PICK_TASK",):
        parts.append(f"## Current Step: {current_state}\n")
        parts.append(step_instructions)
        parts.append("")

    parts.append(
        "Resume immediately from where you left off. "
        "Execute the next step of your plan.\n\n"
        "RULES FOR CONTINUATION SEGMENTS:\n"
        "- Do NOT run jj diff/status to verify previous edits — they are on disk.\n"
        "- Do NOT re-read files you already read in the previous segment.\n"
        "- Do NOT pick a new task.\n"
        "- Do NOT repeat completed work.\n"
        "- If you need to verify previous edits worked, run the relevant "
        "test or cargo check — not jj diff.\n"
        "- Start with the NEXT action from your plan, not the first one."
    )
    parts.append("Output COMPLETED:<id>, BLOCKED:<id>, or CLOSED:<id> when done.")

    return "\n".join(parts)


def main():
    if len(sys.argv) < 2:
        print("Usage: ralph-compact.py <segment-log.jsonl> [--state STATE_FILE]",
              file=sys.stderr)
        sys.exit(1)

    log_path = sys.argv[1]
    state_path = DEFAULT_STATE

    if "--state" in sys.argv:
        idx = sys.argv.index("--state")
        if idx + 1 < len(sys.argv):
            state_path = sys.argv[idx + 1]

    if not Path(log_path).exists():
        print(f"Error: {log_path} not found", file=sys.stderr)
        sys.exit(1)

    print(compact(log_path, state_path))


if __name__ == "__main__":
    main()
