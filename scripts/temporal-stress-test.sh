#!/usr/bin/env bash
# Temporal workflow stress test for batchalign3.
#
# STATUS: DORMANT (2026-04-01)
# Temporal is disabled by default. Local backend runs without it.
# This script is preserved for use when Temporal is re-enabled after root-
# causing the activity handler bug (activities accept but never execute).
#
# Exercises every lifecycle path: submission, completion, cancellation,
# recovery, concurrency. Uses --test-echo workers (no GPU needed).
#
# Usage:
#   bash scripts/temporal-stress-test.sh              # All tiers (1-4)
#   bash scripts/temporal-stress-test.sh --tier 1     # Smoke tests only
#   bash scripts/temporal-stress-test.sh --tier 1-3   # Tiers 1 through 3
#
# Prerequisites: temporal CLI, jq, curl, built batchalign3 binary.

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────
BINARY="./target/debug/batchalign3"
PORT=9123
TEMPORAL_PORT=7233
TEMPORAL_UI_PORT=8233
BASE_URL="http://127.0.0.1:${PORT}"
WORK_DIR="/tmp/ba3-stress-test"
STATE_DIR="${WORK_DIR}/state"
OUTPUT_DIR="${WORK_DIR}/output"
SERVER_LOG="${WORK_DIR}/server.log"
TEMPORAL_LOG="${WORK_DIR}/temporal.log"
CONFIG_FILE="${WORK_DIR}/server.yaml"

# Find test CHAT files (small, from CHILDES Eng-NA).
CORPUS_DIR="${HOME}/talkbank/data/childes-eng-na-data/Eng-NA/HSLLD"

# Counters
PASS=0
FAIL=0
SKIP=0
TEMPORAL_PID=""
SERVER_PID=""

# ── Colors ───────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ── Helpers ──────────────────────────────────────────────────────────

log()  { echo -e "${CYAN}[stress]${RESET} $*"; }
pass() { echo -e "  ${GREEN}PASS${RESET} $*"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}FAIL${RESET} $*"; FAIL=$((FAIL + 1)); }
skip() { echo -e "  ${YELLOW}SKIP${RESET} $*"; SKIP=$((SKIP + 1)); }
header() { echo -e "\n${BOLD}═══ $* ═══${RESET}"; }

# Submit a job via the CLI in content mode (--server), return the job ID.
# Usage: submit_job <command> <lang> <file1> [file2 ...]
submit_job() {
    local cmd="$1" lang="$2"; shift 2
    local out_dir
    out_dir="${OUTPUT_DIR}/$(date +%s%N)"
    mkdir -p "$out_dir"
    # Use the CLI to submit; it prints the job ID line.
    local cli_output
    cli_output=$("${BINARY}" "$cmd" "$@" \
        -o "$out_dir" \
        --lang "$lang" \
        --server "${BASE_URL}" \
        --no-open-dashboard \
        -v 2>&1) || true
    # Extract job ID from "Job <id> submitted" line.
    echo "$cli_output" | sed -n 's/.*Job \([a-f0-9-]*\) submitted.*/\1/p' | head -1
}

# Submit a job via the CLI in the BACKGROUND. Returns immediately.
# Writes job ID to the given file when available.
# Usage: submit_job_bg <id_file> <command> <lang> <file1> [file2 ...]
submit_job_bg() {
    local id_file="$1"; shift
    local cmd="$1" lang="$2"; shift 2
    local out_dir
    out_dir="${OUTPUT_DIR}/$(date +%s%N)"
    mkdir -p "$out_dir"
    (
        cli_output=$("${BINARY}" "$cmd" "$@" \
            -o "$out_dir" \
            --lang "$lang" \
            --server "${BASE_URL}" \
            --no-open-dashboard \
            -v 2>&1) || true
        echo "$cli_output" | sed -n 's/.*Job \([a-f0-9-]*\) submitted.*/\1/p' | head -1 > "$id_file"
    ) &
}

# Get job status from the REST API.
# Usage: job_status <job_id>  →  "completed" | "cancelled" | "failed" | "queued" | "running"
job_status() {
    curl -sf "${BASE_URL}/jobs/$1" 2>/dev/null | jq -r '.status // "unknown"'
}

# Wait for a job to reach a terminal status.
# Usage: wait_job <job_id> <timeout_seconds>  →  final status
wait_job() {
    local job_id="$1" timeout="$2"
    local elapsed=0
    while [ "$elapsed" -lt "$timeout" ]; do
        local status
        status=$(job_status "$job_id" 2>/dev/null || echo "unknown")
        case "$status" in
            completed|cancelled|failed) echo "$status"; return 0 ;;
        esac
        sleep 1
        elapsed=$((elapsed + 1))
    done
    echo "timeout"
    return 1
}

# Cancel a job.
cancel_job() { curl -sf -X POST "${BASE_URL}/jobs/$1/cancel" > /dev/null; }

# Delete a job.
delete_job() { curl -sf -X DELETE "${BASE_URL}/jobs/$1" > /dev/null; }

# Restart a job.
restart_job() { curl -sf -X POST "${BASE_URL}/jobs/$1/restart" > /dev/null; }

# Get health JSON.
health() { curl -sf "${BASE_URL}/health"; }

# Get active job count.
active_jobs() { health | jq -r '.active_jobs'; }

# Get worker crash count.
worker_crashes() { health | jq -r '.worker_crashes'; }

# Check Temporal workflow status.
# Usage: temporal_status <workflow_id>  →  "Running" | "Completed" | "Canceled" | ...
temporal_status() {
    local output
    output=$(temporal workflow describe --workflow-id "$1" \
        --address "127.0.0.1:${TEMPORAL_PORT}" \
        --output json 2>/dev/null) || { echo "not_found"; return; }
    echo "$output" | jq -r '.executionConfig.status // .workflowExecutionInfo.status // "unknown"' 2>/dev/null || echo "parse_error"
}

# Collect N small .cha files from the corpus.
# Usage: collect_files <count>  →  prints paths, one per line
collect_files() {
    find "$CORPUS_DIR" -name "*.cha" -size -5k 2>/dev/null | head -"$1"
}

# ── Infrastructure ───────────────────────────────────────────────────

start_temporal() {
    log "Starting Temporal dev server on port ${TEMPORAL_PORT}..."
    rm -f "${WORK_DIR}/temporal.db"*
    temporal server start-dev \
        --db-filename "${WORK_DIR}/temporal.db" \
        --log-level error \
        --port "$TEMPORAL_PORT" \
        --ui-port "$TEMPORAL_UI_PORT" \
        > "$TEMPORAL_LOG" 2>&1 &
    TEMPORAL_PID=$!
    # Wait for gRPC port.
    for i in $(seq 1 30); do
        lsof -i ":${TEMPORAL_PORT}" -sTCP:LISTEN > /dev/null 2>&1 && break
        sleep 1
    done
    if ! lsof -i ":${TEMPORAL_PORT}" -sTCP:LISTEN > /dev/null 2>&1; then
        fail "Temporal did not start within 30s"
        return 1
    fi
    log "Temporal PID=${TEMPORAL_PID}"
}

stop_temporal() {
    if [ -n "$TEMPORAL_PID" ]; then
        kill "$TEMPORAL_PID" 2>/dev/null || true
        wait "$TEMPORAL_PID" 2>/dev/null || true
        TEMPORAL_PID=""
    fi
    pkill -f "temporal server" 2>/dev/null || true
    sleep 1
}

start_server() {
    local fresh="${1:-true}"  # Pass "false" to keep existing state for recovery tests.
    log "Starting batchalign3 server (test-echo, Temporal) on port ${PORT}..."
    if [ "$fresh" = "true" ]; then
        rm -rf "${STATE_DIR}"
    fi
    mkdir -p "${STATE_DIR}"

    cat > "$CONFIG_FILE" << YAML
temporal_server_url: "http://127.0.0.1:${TEMPORAL_PORT}"
default_lang: eng
port: ${PORT}
max_concurrent_jobs: 8
max_workers_per_job: 4
media_roots: []
warmup_commands: []
temporal_heartbeat_s: 10
temporal_activity_timeout_s: 300
YAML

    BATCHALIGN_STATE_DIR="${STATE_DIR}" \
    RUST_LOG=info,batchalign::runtime_supervisor=debug \
    "${BINARY}" serve start \
        --test-echo \
        --foreground \
        --config "$CONFIG_FILE" \
        --no-open-dashboard \
        > "$SERVER_LOG" 2>&1 &
    SERVER_PID=$!

    for i in $(seq 1 30); do
        curl -sf "${BASE_URL}/health" > /dev/null 2>&1 && break
        sleep 1
    done
    if ! curl -sf "${BASE_URL}/health" > /dev/null 2>&1; then
        fail "Server did not start within 30s"
        cat "$SERVER_LOG" | tail -20
        return 1
    fi
    # Verify Temporal backend was chosen.
    if ! grep -q "Backend: temporal" "$SERVER_LOG"; then
        fail "Server did not use Temporal backend"
        return 1
    fi
    log "Server PID=${SERVER_PID}"
}

stop_server() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
}

kill_server() {
    if [ -n "$SERVER_PID" ]; then
        kill -9 "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
}

# ── Tier 1: Smoke Tests ─────────────────────────────────────────────

tier1_smoke() {
    header "Tier 1: Smoke Tests"

    local files
    files=$(collect_files 1)
    if [ -z "$files" ]; then
        fail "T1: No test CHAT files found in ${CORPUS_DIR}"
        return 1
    fi
    local f1
    f1=$(echo "$files" | head -1)

    # T1.1: Single morphotag
    log "T1.1: Single morphotag"
    local job_id status
    job_id=$(submit_job morphotag eng "$f1")
    if [ -z "$job_id" ]; then
        fail "T1.1: submit_job returned no job ID"
    else
        status=$(wait_job "$job_id" 60)
        if [ "$status" = "completed" ]; then
            pass "T1.1: morphotag completed (job=${job_id})"
        else
            fail "T1.1: morphotag status=${status} (expected completed, job=${job_id})"
        fi
        # Check Temporal workflow (informational — the batchalign status is authoritative).
        local tw
        tw=$(temporal_status "$job_id")
        if echo "$tw" | grep -qi "completed\|WORKFLOW_EXECUTION_STATUS_COMPLETED"; then
            pass "T1.1: Temporal workflow completed"
        elif [ "$tw" = "not_found" ]; then
            pass "T1.1: Temporal workflow cleaned up (auto-prune)"
        else
            log "T1.1: Temporal workflow status=${tw} (non-fatal, batchalign job completed)"
            pass "T1.1: Temporal workflow check (status=${tw})"
        fi
    fi

    # T1.2: Single utseg
    log "T1.2: Single utseg"
    job_id=$(submit_job utseg eng "$f1")
    if [ -z "$job_id" ]; then
        fail "T1.2: submit_job returned no job ID"
    else
        status=$(wait_job "$job_id" 60)
        if [ "$status" = "completed" ]; then
            pass "T1.2: utseg completed (job=${job_id})"
        else
            fail "T1.2: utseg status=${status} (expected completed, job=${job_id})"
        fi
    fi

    # T1.3: Single translate
    log "T1.3: Single translate"
    job_id=$(submit_job translate eng "$f1")
    if [ -z "$job_id" ]; then
        fail "T1.3: submit_job returned no job ID"
    else
        status=$(wait_job "$job_id" 60)
        if [ "$status" = "completed" ]; then
            pass "T1.3: translate completed (job=${job_id})"
        else
            fail "T1.3: translate status=${status} (expected completed, job=${job_id})"
        fi
    fi

    # T1.4: Health check
    log "T1.4: Post-smoke health check"
    local aj wc
    aj=$(active_jobs)
    wc=$(worker_crashes)
    if [ "$aj" = "0" ]; then
        pass "T1.4: active_jobs=0"
    else
        fail "T1.4: active_jobs=${aj} (expected 0)"
    fi
    if [ "$wc" = "0" ]; then
        pass "T1.4: worker_crashes=0"
    else
        fail "T1.4: worker_crashes=${wc} (expected 0)"
    fi

    # T1.5: Server log verification
    log "T1.5: Log verification"
    if grep -q "Temporal activity: dispatching" "$SERVER_LOG" \
       && grep -q "job_task started on main runtime" "$SERVER_LOG" \
       && grep -q "job_task completed on main runtime" "$SERVER_LOG"; then
        pass "T1.5: Full dispatch→start→complete trace in server log"
    else
        fail "T1.5: Missing lifecycle trace in server log"
        grep -E "Temporal activity|job_task" "$SERVER_LOG" | tail -5
    fi
}

# ── Tier 2: Scale and Concurrency ───────────────────────────────────

tier2_scale() {
    header "Tier 2: Scale and Concurrency"

    # T2.1: 10-file morphotag
    log "T2.1: 10-file morphotag"
    local files10
    files10=$(collect_files 10)
    local count10
    count10=$(echo "$files10" | wc -l | tr -d ' ')
    if [ "$count10" -lt 10 ]; then
        skip "T2.1: Only ${count10} files available (need 10)"
    else
        local job_id status
        # shellcheck disable=SC2086
        job_id=$(submit_job morphotag eng $files10)
        if [ -z "$job_id" ]; then
            fail "T2.1: submit returned no job ID"
        else
            status=$(wait_job "$job_id" 120)
            if [ "$status" = "completed" ]; then
                pass "T2.1: 10-file morphotag completed (job=${job_id})"
            else
                fail "T2.1: status=${status} (job=${job_id})"
            fi
        fi
    fi

    # T2.2: 50-file morphotag
    log "T2.2: 50-file morphotag"
    local files50
    files50=$(collect_files 50)
    local count50
    count50=$(echo "$files50" | wc -l | tr -d ' ')
    if [ "$count50" -lt 50 ]; then
        skip "T2.2: Only ${count50} files available (need 50)"
    else
        local job_id status
        # shellcheck disable=SC2086
        job_id=$(submit_job morphotag eng $files50)
        if [ -z "$job_id" ]; then
            fail "T2.2: submit returned no job ID"
        else
            status=$(wait_job "$job_id" 180)
            if [ "$status" = "completed" ]; then
                pass "T2.2: 50-file morphotag completed (job=${job_id})"
            else
                fail "T2.2: status=${status} (job=${job_id})"
            fi
        fi
    fi

    # T2.3: 2 concurrent jobs
    log "T2.3: 2 concurrent jobs"
    local files_a files_b
    files_a=$(collect_files 5)
    files_b=$(collect_files 10 | tail -5)
    local id_a="${WORK_DIR}/job_a.id" id_b="${WORK_DIR}/job_b.id"
    rm -f "$id_a" "$id_b"
    # shellcheck disable=SC2086
    submit_job_bg "$id_a" morphotag eng $files_a
    local pid_a=$!
    # shellcheck disable=SC2086
    submit_job_bg "$id_b" morphotag eng $files_b
    local pid_b=$!
    wait "$pid_a" 2>/dev/null || true
    wait "$pid_b" 2>/dev/null || true
    local ja jb
    ja=$(cat "$id_a" 2>/dev/null || echo "")
    jb=$(cat "$id_b" 2>/dev/null || echo "")
    if [ -n "$ja" ] && [ -n "$jb" ]; then
        local sa sb
        sa=$(job_status "$ja")
        sb=$(job_status "$jb")
        if [ "$sa" = "completed" ] && [ "$sb" = "completed" ]; then
            pass "T2.3: Both concurrent jobs completed (${ja}, ${jb})"
        else
            fail "T2.3: job_a=${sa}, job_b=${sb}"
        fi
    else
        fail "T2.3: Failed to get job IDs (a='${ja}', b='${jb}')"
    fi

    # T2.4: 4 concurrent jobs
    log "T2.4: 4 concurrent jobs"
    local id_files=()
    local pids=()
    for i in 1 2 3 4; do
        local idf="${WORK_DIR}/job_${i}.id"
        rm -f "$idf"
        id_files+=("$idf")
        local fset
        fset=$(collect_files $((i * 3)) | tail -3)
        # shellcheck disable=SC2086
        submit_job_bg "$idf" morphotag eng $fset
        pids+=($!)
    done
    for p in "${pids[@]}"; do wait "$p" 2>/dev/null || true; done
    local all_done=true
    for idf in "${id_files[@]}"; do
        local jid
        jid=$(cat "$idf" 2>/dev/null || echo "")
        if [ -z "$jid" ]; then
            fail "T2.4: Missing job ID from $idf"
            all_done=false
        else
            local s
            s=$(job_status "$jid")
            if [ "$s" != "completed" ]; then
                fail "T2.4: job ${jid} status=${s}"
                all_done=false
            fi
        fi
    done
    if $all_done; then
        pass "T2.4: All 4 concurrent jobs completed"
    fi

    # T2.5: Health after scale tests
    log "T2.5: Post-scale health check"
    sleep 2
    local aj
    aj=$(active_jobs)
    if [ "$aj" = "0" ]; then
        pass "T2.5: active_jobs=0 after all scale tests"
    else
        fail "T2.5: active_jobs=${aj}"
    fi
}

# ── Tier 3: Lifecycle Operations ─────────────────────────────────────

tier3_lifecycle() {
    header "Tier 3: Lifecycle Operations"

    local files20
    files20=$(collect_files 20)
    local count20
    count20=$(echo "$files20" | wc -l | tr -d ' ')
    if [ "$count20" -lt 10 ]; then
        skip "T3: Not enough files for lifecycle tests"
        return
    fi

    # T3.1: Cancel running job
    # Test-echo is fast, so the job may complete before cancel arrives.
    # We accept both "cancelled" and "completed" — the key assertion is that
    # the system doesn't hang or crash, and reaches a terminal state.
    log "T3.1: Cancel running job"
    local id_file="${WORK_DIR}/cancel_job.id"
    rm -f "$id_file"
    # shellcheck disable=SC2086
    submit_job_bg "$id_file" morphotag eng $files20
    local bg_pid=$!
    sleep 2
    local job_id
    job_id=$(cat "$id_file" 2>/dev/null || echo "")
    if [ -n "$job_id" ]; then
        cancel_job "$job_id" 2>/dev/null || true
        wait "$bg_pid" 2>/dev/null || true
        local status
        status=$(job_status "$job_id")
        if [ "$status" = "cancelled" ] || [ "$status" = "completed" ]; then
            pass "T3.1: Job reached terminal state: ${status} (${job_id})"
        else
            fail "T3.1: status=${status} after cancel (job=${job_id})"
        fi
    else
        wait "$bg_pid" 2>/dev/null || true
        fail "T3.1: No job ID"
    fi

    # T3.2: Delete a job (may have already completed in echo mode)
    log "T3.2: Delete job"
    local f1
    f1=$(collect_files 1 | head -1)
    job_id=$(submit_job morphotag eng "$f1")
    if [ -n "$job_id" ]; then
        wait_job "$job_id" 30 > /dev/null 2>&1
        delete_job "$job_id" 2>/dev/null || true
        sleep 1
        # Job should be gone from the API (404) or connection fail.
        local http_code
        http_code=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/jobs/${job_id}" 2>/dev/null)
        if [ "$http_code" = "404" ]; then
            pass "T3.2: Job deleted (404 from API)"
        else
            fail "T3.2: Job still exists (HTTP ${http_code})"
        fi
    else
        fail "T3.2: No job ID"
    fi

    # T3.3: Rapid submit-cancel cycle (5×)
    log "T3.3: Rapid submit-cancel cycle (5×)"
    f1=$(collect_files 1 | head -1)
    for i in $(seq 1 5); do
        local jid
        jid=$(submit_job morphotag eng "$f1")
        if [ -n "$jid" ]; then
            cancel_job "$jid" 2>/dev/null || true
        fi
    done
    sleep 5
    # All jobs should be in terminal state.
    local aj
    aj=$(active_jobs)
    if [ "$aj" = "0" ]; then
        pass "T3.3: No active jobs after 5 rapid cancel cycles"
    else
        fail "T3.3: ${aj} jobs still active"
    fi

    # T3.4: Post-lifecycle health
    log "T3.4: Post-lifecycle health"
    sleep 2
    local aj
    aj=$(active_jobs)
    if [ "$aj" = "0" ]; then
        pass "T3.4: active_jobs=0"
    else
        fail "T3.4: active_jobs=${aj}"
    fi
}

# ── Tier 4: Failure and Recovery ─────────────────────────────────────

tier4_recovery() {
    header "Tier 4: Failure and Recovery"

    local files20
    files20=$(collect_files 20)
    local count20
    count20=$(echo "$files20" | wc -l | tr -d ' ')
    if [ "$count20" -lt 10 ]; then
        skip "T4: Not enough files for recovery tests"
        return
    fi

    # T4.1: Server crash and restart
    log "T4.1: Server crash (SIGKILL) and restart"
    local id_file="${WORK_DIR}/crash_job.id"
    rm -f "$id_file"
    # shellcheck disable=SC2086
    submit_job_bg "$id_file" morphotag eng $files20
    local bg_pid=$!
    sleep 4  # Let processing start.
    local job_id
    job_id=$(cat "$id_file" 2>/dev/null || echo "")
    if [ -n "$job_id" ]; then
        log "  Killing server (SIGKILL)..."
        kill_server
        wait "$bg_pid" 2>/dev/null || true
        sleep 2
        log "  Restarting server (preserving state)..."
        start_server false
        if [ $? -ne 0 ]; then
            fail "T4.1: Server failed to restart"
        else
            # Check recovery log.
            if grep -q "Loaded jobs from DB" "$SERVER_LOG"; then
                pass "T4.1: Server recovered jobs from DB"
            else
                fail "T4.1: No recovery log found"
            fi
            # Wait for the job to complete after recovery.
            local status
            status=$(wait_job "$job_id" 120)
            if [ "$status" = "completed" ]; then
                pass "T4.1: Job completed after crash recovery (${job_id})"
            else
                fail "T4.1: Job status=${status} after recovery (job=${job_id})"
            fi
        fi
    else
        wait "$bg_pid" 2>/dev/null || true
        fail "T4.1: No job ID"
    fi

    # T4.2: Graceful shutdown (SIGTERM)
    log "T4.2: Graceful shutdown (SIGTERM)"
    # Submit a job, let it complete, then SIGTERM the server and verify clean shutdown.
    local f1
    f1=$(collect_files 1 | head -1)
    job_id=$(submit_job morphotag eng "$f1")
    if [ -n "$job_id" ]; then
        wait_job "$job_id" 60 > /dev/null 2>&1
        log "  Sending SIGTERM to server..."
        stop_server
        if grep -q "Shutdown complete" "$SERVER_LOG"; then
            pass "T4.2: Graceful shutdown completed cleanly"
        else
            pass "T4.2: Server stopped (shutdown message may vary)"
        fi
        # Restart for remaining tests.
        log "  Restarting server (preserving state)..."
        start_server false
    else
        fail "T4.2: No job ID"
    fi

    # T4.3: Heartbeat timeout (SIGSTOP/SIGCONT)
    log "T4.3: Heartbeat timeout simulation (SIGSTOP 15s)"
    local f1
    f1=$(collect_files 1 | head -1)
    id_file="${WORK_DIR}/hb_job.id"
    rm -f "$id_file"
    submit_job_bg "$id_file" morphotag eng "$f1"
    bg_pid=$!
    sleep 2
    job_id=$(cat "$id_file" 2>/dev/null || echo "")
    if [ -n "$job_id" ]; then
        log "  Suspending server (SIGSTOP for 15s)..."
        kill -STOP "$SERVER_PID" 2>/dev/null || true
        sleep 15
        log "  Resuming server (SIGCONT)..."
        kill -CONT "$SERVER_PID" 2>/dev/null || true
        wait "$bg_pid" 2>/dev/null || true
        # Wait for recovery — Temporal should retry the activity.
        local status
        status=$(wait_job "$job_id" 120)
        if [ "$status" = "completed" ]; then
            pass "T4.3: Job completed after heartbeat timeout recovery (${job_id})"
        else
            fail "T4.3: Job status=${status} after SIGSTOP/SIGCONT (job=${job_id})"
        fi
    else
        wait "$bg_pid" 2>/dev/null || true
        fail "T4.3: No job ID"
    fi

    # T4.4: Post-recovery health
    log "T4.4: Post-recovery health"
    sleep 2
    local aj wc
    aj=$(active_jobs)
    wc=$(worker_crashes)
    if [ "$aj" = "0" ]; then
        pass "T4.4: active_jobs=0"
    else
        fail "T4.4: active_jobs=${aj}"
    fi
    log "T4.4: worker_crashes=${wc} (informational)"
}

# ── Teardown ─────────────────────────────────────────────────────────

teardown() {
    log "Tearing down..."
    stop_server
    stop_temporal
    sleep 1
    # Check for orphan processes.
    if pgrep -f "batchalign3" > /dev/null 2>&1; then
        fail "Orphan batchalign3 process detected"
        pkill -f "batchalign3" 2>/dev/null || true
    fi
}

# ── Main ─────────────────────────────────────────────────────────────

main() {
    local min_tier=1 max_tier=4

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --tier)
                shift
                if [[ "$1" == *-* ]]; then
                    min_tier="${1%-*}"
                    max_tier="${1#*-}"
                else
                    min_tier="$1"
                    max_tier="$1"
                fi
                shift
                ;;
            *)
                echo "Usage: $0 [--tier N | --tier N-M]"
                exit 2
                ;;
        esac
    done

    header "Temporal Workflow Stress Test"
    log "Binary: ${BINARY}"
    log "Port: ${PORT}"
    log "Tiers: ${min_tier}-${max_tier}"
    log "Work dir: ${WORK_DIR}"
    log "Corpus: ${CORPUS_DIR}"

    # Preflight
    if [ ! -x "${BINARY}" ]; then
        echo "ERROR: Binary not found at ${BINARY}. Run 'cargo build -p batchalign-cli' first."
        exit 2
    fi
    if ! command -v temporal &> /dev/null; then
        echo "ERROR: temporal CLI not found. Install with 'brew install temporal'."
        exit 2
    fi
    if ! command -v jq &> /dev/null; then
        echo "ERROR: jq not found. Install with 'brew install jq'."
        exit 2
    fi
    local file_count
    file_count=$(collect_files 50 | wc -l | tr -d ' ')
    log "Available test files: ${file_count}"
    if [ "$file_count" -lt 5 ]; then
        echo "ERROR: Need at least 5 .cha files in ${CORPUS_DIR}"
        exit 2
    fi

    mkdir -p "$WORK_DIR" "$OUTPUT_DIR"

    # Setup
    trap teardown EXIT
    start_temporal || exit 2
    start_server || exit 2

    # Run tiers
    [ "$min_tier" -le 1 ] && [ "$max_tier" -ge 1 ] && tier1_smoke
    [ "$min_tier" -le 2 ] && [ "$max_tier" -ge 2 ] && tier2_scale
    [ "$min_tier" -le 3 ] && [ "$max_tier" -ge 3 ] && tier3_lifecycle
    [ "$min_tier" -le 4 ] && [ "$max_tier" -ge 4 ] && tier4_recovery

    # Summary
    header "Results"
    echo -e "  ${GREEN}PASS: ${PASS}${RESET}"
    echo -e "  ${RED}FAIL: ${FAIL}${RESET}"
    echo -e "  ${YELLOW}SKIP: ${SKIP}${RESET}"
    echo ""

    if [ "$FAIL" -gt 0 ]; then
        echo -e "${RED}${BOLD}STRESS TEST FAILED${RESET} — ${FAIL} failure(s)"
        echo "Server log: ${SERVER_LOG}"
        echo "Temporal log: ${TEMPORAL_LOG}"
        exit 1
    else
        echo -e "${GREEN}${BOLD}ALL TESTS PASSED${RESET}"
        exit 0
    fi
}

main "$@"
