#!/usr/bin/env python3
"""
YAML-driven multi-REPL harness.

Reads a scenario file describing a sequence of typed actions, manages
one or more pexpect-driven `fmpl-cli` REPL processes (called sessions),
captures their output, and substitutes `{{ var }}` placeholders from
captured values.

Action types
------------
- narrate:        free-form text echoed to transcript
- shell:          run a shell command, echo output
- spawn:          start a new REPL session, name it
- reset:          send `.reset` to a session
- open_store:     send `.open-store <path>` to a session
- repl_eval:      send an FMPL expression to a session, capture result
- store_source:   send `.store-source <var>`, capture printed hash
- store_value:    send `.store-value <var>`, capture printed hash
- store_bytecode: send `.store-bytecode <var>`, capture printed hash
- fetch:          send `.fetch <hash>`, capture printed source/bytes
- assert_equal:   assert two values from captured-vars are equal

Variable substitution
---------------------
Any string field containing `{{ name }}` is substituted with the
captured value before the action runs. Capture keys are written by
actions with a `capture_*` field.

Run:
    .demo-venv/bin/python demo/harness.py demo/scenarios/<name>.yaml
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

import pexpect
import pexpect.popen_spawn
import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
PROMPT = "fmpl> "
# CSI sequences: ESC `[` then optional `?` (private mode), then params
# (digits + `;`), then a final byte in the range `@`-`~`. The `?` form
# is what rustyline emits for bracketed-paste mode toggles
# (`ESC [ ? 2004 h` / `ESC [ ? 2004 l`); the non-? form is plain SGR.
ANSI = re.compile(r"\x1b\[\??[0-9;]*[@-~]")
RESULT_LINE = re.compile(r"^\s*=>\s*(.*)$")
ERROR_LINE = re.compile(r"^\s*(Error:\s.*)$")
HASH_LINE = re.compile(r"^\s*hash:\s+([0-9a-f]{64})\s*$")
LOADED_LINE = re.compile(
    r"^\s*loaded\s+(\d+)\s+bytes\s+for\s+hash\s+([0-9a-f]{64})\s*$"
)
SOURCE_LINE = re.compile(r'^\s*source:\s+"(.*)"\s*$')
KIND_LINE = re.compile(r"^\s*kind:\s+(\S+)\s*$")
BYTES_LINE = re.compile(r"^\s*bytes:\s+(\d+)\s*$")
SUBST_RE = re.compile(r"\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}")


def strip_ansi(s: str) -> str:
    return ANSI.sub("", s)


# ─────────────────────────────────────────────────────────────────────
# Transcript writer (stdout colored + plain file)
# ─────────────────────────────────────────────────────────────────────


class Out:
    """Writes to a transcript file always; optionally echoes to stdout.

    - `color`: if True, stdout gets ANSI colors. Transcript file is
      always plain (ANSI-stripped).
    - `echo`: if False, suppress stdout entirely (transcript-only).
      Useful for headless / unattended runs where the live terminal
      output is just noise.
    """

    def __init__(self, transcript_path: Path,
                 color: bool = True, echo: bool = True):
        self.transcript = transcript_path.open("w", encoding="utf-8")
        self.color = color
        self.echo = echo

    def close(self) -> None:
        self.transcript.close()

    def _emit(self, plain: str, colored: str | None = None) -> None:
        # Transcript always gets the plain, ANSI-stripped form.
        self.transcript.write(strip_ansi(plain) + "\n")
        if not self.echo:
            return
        live = colored if (colored is not None and self.color) else plain
        if not self.color:
            live = strip_ansi(live)
        print(live)

    def banner(self, text: str) -> None:
        bar = "═" * 72
        self._emit(f"\n{bar}\n  {text}\n{bar}\n",
                   f"\n\033[1;33m{bar}\n  {text}\n{bar}\033[0m\n")

    def section(self, text: str) -> None:
        bar = "─" * 72
        self._emit(f"\n{bar}\n{text}\n{bar}\n",
                   f"\n\033[1;36m{bar}\n{text}\n{bar}\033[0m\n")

    def narrate(self, text: str) -> None:
        self._emit(f"# {text}", f"\033[2;37m# {text}\033[0m")

    def prompt(self, session: str, line: str) -> None:
        self._emit(f"[{session}] fmpl> {line}",
                   f"\033[0;36m[{session}]\033[0m "
                   f"\033[0;32mfmpl> \033[0m{line}")

    def shell_echo(self, line: str) -> None:
        self._emit(f"$ {line}", f"\033[0;34m$ \033[0m{line}")

    def code_block(self, caption: str, lines: list[str], lang: str = "fmpl") -> None:
        """Render a fenced code block to the transcript. Reinforces
        that the demo is over a real source artifact, not magic."""
        if caption:
            self._emit(f"# source: {caption}",
                       f"\033[2;36m# source: {caption}\033[0m")
        fence = f"```{lang}"
        self._emit(fence, f"\033[2;37m{fence}\033[0m")
        for ln in lines:
            self._emit(ln, f"\033[0;37m{ln}\033[0m")
        self._emit("```", f"\033[2;37m```\033[0m")

    def output(self, line: str) -> None:
        self._emit(line)

    def result(self, line: str) -> None:
        self._emit(line, f"\033[1;35m{line}\033[0m")

    def error(self, line: str) -> None:
        self._emit(line, f"\033[1;31m{line}\033[0m")

    def info(self, line: str) -> None:
        self._emit(line, f"\033[2;37m{line}\033[0m")

    def capture(self, name: str, value: str) -> None:
        # Slightly different shade so captures are easy to spot in transcript.
        truncated = value if len(value) <= 70 else value[:67] + "…"
        self._emit(f"@captured {name} = {truncated}",
                   f"\033[0;33m@captured\033[0m "
                   f"\033[1;33m{name}\033[0m = "
                   f"\033[0;33m{truncated}\033[0m")


# ─────────────────────────────────────────────────────────────────────
# REPL session
# ─────────────────────────────────────────────────────────────────────


@dataclass
class Session:
    """A named REPL process driven by pexpect over pipes (no pty).

    Using `pexpect.popen_spawn.PopenSpawn` instead of the pty-backed
    `pexpect.spawn` means the child's stdin/stdout are plain pipes —
    `isatty()` returns False inside the child, which makes fmpl-cli
    auto-select its scripting-friendly mode (no rustyline, no ANSI,
    no bracketed-paste escapes).
    """

    name: str
    child: pexpect.popen_spawn.PopenSpawn
    history: list[str] = field(default_factory=list)

    def send(self, line: str, out: Out, timeout: float = 60.0) -> str:
        """Send one line, wait for the next prompt, return captured text."""
        out.prompt(self.name, line)
        # PopenSpawn.sendline appends `\n`; line should be a single line.
        self.child.sendline(line)
        try:
            self.child.expect(PROMPT, timeout=timeout)
        except pexpect.exceptions.TIMEOUT:
            raise RuntimeError(
                f"timeout waiting for prompt in session {self.name!r} "
                f"after sending: {line!r}"
            )
        captured = self.child.before or ""
        self.history.append(captured)
        return captured

    def close(self) -> None:
        try:
            self.child.sendline(".quit")
            # PopenSpawn doesn't always cleanly raise EOF; close stdin
            # and wait briefly.
            try:
                self.child.sendeof()
            except Exception:
                pass
            try:
                self.child.expect(pexpect.EOF, timeout=5)
            except pexpect.exceptions.TIMEOUT:
                pass
        except (pexpect.exceptions.EOF, OSError):
            pass
        # Force-terminate as a fallback if the process is still alive.
        try:
            self.child.kill(15)  # SIGTERM
        except Exception:
            pass


def spawn_repl(name: str) -> Session:
    # `cargo run -p fmpl-cli --quiet` printed via the workspace usually
    # emits a `Compiling`/`Finished` summary on stderr the first run; we
    # let it through unmodified — Popen merges stderr into the child
    # process's stderr stream (not stdout), so PopenSpawn's reads only
    # see fmpl-cli's stdout output.
    child = pexpect.popen_spawn.PopenSpawn(
        ["cargo", "run", "-p", "fmpl-cli", "--quiet"],
        encoding="utf-8",
        timeout=240,
        cwd=str(REPO_ROOT),
    )
    child.expect(PROMPT, timeout=240)
    return Session(name=name, child=child)


# ─────────────────────────────────────────────────────────────────────
# Captured-vars + substitution
# ─────────────────────────────────────────────────────────────────────


class Vars:
    def __init__(self) -> None:
        self.values: dict[str, Any] = {}

    def set(self, key: str, value: Any) -> None:
        self.values[key] = value

    def get(self, key: str) -> Any:
        if key not in self.values:
            raise KeyError(f"no captured variable named {key!r}. "
                           f"Known: {sorted(self.values)}")
        return self.values[key]

    def subst(self, s: str) -> str:
        return SUBST_RE.sub(lambda m: str(self.get(m.group(1))), s)


# ─────────────────────────────────────────────────────────────────────
# Action handlers
# ─────────────────────────────────────────────────────────────────────


def expect_field(action: dict, name: str) -> Any:
    if name not in action:
        raise ValueError(f"action {action.get('type')!r} missing field {name!r}")
    return action[name]


def report_repl_output(captured: str, out: Out) -> dict[str, str]:
    """Print captured REPL output line-by-line; return parsed extras
    (hash, source, kind, bytes, result, error)."""
    parsed: dict[str, str] = {}
    for ln in captured.splitlines():
        plain = ln.rstrip()
        if not plain.strip():
            continue
        m = HASH_LINE.match(plain)
        if m:
            parsed["hash"] = m.group(1)
            out.result(plain.strip())
            continue
        m = LOADED_LINE.match(plain)
        if m:
            parsed["loaded_bytes"] = m.group(1)
            parsed["hash"] = m.group(2)
            out.result(plain.strip())
            continue
        m = SOURCE_LINE.match(plain)
        if m:
            parsed["source"] = m.group(1)
            out.result(plain.strip())
            continue
        m = KIND_LINE.match(plain)
        if m:
            parsed["kind"] = m.group(1)
            out.info(plain.strip())
            continue
        m = BYTES_LINE.match(plain)
        if m:
            parsed["bytes"] = m.group(1)
            out.info(plain.strip())
            continue
        m = RESULT_LINE.match(plain)
        if m:
            parsed["result"] = m.group(1)
            out.result(plain.strip())
            continue
        m = ERROR_LINE.match(plain)
        if m:
            parsed["error"] = m.group(1)
            out.error(plain.strip())
            continue
        # Plain echo lines we don't care about (REPL banner, etc.) we
        # still surface as info so the transcript is faithful.
        out.info(plain.strip())
    return parsed


def run_action(
    action: dict,
    sessions: dict[str, Session],
    vars: Vars,
    out: Out,
    pause: float,
) -> bool:
    """Run one action. Returns False if it failed."""
    a_type = action.get("type")
    if not a_type:
        out.error(f"action missing `type:` field: {action!r}")
        return False

    # Resolve all string fields against captured vars (deep-shallow:
    # one level of dict values; lists not used in this schema).
    def resolved(name: str) -> Any:
        v = action.get(name)
        if isinstance(v, str):
            return vars.subst(v)
        return v

    if a_type == "narrate":
        text = resolved("text") or ""
        out.narrate(text)
        return True

    if a_type == "section":
        out.section(resolved("title") or "")
        return True

    if a_type == "banner":
        out.banner(resolved("title") or "")
        return True

    if a_type == "comment":
        # Multi-line annotation block. Renders each non-empty line as a
        # narrate-style `# ...` so the transcript reads like a paginated
        # explanation. Different from `narrate` (which is one paragraph)
        # in that authors can group related sentences and have them
        # rendered separately. Title is optional.
        title = resolved("title")
        body = resolved("body") or resolved("text") or ""
        if title:
            out.section(title)
        for line in str(body).splitlines():
            stripped = line.strip()
            if stripped:
                out.narrate(stripped)
            else:
                out.output("")
        return True

    if a_type == "show_source":
        # Render a code block to the transcript. Reinforces that the
        # demo runs over real source — when the next action queries
        # `world.rd(:room)`, the reader has just seen the `world.out(...)`
        # line that created it.
        #
        # Three modes:
        #   1. file: <path>, lines: "N-M"   → read range from repo file
        #   2. file: <path>                 → whole file
        #   3. snippet: "..."               → inline literal
        # `lang:` defaults to `fmpl`; override for shell, rust, etc.
        # `caption:` optional; printed before the fence.
        caption = resolved("caption") or ""
        lang = resolved("lang") or "fmpl"
        snippet = resolved("snippet")
        file_arg = resolved("file")
        lines_arg = resolved("lines")
        if snippet is not None:
            content_lines = str(snippet).splitlines()
            if not caption:
                caption = "(inline)"
        elif file_arg:
            path = REPO_ROOT / str(file_arg)
            try:
                all_lines = path.read_text().splitlines()
            except OSError as e:
                out.error(f"show_source: cannot read {path}: {e}")
                return False
            if lines_arg:
                m = re.match(r"^\s*(\d+)\s*-\s*(\d+)\s*$", str(lines_arg))
                if not m:
                    out.error(f"show_source: bad lines spec "
                              f"{lines_arg!r} (want \"N-M\")")
                    return False
                start = max(int(m.group(1)), 1)
                end = min(int(m.group(2)), len(all_lines))
                content_lines = all_lines[start - 1:end]
                if not caption:
                    caption = f"{file_arg} (lines {start}-{end})"
            else:
                content_lines = all_lines
                if not caption:
                    caption = f"{file_arg} (whole file)"
        else:
            out.error("show_source: need either `snippet:` or `file:`")
            return False
        out.code_block(caption, content_lines, lang=lang)
        return True

    if a_type == "shell":
        cmd = expect_field(action, "cmd")
        if isinstance(cmd, str):
            argv = ["bash", "-c", vars.subst(cmd)]
            display = vars.subst(cmd)
        else:
            argv = [vars.subst(str(x)) for x in cmd]
            display = " ".join(argv)
        out.shell_echo(display)
        proc = subprocess.run(argv, capture_output=True, text=True,
                              cwd=str(REPO_ROOT))
        for line in (proc.stdout or "").splitlines():
            out.output(line)
        for line in (proc.stderr or "").splitlines():
            out.error(line)
        capture_as = action.get("capture_stdout_as")
        if capture_as:
            vars.set(capture_as, (proc.stdout or "").strip())
            out.capture(capture_as, vars.get(capture_as))
        if proc.returncode != 0:
            out.error(f"(shell exited {proc.returncode})")
            return False
        return True

    if a_type == "spawn":
        name = expect_field(action, "name")
        if name in sessions:
            out.error(f"session {name!r} already exists")
            return False
        out.info(f"# spawning REPL session: {name}")
        sessions[name] = spawn_repl(name)
        return True

    if a_type == "reset":
        sess = sessions[expect_field(action, "in")]
        sess.send(".reset", out)
        return True

    if a_type == "close_session":
        name = expect_field(action, "name")
        sess = sessions.pop(name, None)
        if sess is None:
            out.error(f"no session named {name!r} to close")
            return False
        out.info(f"# closing REPL session: {name}")
        sess.close()
        return True

    if a_type == "open_store":
        sess = sessions[expect_field(action, "in")]
        path = resolved("path")
        captured = sess.send(f".open-store {path}", out)
        report_repl_output(captured, out)
        return True

    if a_type == "repl_eval":
        sess = sessions[expect_field(action, "in")]
        expr = resolved("expr")
        captured = sess.send(expr, out)
        parsed = report_repl_output(captured, out)
        capture_result_as = action.get("capture_result_as")
        if capture_result_as and "result" in parsed:
            vars.set(capture_result_as, parsed["result"])
            out.capture(capture_result_as, parsed["result"])
        # By default an error in REPL output is a scenario failure.
        # `allow_error: true` says "we know this might error, that's OK"
        # (e.g. capability denial demos where the error IS the feature).
        # `expect_error: true` is stricter: success requires an error.
        if action.get("expect_error"):
            return "error" in parsed
        if action.get("allow_error"):
            return True
        return "error" not in parsed

    def store_op(cmd_name: str) -> bool:
        sess = sessions[expect_field(action, "in")]
        var = expect_field(action, "var")
        captured = sess.send(f".{cmd_name} {var}", out)
        parsed = report_repl_output(captured, out)
        capture_hash_as = action.get("capture_hash_as")
        if capture_hash_as and "hash" in parsed:
            vars.set(capture_hash_as, parsed["hash"])
            out.capture(capture_hash_as, parsed["hash"])
        return "error" not in parsed and "hash" in parsed

    if a_type == "store_source":
        return store_op("store-source")
    if a_type == "store_value":
        return store_op("store-value")
    if a_type == "store_bytecode":
        return store_op("store-bytecode")

    if a_type == "fetch":
        sess = sessions[expect_field(action, "in")]
        h = resolved("hash")
        captured = sess.send(f".fetch {h}", out)
        parsed = report_repl_output(captured, out)
        capture_source_as = action.get("capture_source_as")
        if capture_source_as and "source" in parsed:
            vars.set(capture_source_as, parsed["source"])
            out.capture(capture_source_as, parsed["source"])
        return "error" not in parsed

    if a_type == "assert_equal":
        lhs = resolved("lhs")
        rhs = resolved("rhs")
        label = action.get("label") or "assertion"
        if lhs == rhs:
            out.result(f"PASS: {label}  ({lhs!r} == {rhs!r})")
            return True
        else:
            out.error(f"FAIL: {label}")
            out.error(f"  lhs: {lhs!r}")
            out.error(f"  rhs: {rhs!r}")
            return False

    if a_type == "sleep":
        time.sleep(float(action.get("seconds", 0.5)))
        return True

    out.error(f"unknown action type: {a_type}")
    return False


# ─────────────────────────────────────────────────────────────────────
# Runner
# ─────────────────────────────────────────────────────────────────────


def run_scenario(
    scenario_path: Path,
    fast: bool,
    echo: bool,
    color: bool,
    sleep_default: float | None = None,
) -> int:
    scenario = yaml.safe_load(scenario_path.read_text())
    title = scenario.get("title") or scenario_path.stem
    description = scenario.get("description") or ""
    actions: list[dict] = scenario.get("actions") or []

    # Resolve sleep defaults. Priority:
    #   --fast (CLI)   → 0.0  (overrides everything)
    #   --sleep N (CLI) → N
    #   scenario `defaults.sleep_after` → that value
    #   else           → 0.9
    if fast:
        default_sleep = 0.0
    elif sleep_default is not None:
        default_sleep = sleep_default
    else:
        scenario_defaults = scenario.get("defaults") or {}
        default_sleep = float(scenario_defaults.get("sleep_after", 0.9))

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    transcript_path = (REPO_ROOT / "demo"
                       / f"transcript-{scenario_path.stem}-{timestamp}.txt")
    out = Out(transcript_path, color=color, echo=echo)

    sessions: dict[str, Session] = {}
    vars = Vars()
    # Pre-seed the convenience var `tmpdir`.
    tmpdir = Path(subprocess.check_output(["mktemp", "-d",
                                           "/tmp/fmpl-harness.XXXXXX"],
                                          text=True).strip())
    vars.set("tmpdir", str(tmpdir))

    failures = 0
    try:
        out.banner(title)
        if description:
            out.narrate(description)
        out.info(f"# scenario: {scenario_path}")
        out.info(f"# transcript: {transcript_path}")
        out.info(f"# tmpdir: {tmpdir}")
        out.info(f"# echo: {'on' if echo else 'off (headless)'}")
        out.info(f"# default sleep_after: {default_sleep}s")
        out.info(f"# started: {datetime.now().isoformat(timespec='seconds')}")

        for action in actions:
            ok = run_action(action, sessions, vars, out, default_sleep)
            if not ok:
                failures += 1
            # Per-action `sleep_after` overrides the default. `0` is a
            # valid value (no sleep), so we check for presence, not
            # truthiness.
            this_sleep = (action.get("sleep_after")
                          if "sleep_after" in action
                          else default_sleep)
            if this_sleep:
                time.sleep(float(this_sleep))

        out.banner("Summary")
        out.info(f"Actions run: {len(actions)}")
        out.info(f"Failures:    {failures}")
        out.info(f"Sessions:    {sorted(sessions.keys())}")
        out.info(f"Captured:    {sorted(vars.values.keys())}")
        out.info(f"Transcript:  {transcript_path}")
        return 0 if failures == 0 else 1
    finally:
        for sess in sessions.values():
            sess.close()
        shutil.rmtree(tmpdir, ignore_errors=True)
        out.close()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run a YAML-driven FMPL REPL scenario."
    )
    parser.add_argument("scenario", type=Path,
                        help="path to a YAML scenario file")
    parser.add_argument("--fast", action="store_true",
                        help="no inter-action pauses (overrides --sleep)")
    parser.add_argument("--sleep", type=float, metavar="SECONDS",
                        default=None,
                        help="default sleep after each action; "
                             "overrides scenario defaults but not --fast")
    parser.add_argument("--no-echo", action="store_true",
                        help="don't print to stdout — transcript only")
    parser.add_argument("--no-color", action="store_true",
                        help="strip ANSI from stdout (transcript is always plain)")
    args = parser.parse_args()
    if not args.scenario.exists():
        print(f"no such scenario: {args.scenario}", file=sys.stderr)
        return 2
    return run_scenario(
        args.scenario,
        fast=args.fast,
        echo=not args.no_echo,
        color=not args.no_color,
        sleep_default=args.sleep,
    )


if __name__ == "__main__":
    sys.exit(main())
