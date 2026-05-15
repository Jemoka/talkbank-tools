# batchalign — HTTP Server, Job Store, and NLP Orchestration

**Status:** Current
**Last modified:** 2026-05-11 08:29 EDT

## Overview

Axum-based REST server managing job lifecycle, Python worker dispatch, and server-side
CHAT orchestration (CHAT ownership boundary — server owns parse/cache/inject/serialize,
Python workers provide stateless NLP inference only).

## Module Map

| Module | Purpose |
|--------|---------|
| `lib.rs` | `create_app()`, WebSocket handler, graceful shutdown |
| `state.rs` | `AppState`, capability gate (`validate_infer_capability_gate()`) |
| `cache/` | Tiered cache for FA word timings and UTR ASR results: moka in-memory hot layer + SQLite cold backend (BLAKE3 keys). Text NLP tasks (morphotag/utseg/translate/coref) deliberately do NOT cache. |
| `store/` | `JobStore` composition, `JobRegistry` actor, `OperationalCounterStore`, SQLite write-through, conflict detection, memory gating |
| `runner/` | Per-job async task: dispatch routing, parallelism, preflight. `runner/policy.rs` has `infer_task_for_command()` and `command_requires_infer()`. `runner/util/` has progress helpers |
| `runner/dispatch/` | Legacy dispatch implementations: `infer_batched.rs`, `fa_pipeline.rs`, `transcribe_pipeline.rs`, `benchmark_pipeline.rs`, `media_analysis_v2.rs` |
| `command_model/` | Authoritative command registry: `CommandSpec`, typed execution shapes, `io_profile_for()` |
| `planning/` | Immutable job plans: `build_job_plan()`, `JobPlan`, `IoMode`, work-unit enumeration |
| `execution/` | Recipe-driven execution kernel: `StageExecutor` trait, `ExecutionKernel`, `WorkerGateway`. Compare is the first migrated command |
| `db/` | SQLite persistence (WAL): `schema.rs`, `insert.rs`, `query.rs`, `update.rs`, `recovery.rs` |
| `error.rs` | Typed errors → HTTP status codes (404, 409, 500) |
| `morphosyntax/` | Server-side morphosyntax orchestrator (parse→clear→collect→cache→infer→inject→serialize) |
| `pipeline/` | `PipelineServices`, transcribe pipeline (Rust-only number expansion), text infer pipeline, morphosyntax batch |
| `utseg.rs` | Utterance segmentation orchestrator |
| `translate.rs` | Translation orchestrator (injects `%xtra`) |
| `coref.rs` | Coreference resolution (document-level, sparse, English-only) |
| `fa/` | Forced alignment orchestrator (per-file, multi-group, audio-aware, DP alignment, incremental FA) |
| `workflow/` | Workflow-family registry, typed descriptors, traits, and per-command implementations |
| `worker/` | Worker pool, IPC handle, V2 request builders and result types |
| `media.rs` | Media file resolution with walk cache |
| `ws.rs` | WebSocket broadcast event types |
| `websocket.rs` | WebSocket route and handler |
| `hostname.rs` | Tailscale IP→hostname resolution |
| `routes/` | HTTP endpoints: health, jobs (CRUD+SSE), media, dashboard, bug reports, traces |
| `types/` | API models, parameter structs, worker IPC types, scheduling types, and re-exports of shared domain newtypes from `batchalign-types` |

## Job Registry Concurrency Model

`JobRegistry` no longer exposes a shared `Mutex<HashMap<...>>` boundary.
`JobStore` creates one owned actor task with an `mpsc::UnboundedSender`
mailbox. Callers submit either:

- `Inspect` closures for read-only projections
- `Mutate` closures for in-place transitions

Each request pairs with a `oneshot` reply so callers still `await` a typed
result. Prefer the named store/registry methods for normal job-local work;
`inspect_all()` / `mutate_all()` remain the bulk escape hatches for recovery and
other collection-wide operations.

Route, query, and runner code should think in terms of job transitions and
projections, not in terms of "lock the map and poke fields."

## Key Commands

```bash
cargo nextest run -p batchalign-cli
cargo clippy -p batchalign-cli -- -D warnings
```

## Route Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/jobs` | Submit job (validates command, checks conflicts) |
| GET | `/jobs`, `/jobs/{id}` | List/get jobs |
| GET | `/jobs/{id}/results[/{filename}]` | Download results |
| GET | `/jobs/{id}/stream` | SSE streaming (real-time progress) |
| POST | `/jobs/{id}/cancel`, `/jobs/{id}/restart` | Lifecycle |
| DELETE | `/jobs/{id}` | Permanent delete |
| GET | `/health` | Version, capabilities, worker state |

## Job Lifecycle and Requeue Invariant

Every submitted job gets a `job_task` runner — a self-contained async task
that owns the full lifecycle from semaphore acquire through finalization.

`job_task` is a **non-recursive function returning `Pin<Box<dyn Future>>`**
(not `async fn`). This is deliberate: the `Requeued` branch spawns a fresh
`job_task` inside `tokio::spawn`. With `async fn`, Rust's Send inference
becomes circular on the self-referential opaque return type. The explicit
boxed future gives the inner call a concrete, provably-Send type.

**Requeue invariant** — when the memory gate rejects a job:

1. `run_hosted_job` returns `Ok(HostedJobRunOutcome::Requeued { retry_at })`
2. `job_task` catches this in the `match` arm (not `if let Err(...)`)
3. It spawns a new delayed `job_task` via `tokio::spawn(sleep + job_task(...))`
4. The current `job_task` instance finishes: `lease_task.abort()` +
   `release_runner_claim()` run unconditionally at the bottom
5. The new task re-acquires the semaphore and memory gate after the backoff

Without step 3, a requeued job stays `Queued` forever with no runner and
blocks all future submissions of the same files (409 conflict).

**Bootstrap invariant** — queued jobs loaded from the DB at startup:

`bootstrap_test_server_backend` calls `store.queued_job_ids()` immediately
after `load_from_db()` and spawns `job_task` for each recovered `Queued` job
via `runtime.spawn_detached(job_task(job_id, host.clone()))`. This fulfills
the recovery path when the daemon is restarted after a crash or a memory-gate
rejection that lost its runner.

**Recovery is a two-step sequence that only fires at startup:**

1. `db.recover_interrupted()` (`db/recovery.rs`) is a SQL migration that
   flips rows in `('queued', 'running')` to `interrupted`. It does NOT
   touch existing `interrupted` rows and does NOT requeue.
2. `store.load_from_db()` (`store/queries/recovery.rs`) reads each row
   back into memory. For any job with `status ∈ {Interrupted, Running}`,
   it calls `Job::reconcile_recovered_runtime_state()` — which transitions
   the in-memory job (and writes back to the DB) to `Queued` if any file
   is resumable, or to a terminal state otherwise.

If the daemon stays alive across CLI sessions, neither step runs — the
bootstrap spawn after `load_from_db()` is the only mechanism that rescues
orphaned `Queued` jobs.

### Cancelled vs Interrupted at shutdown

`JobStatus::Cancelled` is reserved for user gestures (TUI cancel, HTTP
DELETE/cancel). It is permanent — a Cancelled job is never auto-resumed.

`JobStatus::Interrupted` is the system-initiated counterpart. The graceful
shutdown handler in `temporal_backend::interrupt_all_for_shutdown` writes
`Interrupted` (not `Cancelled`) for in-flight jobs, with an audit row in the
`cancellations` table tagged `source=signal, reason=server-cancel-all`. On
the next server start, the recovery sequence above transitions any
Interrupted job whose file work is not yet complete back to `Queued`.

The Temporal reconciler also consults the audit table on restart: when
Temporal reports a workflow as `Cancelled` but the most-recent local audit
row is `signal/server-cancel-all` or `signal/temporal-activity-forwarded`,
the reconciler returns `NoChange` rather than `MarkCancelled` so the Queued
state survived from recovery is not overwritten.

This matters because a server bounce mid-job (deploy, OS restart, crash)
would otherwise be indistinguishable from a user cancel in the local DB —
and the user's dashboard would show the job as "cancelled" even though no
user pressed cancel. See `temporal_reconciler::is_system_initiated_shutdown_cancel`
for the predicate, and the matching reason strings at
`temporal_backend::interrupt_all_for_shutdown` (the body, not the trait
method) and `temporal_backend.rs:287-308` (the activity-side cancel
forwarder).

## Dispatch Routing (runner/)

Dispatch shapes (driven by `command_model/` specs):
1. **Batched text infer** (`runner/dispatch/infer_batched.rs`) — morphotag, translate, coref: pool all utterances from all files, group by language, dispatch language groups with **semaphore-bounded concurrency** (`morphosyntax/batch.rs`, `max_total_workers / max_workers_per_key` concurrent groups), and within each group split into chunks across multiple workers (`morphosyntax/worker.rs`, up to `max_workers_per_key`). Unsupported languages filtered at preflight (`stanza_languages.rs`). Per-language processor availability (MWT, constituency) determined by the **Stanza capability table** (`python/batchalign/worker/_stanza_capabilities.py`), which reads Stanza's `resources.json` — not hardcoded.
1a. **Per-file utseg** (`execution/utseg.rs::dispatch_utseg_job`) — utseg specifically does NOT go through the batched-text-infer pool. Each file gets its own `gateway.utseg_batch(&[one_file], lang)` call, run sequentially with per-file writeback before the next file starts. Trade-offs: incremental output, file-level failure isolation, daemon-redeploy resilience (each completed file is durable on disk before the next starts). Cost: no cross-file batching for GPU efficiency. The trade is right because utseg's BERT inference is single-thread CPU-bound on macOS (no MPS), and the batched pattern was empirically fragile to interruption — a daemon redeploy mid-run lost hours of work on a long batched run. morphotag/translate/coref retain the batched dispatch (they may legitimately benefit from cross-file batching when GPU is back).
2. **Per-file FA** (`runner/dispatch/fa_pipeline.rs`) — align: files processed concurrently via `JoinSet` + `Semaphore(num_workers)`. UTR pre-pass runs before FA grouping with ASR result caching. Fallback UTR retries timing recovery after FA failures. For mostly-timed files (with sufficient existing timing coverage and audio length), partial-window ASR runs only on untimed regions.
3. **Per-file transcribe** (`runner/dispatch/transcribe_pipeline.rs`) — transcribe, transcribe_s: per-file audio processing with optional diarization, utseg, and morphosyntax. ASR post-processing uses a single Rust per-word expansion pass via `prepare_asr_chunks()` in `pipeline/transcribe.rs` (no Python `num2words` IPC; see `book/src/architecture/number-expansion.md`).
4. **Per-file benchmark** (`runner/dispatch/benchmark_pipeline.rs`) — composite transcribe + compare.
5. **Recipe-driven compare** (`execution/`) — gold-anchored comparison via `ExecutionKernel` + `CompareStageExecutor`. First command migrated from legacy dispatch to the recipe execution model.
6. **Per-file media analysis** (`runner/dispatch/media_analysis_v2.rs`) — opensmile, avqi: concurrent files via `JoinSet` + `Semaphore(num_workers)`, worker `execute_v2`.

**Post-validation is warn-only** — output is always serialized and written even if
post-validation finds issues. This ensures output CHAT can be inspected for debugging.

## Type System

Domain newtypes are defined in `batchalign-types` using `string_id!` and `numeric_id!`:
- **`../batchalign-types/src/macros.rs`** — macro definitions (generates Deref, serde transparent, From, Borrow, etc.)
- **`../batchalign-types/src/domain/`** — `JobId`, `CommandName`, `ReleasedCommand`, `LanguageCode3`, `LanguageSpec`, `DisplayPath`, `EngineVersion`, `CorrelationId`, `NumSpeakers`, `UnixTimestamp`, `DurationMs`, `MemoryMb`, etc.
- **`../batchalign-types/src/scheduling.rs`** — `AttemptId`, `WorkUnitId`
- **`types/params.rs`** — `CachePolicy`, `WorTierPolicy` enums; `MorphosyntaxParams`, `FaParams`, `AudioContext` structs
- **`pipeline/mod.rs`** — `PipelineServices` (shared infrastructure refs: pool, cache, engine_version)

**Boundary patterns:** Raw `String` from HTTP → `JobId::from()` at handler entry. `&Path` in domain code → `to_string_lossy()` at IPC/JSON. `bool` from CLI → `CachePolicy::from()` at dispatch. See `book/src/architecture/type-driven-design.md`.

## Admission and Eviction Gates

Three admission contexts, each with its own explicit policy
(`worker/pool/lifecycle.rs` derives `PoolGateState` per spawn
attempt). Per the spec at
`<workspace>/docs/architecture/2026-05-10-tier-aware-memory-consolidation.md`
(Principle 4), each context is named at the call site rather than
inferred:

| Context | Where | Policy |
|---|---|---|
| **Boot** | `host_facts/validation.rs` | Validates `max_concurrent_jobs × worst_case_per_job_peak_ram_mb(tier) ≤ ram_total_mb`. On refusal, the error suggests `--sequential`. |
| **ColdStart** | `worker/pool/{cpu_gate,memory_gate}.rs` | First worker for a `(profile, lang, engine)` class. Admits unconditionally — back-pressure has nothing to push against on an empty pool, and refusing here leaves the pool dead-on-arrival on memory-tight hosts. |
| **Warm** | `worker/pool/{cpu_gate,memory_gate}.rs` | N+1 worker for a class with existing workers. Runs the projection: `available_mb − new_worker_estimate > host_min_free_mb_threshold_for_tier(tier)`. The CPU gate runs the analogous `getloadavg(3).one < available_parallelism()` check. |
| Pre-pass (eviction) | `worker/pool/idle_eviction.rs` | When `available_mb ≤ EVICTION_PRESSURE_THRESHOLD_MB (= 4096)`, evict idle workers largest-RSS first. |

The Warm-gate floor `host_min_free_mb_threshold_for_tier` is
tier-scaled (1024 MB Small / 2048 MB Medium / 4096 MB Large+Fleet),
not the fixed `MIN_FREE_MEMORY_MB = 2048` used historically. The
constant still exists as the Medium-tier floor and the default for
the JobStore-level gate (see below).

The new-worker estimate prefers the observed average RSS of
same-profile idle peers (`worker/pool/rss_observer.rs`) when peers
exist; otherwise falls back to per-tier `MemoryTier::*_startup_mb`
(the canonical source per Principle 1; `tier.gpu_startup_mb`,
`tier.stanza_startup_mb`, `tier.io_startup_mb`).

Admission is back-pressure, not safety (Principle 5). The
correctness floor is `worker/memory_guard.rs` plus the OS OOM
killer; admission heuristics can be over-permissive without
compromising correctness — a doomed worker dies at spawn with
bounded cost. ColdStart bypass implements the over-permissive
bias.

The host-memory snapshot is shared across every poll
(`host_memory::system_memory_snapshot`, 1 s TTL) so admission gates,
eviction pre-pass, in-spawn `memory_guard`, and host-facts probes
all see the same reading.

A separate **JobStore-level memory gate** at the job-admission seam
polls `sysinfo::available_memory()` with a configurable threshold
(`memory_gate_mb`, default 2048 MB). **Idle worker bypass**: skips
the memory check when the pool has reusable workers for the job's
`(command, lang)` — prevents deadlock where loaded workers hold RAM.

See [`book/src/batchalign/developer/memory-safety.md`](../../book/src/batchalign/developer/memory-safety.md)
for the full layered defense.

## Middleware Stack

CORS → body limit (`max_body_bytes_mb`, configurable) → panic catching → timeout → tracing → compression.

Axum's built-in `Json` extractor limit is disabled on job routes so the
outer `RequestBodyLimitLayer` is the sole body-size guard.  See
`book/src/developer/http-body-limits.md` for the full story.
