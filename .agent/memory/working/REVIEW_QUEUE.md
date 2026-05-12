# Review Queue

**Pending:** 16
**Oldest staged:** 2026-05-10T20:40:55.273010+00:00

Run `python .agent/tools/list_candidates.py` for detail, then:
- `python .agent/tools/graduate.py <id> --rationale "..."` to accept
- `python .agent/tools/reject.py <id> --reason "..."` to reject
- Review in a batch so cross-candidate contradictions are caught.

## Priority order (top 10)

- **d49365f28837** (priority=35813.57, size=1857, rejections=1) — FAILURE in claude-code: Command failed: cat > /tmp/test_lex.rs << 'EOF' | THIS S
- **70c3e012fa1b** (priority=6246.00, size=347, rejections=0) — FAILURE in claude-code: Command failed: jj new -m "docs(iter-0004d.4): scenario 
- **9722f083bd4c** (priority=2682.00, size=149, rejections=0) — FAILURE in claude-code: Command failed: jj new -m "docs(iter-0004d.4): scenario 
- **a6a5972d6a43** (priority=1683.64, size=90, rejections=0) — FAILURE in claude-code: Command failed: rtk cargo test -p fmpl-core 2>&1 | grep 
- **1ad6f2c9cb48** (priority=882.00, size=49, rejections=0) — FAILURE in claude-code: Command failed: rtk cargo test -p fmpl-core --no-fail-fa
- **5243c8cb6b09** (priority=621.00, size=46, rejections=0) — Wrote /Users/ndn/development/fmpl/docs/superpowers/specs/2026-05-12-scenario-run
- **4ea6db7f3696** (priority=602.37, size=46, rejections=0) — High-stakes op completed (migrate): jj describe -m "$(cat <<'EOF'
- **7db33f9fe13b** (priority=472.50, size=35, rejections=0) — Edited /Users/ndn/development/fmpl/docs/superpowers/iterations/requirements/EPIC
- **1ee1e88edac8** (priority=433.93, size=30, rejections=1) — Tool Agent completed successfully
- **65dca9e74421** (priority=159.11, size=11, rejections=0) — High-stakes op completed (schema): python3 .agent/tools/reject.py 50986dd6bff7 -
