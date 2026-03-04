Run verification:

1. `cargo fmt` — format all code.
2. ONE `cargo test` run (filtered to your changes). Must pass.
3. Clippy auto-fix, then check for remaining warnings. Must have zero warnings.

### Format

```
cargo fmt
```

### Clippy procedure — follow exactly:

**Step 1: Auto-fix (workspace-wide)**
```
cargo clippy --fix 2>&1 | grep -v objfs | grep -E '^(error|warning:|Fixed)' | head -30
```

**Step 2: Check remaining warnings (workspace-wide)**
```
cargo clippy 2>&1 | grep -v objfs | grep -E '^(error|warning:)' | grep -v 'generated.*warnings' | head -30
```

Zero warnings required — including build-script warnings, dead code, unused fields, cfg warnings.
Fix them all. Do NOT filter warnings out with grep patterns.

Do NOT target individual test files with clippy (e.g., `--test foo`).

If tests/clippy/fmt fail, you'll return to IMPLEMENT. Fix and retry.
If all pass, you'll advance to REVIEW.
