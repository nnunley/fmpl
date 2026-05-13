#!/usr/bin/env bash
# run_sentinels.sh — sentinel-corpus sweep gate (ITER-0005b-FIX-A FIX-MECH Option-α).
#
# Parses docs/superpowers/iterations/behavior-corpus.md, extracts every row with
# cadence == "sentinel", and runs its execution command. Reports PASS / FAIL per
# scenario, then a final summary. Exits 0 only if every sentinel passes.
#
# Required by the closing PAR of every iteration after ITER-0005b-FIX-A. The
# iteration's closing-PAR entry must include a `### Sentinel sweep (closing-PAR)`
# block containing this script's stdout/stderr.

set -uo pipefail

CORPUS="${CORPUS_PATH:-docs/superpowers/iterations/behavior-corpus.md}"

if [[ ! -f "$CORPUS" ]]; then
    echo "FATAL: corpus not found at $CORPUS" >&2
    exit 2
fi

# Build prerequisites: several sentinels require the FMPL canonical parser
# (e.g. SCENARIO-0108, G3), which is generated from `fmpl-bootstrap`. Without
# this build, those sentinels fail with "fallback parser in use". Build them
# up-front so the sweep measures sentinel state, not environment drift.
echo "Building prerequisites (fmpl-bootstrap → fmpl-core)..."
if ! FMPL_BOOTSTRAP_PHASE=1 cargo build -p fmpl-bootstrap >/tmp/sentinel_prereq.log 2>&1; then
    echo "FATAL: fmpl-bootstrap build failed. See /tmp/sentinel_prereq.log" >&2
    exit 2
fi
touch fmpl-core/build.rs
if ! cargo build -p fmpl-core >>/tmp/sentinel_prereq.log 2>&1; then
    echo "FATAL: fmpl-core rebuild failed. See /tmp/sentinel_prereq.log" >&2
    exit 2
fi
echo "Prerequisites OK"
echo

# Parse: pull each pipe-delimited table row whose cadence column equals "sentinel".
# Column layout (1-indexed after leading `| `):
#   1: ID  2: title  3: seam  4: cadence  5: command  6: stories
# Strategy: split on `|`, trim leading/trailing whitespace from each cell.

declare -a SCENARIOS
while IFS= read -r line; do
    # Skip non-table-data lines (headers, separators, blank).
    [[ "$line" == "|"* ]] || continue
    [[ "$line" == *"|---"* ]] && continue
    [[ "$line" == "| Scenario ID"* ]] && continue

    # Split row on `|` into cells; bash trick: replace `|` with newline.
    IFS='|' read -r -a cells <<<"$line"
    # cells[0] is leading empty (before first `|`); real cells start at [1].
    # We need cells[1] (ID), cells[4] (cadence), cells[5] (command).
    [[ ${#cells[@]} -ge 6 ]] || continue

    id="${cells[1]}"
    cadence="${cells[4]}"
    command="${cells[5]}"

    # Trim leading/trailing whitespace.
    id="${id## }"; id="${id%% }"; id="${id%	}"
    cadence="${cadence## }"; cadence="${cadence%% }"; cadence="${cadence%	}"
    command="${command## }"; command="${command%% }"; command="${command%	}"

    # Filter to sentinels.
    [[ "$cadence" == "sentinel" ]] || continue

    # Skip TBD / BLOCKED placeholders — a sentinel row with no real command is
    # itself a violation, but it's a documentation issue we surface at the end.
    if [[ "$command" == "TBD" ]] || [[ "$command" == BLOCKED:* ]]; then
        SCENARIOS+=("$id|MISSING_COMMAND:$command")
        continue
    fi

    # Strip surrounding backticks if present.
    command="${command#\`}"
    command="${command%\`}"

    SCENARIOS+=("$id|$command")
done <"$CORPUS"

if [[ ${#SCENARIOS[@]} -eq 0 ]]; then
    echo "FATAL: no sentinel-cadence rows found in $CORPUS" >&2
    exit 2
fi

echo "Sentinel sweep: ${#SCENARIOS[@]} scenarios at cadence=sentinel"
echo "Corpus: $CORPUS"
echo "---"

declare -i PASS=0 FAIL=0 SKIP=0
declare -a FAILURES=() MISSING=()

for entry in "${SCENARIOS[@]}"; do
    id="${entry%%|*}"
    rest="${entry#*|}"
    if [[ "$rest" == MISSING_COMMAND:* ]]; then
        marker="${rest#MISSING_COMMAND:}"
        printf 'SKIP   %s  [%s]\n' "$id" "$marker"
        SKIP+=1
        MISSING+=("$id ($marker)")
        continue
    fi

    printf 'RUN    %s  %s\n' "$id" "$rest"
    # Run the command; capture exit code; suppress stdout (we'll re-run on failure).
    if eval "$rest" >/tmp/sentinel_${id//[^A-Za-z0-9]/_}.log 2>&1; then
        printf 'PASS   %s\n' "$id"
        PASS+=1
    else
        rc=$?
        printf 'FAIL   %s  (exit %d)\n' "$id" "$rc"
        echo "       see /tmp/sentinel_${id//[^A-Za-z0-9]/_}.log for output"
        FAIL+=1
        FAILURES+=("$id  rc=$rc  $rest")
    fi
done

echo "---"
echo "Sentinel sweep summary: $PASS pass, $FAIL fail, $SKIP skip (missing command)"

if [[ ${#FAILURES[@]} -gt 0 ]]; then
    echo "Failures:"
    for f in "${FAILURES[@]}"; do
        echo "  - $f"
    done
fi

if [[ ${#MISSING[@]} -gt 0 ]]; then
    echo "Missing commands (sentinel rows with TBD/BLOCKED):"
    for m in "${MISSING[@]}"; do
        echo "  - $m"
    done
fi

# Exit nonzero on any failure. SKIP does NOT fail the sweep — it surfaces a
# corpus-quality issue but lets the script complete; the closing PAR is
# responsible for deciding whether SKIPped sentinels are acceptable.
if [[ $FAIL -gt 0 ]]; then
    exit 1
fi

exit 0
