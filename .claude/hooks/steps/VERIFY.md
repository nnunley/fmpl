Run verification:

1. ONE `cargo test` run (filtered to your changes). Must pass.
2. Clippy auto-fix, then check for remaining warnings. Must have zero warnings.

### Clippy procedure — follow exactly:

**Step 1: Auto-fix**
```
cargo clippy --fix --allow-dirty -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:.*fmpl|Fixed)' | head -30
```

**Step 2: Check remaining warnings**
```
cargo clippy -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:.*fmpl)' | head -30
```

If Step 2 shows any warnings, fix them manually. Zero warnings required, not just zero errors.

Do NOT target individual test files with clippy (e.g., `--test foo`). Clippy runs on the crate.

If tests/clippy fail, you'll return to IMPLEMENT. Fix and retry.
If both pass, you'll advance to REVIEW.
