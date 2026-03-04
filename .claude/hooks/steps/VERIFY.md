Run verification:

1. ONE `cargo test` run (filtered to your changes). Must pass.
2. ONE `cargo clippy` run on the **crate** (not individual test files). Must pass with zero errors.

Clippy command — use exactly this, do not vary:
```
cargo clippy -p <crate> 2>&1 | grep -v objfs | grep -E '^(error|warning:)' | head -30
```

Do NOT target individual test files with clippy (e.g., `--test foo`). Clippy runs on the crate.

If tests/clippy fail, you'll return to IMPLEMENT. Fix and retry.
If both pass, you'll advance to REVIEW.
