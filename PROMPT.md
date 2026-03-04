# Headless Task Loop — Single Iteration

This is a headless automation loop. Skip ALL process/interactive skills (where-was-i, brainstorming, context-snapshot, episodic-memory). Go directly to task selection.

@a study specs/README.md

## Step 0: Health Check

Run `cargo test -p fmpl-core 2>&1 | grep -E '^(test result:|FAILED)' | head -5` first.

If there are ANY test failures, **fixing them is your task for this iteration.** Do not pick
a new task from the issue tracker. Do not dismiss failures as "pre-existing." The build must
be green before new work begins. Create an issue for the fix if one doesn't exist, fix it,
commit it, and output `COMPLETED:<id> fix: <description>`.

If tests pass, proceed to Step 1.

## Step 1: Pick Task

`jj issue ready | head -2` → pick the top task → `jj issue show <id>`.

The issue description IS your research. Do NOT re-read files already quoted in the issue.

**You are now committed to this task.** You may not pick a different task (except via Step 2 close-and-pick).

## Step 2: Triage

Check if the issue is already done. Max 3 close-and-pick loops total:

- If comments say the work is done → verify with one test → if pass, close and loop to Step 1.
- If subtasks exist and are all closed → close parent, loop to Step 1.
- If a test already passes → close and loop to Step 1.

If not already done → go to Step 3. **Do not go back to Step 1.**

## Step 3: Scope and Implement

You MUST produce code in this step. Triage and decomposition are not deliverables.

**If the task is single-crate**: implement it directly.

**If the task spans multiple crates** or is too large for one iteration:
1. Decompose into subtasks (`jj issue create '<title>' --description="<desc>"`).
2. Pick the first subtask.
3. Implement it — write code, write tests.

Either way, you must have **committed code** before moving to Step 4.

**Use subagents for non-conflicting work:**
- Use `Explore` subagent to understand code structure before editing.
- Use `codebase-analyzer` to trace call chains or find usage patterns.
- Use `context7` for external crate API docs (never grep `~/.cargo/registry`).
- Use subagents for research in parallel while you plan your edits.
- Do NOT use subagents to write code in files you're also editing (conflicts).

**Anti-avoidance rules:**
- "It's cross-crate" is not a reason to skip. Decompose and implement the first piece.
- "I'll decompose now and implement next iteration" is not allowed.
- "Let me pick something easier" is not allowed. You are committed from Step 1.
- If you catch yourself creating subtasks without writing code, stop and write code.
- Using a subagent for research does not count as implementation work.

## Step 4: Verify & Commit

1. ONE `cargo test` run (filtered). Must pass.
2. ONE `cargo clippy` run (filtered). Must pass, zero warnings.
3. Commit with jj (use jj-workflow skill for commit message conventions).

If the build is broken, fix it. Do not declare done while broken.

## Step 5: Output

Print exactly one line:
```
COMPLETED:<id> <conventional commit message>
```

Or if blocked:
```
BLOCKED:<id> <reason>
```

## Budget

- 40 tool calls max per iteration
- 3 close-and-pick loops max in Step 2
- 3-strike rule: same error 3 times → write spec, comment on issue, stop
- Do NOT use TodoWrite — the issue tracker is the task list
