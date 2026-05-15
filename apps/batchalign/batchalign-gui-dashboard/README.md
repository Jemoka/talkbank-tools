# Batchalign3 Desktop App (Tauri)

**Status:** Current
**Last updated:** 2026-04-28 23:04 EDT

Tauri desktop shell wrapping the canonical React frontend (`../../frontend`).
Provides a native GUI so researchers can pick files, choose a command, and watch
progress without opening a terminal.

This is the current Batchalign desktop shell. It is distinct from the
repository-root `desktop/` app, which is the separate Chatter desktop
validation surface for CHAT-first workflows.

## Architecture

```
Tauri Shell (this directory)
  ├── Plugins: dialog (file picker), shell (open folders)
  ├── Custom commands: file discovery, setup/config I/O, local server lifecycle
  ├── Local ownership: first-launch config + managed batchalign3 child process
  └── Webview loads React SPA from apps/batchalign/cli-web-statuspage/dist/

React SPA (../../apps/batchalign/cli-web-statuspage/)
  ├── desktop/protocol.ts inventories raw shell commands/events
  ├── DesktopContext fans that protocol into focused capability hooks
  ├── /process    ← End-user processing flow (default in desktop)
  ├── /dashboard  ← Fleet monitoring (power users)
  └── HTTP/SSE/WS to batchalign3 server at localhost:18000

batchalign3 server (auto-managed)
  ├── Spawned as child process: batchalign3 serve start --foreground --port 18000
  ├── Auto-started on app launch, auto-stopped on exit
  └── Manual start/stop via server status bar
```

The Tauri Rust side stays thin — native dialogs, config I/O, and local
server/process management only. All shared UI logic lives in the React
frontend, which consumes shell-only capabilities through
`apps/batchalign/cli-web-statuspage/src/desktop/DesktopContext.tsx`; `apps/batchalign/cli-web-statuspage/src/desktop/protocol.ts`
inventories the raw command/event boundary; `apps/batchalign/cli-web-statuspage/src/lib/tauri.ts` keeps
the dynamic Tauri imports and browser fallbacks in one place.

The desktop app auto-starts a local batchalign3 server on port 18000 and talks
to it over HTTP, the same way the web dashboard does. It uses `paths_mode: true`
for `POST /jobs`, sending absolute file paths so the server reads/writes files
directly from disk.

## Prerequisites

- Node.js and npm
- Rust toolchain
- Tauri v2 prerequisites for your platform (see [Tauri docs](https://tauri.app/start/prerequisites/))

## Setup

```bash
cd apps/dashboard-desktop
npm ci
```

The frontend dependencies must also be installed:

```bash
cd ../../frontend
npm ci
```

## Development

```bash
cd apps/dashboard-desktop
npm run dev
```

This starts Tauri dev mode: the frontend dev server on `:1420` with hot reload,
and the Tauri webview pointing at `/process?server=http://127.0.0.1:18000`.

The app auto-starts a batchalign3 server on port 18000. If `batchalign3` is not
on PATH, the status bar shows install instructions.

## Build

```bash
cd apps/dashboard-desktop
npm run build
```

Produces a platform-native bundle (`.app` on macOS, `.msi` on Windows, `.deb`/`.AppImage` on Linux).

## Release hardening

Public macOS distribution of this Tauri shell should ship as a signed and
notarized `Batchalign3.app` (bundle identifier
`org.talkbank.batchalign3.dashboard`), normally inside a DMG or release zip.
Reuse the private signing reference in
`../../../book/src/operations/code-signing-and-distribution.md`, which already extracts the
institutional knowledge from `../../../java-chatter-stable/build-mac-app.sh`
and `../../../java-chatter-stable/notarize.sh`.

- This is a Rust/Tauri app, so the Java 25 JIT entitlements from legacy Chatter
  do **not** apply. Start with hardened runtime + timestamp and only add
  entitlements if a future Tauri capability requires them.
- The current shell launches `batchalign3` from PATH. Notarizing the app does
  **not** sign or notarize a separately distributed CLI artifact.
- If we later bundle the CLI inside the app, sign the nested executable before
  sealing the `.app` and submitting the outer DMG/zip.
- This desktop surface remains dormant from a release perspective. Keep signing
  prep staged until end-user distribution resumes.

## Backend Target

Desktop mode defaults to `http://127.0.0.1:18000`.
Override at runtime with query parameter: `?server=http://host:port`

## Tauri Commands

| Command | Purpose |
|---------|---------|
| `discover_files(dir, extensions)` | Walk a directory, return paths matching extensions. Bridges folder picker → `POST /jobs` `source_paths`. |
| `start_server()` | Spawn `batchalign3 serve start --foreground --port 18000` as a managed child process and return the resulting `{ running, port, binary_path, pid }` snapshot. |
| `stop_server()` | Kill the managed server child process and return the resulting `{ running, port, binary_path, pid }` snapshot. |
| `server_status()` | Return `{ running, port, binary_path, pid }` for the status bar. |
| `get_batchalign_path()` | Return the path to the `batchalign3` binary, or null if not found. |
| `is_first_launch()` | True if `~/.batchalign.ini` doesn't exist — triggers setup wizard. |
| `read_config()` | Read user config (engine + Rev.AI key) from `~/.batchalign.ini`. |
| `write_config(config)` | Write user config to `~/.batchalign.ini` and return `{ message }`. |

## Tauri Events

| Event | Payload | Purpose |
|-------|---------|---------|
| `desktop://server-status-changed` | `{ status: { running, port, binary_path, pid } }` | Shell-owned server lifecycle updates consumed by `useServerLifecycle`. |

The config commands resolve `~/.batchalign.ini` via `HOME`, then Windows-style
`USERPROFILE` or `HOMEDRIVE` + `HOMEPATH` fallbacks so the setup wizard works
consistently across desktop shells.

The Rust side mirrors this transport contract in `src-tauri/src/protocol.rs` so
command/event names and payload shapes stay easy to audit next to the frontend
inventory in `apps/batchalign/cli-web-statuspage/src/desktop/protocol.ts`.

## Shell Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

The Rust-side shell tests intentionally stay narrow and fast:

- `src-tauri/src/protocol.rs` — protocol identifier stability and event payload serialization
- `src-tauri/src/main.rs` — `discover_files_in_dir()` recursion, filtering, and
  sorting
- `src-tauri/src/config.rs` — config roundtrip plus Windows-friendly home-dir
  fallbacks
- `src-tauri/src/server.rs` — `ServerProcess` empty/running/exited child
  lifecycle behavior

Frontend seam checks live in `apps/batchalign/cli-web-statuspage/e2e/tests/mock-server.spec.mjs` and use a
small fake Tauri runtime to verify first-launch config flow, file discovery, and
server status event wiring without booting a native shell.

Keep new shell logic behind pure helpers or `ServerProcess` methods so this
suite can validate native contracts without booting a full Tauri app.

## Tauri Plugins

| Plugin | Capabilities | Used For |
|--------|-------------|----------|
| `dialog` | `dialog:allow-open`, `dialog:allow-save` | Native file/folder picker |
| `shell` | `shell:allow-open` | Open output folders in Finder/Explorer |
