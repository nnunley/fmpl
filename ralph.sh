#!/usr/bin/env bash
# ralph.sh — Two-phase headless task loop for FMPL development
#
# Phase 1: Run PROMPT.md through claude, capture full stream-json log
#   Each iteration is split into segments with context compaction between them.
#   Segments are limited by --max-budget-usd; between segments, ralph-compact.py
#   summarizes the conversation so a fresh claude call starts with compact context.
# Phase 2: Run ralph-analyze.py on the log to extract structured summary
#
# Usage:
#   ./ralph.sh                     # Run one iteration (default)
#   ./ralph.sh -n 5                # Run up to 5 iterations
#   ./ralph.sh -n 0                # Run until blocked or Ctrl-C
#   ./ralph.sh -p PROMPT2.md       # Use alternate prompt file
#   ./ralph.sh --analyze LOGFILE   # Skip phase 1, just analyze a log
#   ./ralph.sh --dry-run           # Show config without executing

set -euo pipefail

# --- Configuration ---
MAX_ITERATIONS=1
PROMPT_FILE="PROMPT.md"
LOG_DIR=".ralph-logs"
DRY_RUN=false
ANALYZE_ONLY=""
PURGE=false
CLAUDE_FLAGS="--dangerously-skip-permissions --verbose"
SEGMENT_BUDGET=0.50    # USD per segment (~15-20 turns)
MAX_SEGMENTS=6         # Max segments per iteration

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--max-iterations)
            MAX_ITERATIONS="$2"
            shift 2
            ;;
        -p|--prompt)
            PROMPT_FILE="$2"
            shift 2
            ;;
        --segment-budget)
            SEGMENT_BUDGET="$2"
            shift 2
            ;;
        --max-segments)
            MAX_SEGMENTS="$2"
            shift 2
            ;;
        --analyze)
            ANALYZE_ONLY="$2"
            shift 2
            ;;
        --purge)
            PURGE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            cat <<'HELP'
Usage: ralph.sh [OPTIONS]

Phase 1 (execute):
  -n, --max-iterations N   Max iterations (0=unlimited, default: 1)
  -p, --prompt FILE        Prompt file (default: PROMPT.md)
  --segment-budget USD     Budget per segment (default: 0.50)
  --max-segments N         Max segments per iteration (default: 6)
  --dry-run                Show config without running

Phase 2 (analyze):
  --analyze LOGFILE        Analyze a previous iteration log (skip phase 1)
  --purge                  Remove raw .jsonl logs, keep summaries and session logs
HELP
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANALYZE_SCRIPT="$SCRIPT_DIR/ralph-analyze.py"
COMPACT_SCRIPT="$SCRIPT_DIR/ralph-compact.py"

# --- Analyze-only mode ---
if [[ -n "$ANALYZE_ONLY" ]]; then
    if [[ ! -f "$ANALYZE_ONLY" ]]; then
        echo "Error: $ANALYZE_ONLY not found" >&2
        exit 1
    fi
    if [[ -f "$ANALYZE_SCRIPT" ]]; then
        python3 "$ANALYZE_SCRIPT" "$ANALYZE_ONLY"
    else
        echo "Error: $ANALYZE_SCRIPT not found" >&2
        exit 1
    fi
    exit 0
fi

# --- Purge mode ---
if [[ "$PURGE" == "true" ]]; then
    if [[ ! -d "$LOG_DIR" ]]; then
        echo "Nothing to purge: $LOG_DIR does not exist"
        exit 0
    fi
    # Delete raw stream-json logs but keep summaries (*.summary.txt)
    RAW_COUNT=$(find "$LOG_DIR" \( -name 'iter-*.jsonl' -o -name 'iter-*.txt' \) -not -name '*.summary.txt' | wc -l | tr -d ' ')
    RAW_SIZE=$(find "$LOG_DIR" \( -name 'iter-*.jsonl' -o -name 'iter-*.txt' \) -not -name '*.summary.txt' -exec stat -f%z {} + 2>/dev/null | awk '{s+=$1} END {printf "%.1fMB", s/1048576}')
    if [[ "$RAW_COUNT" -eq 0 ]]; then
        echo "Nothing to purge"
        exit 0
    fi
    echo "Purging $RAW_COUNT raw logs ($RAW_SIZE)"
    echo "Keeping: *.summary.txt, session-*.log, results-*.jsonl"
    find "$LOG_DIR" \( -name 'iter-*.jsonl' -o -name 'iter-*.txt' \) -not -name '*.summary.txt' -delete
    echo "Done"
    exit 0
fi

# --- Validate ---
if [[ ! -f "$PROMPT_FILE" ]]; then
    echo "Error: $PROMPT_FILE not found" >&2
    exit 1
fi

if ! command -v claude &>/dev/null; then
    echo "Error: claude CLI not found in PATH" >&2
    exit 1
fi

# --- Setup ---
mkdir -p "$LOG_DIR"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
SESSION_LOG="$LOG_DIR/session-$TIMESTAMP.log"
RESULTS_LOG="$LOG_DIR/results-$TIMESTAMP.jsonl"

log() {
    local msg="[$(date +%H:%M:%S)] $*"
    echo "$msg"
    echo "$msg" >> "$SESSION_LOG"
}

# --- Dry run ---
if [[ "$DRY_RUN" == "true" ]]; then
    echo "Ralph Loop Configuration:"
    echo "  Prompt:          $PROMPT_FILE ($(wc -l < "$PROMPT_FILE" | tr -d ' ') lines)"
    echo "  Max iterations:  $MAX_ITERATIONS (0=unlimited)"
    echo "  Segment budget:  \$$SEGMENT_BUDGET per segment"
    echo "  Max segments:    $MAX_SEGMENTS per iteration"
    echo "  Log dir:         $LOG_DIR"
    echo "  Claude flags:    $CLAUDE_FLAGS"
    echo "  Analyze script:  $ANALYZE_SCRIPT ($(test -f "$ANALYZE_SCRIPT" && echo 'found' || echo 'MISSING'))"
    echo "  Compact script:  $COMPACT_SCRIPT ($(test -f "$COMPACT_SCRIPT" && echo 'found' || echo 'MISSING'))"
    echo ""
    echo "Would run: echo '...' | claude $CLAUDE_FLAGS -p --append-system-prompt \"\$(cat $PROMPT_FILE)\" --max-budget-usd $SEGMENT_BUDGET --output-format=stream-json"
    exit 0
fi

# --- Helper: run one claude segment ---
# Runs claude with the given user message, writes to the given log file.
# Returns 0 on success, 1 on unrecoverable error.
# Sets SEGMENT_RESULT_LINE, SEGMENT_IS_ERROR, SEGMENT_SUBTYPE, SEGMENT_RAW_LINES
run_segment() {
    local user_msg="$1"
    local seg_log="$2"
    local prompt_content="$3"

    SEGMENT_RESULT_LINE=""
    SEGMENT_IS_ERROR="false"
    SEGMENT_SUBTYPE="unknown"

    echo "$user_msg" | \
        claude $CLAUDE_FLAGS -p \
            --append-system-prompt "$prompt_content" \
            --max-budget-usd "$SEGMENT_BUDGET" \
            --output-format=stream-json 2>/dev/null | \
        tee "$seg_log" | \
        jq -j --unbuffered '
            if .type == "assistant" then
                .message.content[]? |
                if .type == "text" then .text
                elif .type == "tool_use" then "\n[tool: \(.name)]\n"
                else empty
                end
            elif .type == "result" then
                "\n[result: turns=\(.num_turns) cost=$\(.total_cost_usd)]\n"
            else empty
            end
        ' 2>/dev/null || true

    SEGMENT_RAW_LINES=$(wc -l < "$seg_log" | tr -d ' ')

    if [[ "$SEGMENT_RAW_LINES" -eq 0 ]]; then
        return 1
    fi

    # Check for API-level errors
    local api_error
    api_error=$(jq -r 'select(.type == "error") | .error.type // empty' "$seg_log" 2>/dev/null | head -1 || true)
    if [[ -n "$api_error" ]]; then
        local api_error_msg
        api_error_msg=$(jq -r 'select(.type == "error") | .error.message // empty' "$seg_log" 2>/dev/null | head -1 || true)
        log "  API ERROR: $api_error — $api_error_msg"

        if [[ "$api_error" == "rate_limit_error" || "$api_error" == "overloaded_error" ]]; then
            local wait_time=60
            [[ "$api_error" == "overloaded_error" ]] && wait_time=120
            log "  Backing off ${wait_time}s before retry..."
            sleep "$wait_time"
            return 2  # Retryable
        fi
        return 1  # Unrecoverable
    fi

    # Check result event
    SEGMENT_IS_ERROR=$(jq -r 'select(.type == "result") | .is_error // false' "$seg_log" 2>/dev/null | tail -1 || echo "false")
    SEGMENT_SUBTYPE=$(jq -r 'select(.type == "result") | .subtype // "unknown"' "$seg_log" 2>/dev/null | tail -1 || echo "unknown")

    # Extract structured result line (COMPLETED/BLOCKED/CLOSED)
    SEGMENT_RESULT_LINE=$(jq -r '
        if .type == "assistant" then
            .message.content[]? | select(.type == "text") | .text
        else empty end
    ' "$seg_log" 2>/dev/null | grep -E '^(COMPLETED|BLOCKED|CLOSED):' | tail -1 || true)

    return 0
}

# --- Main loop ---
ITERATION=0
COMPLETED=0
BLOCKED=0
CLOSED=0

log "Ralph loop started: prompt=$PROMPT_FILE max=$MAX_ITERATIONS segments=$MAX_SEGMENTS budget=\$$SEGMENT_BUDGET/seg"

INTERRUPTED=false

cleanup() {
    echo ""
    # Clean up state machine
    python3 .claude/hooks/ralph-preflight.py --clear 2>/dev/null || true
    if [[ "$INTERRUPTED" == "true" ]]; then
        log "Interrupted by user (Ctrl-C)"
    fi
    log "=== Ralph Loop Summary ==="
    log "Iterations: $ITERATION  Completed: $COMPLETED  Blocked: $BLOCKED  Closed: $CLOSED"
    log "Session: $SESSION_LOG"
    log "========================="
}
trap cleanup EXIT
trap 'INTERRUPTED=true; exit 130' INT

while true; do
    # Check max iterations (0 = unlimited)
    if [[ "$MAX_ITERATIONS" -gt 0 && "$ITERATION" -ge "$MAX_ITERATIONS" ]]; then
        log "Reached max iterations ($MAX_ITERATIONS)"
        break
    fi

    ITERATION=$((ITERATION + 1))

    log "--- Iteration $ITERATION ---"
    ITER_START=$(date +%s)
    ITER_NUM=$(printf '%03d' "$ITERATION")

    # Pre-flight: run health check, detect uncommitted changes, init state machine.
    PREFLIGHT_MSG=$(python3 .claude/hooks/ralph-preflight.py 2>>"$SESSION_LOG")
    log "Pre-flight: $(python3 -c "import json; s=json.load(open('.claude/.ralph-state.json')); print(f\"state={s['state']} protected={len(s.get('protected_files',[]))} uncommitted={len(s.get('uncommitted_files',[]))}\")" 2>/dev/null || echo "no state")"

    PROMPT_CONTENT=$(cat "$PROMPT_FILE")
    ITER_DONE=false
    ITER_STOP=false
    SEGMENT=0
    PREV_SEG_LOG=""

    # --- Inner segment loop ---
    while [[ "$SEGMENT" -lt "$MAX_SEGMENTS" && "$ITER_DONE" == "false" ]]; do
        SEGMENT=$((SEGMENT + 1))
        SEG_NUM=$(printf '%02d' "$SEGMENT")
        SEG_LOG="$LOG_DIR/iter-$TIMESTAMP-$ITER_NUM-seg${SEG_NUM}.jsonl"

        if [[ "$SEGMENT" -eq 1 ]]; then
            USER_MSG="$PREFLIGHT_MSG"
        else
            # Compact the previous segment into a continuation message
            log "  Compacting segment $((SEGMENT - 1)) -> segment $SEGMENT"
            USER_MSG=$(python3 "$COMPACT_SCRIPT" "$PREV_SEG_LOG" 2>>"$SESSION_LOG")
        fi

        log "  Segment $SEGMENT/$MAX_SEGMENTS (budget: \$$SEGMENT_BUDGET)"

        run_segment "$USER_MSG" "$SEG_LOG" "$PROMPT_CONTENT"
        SEG_RC=$?

        SEG_LINES=$(wc -l < "$SEG_LOG" | tr -d ' ')
        SEG_COST=$(jq -r 'select(.type == "result") | .total_cost_usd // 0' "$SEG_LOG" 2>/dev/null | tail -1 || echo "0")
        SEG_TURNS=$(jq -r 'select(.type == "result") | .num_turns // 0' "$SEG_LOG" 2>/dev/null | tail -1 || echo "0")
        log "  Segment $SEGMENT: $SEG_LINES events, $SEG_TURNS turns, \$$SEG_COST"

        # Handle segment return code
        if [[ "$SEG_RC" -eq 1 ]]; then
            # Unrecoverable error
            log "Stopping: unrecoverable error in segment $SEGMENT"
            ITER_DONE=true
            ITER_STOP=true
            break
        elif [[ "$SEG_RC" -eq 2 ]]; then
            # Retryable — redo this segment
            SEGMENT=$((SEGMENT - 1))
            continue
        fi

        # Check for structured result (COMPLETED/BLOCKED/CLOSED)
        if [[ -n "$SEGMENT_RESULT_LINE" ]]; then
            ITER_DONE=true
        fi

        # Check for execution errors
        if [[ "$SEGMENT_IS_ERROR" == "true" ]]; then
            case "$SEGMENT_SUBTYPE" in
                error_max_budget_usd)
                    # Budget exhausted for this segment — this is a segment boundary.
                    # Compact and continue to the next segment.
                    log "  Segment $SEGMENT hit budget limit — compacting and continuing"
                    ;;
                error_max_turns)
                    log "  Segment $SEGMENT hit max turns — compacting and continuing"
                    ;;
                *)
                    log "Stopping: execution error ($SEGMENT_SUBTYPE) in segment $SEGMENT"
                    ITER_DONE=true
                    ITER_STOP=true
                    ;;
            esac
        fi

        PREV_SEG_LOG="$SEG_LOG"
    done

    if [[ "$SEGMENT" -ge "$MAX_SEGMENTS" && "$ITER_DONE" == "false" ]]; then
        log "  Hit max segments ($MAX_SEGMENTS) without completion"
    fi

    ITER_END=$(date +%s)
    ITER_DURATION=$((ITER_END - ITER_START))
    log "Iteration $ITERATION: $SEGMENT segments, ${ITER_DURATION}s total"

    # Phase 2: Extract structured summary from the last segment log
    SUMMARY="$LOG_DIR/iter-$TIMESTAMP-$ITER_NUM.summary.txt"
    LAST_SEG_LOG="$LOG_DIR/iter-$TIMESTAMP-$ITER_NUM-seg$(printf '%02d' "$SEGMENT").jsonl"
    if [[ -f "$ANALYZE_SCRIPT" && -f "$LAST_SEG_LOG" ]]; then
        python3 "$ANALYZE_SCRIPT" "$LAST_SEG_LOG" > "$SUMMARY" 2>/dev/null || true
    fi

    # Record result
    if [[ -n "$SEGMENT_RESULT_LINE" ]]; then
        TYPE=$(echo "$SEGMENT_RESULT_LINE" | cut -d: -f1)
        ISSUE_ID=$(echo "$SEGMENT_RESULT_LINE" | cut -d: -f2 | xargs)
        MESSAGE=$(echo "$SEGMENT_RESULT_LINE" | cut -d: -f3- | xargs)

        case "$TYPE" in
            COMPLETED)
                COMPLETED=$((COMPLETED + 1))
                log "COMPLETED [$ISSUE_ID] $MESSAGE (${ITER_DURATION}s, $SEGMENT segs)"
                ;;
            BLOCKED)
                BLOCKED=$((BLOCKED + 1))
                log "BLOCKED [$ISSUE_ID] $MESSAGE (${ITER_DURATION}s, $SEGMENT segs)"
                log "Stopping: task is blocked"
                ITER_STOP=true
                ;;
            CLOSED)
                CLOSED=$((CLOSED + 1))
                log "CLOSED [$ISSUE_ID] $MESSAGE (${ITER_DURATION}s, $SEGMENT segs)"
                ;;
        esac

        echo "{\"iteration\":$ITERATION,\"type\":\"$TYPE\",\"issue\":\"$ISSUE_ID\",\"message\":\"$MESSAGE\",\"duration\":$ITER_DURATION,\"segments\":$SEGMENT,\"log\":\"$LAST_SEG_LOG\"}" >> "$RESULTS_LOG"
    else
        log "NO STRUCTURED OUTPUT (${ITER_DURATION}s, $SEGMENT segs) — check segment logs"
        echo "{\"iteration\":$ITERATION,\"type\":\"unstructured\",\"issue\":\"\",\"message\":\"no result line\",\"duration\":$ITER_DURATION,\"segments\":$SEGMENT,\"log\":\"$LAST_SEG_LOG\"}" >> "$RESULTS_LOG"

        if [[ "$SEGMENT" -eq 1 && "$SEGMENT_RAW_LINES" -eq 0 ]]; then
            log "Stopping: claude produced no output"
            break
        fi
    fi

    if [[ "$ITER_STOP" == "true" ]]; then
        break
    fi
done
