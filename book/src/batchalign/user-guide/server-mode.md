# Server Mode

**Status:** Current
**Last updated:** 2026-05-11 11:54 EDT

Batchalign includes a built-in HTTP server managed by `batchalign3 serve ...`.
Ordinary local processing commands can still run inline, but the CLI no longer
treats the direct host as the only default. When `auto_daemon: true` (the
default), the router first tries to reuse or start a loopback daemon so warm
workers survive across commands. `--no-server` and `--sequential` still force
direct local execution.

## Current routing rules

- With `--server URL`, the CLI submits supported jobs to that server in content
  mode.
  when `auto_daemon` is enabled. In that case the CLI tries to reuse or start a
  loopback daemon first; if that local daemon path is unavailable, it falls back
  to the explicit `--server` target.
- Without an explicit remote target, `auto_daemon: true` makes the CLI reuse or
  start a loopback daemon before it falls back to the shared direct host.
- If daemon startup is unavailable, the CLI still reuses any already-running
  loopback server on the configured port before it falls back to direct local
  execution.
- Local-daemon and auto-detected loopback-server paths use shared-filesystem
  `paths_mode` for local-audio commands such as `align`, `transcribe`,
  content mode, even when the URL is `localhost`.
- This matters most on Apple CPU-only hosts: repeated `align` / `transcribe` /
  `benchmark` runs are much faster when warm workers are preserved behind a
  loopback daemon than when each command pays the full cold direct-local cost.

## Start a server

Foreground:

```bash
batchalign3 serve start --foreground
```

Background:

```bash
batchalign3 serve start
```

Useful flags:

```bash
batchalign3 serve start --foreground --port 8000
batchalign3 serve start --foreground --config ~/server.yaml
batchalign3 serve start --foreground --warmup minimal
batchalign3 serve start --foreground --test-echo
batchalign3 serve start --foreground --test-echo
```

## Backend selection

The server has two control-plane backends, chosen by the
`temporal_server_url` field in `server.yaml`:

- **In-process backend** — selected when `temporal_server_url` is empty
  or set to the sentinel `"none"`, `"local"`, or `"disabled"`. The
  control plane is hosted inside the `batchalign3` server process; no
  external service required. (Implemented by `bootstrap_test_server_backend`
  in `crates/batchalign/src/server_backend.rs` — the name dates from when
  this path was integration-test-only, but it now also serves as the
  production non-Temporal backend.)
- **Temporal backend** — selected when `temporal_server_url` is a real
  URL (e.g. `http://127.0.0.1:7233`). The Batchalign server hands
  queued-job orchestration, retry timing, and durable cancellation/restart
  state to Temporal workflows and activities. Implementation:
  `bootstrap_temporal_server_backend` in
  `crates/batchalign/src/temporal_backend.rs`.

The selection logic lives in `ServerConfig::temporal_backend()`
(`crates/batchalign/src/types/config/resolve.rs:26`); there is **no**
`--backend` CLI flag. The `backend:` YAML key is accepted for backward
compatibility with older `server.yaml` files (`backend_compat` in
`types/config/server.rs:89`) but is not read at runtime.

The fleet's pyinfra renderer
(`automation/.../batchalign_render.py::_temporal_server_url_for`) emits
`temporal_server_url` only when the host has the `temporal_enabled` or
`temporal_worker` host flag set; otherwise the field is omitted and the
server falls into the in-process path. The current fleet posture is
in-process by default with Temporal opt-in per host.

### What happens to in-flight jobs when the server restarts

The two backends behave very differently when the `batchalign3` server
process is stopped and restarted (for example during a redeploy). This
matters because long-running `align` or `transcribe` batches over hundreds
of files commonly outlive the server uptime between deploys.

| Backend | In-flight job survives a server restart? |
|---|---|
| In-process | No. The control plane is in-process; stopping the server cancels every running job. |
| Temporal | Yes. The workflow lives on the Temporal server; the activity is re-leased to a worker after the Batchalign server reconnects. |

How to read the database after a restart-with-survival:

- `status` returns to `running` once the workflow re-leases the activity.
- `completed_at` may be stamped (the in-process attempt did terminate),
  but it is **not** a terminal state when running on the Temporal
  backend. Trust `status` plus dashboard `completed_files` advancing.
- A `last_cancelled_source = signal` /
  `last_cancelled_reason = temporal-activity-forwarded` row indicates the
  cancellation signal was caught by Temporal and the activity was handed
  to another worker, not that the user-submitted job was cancelled.

If you need to confirm a specific job's state after a restart, the
authoritative endpoint is `http://<server>:<port>/jobs` (the SPA at
`/dashboard/...` is just a shell — there is no `/api/jobs` route). For
deeper inspection, query `~/.batchalign3/jobs.db` directly on the server
host.

To try the Temporal backend locally, start a Temporal dev server and
point `temporal_server_url` at it in your `server.yaml`:

```bash
temporal server start-dev
# In ~/.batchalign3/server.yaml: temporal_server_url: http://127.0.0.1:7233
batchalign3 serve start --foreground --test-echo --warmup off
```

## Check and stop a server

```bash
batchalign3 serve status
batchalign3 serve status --server http://myserver:8000
batchalign3 serve stop
```

Inspect remote jobs:

```bash
batchalign3 jobs --server http://myserver:8000
batchalign3 jobs --server http://myserver:8000 <JOB_ID>
```

When the target server is running on the Temporal backend (i.e. its
`server.yaml` has a non-empty `temporal_server_url`), `batchalign3 jobs
--server ... <JOB_ID>` also reports the Temporal workflow ID, run ID,
workflow status, task queue, and history length for that Batchalign
job.

Important `--test-echo` caveat:

- `--test-echo` is a control-plane smoke path, not a full model simulation.
- Text-only infer-task commands such as `morphotag`, `utseg`, `translate`,
  `coref`, and `compare` are expected to fail under `--test-echo` because the
  echo worker does not advertise real `infer_tasks`.
- Use it to validate startup, submission, restart, cancellation, deletion, and
  remote job inspection. Do not treat it as proof that infer-task commands are
  semantically correct.

## Server configuration

Default config path:

```text
~/.batchalign3/server.yaml
```

Minimal example:

```yaml
default_lang: eng
port: 8000
max_concurrent_jobs: 8
auto_daemon: true
warmup_commands: [morphotag, align, transcribe]
media_roots: []
media_mappings: {}
```

Temporal-specific config keys (only the URL switches the backend on;
the others tune Temporal once enabled):

```yaml
temporal_server_url: http://127.0.0.1:7233   # non-empty URL → Temporal backend
temporal_namespace: default                   # default
temporal_task_queue: batchalign3-<hostname>   # default is per-host (sysinfo::System::host_name); set explicitly to share a queue
temporal_heartbeat_s: 10                      # default
temporal_activity_timeout_s: 3600             # default
```

Set `temporal_server_url:` to one of `""`, `none`, `local`, or
`disabled` (or omit the key) to run on the in-process backend instead.
The legacy `backend:` YAML key is accepted but unread; the URL field
alone selects the backend.

`warmup_commands` now marks commands that are *eligible* for warmup. The
current production startup path remains lazy by default, so this key is no
longer a promise that those workers will preload on every boot.

When warmup does spawn TCP daemons, they are now treated as **server-owned**
workers: reusable for that server instance, but cleaned up on routine shutdown.
If you want a daemon to survive server restarts, start it externally and let the
server discover it from `workers.json`. Direct local execution does not perform
that registry discovery step.

## Cold-start capability checks

On a cold server, especially with `--warmup off`, the startup path may only know
an **optimistic** command list before any real worker has been spawned. That is
intentional: startup no longer pays for a dedicated probe worker just to fill in
capability metadata.

What matters operationally is that execution now does a live check before it
trusts infer-task gating. The first real `morphotag`, `align`, `compare`, or
similar job boots the needed worker, probes its actual infer-task support, and
then runs. If the backend is truly unavailable, the job now fails with the
worker/bootstrap error instead of with a stale placeholder `infer_tasks: []`
message.

If startup finds healthy registry daemons that are already running, the server
now seeds capability state from those live workers immediately. That means a
server can come up with a real `/health.capabilities` surface without spawning a
fresh probe worker, as long as discoverable daemons already exist.

## Registry daemon ownership

`~/.batchalign3/workers.json` can now contain two daemon kinds:

- **external** daemons, started outside the current server lifecycle
- **server-owned** daemons, started by the current Rust server instance

Routine shutdown only kills the current server's own server-owned daemons.
External daemons are preserved and rediscovered on the next startup.

At startup, registry discovery reuses healthy external daemons, preserves live
foreign server-owned daemons by skipping them, and reaps stale foreign
server-owned daemons whose owning server is gone.

Important keys:

- `port` — server listen port
- `host` — bind address (defaults to `0.0.0.0`)
- `backend` — accepted for backward compatibility with older `server.yaml` files; not read at runtime. Backend selection is via `temporal_server_url` (empty/sentinel = in-process; URL = Temporal).
- `max_concurrent_jobs` — `0` means auto-tune
- `auto_daemon` — reuse or start a loopback daemon for ordinary CLI processing
- `warmup_commands` — list of commands eligible for warmup (see [Worker Tuning](worker-tuning.md))
- `media_roots` — local execution-host media lookup roots
- `media_mappings` — local execution-host root mappings from corpus paths to
  mounted media paths; useful when the CHAT/data clone root differs from the
  media root on the same machine
- `temporal_server_url` — selects the Temporal backend when set to a non-empty URL; selects the in-process backend when empty / `none` / `local` / `disabled`. This is the actual backend switch.
- `temporal_namespace` (default `"default"`) / `temporal_task_queue` (default per-host hostname-derived; the loader panics if `sysinfo::System::host_name()` returns `None`, so set explicitly to share a queue across hosts) — Temporal connection settings used when the URL field selects Temporal.
- `temporal_heartbeat_s` (default 10) / `temporal_activity_timeout_s` (default 3600) — Temporal activity heartbeat and per-attempt timeout controls.
- `memory_tier` — override auto-detected tier: `small`, `medium`, `large`, `fleet` (also controls task-vs-profile bootstrap mode)
- `memory_gate_mb` — host headroom reserve in MB (default: 2048; the worker-pool admission gate enforces the same floor live on every spawn attempt)
- `gpu_startup_mb` / `stanza_startup_mb` / `io_startup_mb` — per-profile startup reservation overrides (0 = tier default)
- `worker_health_interval_s` — health check frequency in seconds (default: 30)
- `job_ttl_days` — auto-delete completed jobs after this many days (default: 7)

OTLP tracing can be enabled by setting `BATCHALIGN_OTLP_ENDPOINT`
(or `OTEL_EXPORTER_OTLP_ENDPOINT`) in the server environment.

`server.yaml` uses a strict schema. Unknown keys are rejected at startup
instead of being silently ignored, so stale config like `warmup: false` must
be updated to the current `warmup_commands: []` form.

## Remote use

Commands that support explicit remote dispatch look like this:

```bash
batchalign3 --server http://myserver:8000 morphotag corpus/ -o output/
batchalign3 --server http://myserver:8000 align corpus/ -o output/
```

For audio commands, `--server` now means "run this on a host that can already
see these filesystem paths." The clean operational model is to run the CLI on
the execution host itself (or to reach it over SSH/VNC) rather than expecting
the server to infer media from a different client machine's directory layout.
When the corpus clone root and the mounted media root differ on that execution
host, use local `media_mappings` or `--media-dir` as explicit root replacement.

Health checks:

```bash
curl -s http://myserver:8000/health | python3 -m json.tool
batchalign3 serve status --server http://myserver:8000
```

The `/health` response includes a `capabilities` list. On a warm server, or on a
startup that already discovered live registry daemons, treat that list as the
detected command surface. On a cold lazy server with no discovered workers, it
may still be the optimistic startup surface until the first live worker probe
completes.

If a command is missing from `/health` **after** the server has warmed or run a
real job for that family, the server's Python environment is likely missing a
required package. See
[Troubleshooting: "Command not supported"](troubleshooting.md#command-not-supported-or-missing-commands).

## launchd example (macOS)

For always-on macOS hosts, write a LaunchDaemon plist that runs
`batchalign3 serve start --foreground` with your operator user and a
known log path. Standard `KeepAlive` + `RunAtLoad` semantics apply;
the binary path comes from the `uv tool install` location (typically
`~/.local/bin/batchalign3` or the venv-managed path). Install with
`sudo launchctl bootstrap system /Library/LaunchDaemons/<plist>` and
verify with `launchctl print system/<label>`.
