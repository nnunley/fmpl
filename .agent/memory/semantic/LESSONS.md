# Lessons

> _Auto-managed below. Hand-curated preamble + seed lessons above the sentinel are preserved across renders._

## Auto-promoted entries will be appended below

### 2026-05

- When planning a refactor that deletes producer code paths AND also wants to rename surviving consumer code (e.g., bytecode opcodes), enumerate the rename targets AFTER a cargo-check view of what survives the deletion phase, not before. PAR scope review catches citation drift but cannot enumerate dependent sites the planner didn't grep for. The pattern: build a tooling precursor or skeleton-deletion first, let the compiler surface the real consumer set, then plan the rename against that ground truth.  <!-- status=accepted confidence=0.6 evidence=1 id=lesson_f2576de9c008 -->

### 2026-04

- Always serialize timestamps in UTC to avoid cross-region comparison bugs  <!-- status=accepted confidence=0.46 evidence=1 id=lesson_422695ae5b2d -->
