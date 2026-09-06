# Dashboard Architecture

**Status:** Current
**Last updated:** 2026-05-01 17:07 EDT

The Batchalign dashboard is one React UI shipped two ways: the web
dashboard served by the Rust control plane, and a desktop operator
app via Tauri that hosts the same React code. Rust OpenAPI is the
canonical dashboard contract source.

## Two delivery surfaces, one UI

```text
┌──────────────────────────────────────┐
│  Same React codebase (one UI)        │
└──────────────────────────────────────┘
            │                 │
            ▼                 ▼
   Web dashboard      Tauri desktop shell
   (served by         (apps/dashboard-desktop)
    Rust server)
            │                 │
            ▼                 ▼
   ┌──────────────────────────────────────┐
   │  Rust control plane (batchalign)     │
   │  OpenAPI = canonical contract        │
   └──────────────────────────────────────┘
```

## React layer

React is the canonical dashboard UI implementation. The web
dashboard is the supported public surface; the Tauri shell is a
desktop wrapper around the same React code with a
researcher-friendly processing flow.

The Tauri side is intentionally thin — plugins + one custom
command. All UI logic lives in React. Reasons:

- One UI codebase across web and desktop.
- Mature ecosystem for web UI quality, testing, and observability.
- Strong desktop packaging / update path via Tauri with minimal
  bespoke runtime code.

## Tauri shell features

The desktop processing flow (`apps/dashboard-desktop`) provides:

- Command picker.
- Native folder dialog.
- Job submission with `paths_mode`.
- SSE-driven progress.
- "Open output folder" action.
- Server auto-start on launch, auto-stop on exit.
- Status bar with manual start / stop controls.
- First-time setup wizard (engine selection + Rev.AI key) matching
  batchalign2's mandatory `interactive_setup()` gate.

The CLI also gates processing commands on `~/.batchalign.ini`
existence, matching the BA2 behavior.

## Rust OpenAPI contract

The dashboard contract is generated from Rust types. Both the web
dashboard and the Tauri shell consume the same OpenAPI spec, so
schema drift between the two delivery surfaces is impossible by
construction.

## Scope rules

- React is the dashboard feature-development target.
- The web dashboard is the supported current public surface.
- The Tauri shell provides the end-user processing flow
  (`/process` route) for researchers who aren't comfortable with
  terminals.
- The Tauri side stays thin (plugins + one custom command); all UI
  logic lives in React.
- Rust-only end-to-end UI stacks are not pursued — adding
  Node/TypeScript toolchain ownership is the conscious tradeoff
  for ecosystem maturity.

## Source layout

| Path | Role |
|---|---|
| `apps/batchalign/cli-web-statuspage/` | React dashboard sources (served by `batchalign` server) |
| `apps/batchalign/dashboard-desktop/` | Tauri shell (rust-tauri + React UI build) |
| `crates/batchalign/src/server/openapi.rs` | OpenAPI schema generation |

For the chatter desktop app (a separate Tauri product, not the
batchalign dashboard), see
[`apps/chatter/chatter-gui/`](https://github.com/TalkBank/batchalign/tree/main/apps/chatter-gui).
