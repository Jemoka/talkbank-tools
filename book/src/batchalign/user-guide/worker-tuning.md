# Worker Tuning

**Status:** Current
**Last updated:** 2026-05-10 12:12 EDT

This page explains how the server decides how many workers to run, how memory
budgets work, and how to configure warmup and tuning for your hardware.

## The `--workers` flag

Control how many files are processed in parallel:

```bash
batchalign3 --workers 1 transcribe corpus/ -o output/    # One file at a time (safest)
batchalign3 --workers 4 morphotag corpus/ -o output/     # Four files in parallel
batchalign3 transcribe corpus/ -o output/                 # Auto-tune (default)
```

All commands now use a two-stage policy: the runner computes a **requested**
worker count from file count, CPU, and category caps, then the host-memory
coordinator clamps that request to what the machine can safely fit right now.
GPU-heavy commands (`transcribe`, `align`, `benchmark`) are capped by both
`max_gpu_workers` and `gpu_thread_pool_size`.

> **CPU-only machines (Apple Silicon, no CUDA):** the host-facts
> recommendation now sets `gpu_thread_pool_size = 1` automatically
> when no functional GPU is detected. Leave the field absent in
> `server.yaml` (or set it to `0` — the legacy "auto" sentinel that
> still deserializes to "no override"). The recommendation also
> sets `force_cpu = true` on the same hosts so workers skip GPU
> detection entirely.
>
> PyTorch releases the GIL only during CUDA/MPS native calls; with
> MPS excluded for batchalign3, every Whisper inference is GIL-bound
> CPU work and there is no compute parallelism to gain. A higher
> value lets multiple `execute_v2` calls into a single Python
> process where they fight for cores, slowing each other down by
> the contention factor. Verify your host's resolved values with
> `batchalign3 doctor --check` (see [Doctor](doctor.md)).
>
> The Rust-side `dispatch_semaphore` mirrors `gpu_thread_pool_size`
> permit-for-thread, so Rust dispatch and Python serving share one
> ceiling. Set this knob to the parallelism your device actually has:
> 1 on CPU-bound platforms, 2-4 on real GPU. See
> [MPS Exclusion Decision](../developer/apple-mps-workarounds.md) and
> [Worker Protocol V2 § The dispatch semaphore contract](../developer/worker-protocol-v2.md#the-dispatch-semaphore-contract).
>
> Measured Apple behavior on `dev-machine` confirms that the biggest win is not a
> larger CPU thread pool. Warm loopback-daemon reuse dominated everything else:
> `align --no-utr --fa-engine wav2vec` dropped from `17.65s` direct / `9.64s`
> sequential to `1.06s` warm-daemon, and `transcribe --asr-engine whisper`
> dropped from `93.92s` direct to `13.58s` warm-daemon. On Apple CPU-only
> hosts, preserving warm workers matters much more than tuning
> `gpu_thread_pool_size`.

Override with `--workers N` when you want explicit control, or set
`max_workers_per_job` in `server.yaml` for a persistent override.

## How worker planning works

When you submit a job, the server decides how many parallel file workers to
assign.

1. Compute a requested worker count from file count, CPU, and category caps:
   - GPU commands: `min(max_gpu_workers, gpu_thread_pool_size)`
   - CPU/IO commands: `max_thread_workers`
2. Ask the host-memory coordinator for a job execution plan.
3. The coordinator subtracts active local reservations, preserves
   `memory_gate_mb` as host headroom, and grants the largest safe worker count.
4. If nothing safe fits, the job is re-queued instead of speculatively running.

For a single file, the server always uses 1 worker — no parallelism needed.

If `max_workers_per_job` is set in `server.yaml`, it overrides auto-tuning
(still capped by file count and the category max).

**Why GPU commands allow parallelism:** GPU-heavy commands share a single
`SharedGpuWorker` process with a thread pool. While file N's ASR runs on the
GPU, file N+1 can do post-processing, utseg, or morphosyntax on CPU. The GPU
itself serializes inference, but pipeline stages overlap. On a machine with
256 GB RAM, the coordinator may grant 4–8 parallel files for transcribe.

## Worker profiles and host bootstrap mode

The server groups related commands into three worker profiles that share loaded
models within a single process:

| Profile | Commands | What it shares |
|---------|----------|---------------|
| **GPU** | `align`, `transcribe`, `transcribe_s`, `benchmark` | Whisper, Wave2Vec, and speaker models in one process |
| **Stanza** | `morphotag`, `utseg`, `coref`, `compare` | Stanza NLP models (POS, constituency, coreference) |

On large machines, this means running `align` followed by `transcribe` reuses
the same GPU worker process — the ASR model loaded for transcription stays in
memory and the FA model for alignment lives in the same process. On a 64 GB
machine, this saves roughly 3 GB compared to loading each model in a separate
process.

On **small-memory hosts**, the server now resolves a different host execution
policy: local workers use **task bootstrap** instead of full profile bootstrap.
That lets a weak laptop load only `infer:asr` or `infer:morphosyntax` instead
of speculatively loading every model in a profile. The machine trades some
reuse for a much lower idle footprint.

GPU workers handle multiple requests concurrently via internal threading only on
CUDA-capable hosts. On CPU-only hosts they stay sequential to avoid
oversubscribing OpenMP threads. Stanza and IO workers handle one request at a
time but can run multiple processes in parallel for CPU-bound workloads.

## Per-command memory profiles

Each command loads different ML models with different memory footprints. These
values come from `runtime_constants.toml` (generated from `crates/batchalign-types/src/command_spec.rs`
via `xtask gen-runtime-toml`; shared between Rust and Python at compile/import time):

| Command | Memory per worker (MB) | What drives it |
|---------|----------------------|----------------|
| `morphotag` | 2,000 | Stanza POS/lemma/depparse models (per language) |
| `align` | 4,000 | Whisper or Wave2Vec forced alignment model |
| `transcribe` | 1,500 | Whisper ASR model |
| `utseg` | 2,000 | Stanza constituency parser |
| `translate` | 4,000 | Translation model (Seamless M4T or Google) |
| `coref` | 2,000 | Stanza coreference model |
| `compare` | 2,000 | Stanza models (for normalization) |

These are the *thread worker* values (shared-model mode). Process worker values
are higher because each worker loads its own copy of the models.

Commands in the same profile share a worker process, so the total memory for
a mixed job (e.g., `align` + `transcribe`) is roughly the sum of their models
loaded once, not separately. The GPU profile typically uses ~5 GB total for all
its models (ASR + FA + Speaker), regardless of how many commands run.

## Warmup configuration

### The `--warmup` flag

Control which models are pre-loaded at server startup:

```bash
# Presets
batchalign3 serve start --warmup off           # No warmup — workers spawn on first job
batchalign3 serve start --warmup minimal        # Morphotag only
batchalign3 serve start --warmup full           # Morphotag + align + transcribe (default)

# Explicit command list
batchalign3 serve start --warmup align          # Only forced alignment
batchalign3 serve start --warmup morphotag,align  # Both morphotag and align
```

`warmup_commands` still describes which commands are *eligible* for warmup.
However, the current production startup path stays lazy by default: real worker
warmup is normally skipped unless you are using a test-echo or test harness
path. That matches the current resource-first architecture — small and medium
machines should not speculatively preload models at process start.

### server.yaml warmup key

```yaml
# List of commands to pre-warm at startup.
# Default: [morphotag, align, transcribe] (the "full" preset)
# Empty list = no warmup.
warmup_commands:
  - morphotag
  - align
```

The `--warmup` CLI flag overrides this config key.

### Background warmup

When warmup is actually enabled, it runs in the background — the HTTP port binds
immediately. While models are loading:

- The `/health` endpoint reports `"warmup_status": "in_progress"`
- Jobs that need a model still loading will wait for the warmup to finish
  (no duplicate worker spawns — the job reuses the in-progress warmup)
- Once complete, `/health` reports `"warmup_status": "complete"`

Warmup still fans out across commands, but each heavy worker startup must now
acquire a host-wide startup lease. Host policy may also suppress warmup
entirely, and constrained hosts keep warmup off so background preload cannot
stampede the machine.

### On-demand loading

With `--warmup off`, no workers are pre-loaded. Workers spawn lazily on the
first job that needs them. This is ideal for:

- Testing and development
- Users who only run one command type
- Memory-constrained machines where you don't want idle model overhead

Lazy startup does **not** mean the first real command is allowed to run against
unknown infer-task metadata. The current server resolves that by forcing a live
capability probe from the worker it is actually about to use. In practice:

- `/health` may still show an optimistic command surface immediately after boot
- the first real job pays the worker startup cost and records the detected
  infer-task/engine-version view
- later jobs reuse that detected capability view instead of the cold-start
  placeholder

## server.yaml reference

Key tuning parameters:

```yaml
# Worker parallelism
max_workers_per_job: 0          # 0 = auto-plan from files, CPU, and category caps
max_concurrent_jobs: 0          # 0 = CPU-based runner slot cap
gpu_thread_pool_size: 4         # Concurrent execute_v2 per shared GPU worker.
                                 # Rust dispatch_semaphore + Python ThreadPoolExecutor
                                 # share this ceiling — set to the device's real
                                 # parallelism (1 on Apple Silicon CPU-only;
                                 # 2-4 on CUDA where the GIL is released).
max_concurrent_worker_startups: 1

# Memory tier override — force a specific tier instead of auto-detecting
# from total RAM. Values: small, medium, large, fleet. Omit to auto-detect.
# memory_tier: small

# Host-memory reserve/headroom (MB) preserved after reservations
# 0 = disable explicit reserve. Default: 2048 MB (one absolute floor on
# every host; the worker-pool admission gate enforces the same number
# live on every spawn attempt — see book/src/batchalign/developer/memory-safety.md)
memory_gate_mb: 4000

# Per-profile startup reservation overrides (MB). 0 = use tier default.
# These control how much memory the coordinator reserves while a worker
# loads its models. Reduce on small machines if the tier defaults are
# too conservative for your actual model sizes.
# gpu_startup_mb: 6000
# stanza_startup_mb: 3000
# io_startup_mb: 2000

# Worker lifecycle
# Idle workers are evicted by host memory pressure (largest-RSS first
# when available memory drops below the eviction threshold) — there is
# no fixed idle timeout. Health checks run every `worker_health_interval_s`.
worker_health_interval_s: 30    # Health check frequency

# Warmup — list of commands to pre-load at startup
# Default: [morphotag, align, transcribe]
# Empty list disables warmup entirely.
warmup_commands:
  - morphotag
  - align
  - transcribe
```

## Scenarios

### 16 GB laptop / shared developer machine

```yaml
# The Small tier (<24 GB) auto-detects these defaults, so this config
# is only needed if you want to further customize on a small machine.
memory_tier: small              # Force small tier (auto-detected on <24 GB)
max_workers_per_job: 1
memory_gate_mb: 2000            # Small tier default
stanza_startup_mb: 3000         # Actual Stanza RSS is ~2-3 GB
gpu_startup_mb: 6000            # Whisper float32 is ~4-5 GB
max_concurrent_worker_startups: 1
gpu_thread_pool_size: 1
warmup_commands: []             # No warmup — workers spawn on demand
```

The Small tier now also switches local workers to **task bootstrap** and clamps
eligible file-parallel commands to one file at a time. That keeps the execution
shape honest for 16 GB laptops: no speculative profile preload, no multi-file
GPU stampede, and no assumption that the machine can afford idle models it is
not about to use.

Or start with no warmup:

```bash
batchalign3 serve start --warmup off
```

### 32 GB desktop

Default settings usually work well, but keep warmup conservative if the machine
also runs other inference tools. The coordinator will clamp jobs as host
pressure changes.

### 256 GB server (production)

```yaml
max_workers_per_job: 0          # Coordinator-backed auto planning
max_concurrent_jobs: 8
max_concurrent_worker_startups: 1
memory_gate_mb: 8000            # Operator override; the default is 2048 MB (MIN_FREE_MEMORY_MB)
warmup_commands:
  - morphotag
  - align
  - transcribe
```

With this much RAM, worker profiles let the server run multiple concurrent jobs
efficiently. A GPU worker handling an `align` job and a Stanza worker handling a
`morphotag` job run in parallel without duplicating models, leaving plenty of
headroom for additional jobs.

### Testing with --warmup off

For quick iteration during development:

```bash
batchalign3 serve start --warmup off --foreground --test-echo
```

Workers start instantly (no ML models loaded). Useful for testing server
infrastructure without waiting for model initialization.

## Troubleshooting

### "Job deferred due to memory pressure"

The host-memory coordinator could not fit the requested execution plan. Possible causes:

1. **Too many concurrent workers.** Reduce `max_workers_per_job` or
   `gpu_thread_pool_size`.
2. **Other processes using RAM.** Check system memory usage.
3. **Idle workers holding memory.** Workers that haven't been used in a while
    still hold their loaded models. The pool's pressure-driven eviction
    releases idle workers automatically when host available memory drops
    below the eviction threshold; if pressure is genuine but eviction
    isn't firing fast enough, restart the server to reclaim immediately.
4. **Another local batchalign3 server or test run is already holding leases.**
   Check `/health` for `host_memory_*` fields.

Jobs are re-queued when the plan does not fit. `/health` now reports
`host_memory_pressure`, current reservations, and active lease labels.

### Only 1 worker running

The coordinator decided that only 1 worker currently fits. Check:

- `/health` `host_memory_pressure` and `host_memory_reserved_mb`
- `memory_gate_mb`
- `gpu_thread_pool_size` for GPU commands
- other local `batchalign3` servers or ML tools on the same host

Override with `max_workers_per_job` if you know your system can handle more.

### Startup takes too long

Warmup loads ML models from disk (or downloads them on first run). To speed up:

- Use `--warmup minimal` or `--warmup off` if you don't need all commands
- The first run after installation is slowest (model downloads)
- Subsequent starts load from the model cache (~5-20 seconds per model)
- Keep the daemon running (`batchalign3 serve start`) to avoid repeated
  cold starts

See also [Performance](performance.md) and [Server Mode](server-mode.md).
