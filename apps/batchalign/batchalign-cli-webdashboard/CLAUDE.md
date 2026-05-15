# batchalign3 Frontend — Dashboard + Desktop Processing UI

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Overview

React SPA serving two surfaces:

1. **`/process`** — End-user processing flow for the desktop app. Researchers
   pick a command, choose files, and watch progress without a terminal.
2. **`/dashboard`** — Fleet monitoring for power users. Real-time job status,
   file-level progress, error grouping, server health, and algorithm
   visualizations.

In desktop mode (Tauri webview), `/` redirects to `/process` after any
first-launch setup gate completes.
In web mode (browser), `/` shows the dashboard.

## Tech Stack

(Versions live in `package.json`.)

| Category | Technology |
|----------|-----------|
| Framework | React |
| Language | TypeScript |
| Router | Wouter (lightweight) |
| State | Zustand |
| Data Fetching | TanStack React Query |
| Styling | Tailwind CSS |
| Build | Vite |
| API Types | openapi-typescript (auto-generated from Rust server) |
| Desktop APIs | @tauri-apps/api, @tauri-apps/plugin-dialog, @tauri-apps/plugin-shell |

## Key Commands

```bash
npm run dev              # Dev server (proxies to localhost:8000)
npm run build            # TypeScript check + Vite build
npm run generate:schema  # Regenerate OpenAPI types from Rust server
npm run check:api        # Validate API drift
npm run e2e:install      # Install Playwright deps for apps/batchalign/cli-web-statuspage/e2e
npm run e2e:setup        # Full e2e environment setup (install + browsers)
npm run test:e2e         # Run e2e tests against mock server (no Batchalign binary needed)
npm run test:e2e:headed  # Run e2e tests in headed mode (visible browser)
```

## E2E Testing Entry Points

The dashboard has a unified e2e testing strategy across local development and CI. All paths
route through the same Playwright test files under `apps/batchalign/cli-web-statuspage/e2e/tests/`.

### Test Modes

| Mode | Entry Point | Use Case | Requirements |
|------|---|---|---|
| **Mock server** | `npm run test:e2e` (default) | Fast local iteration, no deps | None (mock server built-in) |
| **Real server** | `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh` | Full integration, real Batchalign binary | Rust binary, Python installation |
| **CI canonical** | `dashboard-e2e` job in `batchalign-python.yml` | Pre-merge gate | Same as real-server |
| **Desktop app** | `npm run test:e2e` from `apps/batchalign/dashboard-desktop/` | Tauri webview integration | Tauri + all deps |

### Local Development

**Quick smoke test (no build, no binary required):**
```bash
cd frontend
npm ci && npm run test:e2e
```

**Full integration with real Batchalign (optional, slow):**
```bash
BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh
```

**Headed mode (watch tests in browser):**
```bash
cd frontend
npm run test:e2e:headed
```

### CI Workflow

The `dashboard-e2e` job in `.github/workflows/batchalign-python.yml`:
1. Builds the wheel from the Batchalign Rust/Python stack
2. Sets `BATCHALIGN_REAL_SERVER_E2E=1` and `BATCHALIGN_PLAYWRIGHT_WITH_DEPS=1`
3. Runs `bash scripts/run_react_dashboard_smoke.sh` which orchestrates:
   - API type generation
   - Frontend build
   - E2E environment setup (Playwright browsers + dependencies)
   - Playwright tests against a real `batchalign3` server instance

This job only runs on main and manual workflow_dispatch (not on all PRs due to performance).

### Orchestration Scripts

**`scripts/run_react_dashboard_smoke.sh`** — canonical orchestration script:
- Generates dashboard API types from Rust OpenAPI spec
- Builds frontend bundle
- Sets up Playwright environment
- Runs tests in mock or real server mode (via `BATCHALIGN_REAL_SERVER_E2E` env var)
- Used by both local developers and CI

**`scripts/build_react_dashboard.sh`** — deployment script:
- Generates API types
- Builds frontend
- Copies built artifacts to target directory (default: `~/.batchalign3/dashboard`)

**`scripts/check_dashboard_api_drift.sh`** — validation gate:
- Ensures `openapi.json` and `apps/batchalign/cli-web-statuspage/src/generated/api.ts` are in sync
- Fails if generated artifacts are stale
- Part of both Makefile and CI gates

## Comment Discipline

All new and modified TypeScript files must explain their architectural role in
the code itself:

- file-level comments for modules that own routing, controller logic, state sync,
  or runtime detection
- JSDoc on exported hooks, components, and helpers
- inline comments where ownership boundaries or cache/store synchronization would
  otherwise be surprising to a new contributor

## Project Structure

```
apps/batchalign/cli-web-statuspage/
├── src/
│   ├── app.tsx            # Composition root: fleet sync + first-launch desktop gate
│   ├── main.tsx           # Entry point (React Query + DesktopProvider)
│   ├── AppRoutes.tsx      # Route table: /process, /dashboard, /dashboard/jobs/:id, visualizations
│   ├── state.ts           # Zustand store (jobs, health, WebSocket status)
│   ├── api.ts             # REST client (fetchJobs, submitJob, fetchHealth, etc.)
│   ├── ws.ts              # WebSocket client (auto-reconnect, multi-server)
│   ├── query.ts           # React Query config (retry policy, stale times)
│   ├── runtime.ts         # Server URL detection + desktop environment check
│   ├── utils.ts           # Formatting helpers (duration, status colors, command styles)
│   ├── types.ts           # Re-exported OpenAPI types + client-side enrichments
│   ├── generated/api.ts   # Auto-generated OpenAPI types (DO NOT EDIT)
│   │
│   ├── lib/
│   │   └── tauri.ts       # Low-level Tauri adapter (dynamic imports, protocol dispatch, browser fallbacks)
│   │
│   ├── desktop/
│   │   ├── protocol.ts       # Raw Tauri command/event names + paired transport payload types
│   │   ├── capabilities.ts   # Narrow capability interfaces: environment, files, config, server
│   │   └── DesktopContext.tsx  # React provider exposing focused desktop capability hooks
│   │
│   ├── components/
│   │   ├── Layout.tsx     # Header (nav, stats, connection status) + main wrapper
│   │   ├── process/       # End-user processing flow components
│   │   │   ├── CommandPicker.tsx        # Command card grid
│   │   │   ├── FolderPicker.tsx         # Native folder picker via Tauri dialog
│   │   │   ├── OutputModeSelector.tsx   # Separate folder vs in-place toggle
│   │   │   ├── ProcessForm.tsx          # Main form (command → configure → processing)
│   │   │   ├── ProcessingProgress.tsx   # SSE-driven live file progress
│   │   │   ├── RecentJobs.tsx           # Compact recent jobs list
│   │   │   ├── ServerStatusBar.tsx      # Server status dot + start/stop controls
│   │   │   ├── ErrorRecovery.tsx        # Structured error messages + suggested actions
│   │   │   ├── OnboardingOverlay.tsx    # First-time 3-step guide (dismissible)
│   │   │   └── HelpPanel.tsx            # Slide-out panel: command descriptions + FAQ
│   │   ├── setup/           # First-time setup wizard
│   │   │   ├── SetupWizard.tsx          # Multi-step: welcome → engine → API key → done
│   │   │   └── EngineCard.tsx           # Rev.AI vs Whisper card with pros/cons
│   │   ├── JobCard.tsx, JobList.tsx, JobDetailPageView.tsx  # Dashboard job views
│   │   ├── FileTable.tsx, FilterTabs.tsx, PaginatedFileList.tsx  # File-level views
│   │   ├── ErrorPanel.tsx, ErrorCodeGroup.tsx  # Error display
│   │   ├── StatusBadge.tsx, ProgressBar.tsx, StatsRow.tsx  # Shared UI
│   │   ├── PipelineStageBar.tsx       # Pipeline progress indicator (Read→Transcribe→Align→Analyze→Finalize)
│   │   ├── WorkerProfilePanel.tsx     # Worker profile status (GPU/Stanza/IO) from health
│   │   ├── MemoryPanel.tsx            # System RAM gauge with gate threshold
│   │   ├── VitalsRow.tsx              # Operational counter badges (crashes, retries, etc.)
│   │   └── visualizations/  # Algorithm trace visualizations
│   │
│   ├── hooks/
│   │   ├── useFleetDashboardSync.ts  # Fleet bootstrap + WebSocket live sync
│   │   ├── useServerLifecycle.ts     # Server auto-start, status tracking, start/stop (desktop)
│   │   ├── useServerHealth.ts        # Health polling for process flow
│   │   ├── useSubmitJob.ts           # React Query mutation for POST /jobs
│   │   ├── useJobStream.ts           # SSE EventSource wrapper for job progress
│   │   ├── useJobsQueries.ts         # Per-server job list queries
│   │   ├── useJobPageController.ts   # Job detail route controller
│   │   ├── useJobDetailQuery.ts      # Per-server cached detail payload
│   │   ├── useJobLookupQuery.ts      # Multi-server job discovery
│   │   ├── useFileFilters.ts         # File status filter state
│   │   └── useTraceQuery.ts          # Trace visualization data
│   │
│   ├── pages/
│   │   ├── ProcessPage.tsx    # /process route shell (command picker + recent jobs)
│   │   ├── DashboardPage.tsx  # /dashboard route shell
│   │   └── JobPage.tsx        # /dashboard/jobs/:id route shell
│   │
│   ├── liveSync/
│   │   └── handleDashboardMessage.ts  # WebSocket → Zustand + React Query sync
│   │
│   └── engines/            # Client-side trace simulation (DP alignment, retokenize)
│
├── package.json
└── vite.config.ts
```

## Data Flow

### Dashboard (fleet monitoring)

1. **Init**: Resolve the server URL from runtime config
2. **WebSocket**: Connect to the server and receive snapshot + real-time updates
3. **REST**: React Query fetches job lists and details
4. **State**: Zustand store tracks dashboard and connection state
5. **Updates**: WebSocket patches store + query cache in real time

Detail pages follow the same split:

- `pages/JobPage.tsx` is the route shell
- `hooks/useJobPageController.ts` owns job lookup, server resolution, and store sync
- `components/JobDetailPageView.tsx` owns detail presentation and file filters

### Process flow (desktop)

1. **Setup**: `App` checks `useDesktopConfig().isFirstLaunch()` and shows
   `SetupWizard` before routes load
2. **Server**: `useServerLifecycle` reads `useDesktopServer().serverStatus()`,
   subscribes to `desktop://server-status-changed`, and auto-starts via
   `useDesktopServer().startServer()`
3. **Health**: `useServerHealth` polls `GET /health` on a fixed interval
4. **Command**: User picks from the command card grid
5. **Files**: `useDesktopFiles().pickFolder()` → native dialog →
   `useDesktopFiles().discoverFiles()` → file list
6. **Submit**: `useSubmitJob` sends `POST /jobs` with `paths_mode: true`
7. **Progress**: `useJobStream` opens SSE to `/jobs/{id}/stream` for live updates
8. **Complete**: `useDesktopFiles().openPath()` reveals the output folder in
   Finder/Explorer

## Desktop Runtime Seam

- `runtime.ts` owns environment detection only.
- `desktop/protocol.ts` inventories raw command/event identifiers and keeps the
  transport request/response types visibly paired.
- `lib/tauri.ts` is the low-level adapter: dynamic Tauri imports, protocol
  dispatch, and browser fallbacks.
- `desktop/DesktopContext.tsx` fans that adapter out into focused React hooks:
  `useDesktopEnvironment()`, `useDesktopFiles()`, `useDesktopConfig()`, and
  `useDesktopServer()`.
- `main.tsx` must keep `DesktopProvider` above the app tree so desktop and web
  mode share one explicit runtime boundary.

## Key Patterns

- **Server-qualified keys**: `${server}|${job_id}` keeps retained aggregation
  paths collision-safe even though the released surface is single-server-first
- **WebSocket resilience**: Independent connections, exponential backoff reconnect
- **OpenAPI sync**: `npm run generate:schema` keeps TypeScript types in sync with Rust
- **React Compiler**: Babel plugin for automatic memoization
- **Desktop runtime seam**: Components/hooks consume the smallest possible
  capability hook (`useDesktopEnvironment`, `useDesktopFiles`,
  `useDesktopConfig`, `useDesktopServer`). Keep raw `@tauri-apps/*` imports and
  command/event names in `lib/tauri.ts` + `desktop/protocol.ts`, and only
  extend one capability when a new desktop-only surface is truly needed.

## Deployment

Built SPA is served by `batchalign-server` via `ServeDir` with SPA fallback
(all routes serve `index.html`, client-side routing handles the rest).

For desktop: Tauri bundles the built SPA into a native app via
`apps/batchalign/dashboard-desktop/`.
