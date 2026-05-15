#!/usr/bin/env bash
# test-bg.sh — fire-and-forget background runner for test invocations.
#
# Wraps any command, runs it detached, writes structured logs, and
# posts a desktop notification on completion. Composes with any
# existing make target: `bash scripts/test-bg.sh bazel test //...-rust`.
#
# Why this exists: the cost function for test runs is wall-clock
# time spent *waiting*, not just time spent running. This script
# removes waiting from the critical path — the developer keeps
# working, gets pinged when the run finishes.
#
# Log layout (per run):
#   ~/.batchalign3/bg-test/<slug>/<ts>.log     — full stdout+stderr
#   ~/.batchalign3/bg-test/<slug>/<ts>.status  — exit code (written on completion)
#   ~/.batchalign3/bg-test/<slug>/<ts>.meta    — cmd, pid, start/end timestamps
#
# The .status file being present is the unambiguous "done" signal.
# Log tails carry a sentinel line `=== TEST-BG COMPLETED: exit=N duration=Ns ===`
# so a watcher (Monitor tool, tail -f, etc.) can detect completion
# without polling the filesystem.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: bash scripts/test-bg.sh [--slug NAME] [--quiet] -- CMD [ARGS...]
       bash scripts/test-bg.sh CMD [ARGS...]

Options:
  --slug NAME   Label this run (default: derived from the command).
  --quiet       Suppress the desktop notification on success.
                Failures always notify.
  --help        This message.

Examples:
  bash scripts/test-bg.sh bazel test //...-rust
  bash scripts/test-bg.sh --slug golden-fra -- uv run pytest -m 'golden and mwt_probe' -k fra
EOF
}

SLUG=""
QUIET_ON_SUCCESS=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --slug) SLUG="$2"; shift 2 ;;
        --quiet) QUIET_ON_SUCCESS=1; shift ;;
        --help|-h) usage; exit 0 ;;
        --) shift; break ;;
        -*) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
        *) break ;;
    esac
done

if [[ $# -eq 0 ]]; then
    echo "error: no command supplied" >&2
    usage >&2
    exit 2
fi

if [[ -z "$SLUG" ]]; then
    # Derive a slug from the first two tokens of the command, sanitized.
    SLUG="$(printf '%s-%s' "${1:-cmd}" "${2:-}" | tr -c 'A-Za-z0-9._-' '-' | sed 's/-\+/-/g; s/^-//; s/-$//')"
    SLUG="${SLUG:-cmd}"
fi

TS="$(date +%s)"
RUN_DIR="${HOME}/.batchalign3/bg-test/${SLUG}"
mkdir -p "$RUN_DIR"

LOG="${RUN_DIR}/${TS}.log"
STATUS="${RUN_DIR}/${TS}.status"
META="${RUN_DIR}/${TS}.meta"

# Record metadata before spawning so the caller can inspect state
# even if the child hasn't started writing yet.
{
    printf 'slug=%s\n' "$SLUG"
    printf 'ts_start=%s\n' "$TS"
    printf 'cmd=%q ' "$@"
    printf '\n'
    printf 'cwd=%s\n' "$(pwd)"
    printf 'host=%s\n' "$(hostname -s)"
} > "$META"

# notify() — macOS desktop notification via osascript.
# On other OSes this is a no-op (caller still gets the log).
notify() {
    local title="$1" body="$2" sound="${3:-}"
    if ! command -v osascript >/dev/null 2>&1; then
        return 0
    fi
    # Escape double quotes for osascript's AppleScript literal.
    local t="${title//\"/\\\"}" b="${body//\"/\\\"}"
    local script
    if [[ -n "$sound" ]]; then
        script="display notification \"${b}\" with title \"${t}\" sound name \"${sound}\""
    else
        script="display notification \"${b}\" with title \"${t}\""
    fi
    osascript -e "$script" >/dev/null 2>&1 || true
}

# Spawn the child in a detached subshell so the caller returns
# immediately. The subshell records start/end, runs the command,
# writes .status + sentinel on exit, and notifies.
#
# Per-test history (Phase B2): if BATCHALIGN_TEST_HISTORY_DB isn't
# already set in the caller's env, default it to a shared DB under
# the user's cache dir. The pytest conftest writes there iff the env
# var is set. Setting BATCHALIGN_TEST_HISTORY_OFF=1 disables this.
if [[ -z "${BATCHALIGN_TEST_HISTORY_DB:-}" && -z "${BATCHALIGN_TEST_HISTORY_OFF:-}" ]]; then
    export BATCHALIGN_TEST_HISTORY_DB="${HOME}/.batchalign3/test-history.sqlite"
fi

(
    trap '' HUP
    START_EPOCH="$(date +%s)"
    printf '=== TEST-BG STARTED: slug=%s ts=%s cmd=%s ===\n' "$SLUG" "$TS" "$*" > "$LOG"

    # Run the command, capturing combined stdout/stderr.
    set +e
    "$@" >> "$LOG" 2>&1
    RC=$?
    set -e

    END_EPOCH="$(date +%s)"
    DURATION=$(( END_EPOCH - START_EPOCH ))

    printf '%s\n' "$RC" > "$STATUS"
    {
        printf 'ts_end=%s\n' "$END_EPOCH"
        printf 'duration_s=%s\n' "$DURATION"
        printf 'exit=%s\n' "$RC"
    } >> "$META"

    printf '=== TEST-BG COMPLETED: exit=%s duration=%ss ===\n' "$RC" "$DURATION" >> "$LOG"

    if [[ "$RC" -eq 0 ]]; then
        if [[ "$QUIET_ON_SUCCESS" -eq 0 ]]; then
            notify "test-bg OK: ${SLUG}" "Finished in ${DURATION}s" "Glass"
        fi
    else
        notify "test-bg FAIL: ${SLUG}" "Exit ${RC} after ${DURATION}s — ${LOG}" "Basso"
    fi
) </dev/null >/dev/null 2>&1 &

BG_PID=$!
printf 'pid=%s\n' "$BG_PID" >> "$META"

disown "$BG_PID" 2>/dev/null || true

# Immediately echo the essentials so the caller knows where to watch.
cat <<EOF
test-bg started
  slug:   $SLUG
  pid:    $BG_PID
  log:    $LOG
  status: $STATUS (written on completion)
  tail:   tail -f $LOG
EOF
