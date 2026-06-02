# Web Dashboard

**Status:** Current
**Last updated:** 2026-05-11 10:55 EDT

The batchalign3 web dashboard is a real-time monitoring interface for watching
job progress, inspecting worker state, and tracking system resource usage.
It opens automatically in your browser when you submit a job (unless you pass
`--no-open-dashboard`).

## Accessing the Dashboard

When the batchalign3 server is running, the dashboard is available at:

```text
http://localhost:8000/dashboard
```

Replace `localhost:8000` with the server's address if running remotely (e.g.,
`http://your-server:8001/dashboard`).

The CLI opens the dashboard automatically when you submit a job:

```bash
batchalign3 transcribe corpus/ -o output/ --lang eng
# ↑ browser opens to /dashboard/jobs/<job-id>
```

To suppress the browser auto-open:

```bash
batchalign3 --no-open-dashboard transcribe corpus/ -o output/ --lang eng
```

## Dashboard Layout

The main dashboard page (`/dashboard`) uses a two-column layout:

```text
┌──────────────────────────────────────────────────────────────┐
│  batchalign   Dashboard   Visualizations       2 active  ●  │
├─────────────────────────────────────┬────────────────────────┤
│                                     │                        │
│  Job List                           │  Workers Panel         │
│  ┌─────────────────────────────┐    │  GPU: 1 process        │
│  │ TRANSCRIBE  ● running       │    │  Stanza: 2 processes   │
│  │ …/corpus/  3/50 files (6%)  │    │  IO: idle              │
│  │ ▓▓▓░░░░░░░░░░░░░░░░░░░░░░  │    │                        │
│  └─────────────────────────────┘    ├────────────────────────┤
│  ┌─────────────────────────────┐    │                        │
│  │ MORPHOTAG  ● completed      │    │  Memory Panel          │
│  │ 74/74 files    2m 15s       │    │  ▓▓▓▓▓▓▓▓▓▓░░░░░  │  │
│  └─────────────────────────────┘    │  148 GB used           │
│  ┌─────────────────────────────┐    │  108 GB available      │
│  │ ALIGN  ✗ failed             │    │  Gate: 2 GB ● safe     │
│  │ 12/15 files  3 failed       │    │                        │
│  └─────────────────────────────┘    ├────────────────────────┤
│                                     │                        │
│                                     │  Vitals                │
│                                     │  42 attempts  1 retry  │
│                                     │                        │
└─────────────────────────────────────┴────────────────────────┘
```

On mobile or narrow screens, the right column stacks below the job list.

## Job List

Each job card shows:

- **Command badge** — color-coded by command type (green for transcribe, indigo
  for align, violet for morphotag, etc.)
- **Status** — queued (amber), running (blue, pulsing), completed (green),
  failed (red), cancelled (gray)
- **Source directory** — abbreviated path to the input files
- **File progress** — `completed/total files (percent)`
- **Workers** — how many concurrent file workers are assigned
- **Duration or age** — elapsed time (running) or "3m ago" (completed)
- **Error count** — if any files failed, shown in red

Click a job card to open the job detail page.

## Job Detail Page

The detail page at `/dashboard/jobs/<id>` shows:

### Header
- Command badge, status dot, job ID
- Action buttons: cancel (if running), restart, delete

### Metadata Grid
- Files: `completed / total (percent)`
- Submitted: relative time ("3m ago")
- Duration: elapsed wall clock
- Workers: concurrent file count

### Progress Bar
For active jobs, an animated progress bar with striped fill. Shows an
indeterminate shimmer when the job is queued but no files have started.

### Command Args
The original submission options as formatted JSON — useful for debugging
which engine, language, and flags produced this output.

### File Table
Every file in the job, grouped by directory. Each row shows:

- **Filename** — just the basename (directory shown as a collapsible group header)
- **Status** — dot + label + optional error category badge
- **Pipeline phase indicator** — for processing files, a 5-segment bar showing
  which pipeline phase is active (Read → Transcribe → Align → Analyze → Finalize)
- **Sub-file progress** — when available, a mini progress bar with counter
  (e.g., "Aligning 3/7")
- **Stage label** — the current processing stage in italic text
- **Duration** — how long this file took (done files)
- **Error detail** — click to expand full error text (error files)

### Error Panel
If files have failed, errors are grouped by error code with counts. Each group
shows the error category (Parse, Media, System, Engine, Pipeline Bug) and
affected filenames.

### Filter Tabs
Filter the file table by status: All, Processing, Done, Error, Queued. A search
box lets you filter by filename.

## Workers Panel

Shows the three worker profiles and their current state:

### GPU Profile (amber)
- Shared ASR + Forced Alignment + Speaker models in one process
- When active: "1 process" with language tags (e.g., `eng shared`)
- Key callout: "Models shared — align + transcribe reuse one process"
- Commands served: align, transcribe, transcribe_s, benchmark

### Stanza Profile (indigo)
- NLP processors (POS tagging, dependency parse, coreference)
- Multiple processes for CPU parallelism (e.g., "2 processes")
- Shows idle/total per language: `eng 1/2 idle`
- Commands served: morphotag, utseg, coref, compare

### IO Profile (emerald)
- Lightweight API/library calls (translation, audio analysis)
- Usually 1 process per language

### Warmup Status
If the server is still loading models on startup, a blue spinner shows
"Warming up models..." until complete.

## Memory Panel

Real-time system RAM usage:

- **Gauge bar** — colored segment showing used vs available memory. Changes
  color based on proximity to the memory gate threshold:
  - **Green** — plenty of headroom (available > 4× threshold)
  - **Amber** — getting close (available between 2× and 4× threshold)
  - **Red** — danger zone (available < 2× threshold)
- **Numbers** — "148 GB used" / "108 GB available"
- **Gate threshold** — shown as a vertical marker on the gauge and a status
  badge (e.g., "Gate: 2 GB threshold")
- **Gate rejections** — if any jobs have been rejected due to memory pressure,
  shown as a red count badge

The memory gate prevents new jobs from starting when available RAM drops below
the configured threshold (default: 2 GB). This protects against OOM crashes
when running large ML models.

## Vitals Panel

Compact operational counters since server start:

| Counter | Color | Meaning |
|---------|-------|---------|
| **crashes** | Red | Worker processes that crashed unexpectedly |
| **forced kills** | Red | Files force-terminated (OOM, stuck) |
| **gate rejects** | Amber | Jobs rejected by the memory gate |
| **attempts** | Gray | Total file processing attempts started |
| **retries** | Amber | Attempts that were retried after transient failures |
| **deferred** | Gray | Work units deferred for later execution |

Only nonzero counters are shown. If everything is healthy, the vitals panel
shows only the attempt count.

## Pipeline Stages

When a file is actively processing, the dashboard shows which **pipeline phase**
it's in using a 5-segment indicator:

| Phase | What's happening | Typical duration |
|-------|-----------------|-----------------|
| **Read** | Loading CHAT, resolving audio, checking cache | Seconds |
| **Transcribe** | ASR inference, timing recovery | Minutes (proportional to audio length) |
| **Align** | Forced alignment on utterance groups | Minutes |
| **Analyze** | Morphosyntax, segmentation, translation, coreference | Seconds to minutes |
| **Finalize** | Post-processing, building CHAT, writing output | Seconds |

Not all files go through all phases — an `align` job skips Transcribe and
Analyze; a `morphotag` job skips Transcribe and Align.

## Connection Status

The header shows a connection indicator:

- **Green dot + "Connected"** — live WebSocket connection to the server. Updates
  stream in real time.
- **Red dot + "Reconnecting..."** — connection lost. The dashboard will
  automatically reconnect with exponential backoff.

When connected, job and file status updates arrive via WebSocket push — no
manual refresh needed. The dashboard also polls the health endpoint every few
seconds to keep the memory and worker panels current.

## Algorithm Visualizations

The dashboard includes interactive algorithm visualizations at
`/dashboard/visualizations`:

- **DP Alignment Explorer** — step through the dynamic programming cost matrix
  used for word alignment, with fill and traceback animation
- **Retokenization Mapper** — see how Stanza word splits/merges are resolved
  back to CHAT words
- **ASR Pipeline Waterfall** — (planned) stage-by-stage ASR post-processing
- **FA Timeline** — (planned) DAW-style forced alignment timing visualization

Each visualization has a **static mode** (editable sample data, no server needed)
and a **live mode** (actual trace data from a completed job with `debug_traces`
enabled).

## Keyboard Shortcuts

The dashboard is mouse-driven. The job detail page supports:
- **Tab** — cycle through filter tabs
- **Arrow keys** — navigate file list pagination

## Tips

- **Frozen progress during batch commands** (morphotag, utseg, translate, coref):
  this is normal. The model processes all files at once, so individual files
  don't advance until the batch completes. The elapsed timer keeps ticking.
- **Multiple concurrent jobs**: the dashboard shows all jobs. Use the status
  filter tabs to focus on active or failed jobs.
- **Error investigation**: click an error row to expand the full error message.
  Error codes link to specific failure categories that help diagnose whether the
  issue is in your input, media files, or the processing engine.
- **Large corpora**: for jobs with hundreds of files, use the search box in the
  file table to find specific files by name.
