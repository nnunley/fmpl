# Review Queue

**Pending:** 35
**Oldest staged:** 2026-05-10T20:40:55.274099+00:00

Run `python .agent/tools/list_candidates.py` for detail, then:
- `python .agent/tools/graduate.py <id> --rationale "..."` to accept
- `python .agent/tools/reject.py <id> --reason "..."` to reject
- Review in a batch so cross-candidate contradictions are caught.

## Priority order (top 10)

- **7faefc7643d4** (priority=104610.34, size=3728, rejections=0) — FAILURE in claude-code: High-stakes op FAILED (schema): rtk grep -n "^\s*[A-Z][a
- **1982365a8b49** (priority=89968.26, size=2829, rejections=0) — FAILURE in claude-code: Tool Agent completed with failure | THIS SKILL HAS FAILE
- **a6a5972d6a43** (priority=2132.61, size=90, rejections=0) — FAILURE in claude-code: Command failed: rtk cargo test -p fmpl-core 2>&1 | grep 
- **1ad6f2c9cb48** (priority=1134.00, size=49, rejections=0) — FAILURE in claude-code: Command failed: rtk cargo test -p fmpl-core --no-fail-fa
- **c355d8689a9a** (priority=867.86, size=60, rejections=0) — Tool Agent completed successfully
- **4ea6db7f3696** (priority=774.48, size=46, rejections=0) — High-stakes op completed (migrate): jj describe -m "$(cat <<'EOF'
- **5243c8cb6b09** (priority=754.07, size=46, rejections=0) — Wrote /Users/ndn/development/fmpl/docs/superpowers/specs/2026-05-12-scenario-run
- **ced6c1348194** (priority=648.00, size=42, rejections=0) — Tool Agent completed successfully
- **cb0a9cda8fa8** (priority=636.43, size=44, rejections=0) — Tool Agent completed successfully
- **7db33f9fe13b** (priority=573.75, size=35, rejections=0) — Edited /Users/ndn/development/fmpl/docs/superpowers/iterations/requirements/EPIC
