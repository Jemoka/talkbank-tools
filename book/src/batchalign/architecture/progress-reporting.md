# Progress Reporting

**Status:** Current
**Last updated:** 2026-04-14 09:54 EDT

The server reports per-file progress to all connected clients (CLI, TUI, React
dashboard) in real time. This chapter covers the data model, data flow, and how
to add progress reporting to new commands.

**Scope.** This page covers **per-file** progress — the stage codes and
sub-file counters emitted by each file's orchestrator. **Batch-level**
progress (per-language-group utterance counters for cross-file batched
commands like morphotag, utseg, translate, coref) is documented separately
in [Observability Architecture](observability.md#per-language-group-batch-progress),
including the per-language tagger (`morphosyntax/worker.rs::infer_batch`)
that rewrites worker progress events before the drain loop keys them. The
two surfaces co-exist: each file shows its own stage here, and the batch
as a whole shows language-group counters there.

## Data Model

Four fields on `FileStatus` carry progress information. Three are ephemeral
in-memory fields, and one is the derived display label exposed at the API
edge:

| Field | Type | Purpose |
|-------|------|---------|
| `progress_stage` | `Option<FileProgressStage>` | Stable machine-readable stage code |
| `progress_label` | `Option<String>` | Human-readable label derived from `progress_stage` |
| `progress_current` | `Option<i64>` | Current counter (e.g. group 3) |
| `progress_total` | `Option<i64>` | Total items (e.g. 7 groups) |

The typed stage and numeric counters are **never persisted to SQLite** — they
exist only in the in-memory `JobStore` and are broadcast via WebSocket. They
are cleared automatically when a file reaches a terminal state (Done or Error).
`progress_label` is not stored independently; the server derives it from the
typed stage when projecting the API response.

## Data Flow

```text
Orchestrator (fa.rs, transcribe pipeline, etc.)
  → ProgressSender (unbounded channel)
    → Forwarder task (spawned per file)
      → set_file_progress() — updates FileStatus with `progress_stage` + calls notify_file()
        → WebSocket broadcast → all connected clients
```

The CLI TUI consumes the same progress stream through a reducer boundary
instead of shared UI state:

```mermaid
flowchart LR
    poll["CLI poll loop"] --> sink["TuiProgress"]
    sink --> queue["Unbounded TuiUpdate queue"]
    queue --> runtime["TuiRuntime"]
    runtime --> state["AppState reducer"]
    state --> draw["ratatui draw"]
```

## Two Tiers of Progress

### Tier 1: Stage Codes (dispatch layer)

The dispatch layer sets a typed `FileStage` at lifecycle transitions. Every
processing file shows at least a stage name ("Reading", "Resolving audio",
"Aligning", "Writing"), but the label is derived later from the stage code.
No orchestrator changes needed.

`set_file_progress()` in `runner/util.rs` is the helper:

```rust,ignore
set_file_progress(store, job_id, filename, FileStage::Aligning, None, None).await;
```

### Tier 2: Sub-file Numeric Progress (orchestrator)

Orchestrators report fine-grained progress via a `ProgressSender` channel.
The dispatch layer creates the channel with `spawn_progress_forwarder()` and
passes the sender to the orchestrator.

```rust,ignore
let progress_tx = spawn_progress_forwarder(store.clone(), job_id, filename);

process_fa(..., Some(&progress_tx)).await;
```

Inside the orchestrator:

```rust,ignore
if let Some(tx) = progress {
    let _ = tx.send(ProgressUpdate::new(
        FileStage::Aligning,
        Some(3),
        Some(7),
    ));
}
```

## Per-Command Progress Stages

### align (forced alignment)

| Stage | Label | current/total |
|-------|-------|---------------|
| Mark processing | "Reading" | — |
| Read CHAT | "Resolving audio" | — |
| UTR pre-pass (partial) | "Recovering utterance timing" | 1/W, 2/W, ... W/W windows |
| UTR pre-pass (full-file) | "Recovering utterance timing" | 0/1 |
| Audio resolved | "Aligning" | — |
| Cache check | "Checking cache" | 0/N groups |
| Cache partition | "Aligning" | hits/N groups |
| Each group done | "Aligning" | done/N groups |
| Apply results | "Applying results" | N/N |
| Write output | "Writing" | — |

### transcribe

| Stage | Label | current/total |
|-------|-------|---------------|
| Mark processing | "Resolving audio" | — |
| Audio resolved | "Transcribing" | — |
| ASR inference | "Transcribing" | 0/total_stages |
| Post-processing | "Post-processing" | 1/total_stages |
| Build CHAT | "Building CHAT" | 2/total_stages |
| Optional utseg | "Segmenting utterances" | 3/total_stages |
| Optional morphosyntax | "Analyzing morphosyntax" | 4/total_stages |
| Finalize | "Finalizing" | N/total_stages |
| Write output | "Writing" | — |

### morphotag / utseg / translate / coref (batched)

| Stage | Label | current/total |
|-------|-------|---------------|
| Mark processing | Command-specific label | — |
| Read each file | "Reading" | — |
| Pre-batch count | Command-specific label | 0/N files |
| Orchestrator running | (same label) | 0/N (frozen) |
| Write each result | "Writing" | 1/N, 2/N, ... N/N |

Labels by command: morphotag → "Analyzing", utseg → "Segmenting",
translate → "Translating", coref → "Resolving coreference",
compare → "Comparing".

The batch total is published before inference starts so the frontend can show
how many files are in the batch, even though individual files don't advance
during the inference call. After inference, each file transitions to "Writing"
with a per-file counter as results are saved to disk.


| Stage | Label |
|-------|-------|
| Audio prep and conversion | "Resolving audio" |
| Worker request running | "Processing" |
| Writing output artifact | "Writing" |

### benchmark (Rust-owned benchmark pipeline)

| Stage | Label |
|-------|-------|
| Mark processing | "Resolving audio" |
| Rust benchmark orchestrator running | "Benchmarking" |
| Writing output artifacts | "Writing" |

## What Users See

### CLI (indicatif)

```text
  [=====>                  ] 3/50 files  [00:42]
  ⠋ align: Aligning 5/12
```

### TUI (ratatui)

```text
  morphotag — 3/50 files  3✓ 2⠋ 1✗ 44·  [00:42]  ~03:15
  Workers: infer:asr:eng · infer:morphosyntax:eng    Warmup: complete
  Memory: [████████████░░░░░░░░] 148/256 GB   Gate: 2 GB ● safe

  ⠋ corpus001.cha              ●●●○○  Aligning  5/12  1:23
  ⠋ corpus002.cha              ●○○○○  Resolving audio  0:05
  ✓ corpus003.cha                                      2.1s
  · corpus004.cha
  ▼ 42 more below
```

The TUI render thread owns the full `AppState`, grouped into progress,
directory-view, error-panel, metrics, and interaction sub-state. Polling code
only sends typed `TuiUpdate` messages, so rendering and navigation state are
not shared behind a mutex.

**Header:** Status breakdown (`3✓ 2⠋ 1✗ 44·`), elapsed time, and ETA
(throughput-based `~MM:SS`). On completion, shows "Done!" or "Done — N failed".

**Pipeline phase dots** — processing file rows show a 5-dot indicator
(`●●○○○`) using the same phase mapping as the React `PipelineStageBar`.
Completed phases are green, the active phase is cyan, and future phases are
gray. Dots only appear when the server reports a typed `progress_stage`.

**Per-file elapsed** — processing files show a running `M:SS` timer from
`started_at`, helping spot stuck files.

**Scroll indicators** — `▲ N more above` / `▼ N more below` at group edges.

**Auto-collapse** — non-focused all-terminal groups show condensed titles.

**Error codes** — error panel entries include structured codes from poll data.

**Gate warning** — memory gauge warns when near or below gate threshold.

**Health metrics** — the TUI polls `GET /health` every ~5 seconds (slower
than the job status poll) and renders two rows between the header gauge and
the directory groups:

- **Worker line**: lists active `live_worker_keys` and warmup status.
- **Memory gauge**: 20-character bar with used/total GB and gate proximity
  coloring (green >4×, yellow 2-4×, red <2× headroom above gate threshold).

The `m` key toggles the metrics rows. The `ProgressSink` trait has an
`update_health()` method (default no-op) that `TuiProgress` implements to
forward `HealthResponse` into the reducer as a `TuiUpdate::HealthSnapshot`.

### React Dashboard

The dashboard (`apps/batchalign/cli-web-statuspage/`) consumes progress data via both WebSocket push
(real-time `file_update` events) and REST polling (health endpoint for system
panels). It renders several distinct progress surfaces:

#### File-Level Progress (FileTable)

In `apps/batchalign/cli-web-statuspage/src/components/FileTable.tsx`, each processing file row shows:

- **Pipeline phase indicator** (`PipelineStageBar`) — 5 compact segments
  mapping the 23 `FileProgressStage` variants to visual phases:
  Read → Transcribe → Align → Analyze → Finalize. The active segment pulses
  using the existing `status-dot-pulse` CSS animation. Completed phases are
  filled; future phases are gray. Component: `apps/batchalign/cli-web-statuspage/src/components/PipelineStageBar.tsx`.
- **Label-only stages**: italic text next to the status dot
- **Label + counter**: inline blue mini-bar with counter (e.g., "Aligning 3/7")
- **Indeterminate shimmer**: shown for batched commands while no files have
  completed, proving the app is alive during the frozen inference window
- **Stage-specific hints**: subtle italic text explaining *why* a stage is slow
  (e.g., "Rev.AI runs roughly in real-time"). Defined in `stageHint()` in
  `ProcessingProgress.tsx`.
- **Elapsed timer**: always visible while running, ticks every second

#### Dashboard System Panels

The main dashboard page (`/dashboard`) uses a two-column layout. The right
column stacks three system-health panels:

- **WorkerProfilePanel** (`apps/batchalign/cli-web-statuspage/src/components/WorkerProfilePanel.tsx`) —
  parses `live_worker_keys` strings from the health endpoint into profile
  summaries (GPU/Stanza/IO). Shows active/idle counts, languages, engine
  overrides, and a model-sharing callout for the GPU profile. Also shows
  warmup status.

- **MemoryPanel** (`apps/batchalign/cli-web-statuspage/src/components/MemoryPanel.tsx`) — displays system
  RAM usage from the health endpoint fields `system_memory_total_mb`,
  `system_memory_available_mb`, `system_memory_used_mb`. Shows a segmented
  gauge bar with the `memory_gate_threshold_mb` marked as a vertical line.
  Color-codes proximity to the gate threshold (green/amber/red) and shows
  cumulative gate rejection count.

- **VitalsRow** (`apps/batchalign/cli-web-statuspage/src/components/VitalsRow.tsx`) — compact badges for
  operational counters: `worker_crashes`, `forced_terminal_errors`,
  `memory_gate_aborts`, `attempts_started`, `attempts_retried`,
  `deferred_work_units`. Only nonzero counters render. Error counters are red,
  warnings amber, throughput counters gray.

#### Health Endpoint Memory Fields

The `HealthResponse` struct exposes system memory data for the dashboard:

```rust,ignore
pub system_memory_total_mb: u64,      // sysinfo::total_memory()
pub system_memory_available_mb: u64,  // sysinfo::available_memory()
pub system_memory_used_mb: u64,       // total - available
pub memory_gate_threshold_mb: u64,    // from ServerConfig
```

These are queried fresh on each `GET /health` call via `sysinfo::System`. On
macOS, `available_memory()` returns only free + purgeable (not inactive), which
can undercount effective availability. The dashboard shows the raw values
without correction.

#### Stage Type Contract

The dashboard should treat `progress_stage` as the stable contract field.
`progress_label` exists so the UI can render operator-facing text without
copying label-generation logic into every client, but client branching should
key off the typed stage whenever possible.

## Per-Command Progress Expectations (Developer Reference)

When adding progress to a new command, consider:

- **Batched commands** (text NLP): Reading → pre-batch 0/N → inference (frozen) →
  Writing 1/N..N/N. The pre-batch count lets the frontend show the batch size.
  Individual files appear frozen during inference because the model processes
  them all at once.

- **Per-file commands** (align, transcribe): Each file progresses independently
  through its own stages. Use `spawn_progress_forwarder()` for sub-file
  counters. Report meaningful milestones (group completion, window completion)
  rather than every small step.

- **Long sub-stages** (UTR, transcription): If a sub-stage takes more than a
  few seconds, pass a `ProgressSender` so it can report sub-progress. Even
  0/1 for a single-unit operation is better than nothing — it tells the
  frontend which stage is active and enables stage-specific hint text.

- **Stage hints**: The React dashboard shows contextual hints (e.g., "Rev.AI
  runs roughly in real-time") for known slow stages. When adding a new slow
  stage, add a corresponding hint in `stageHint()` in
  `ProcessingProgress.tsx`.

## Adding Progress to a New Command

1. **Tier 1**: Add `set_file_progress()` calls in the dispatch function at
   stage transitions.

2. **Tier 2** (if the command has long-running per-file work):
   - Add `progress: Option<&ProgressSender>` to the orchestrator signature
   - Call `spawn_progress_forwarder()` in the dispatch layer
   - Send `ProgressUpdate` at meaningful points inside the orchestrator
