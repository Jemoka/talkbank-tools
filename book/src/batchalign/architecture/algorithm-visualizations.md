# Algorithm Visualizations

**Status:** Current
**Last updated:** 2026-05-01 05:25 EDT

The dashboard ships interactive visualizations for retokenization
mapping and DP alignment (both static and live-from-job modes).
Visualizations for the ASR pipeline waterfall and FA timeline are
not yet implemented.

The batchalign3 dashboard includes interactive algorithm visualizations that
show the internal workings of key algorithms — DP alignment, ASR
post-processing, forced alignment timing, and retokenization mapping.  Each
visualization supports two modes:

- **Static mode** — educational, with editable sample data and no server
  required.  TypeScript ports of the Rust algorithms run locally in the browser.
- **Live mode** — shows actual intermediate states from a completed job,
  fetched via the `GET /jobs/{id}/traces` REST endpoint.

## Architecture

```text
┌─────────────────────────────────────┐
│ React Dashboard (apps/batchalign/cli-web-statuspage/)         │
│                                     │
│  /dashboard/visualizations/         │
│    ├── dp-alignment   ──┐           │
│    ├── asr-pipeline   ──┤ Static    │
│    ├── fa-timeline    ──┤ sample    │
│    └── retokenize     ──┘ mode      │
│                                     │
│  /dashboard/jobs/:id/traces/        │
│    ├── dp-alignment   ──┐           │
│    ├── asr-pipeline   ──┤ Live      │
│    ├── fa-timeline    ──┤ job       │
│    └── retokenize     ──┘ mode      │
│                                     │
│  engines/  ← TS ports for static    │
│  mode + rendering logic             │
└──────────────┬──────────────────────┘
               │ REST (live mode)
               ▼
┌─────────────────────────────────────┐
│ Rust Server                         │
│                                     │
│  GET /jobs/{id}/traces              │
│    → JobTraces per file             │
│                                     │
│  Structured results:                │
│    FaResult, MorphosyntaxResult     │
│    always carry intermediate data   │
│                                     │
│  Storage: ephemeral in-memory       │
│    (moka LRU, 50 jobs, 1hr TTL)    │
└─────────────────────────────────────┘
```

## Structured Result Types

Orchestrators return rich result types that always carry intermediate data,
regardless of whether traces are stored.  The dispatch layer decides what to
persist based on the job's `debug_traces` flag.

### FaResult

Returned by `process_fa()` in `crates/batchalign/src/fa/`:

```rust,ignore
pub struct FaResult {
    pub chat_text: String,
    pub groups: Vec<FaGroupTrace>,
    pub pre_injection_timings: Vec<Vec<Option<TimingTrace>>>,
    pub timing_mode: FaTimingMode,
    pub violations: Vec<ViolationTrace>,
}
```

The dispatch layer extracts `chat_text` for file output.  When `debug_traces`
is enabled, it calls `into_timeline_trace()` to build a `FaTimelineTrace` and
stores it via `TraceStore::upsert_file()`.

### MorphosyntaxResult

Returned by `process_morphosyntax()` (single-file path):

```rust,ignore
pub struct MorphosyntaxResult {
    pub chat_text: String,
    pub retokenizations: Vec<RetokenizationInfo>,
}
```

`RetokenizationInfo` is emitted by `inject_results()` in
`batchalign` whenever Stanza retokenization occurs — it captures the
original words, Stanza tokens, and the word-to-token mapping for each affected
utterance.

### Design principle

Previous iterations passed a `debug_traces: bool` parameter through the
orchestrator call chain and conditionally collected trace data alongside the
main output.  This added complexity without benefit — the intermediate data
(groups, timings, retokenization mappings) was already computed as part of
normal processing.

The current design makes the orchestrator API surface richer by default:
structured results always carry the intermediate state.  The `debug_traces`
flag only controls whether the dispatch layer *stores* that data in the
ephemeral trace cache.  This is simpler, avoids parameter threading, and opens
the door to other consumers of the structured data (e.g. detailed error
reports, regression analysis).

## Trace Storage

`TraceStore` wraps a `moka::future::Cache<String, Arc<JobTraces>>` with:

- **Capacity:** 50 jobs (LRU eviction)
- **TTL:** 1 hour per entry
- **Location:** field on `JobStore` (accessible everywhere the store is)
- **Concurrency:** uses moka's `and_upsert_with` for per-key atomic
  read-modify-write — concurrent FA file completions for the same job are
  serialized without blocking unrelated jobs

Traces are diagnostic-only and not persisted to SQLite.

The primary write API is `upsert_file(job_id, file_index, file_traces)` which
atomically gets-or-creates the `JobTraces` entry, inserts the file, and puts
it back.  This is safe to call from multiple concurrent `process_one_fa_file`
tasks within the same job.

### Activation

Per-job: set `"debug_traces": true` in the job submission JSON.

```json
POST /jobs  { "command": "align", "debug_traces": true, ... }
```

### REST Endpoint

```text
GET /jobs/{job_id}/traces
  → 200: JobTraces JSON
  → 404: job not found
  → 204: job exists but no traces collected

GET /jobs/{job_id}/traces/{file_index}
  → 200: FileTraces JSON (single file)
  → 404: file index not found
```

## Trace Data Model

All trace types live in `crates/batchalign/src/types/traces.rs`.

```text
JobTraces
  └── files: BTreeMap<usize, FileTraces>
        ├── filename: String
        ├── dp_alignments: Vec<DpAlignmentTrace>
        ├── asr_pipeline: Option<AsrPipelineTrace>
        ├── fa_timeline: Option<FaTimelineTrace>
        └── retokenizations: Vec<RetokenizationTrace>
```

| Trace type | Source orchestrator | What it captures |
|-----------|-------------------|-----------------|
| `DpAlignmentTrace` | `dp_align.rs` | Full cost matrix, traceback path, alignment result |
| `AsrPipelineTrace` | `transcribe.rs` | 7-stage ASR post-processing intermediates |
| `FaTimelineTrace` | `fa.rs` | Group boundaries, pre/post timings, violations |
| `RetokenizationTrace` | `morphosyntax.rs` | Word↔token mapping per utterance |

## Frontend

### Visualizations

| Visualization | Route (static) | Route (live) | Status |
|--------------|----------------|-------------|--------|
| DP Alignment Explorer | `/dashboard/visualizations/dp-alignment` | `/dashboard/jobs/:id/traces/dp-alignment` | Complete |
| Retokenization Mapper | `/dashboard/visualizations/retokenize` | `/dashboard/jobs/:id/traces/retokenize` | Static complete |
| ASR Pipeline Waterfall | `/dashboard/visualizations/asr-pipeline` | — | Planned |
| FA Timeline | `/dashboard/visualizations/fa-timeline` | — | Planned |

### TypeScript Engine Ports

Static mode uses TypeScript ports of the Rust algorithms located in
`apps/batchalign/cli-web-statuspage/src/engines/`:

| Engine file | Rust source | What it ports |
|-------------|------------|--------------|
| `dpAlignment.ts` | `batchalign/src/dp_align.rs` | `align_small` with step-by-step emission |
| `retokenize.ts` | `batchalign/src/retokenize/mapping.rs` | Word↔token mapping |

These are faithful ports — same algorithm, same cost model, same edge cases —
not approximations.

### Dual-Mode Pattern

Each visualization page accepts a route parameter `/:id` for live mode.  When
present, it fetches traces from the server via `useTraceQuery(id)`.  When
absent, it uses local state and the TS engine for static mode.

```tsx
function DPAlignmentPage() {
  const { id } = useParams();
  const { data: traces } = useTraceQuery(id);  // live mode

  const dpResult = useMemo(() => {
    if (id) {
      // Convert server trace to visualization format
      return traceToResult(traces.dp_alignments[selectedIdx]);
    }
    // Static mode: run TS engine locally
    return alignWithSteps(payload, reference, matchMode);
  }, [id, traces, payload, reference, matchMode]);

  // Same visualization components for both modes
  return <CostGrid ... />;
}
```

### Shared Components

Reusable visualization components in `apps/batchalign/cli-web-statuspage/src/components/visualizations/`:

| Component | Purpose |
|-----------|---------|
| `CostGrid` | SVG grid for DP cost matrix with fill/traceback animation |
| `StepControls` | Play/pause/step/skip controls for stepping through algorithm |
| `ModeToggle` | Static ↔ Live mode indicator |
| `SpanRuler` | Horizontal span bar for retokenization mapping |

## Trace Collection Points

| Orchestrator | What to capture | Where in code |
|-------------|-----------------|---------------|
| `fa.rs` | Group boundaries, pre/post timings, violations | After `parse_fa_response()` and `apply_fa_results()` |
| `morphosyntax.rs` | Retokenization mappings per utterance | Return value of `inject_results()` |
| `transcribe.rs` | ASR pipeline intermediate states | Wrap `process_raw_asr()` stages — not yet wired |
| `dp_align.rs` | Cost matrix + traceback | Optional trace output parameter |

## What ships today

- Visualization routes, landing page, shared components.
- Retokenization engine: TypeScript port + static-mode page.
- DP alignment engine: TypeScript port with step emission;
  `CostGrid` visualization with fill / traceback animation; live
  mode via `useTraceQuery` hook against the server's
  `/jobs/{id}/traces` endpoint.
- Structured results (`FaResult`, `MorphosyntaxResult`) carry
  intermediate data through dispatch.
- FA trace collection and storage in the dispatch layer.

## Not yet implemented

- ASR pipeline waterfall (port 7 ASR post-processing stages to
  TypeScript; `DiffView` component for stage-by-stage transforms;
  trace collection in `transcribe.rs`).
- FA timeline (DAW-style SVG timeline with pan / zoom; FA grouping
  and timing-injection animation; post-processing before / after
  comparison).
