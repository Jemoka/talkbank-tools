# Dashboard E2E Test Entry Points

**Status:** Current
**Last updated:** 2026-04-29 13:54 EDT

This document provides a unified reference for all dashboard e2e test entry points across local development, CI workflows, and orchestration scripts.

## Quick Start

**Fast mock-server test (no Batchalign binary needed):**
```bash
cd frontend
npm ci && npm run test:e2e
```

**Full integration test with real server:**
```bash
BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh
```

**From CI/automated flows:**
```bash
bash scripts/run_react_dashboard_smoke.sh
```

## Entry Point Map

All entry points ultimately route through the same Playwright test files under `apps/batchalign/cli-web-statuspage/e2e/tests/`:
- `apps/batchalign/cli-web-statuspage/e2e/tests/mock-server.spec.mjs` — tests against a mock HTTP/WebSocket server
- `apps/batchalign/cli-web-statuspage/e2e/tests/real-server.spec.mjs` — tests against the actual Batchalign binary

### By Context

| Context | Entry Point | Command | Dependencies | CI? | Notes |
|---------|---|---|---|---|---|
| **Local dev (quick)** | npm from frontend | `npm run test:e2e` | None | ❌ | Mock server mode, < 1min |
| **Local dev (full)** | Makefile | `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh` | Rust bin, Python | ❌ | Real server, ~5-10min |
| **Local dev (headed)** | npm from frontend | `npm run test:e2e:headed` | None | ❌ | Visual browser, interactive |
| **Desktop app CI** | npm from apps/dashboard-desktop | `npm run test` | Tauri + deps | ✓ | Via batchalign-desktop.yml |
| **Dashboard CI (main only)** | CI workflow | `dashboard-e2e` job | Pre-built wheel | ✓ | Via batchalign-python.yml |
| **Scripts/deploys** | Bash script | `bash scripts/run_react_dashboard_smoke.sh` | Env-controlled | ❌ | Orchestration layer |

## Test Modes

### 1. Mock Server (Default, Fast)

**Entry:** `npm run test:e2e` (from `apps/batchalign/cli-web-statuspage/`)

**Orchestration:**
- Generates API types
- Builds frontend
- Installs Playwright browsers
- Runs tests against an in-process mock HTTP+WebSocket server

**Test files:** `apps/batchalign/cli-web-statuspage/e2e/tests/mock-server.spec.mjs`

**Use cases:**
- Local iteration (no binary needed)
- Quick regression checking
- Pre-merge smoke test

**Environment:** None required

### 2. Real Server (Comprehensive)

**Entry:** `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh` or `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh`

**Orchestration:**
- Generates API types
- Builds frontend
- Builds/locates Batchalign release binary
- Installs Playwright browsers + system dependencies (`--with-deps`)
- Spawns real `batchalign3` server
- Runs tests against the real server

**Test files:** `apps/batchalign/cli-web-statuspage/e2e/tests/real-server.spec.mjs`

**Use cases:**
- Pre-PR comprehensive validation
- Integration testing
- Detecting server-side regressions

**Environment:**
- `BATCHALIGN_REAL_SERVER_E2E=1` — enables real server mode
- `BATCHALIGN_PLAYWRIGHT_WITH_DEPS=1` — install Playwright with OS deps (required in CI)
- `BATCHALIGN_BIN` — optional override for binary path
- `BATCHALIGN_PYTHON` — optional override for Python executable

## Orchestration Scripts

### `scripts/run_react_dashboard_smoke.sh`

**Purpose:** Canonical e2e orchestration used by both CI and local developers.

**Flow:**
1. Generate dashboard API types (via `generate_dashboard_api_types.sh`)
2. Install frontend dependencies (`npm ci`)
3. Build frontend (`npm run build`)
4. Install e2e dependencies (`npm ci` in e2e/)
5. Optionally install Playwright browsers
6. Run tests in mock or real server mode

**Usage:**
```bash
# Mock server (default)
bash scripts/run_react_dashboard_smoke.sh

# Real server
BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh

# Skip browser install (already cached)
BATCHALIGN_SKIP_BROWSER_INSTALL=1 bash scripts/run_react_dashboard_smoke.sh

# Skip API generation
BATCHALIGN_SKIP_API_SYNC=1 bash scripts/run_react_dashboard_smoke.sh
```

**Used by:**
- Local: `bash scripts/run_react_dashboard_smoke.sh` and `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh`
- CI: `dashboard-e2e` job in `batchalign-python.yml`

### `scripts/generate_dashboard_api_types.sh`

**Purpose:** Generates TypeScript types from the Rust OpenAPI spec.

**Flow:**
1. Runs `cargo run -p batchalign-cli -- openapi` to get latest schema
2. Converts to `apps/batchalign/cli-web-statuspage/openapi.json`
3. Uses `openapi-typescript` to generate `apps/batchalign/cli-web-statuspage/src/generated/api.ts`

**Used by:**
- `run_react_dashboard_smoke.sh`
- `check_dashboard_api_drift.sh`
- Manual: `npm run generate:schema` (frontend)

### `scripts/check_dashboard_api_drift.sh`

**Purpose:** Validation gate — ensures generated artifacts are in sync.

**Flow:**
1. Regenerates API types
2. Diffs against git to detect staleness

**Used by:**
- `bash scripts/check_dashboard_api_drift.sh`
- CI workflows (`batchalign-python.yml`, `batchalign-desktop.yml`)

### `scripts/build_react_dashboard.sh`

**Purpose:** Deployment script for dashboard assets.

**Flow:**
1. Generates API types (unless `BATCHALIGN_SKIP_API_SYNC=1`)
2. Installs frontend deps
3. Builds frontend
4. Copies to target directory (default: `~/.batchalign3/dashboard`)

**Usage:**
```bash
bash scripts/build_react_dashboard.sh                    # Deploy to ~/.batchalign3/dashboard
bash scripts/build_react_dashboard.sh /custom/path       # Deploy to custom path
BATCHALIGN_SKIP_API_SYNC=1 bash scripts/build_react_dashboard.sh  # Skip API gen
```

**Used by:**
- `just batchalign dashboard` (build only, no deploy)
- Manual deployment workflows

## Makefile Targets

### `batchalign-dashboard-api-check`

Verify API artifacts are in sync.

```bash
bash scripts/check_dashboard_api_drift.sh
```

### `batchalign-dashboard-build`

Build dashboard frontend.

```bash
just batchalign dashboard
```

### `batchalign-dashboard-e2e`

Run e2e tests with mock server (quick, no binary).

```bash
bash scripts/run_react_dashboard_smoke.sh
```

**Equivalent to:**
```bash
bash scripts/run_react_dashboard_smoke.sh
```

### `batchalign-dashboard-e2e-real`

Run e2e tests with real Batchalign server (comprehensive).

```bash
BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh
```

**Equivalent to:**
```bash
cd frontend && npm ci
cd apps/batchalign/cli-web-statuspage/e2e && npm ci && npm run install:browsers
BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh
```

## npm Scripts (from `apps/batchalign/cli-web-statuspage/`)

```bash
npm run dev              # Dev server (proxies to localhost:8000)
npm run build            # TypeScript check + Vite build
npm run generate:schema  # Regenerate OpenAPI types from Rust spec
npm run check:api        # Validate API drift (fails if stale)
npm run e2e:install      # Install Playwright
npm run e2e:browsers     # Install Playwright browsers only
npm run e2e:setup        # Full setup (install + browsers)
npm run test:e2e         # Run tests (mock server)
npm run test:e2e:headed  # Run tests in headed mode (visual browser)
```

## CI Workflows

### `batchalign-python.yml` — `dashboard-e2e` job

**Triggers:** `main` branch or manual `workflow_dispatch`

**Steps:**
1. Free disk space
2. Set up Python 3.12 + uv
3. Download pre-built wheel
4. Set up Node.js 20
5. Run: `bash scripts/run_react_dashboard_smoke.sh` with `BATCHALIGN_REAL_SERVER_E2E=1`

**Key env vars:**
- `BATCHALIGN_PLAYWRIGHT_WITH_DEPS=1` — install Playwright with OS dependencies
- `BATCHALIGN_REAL_SERVER_E2E=1` — use real server mode
- `BATCHALIGN_PYTHON` — set to uv-managed Python executable

**Result:** Tests dashboard against real Batchalign server built from main branch.

### `batchalign-desktop.yml` — Desktop bundle workflow

**Note:** Currently builds desktop bundles but does not run e2e tests. The `apps/batchalign/dashboard-desktop/` directory has its own e2e setup if needed.

## Decision Tree: Which Entry Point to Use?

```
Need to test dashboard locally?
├─ Yes, quick iteration (< 1 min)
│  └─ npm run test:e2e
├─ Yes, comprehensive (with real server)
│  └─ BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh
├─ Yes, visual debugging
│  └─ npm run test:e2e:headed
└─ For CI automation
   └─ bash scripts/run_react_dashboard_smoke.sh (with env vars)

Need to build dashboard for deployment?
├─ Yes, local dev
│  └─ npm run build
├─ Yes, deploy to ~/.batchalign3/dashboard
│  └─ bash scripts/build_react_dashboard.sh
└─ Yes, full validation
   └─ just batchalign dashboard

Need to validate API sync?
├─ Yes, quick check
│  └─ npm run check:api
├─ Yes, strict gate
│  └─ bash scripts/check_dashboard_api_drift.sh
└─ Yes, regenerate types
   └─ npm run generate:schema
```

## Troubleshooting

**"batchalign3 binary not found"**
- Mock mode doesn't need it. Use `npm run test:e2e`
- Real mode: build with `cargo build --release -p batchalign-cli`
- Or set `BATCHALIGN_BIN=/path/to/binary` before running

**"Playwright browsers not installed"**
- Run `npm run e2e:setup` from `apps/batchalign/cli-web-statuspage/`
- Or `BATCHALIGN_REAL_SERVER_E2E=1 bash scripts/run_react_dashboard_smoke.sh` (includes setup)

**"API types are stale"**
- Run `npm run generate:schema` (frontend)
- Or `bash scripts/check_dashboard_api_drift.sh` (with strict validation)

**"Tests hang or timeout"**
- Headed mode: use `npm run test:e2e:headed` to see what's happening
- Add `--headed` flag directly: `npm --prefix apps/batchalign/cli-web-statuspage/e2e run test -- --headed`
- Check Playwright configuration: `apps/batchalign/cli-web-statuspage/e2e/playwright.config.mjs`

## Related Documentation

- **Frontend architecture & tech stack:** `apps/batchalign/cli-web-statuspage/CLAUDE.md`
- **Full test files:** `apps/batchalign/cli-web-statuspage/e2e/tests/`
- **Contributing guide:** `CONTRIBUTING.md` (section on dashboard changes)
- **Batchalign-specific e2e tests:** `batchalign/tests/cli/test_cli_e2e.py`
- **Worker LazyProfile e2e:** `scripts/test_lazy_profile_e2e.sh`
