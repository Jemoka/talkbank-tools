# Platform Support Matrix

**Status:** Current
**Last updated:** 2026-04-29 10:15 EDT

This page is the Batchalign-only view of platform support. The
operator-facing repo-wide platform-support matrix (covering `chatter`,
Rust core, grammar, VS Code, and both desktop apps) lives outside the
public book.

## CLI + Server (`batchalign3`)

| Platform | Tier | CI | Wheel | Notes |
|----------|------|----|-------|-------|
| Linux x86_64 | A | Tests + typecheck | Yes | Primary CI platform |
| Linux ARM | B | Release wheel | Yes | Cross-compiled |
| macOS ARM (Apple Silicon) | B | Release wheel | Yes | |
| macOS x86_64 (Intel) | B | Release wheel | Yes | |
| Windows x86_64 | B | Release wheel | Yes | Process lifecycle uses Unix APIs (`pre_exec`, `setsid`, `killpg`); server/worker mode not tested on Windows |

## Dashboard (React)

| Platform | Tier | Notes |
|----------|------|-------|
| All (web browser) | B | Built in CI, Playwright smoke tested |

## Batchalign Desktop (experimental)

| Platform | Tier | Notes |
|----------|------|-------|
| macOS / Windows | C (Experimental) | In-repo Tauri shell only; not a supported public release surface |
| Linux | C (Experimental) | No supported public desktop distribution |

This section is about **Batchalign Desktop** in `apps/batchalign/dashboard-desktop/`, not
the separate **Chatter Desktop** validation app in `apps/chatter/chatter-gui/`.

## Tier Definitions

- **Tier A:** Fully CI-gated. Tests run on every PR. Regressions block merge.
- **Tier B:** Release artifacts built. Smoke-tested where possible. Not full CI coverage.
- **Tier C:** Experimental. May build, may not. No guarantees.

## Known Platform Limitations

- Worker process management uses Unix-specific syscalls (`pre_exec`, `setsid`,
  `killpg`). Windows alternatives needed for full Tier A support.
- Bash-only tooling scripts (installer tests, drift checks) need cross-platform
  equivalents.
- `pyproject.toml` classifiers list macOS and Linux only (Windows build-only,
  not supported for server mode).

## Goal

Promote macOS ARM to Tier A by adding platform-specific CI test jobs.
Windows server mode requires porting Unix process lifecycle APIs before
Tier A is feasible.
