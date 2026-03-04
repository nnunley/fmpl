#!/usr/bin/env python3
"""Analyze a ralph loop iteration log (stream-json from claude CLI).

Reads a .jsonl file of stream-json events and produces a structured summary
for phase 2 analysis: what tools were called, what files were read/written,
how much context was used, and what the agent actually accomplished.

Usage:
    python3 ralph-analyze.py iter-*.jsonl              # Print summary
    python3 ralph-analyze.py iter-*.jsonl --json       # Machine-readable
    python3 ralph-analyze.py iter-*.jsonl --timeline   # Chronological trace
"""

import json
import re
import sys
from collections import Counter
from pathlib import Path


def parse_events(path):
    """Parse stream-json log into structured events."""
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


def extract_turns(events):
    """Extract assistant/user turn pairs with tool calls."""
    turns = []
    for ev in events:
        t = ev.get("type", "")
        if t == "assistant":
            msg = ev.get("message", {})
            turn = {
                "role": "assistant",
                "text_blocks": [],
                "tool_calls": [],
                "thinking_blocks": [],
            }
            for c in msg.get("content", []):
                ct = c.get("type", "")
                if ct == "text":
                    turn["text_blocks"].append(c.get("text", ""))
                elif ct == "tool_use":
                    turn["tool_calls"].append({
                        "name": c.get("name", ""),
                        "id": c.get("id", ""),
                        "input": c.get("input", {}),
                    })
                elif ct == "thinking":
                    turn["thinking_blocks"].append(c.get("thinking", ""))
            turns.append(turn)
        elif t == "user":
            msg = ev.get("message", {})
            turn = {"role": "user", "tool_results": []}
            for c in msg.get("content", []):
                ct = c.get("type", "")
                if ct == "tool_result":
                    content = c.get("content", "")
                    if isinstance(content, list):
                        text = " ".join(
                            item.get("text", str(item))[:200]
                            for item in content
                        )
                    else:
                        text = str(content)[:500]
                    turn["tool_results"].append({
                        "tool_use_id": c.get("tool_use_id", ""),
                        "is_error": c.get("is_error", False),
                        "content_len": len(str(content)),
                        "preview": text[:200],
                    })
            turns.append(turn)
    return turns


def extract_result(events):
    """Extract the final result event."""
    for ev in events:
        if ev.get("type") == "result":
            return {
                "success": ev.get("subtype") == "success",
                "num_turns": ev.get("num_turns", 0),
                "duration_ms": ev.get("duration_ms", 0),
                "cost_usd": ev.get("total_cost_usd", 0),
                "usage": ev.get("usage", {}),
            }
    return None


def analyze_tool_usage(turns):
    """Analyze tool call patterns."""
    tool_counts = Counter()
    tool_details = []
    files_read = set()
    files_written = set()
    commands_run = []

    for turn in turns:
        if turn["role"] != "assistant":
            continue
        for tc in turn.get("tool_calls", []):
            name = tc["name"]
            inp = tc["input"]
            tool_counts[name] += 1

            detail = {"tool": name}

            if name == "Read":
                fp = inp.get("file_path", "")
                files_read.add(fp)
                detail["file"] = fp
            elif name == "Edit":
                fp = inp.get("file_path", "")
                files_written.add(fp)
                detail["file"] = fp
            elif name == "Write":
                fp = inp.get("file_path", "")
                files_written.add(fp)
                detail["file"] = fp
            elif name == "Bash":
                cmd = inp.get("command", "")[:120]
                commands_run.append(cmd)
                detail["command"] = cmd
            elif name == "Grep":
                detail["pattern"] = inp.get("pattern", "")[:60]
            elif name == "Glob":
                detail["pattern"] = inp.get("pattern", "")[:60]
            elif name == "Skill":
                detail["skill"] = inp.get("skill", "")
            elif name == "Task":
                detail["subagent"] = inp.get("subagent_type", "")
                detail["desc"] = inp.get("description", "")[:60]

            tool_details.append(detail)

    return {
        "tool_counts": dict(tool_counts),
        "total_tool_calls": sum(tool_counts.values()),
        "files_read": sorted(files_read),
        "files_written": sorted(files_written),
        "commands_run": commands_run,
        "tool_details": tool_details,
    }


def categorize_bash_commands(commands):
    """Categorize bash commands by type for visibility."""
    categories = {
        "cargo test": [],
        "cargo clippy": [],
        "cargo build/check": [],
        "jj issue": [],
        "jj vcs": [],
        "ls/file ops": [],
        "other": [],
    }
    for cmd in commands:
        if re.search(r'cargo\s+test', cmd):
            categories["cargo test"].append(cmd)
        elif re.search(r'cargo\s+clippy', cmd):
            categories["cargo clippy"].append(cmd)
        elif re.search(r'cargo\s+(build|check)', cmd):
            categories["cargo build/check"].append(cmd)
        elif re.search(r'jj\s+issue', cmd):
            categories["jj issue"].append(cmd)
        elif re.search(r'jj\s', cmd):
            categories["jj vcs"].append(cmd)
        elif re.search(r'^(ls|find|wc|stat)\b', cmd):
            categories["ls/file ops"].append(cmd)
        else:
            categories["other"].append(cmd)
    # Remove empty categories
    return {k: v for k, v in categories.items() if v}


def analyze_tool_result_sizes(events):
    """Measure tool result sizes to find context waste."""
    # Build a map of tool_use_id -> tool_name
    tool_names = {}
    for ev in events:
        if ev.get("type") == "assistant":
            for c in ev.get("message", {}).get("content", []):
                if c.get("type") == "tool_use":
                    tool_names[c.get("id", "")] = c.get("name", "unknown")

    results = []
    for ev in events:
        if ev.get("type") != "user":
            continue
        for c in ev.get("message", {}).get("content", []):
            if c.get("type") != "tool_result":
                continue
            content = c.get("content", "")
            if isinstance(content, (list, dict)):
                size = len(json.dumps(content))
            else:
                size = len(str(content))
            tool_id = c.get("tool_use_id", "")
            tool_name = tool_names.get(tool_id, "unknown")
            results.append({"tool": tool_name, "size": size})

    return results


def detect_waste(turns, tool_analysis, events=None):
    """Flag potential context waste patterns."""
    issues = []

    # Duplicate file reads
    read_files = []
    for d in tool_analysis["tool_details"]:
        if d["tool"] == "Read" and "file" in d:
            read_files.append(d["file"])
    dupes = [f for f in set(read_files) if read_files.count(f) > 1]
    if dupes:
        issues.append(f"Duplicate file reads: {', '.join(dupes)}")

    # Skill invocations (waste in headless mode)
    skills = [d for d in tool_analysis["tool_details"] if d["tool"] == "Skill"]
    if skills:
        names = [s.get("skill", "?") for s in skills]
        issues.append(f"Skill invocations (headless waste): {', '.join(names)}")

    # Multiple test runs
    test_cmds = [
        c for c in tool_analysis["commands_run"]
        if "cargo test" in c
    ]
    if len(test_cmds) > 2:
        issues.append(f"Excessive test runs: {len(test_cmds)} (budget: 2)")

    # Multiple build/clippy runs
    build_cmds = [
        c for c in tool_analysis["commands_run"]
        if "cargo build" in c or "cargo clippy" in c or "cargo check" in c
    ]
    if len(build_cmds) > 2:
        issues.append(f"Excessive build commands: {len(build_cmds)}")

    # Unfiltered cargo output
    unfiltered = [
        c for c in tool_analysis["commands_run"]
        if ("cargo test" in c or "cargo build" in c or "cargo clippy" in c)
        and "grep" not in c
        and "head" not in c
    ]
    if unfiltered:
        issues.append(
            f"Unfiltered cargo commands: {len(unfiltered)} "
            f"(e.g. {unfiltered[0][:80]})"
        )

    # Tool result size analysis
    if events:
        result_sizes = analyze_tool_result_sizes(events)
        large = [r for r in result_sizes if r["size"] > 5000]
        if large:
            total_large = sum(r["size"] for r in large)
            issues.append(
                f"Large tool results (>5k): {len(large)} results, "
                f"{total_large:,} chars total"
            )
        # Context composition
        by_tool = {}
        for r in result_sizes:
            by_tool.setdefault(r["tool"], 0)
            by_tool[r["tool"]] += r["size"]
        total_results = sum(by_tool.values())
        if total_results > 0:
            top = sorted(by_tool.items(), key=lambda x: -x[1])[:3]
            breakdown = ", ".join(
                f"{t}: {s:,} ({s/total_results*100:.0f}%)"
                for t, s in top
            )
            issues.append(f"Context by tool: {breakdown}")

    # Too many tool calls
    total = tool_analysis["total_tool_calls"]
    if total > 20:
        issues.append(f"Over budget: {total} tool calls (budget: 20)")

    return issues


def format_timeline(turns):
    """Format turns as a chronological timeline."""
    lines = []
    step = 0
    for turn in turns:
        if turn["role"] == "assistant":
            step += 1
            for tc in turn.get("tool_calls", []):
                name = tc["name"]
                inp = tc["input"]
                if name == "Read":
                    lines.append(f"  {step}. Read {inp.get('file_path', '?')}")
                elif name == "Edit":
                    lines.append(f"  {step}. Edit {inp.get('file_path', '?')}")
                elif name == "Write":
                    lines.append(f"  {step}. Write {inp.get('file_path', '?')}")
                elif name == "Bash":
                    cmd = inp.get("command", "?")[:80]
                    lines.append(f"  {step}. Bash: {cmd}")
                elif name == "Skill":
                    lines.append(f"  {step}. Skill: {inp.get('skill', '?')}")
                elif name == "Task":
                    lines.append(
                        f"  {step}. Task({inp.get('subagent_type', '?')}): "
                        f"{inp.get('description', '?')[:60]}"
                    )
                elif name == "Grep":
                    lines.append(f"  {step}. Grep: {inp.get('pattern', '?')[:40]}")
                elif name == "Glob":
                    lines.append(f"  {step}. Glob: {inp.get('pattern', '?')[:40]}")
                else:
                    lines.append(f"  {step}. {name}")
            for text in turn.get("text_blocks", []):
                # Show just first line of text output
                first = text.strip().split("\n")[0][:100]
                if first:
                    lines.append(f"  {step}. Output: {first}")
    return "\n".join(lines)


CONTEXT_WINDOW = 200_000


def analyze_context(events):
    """Analyze context window utilization from per-turn usage data."""
    turn_data = []
    for ev in events:
        if ev.get("type") != "assistant":
            continue
        msg = ev.get("message", {})
        usage = msg.get("usage", {})
        model = msg.get("model", "unknown")
        if not usage:
            continue
        inp = usage.get("input_tokens", 0)
        cache_read = usage.get("cache_read_input_tokens", 0)
        cache_create = usage.get("cache_creation_input_tokens", 0)
        out = usage.get("output_tokens", 0)
        total = inp + cache_read + cache_create
        if total > 0:
            turn_data.append({
                "total": total,
                "output": out,
                "model": model,
                "cache_hit_pct": (cache_read / total * 100) if total else 0,
            })

    if not turn_data:
        return None

    # Filter to main model turns (largest context = main model, not subagents)
    main_turns = [t for t in turn_data if t["total"] > 20000]
    if not main_turns:
        main_turns = turn_data

    start_ctx = main_turns[0]["total"]
    end_ctx = main_turns[-1]["total"]
    max_ctx = max(t["total"] for t in main_turns)
    growth = end_ctx - start_ctx
    num_main = len(main_turns)
    per_turn = growth / max(num_main - 1, 1) if num_main > 1 else 0

    total_output = sum(t["output"] for t in turn_data)
    # Use average context per turn * num turns as "total context read"
    # (each turn re-reads the full context, so this reflects actual API reads)
    avg_ctx = sum(t["total"] for t in turn_data) / len(turn_data)
    efficiency = (total_output / avg_ctx * 100) if avg_ctx else 0

    avg_cache_hit = sum(t["cache_hit_pct"] for t in turn_data) / len(turn_data)

    return {
        "start_pct": start_ctx / CONTEXT_WINDOW * 100,
        "end_pct": end_ctx / CONTEXT_WINDOW * 100,
        "max_pct": max_ctx / CONTEXT_WINDOW * 100,
        "growth_per_turn": per_turn,
        "total_turns": len(turn_data),
        "main_turns": num_main,
        "efficiency_pct": efficiency,
        "avg_cache_hit_pct": avg_cache_hit,
        "start_tokens": start_ctx,
        "end_tokens": end_ctx,
    }


def format_summary(events, output_json=False, show_timeline=False):
    """Format the full analysis."""
    turns = extract_turns(events)
    result = extract_result(events)
    tool_analysis = analyze_tool_usage(turns)
    waste = detect_waste(turns, tool_analysis, events)
    ctx = analyze_context(events)

    if output_json:
        return json.dumps({
            "result": result,
            "tools": tool_analysis,
            "context": ctx,
            "waste_flags": waste,
        }, indent=2)

    lines = []
    lines.append("=" * 60)
    lines.append("RALPH ITERATION ANALYSIS")
    lines.append("=" * 60)

    if result:
        status = "OK" if result["success"] else "FAILED"
        lines.append(
            f"Status: {status}  "
            f"Turns: {result['num_turns']}  "
            f"Cost: ${result['cost_usd']:.4f}  "
            f"Duration: {result['duration_ms']}ms"
        )
        usage = result.get("usage", {})
        lines.append(
            f"Tokens: in={usage.get('input_tokens', 0)} "
            f"out={usage.get('output_tokens', 0)} "
            f"cache_read={usage.get('cache_read_input_tokens', 0)} "
            f"cache_create={usage.get('cache_creation_input_tokens', 0)}"
        )

    if ctx:
        lines.append("")
        lines.append(
            f"Context: {ctx['start_pct']:.0f}% -> {ctx['end_pct']:.0f}% "
            f"(max {ctx['max_pct']:.0f}%) of {CONTEXT_WINDOW // 1000}k window"
        )
        lines.append(
            f"  Growth: +{ctx['growth_per_turn']:.0f} tok/turn  "
            f"Efficiency: {ctx['efficiency_pct']:.2f}% (output/context)  "
            f"Cache hit: {ctx['avg_cache_hit_pct']:.1f}%"
        )

    lines.append("")
    lines.append(f"Tool calls: {tool_analysis['total_tool_calls']}")
    for tool, count in sorted(
        tool_analysis["tool_counts"].items(), key=lambda x: -x[1]
    ):
        lines.append(f"  {tool}: {count}")

    if tool_analysis["files_read"]:
        lines.append("")
        lines.append("Files read:")
        for f in tool_analysis["files_read"]:
            lines.append(f"  {f}")

    if tool_analysis["files_written"]:
        lines.append("")
        lines.append("Files written:")
        for f in tool_analysis["files_written"]:
            lines.append(f"  {f}")

    if tool_analysis["commands_run"]:
        lines.append("")
        bash_cats = categorize_bash_commands(tool_analysis["commands_run"])
        lines.append("Bash breakdown:")
        for cat, cmds in bash_cats.items():
            lines.append(f"  {cat}: {len(cmds)}")
        lines.append("")
        lines.append("Commands:")
        for c in tool_analysis["commands_run"]:
            lines.append(f"  $ {c}")

    if waste:
        lines.append("")
        lines.append("WASTE FLAGS:")
        for w in waste:
            lines.append(f"  ! {w}")

    if show_timeline:
        lines.append("")
        lines.append("TIMELINE:")
        lines.append(format_timeline(turns))

    lines.append("")
    return "\n".join(lines)


def find_sibling_segments(path):
    """Find all segment logs belonging to the same iteration.

    Segment logs follow the pattern: iter-TIMESTAMP-ITER-segNN.jsonl
    Given any segment, finds all siblings sorted by segment number.
    Also handles legacy single-file logs (iter-TIMESTAMP-ITER.jsonl).
    """
    p = Path(path)
    name = p.name

    # Match segment pattern: iter-TIMESTAMP-ITER-segNN.jsonl
    m = re.match(r'(iter-\d{8}-\d{6}-\d{3})-seg\d{2}\.jsonl$', name)
    if not m:
        # Not a segment file — return just this file
        return [p]

    prefix = m.group(1)
    parent = p.parent
    segments = sorted(parent.glob(f"{prefix}-seg*.jsonl"))
    return segments if segments else [p]


def format_segments_breakdown(segment_paths):
    """Format a per-segment breakdown for multi-segment iterations."""
    lines = []
    lines.append("")
    lines.append("SEGMENT BREAKDOWN:")
    lines.append("-" * 40)

    total_cost = 0
    total_turns = 0

    for i, seg_path in enumerate(segment_paths, 1):
        events = parse_events(str(seg_path))
        result = extract_result(events)
        ctx = analyze_context(events)
        tool_analysis = analyze_tool_usage(extract_turns(events))

        cost = result["cost_usd"] if result else 0
        turns = result["num_turns"] if result else 0
        total_cost += cost
        total_turns += turns

        success = "OK" if (result and result["success"]) else "budget/error"
        ctx_info = ""
        if ctx:
            ctx_info = f"  ctx: {ctx['start_pct']:.0f}%->{ctx['end_pct']:.0f}%"

        lines.append(
            f"  Seg {i}: {turns} turns, ${cost:.3f}, "
            f"{tool_analysis['total_tool_calls']} tools, "
            f"{success}{ctx_info}"
        )

    lines.append(f"  Total: {total_turns} turns, ${total_cost:.3f}")
    lines.append("")
    return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        print(
            "Usage: ralph-analyze.py <logfile.jsonl> "
            "[--json] [--timeline] [--segments]"
        )
        sys.exit(1)

    path = sys.argv[1]
    output_json = "--json" in sys.argv
    show_timeline = "--timeline" in sys.argv
    show_segments = "--segments" in sys.argv

    if not Path(path).exists():
        print(f"Error: {path} not found", file=sys.stderr)
        sys.exit(1)

    # Find sibling segments for multi-segment iterations
    segments = find_sibling_segments(path)
    is_multi_segment = len(segments) > 1

    if is_multi_segment and not output_json:
        # Concatenate all segment events for full iteration analysis
        all_events = []
        for seg in segments:
            all_events.extend(parse_events(str(seg)))
        if not all_events:
            print(f"Error: no events in segments", file=sys.stderr)
            sys.exit(1)
        print(format_summary(all_events, output_json, show_timeline))
        # Always show segment breakdown for multi-segment
        print(format_segments_breakdown(segments))
    else:
        events = parse_events(path)
        if not events:
            print(f"Error: no events in {path}", file=sys.stderr)
            sys.exit(1)
        print(format_summary(events, output_json, show_timeline))

    # Show segments breakdown on request even for single-segment files
    if show_segments and not is_multi_segment:
        # Check if there are segments we missed (user passed a non-segment file)
        print("(Single segment — no breakdown to show)")


if __name__ == "__main__":
    main()
