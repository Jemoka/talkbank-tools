# CLAUDE.md — Chatter Desktop App

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Overview

Native desktop validation app for CHAT files, built with Tauri v2 (Rust backend, React + TypeScript frontend). Sibling to the VS Code extension (`apps/vscode-extension/`). Designed for linguists and researchers who don't use a terminal.

This is **not** the Batchalign desktop shell. That active Batchalign surface
lives in `apps/batchalign/dashboard-desktop/` and wraps the shared Batchalign
apps/batchalign/cli-web-statuspage/server flow. `desktop/` remains the separate Chatter validation app.

## Functional Parity with TUI

**The desktop app must achieve full functional parity with the `chatter validate` TUI.** Every feature the TUI provides for displaying validation results must work equivalently in the desktop app. The TUI is the reference implementation.

**Current parity status: partial.** See the feature parity tables below for details.

### Error Display Parity (mandatory)

The TUI source is at `crates/chatter-cli/src/ui/validation_tui/`. Every rendering behavior listed below must be matched:

| Feature | TUI implementation | Desktop status |
|---------|-------------------|----------------|
All error rendering is handled by **miette on the Rust side** — the same
`render_error_with_miette_with_source()` function used by the CLI. ANSI output
is converted to HTML via the `ansi-to-html` crate and sent alongside each parse
diagnostic in the `diagnostics` field of each `Errors` event. The frontend
displays it in a `<pre>` block. This guarantees identical output to the CLI —
same box-drawing characters, same caret underlines, same source snippets, same
colors.

| Feature | Status |
|---------|--------|
| **Miette-style error rendering** | **Implemented** — server-side rendering via `render_error_with_miette_with_source()` + `ansi-to-html` |
| **Multi-line context, tab expansion, bullets, underlines, labels, suggestions** | **Implemented** — all handled by miette |
| **Colored output** | **Implemented** — ANSI colors converted to HTML `<span style="color:...">` |

### File List Parity (mandatory)

| Feature | TUI implementation | Desktop status |
|---------|-------------------|----------------|
| **Hide valid files** | TUI tracks `total_files_with_errors()` separately | **Implemented** — only error files shown; header shows "N files with errors / M total" |
| **Alphabetical sort** | Files sorted during validation (`state.files.sort_by`) | **Implemented** — `localeCompare` sort in `buildTree()` |
| **Recursive directory tree** | Full recursive traversal with indented display | **Implemented** — collapsible tree with pruned empty dirs |
| **Error count badges** | Per-file error count in file list | **Implemented** |

### Navigation Parity (mandatory)

| Feature | TUI keybinding | Desktop equivalent |
|---------|---------------|-------------------|
| Switch panes | Tab | Click (mouse-native UI — Tab not applicable) |
| Navigate files | j/k, ↑/↓ | Click |
| Navigate errors | j/k, ↑/↓ (in error pane) | Scroll |
| Open in CLAN | Enter / c | Button per error (needs debug) |
| Revalidate | Ctrl+R / Cmd+R | **Implemented** — keyboard shortcut + button |
| Cancel validation | Escape | **Implemented** — keyboard shortcut + button |
| Quit | q / Esc | Window close |

### Lifecycle Parity (mandatory)

| Feature | TUI implementation | Desktop status |
|---------|-------------------|----------------|
| **Progress throttling** | Configurable file-stride between redraws | Not yet implemented — React batches DOM updates via `requestAnimationFrame` which provides some natural throttling |
| **Streaming vs complete states** | Two distinct UI modes during/after validation | **Implemented** — ProgressBar shows different content per phase |
| **Progress header** | "Done \| X files with errors / Y files" + gauge | **Implemented** — tree header shows "N files with errors / M total" |

## Architecture

```
desktop/
  src-tauri/            Rust backend (Tauri v2)
    src/
      protocol.rs       Shared command/event names + transport request types
      commands.rs       #[tauri::command] handlers (validate, cancel, export, open_in_clan)
      events.rs         ValidationEvent → FrontendEvent bridge (serde camelCase)
      lib.rs            Tauri entry point + plugin registration
    tests/
      validation_bridge.rs   Integration tests (no GUI needed)
  src/                  React + TypeScript frontend
    components/         DropZone, FileTree, ErrorPanel, ProgressBar
    hooks/              useValidation + validationState reducer
    protocol/           Centralized command/event names + TS transport mirrors
    runtime/            Tauri transport + capability-focused runtime seam
  tests/unit/           Focused seam tests (Node test runner + compiled TS)
  tests/e2e/            WebdriverIO smoke tests (Linux/Windows only)
  wdio.conf.ts          WebdriverIO config
```

### Key design decisions

- **Direct Rust linking** — calls `validate_directory_streaming()` from `talkbank-transform` directly, not shelling out to the CLI. Streaming events over crossbeam channels → Tauri emit.
- **Lock-free concurrency** — `ArcSwapOption` for the cancel sender, no mutex. See the [mutex policy](../book/src/architecture/concurrency.md).
- **Centralized protocol contracts** — Tauri command/event names and transport payload types live in `src-tauri/src/protocol.rs` and `src/protocol/desktopProtocol.ts`.
- **serde camelCase bridge** — Rust structs use snake_case with `#[serde(rename_all = "camelCase")]` so JSON matches TypeScript types. The Rust integration tests verify the serialized JSON shape.
- **Single-target contract** — desktop validation accepts one `.cha` file or one folder at a time. Native drag/drop must use Tauri's webview drag-drop API, not browser file-name placeholders.
- **Capability-first runtime seam** — keep `@tauri-apps/*` imports inside `src/runtime/tauriTransport.ts`; components and hooks should depend on narrow capability hooks rather than a whole desktop service object.

## Development

```bash
cd desktop
npm install
cargo tauri dev           # Launch with hot reload (frontend + backend)
cargo tauri build         # Distributable app bundle (DMG/MSI/AppImage)
cargo tauri build --debug # Debug build for E2E testing
```

## Testing

Three tiers — see [Desktop App Testing](../book/src/contributing/desktop-testing.md) for full details.

```bash
# Tier 1: focused apps/batchalign/cli-web-statuspage/runtime seam tests
cd desktop && npm run test:unit

# Tier 2: Rust integration tests (fast, run always)
cargo nextest run -p chatter-gui --test validation_bridge

# Tier 3: E2E smoke tests (slow, Linux/Windows only)
tauri-driver &
npm run test:e2e
```

## App Identity

The official name is **Chatter**, not "chatter-gui". The Cargo package name
is `chatter-gui` to avoid conflicts with the CLI package (`chatter-cli`
produces the `chatter` binary), but the user-visible name everywhere must be
"Chatter":

- `tauri.conf.json` → `productName: "Chatter"` (controls `.app` bundle name)
- Window title: "Chatter — CHAT Validation"
- macOS About dialog: "About Chatter" (requires running as `.app` bundle, not raw binary)
- `cargo tauri dev` runs the raw binary, so About shows "chatter-gui" — this is expected in dev mode only

## CLI Bundling

The desktop app should bundle the `chatter` CLI binary so that power users who
download the GUI can also run the CLI from their terminal (like VS Code ships
the `code` command).

**Approach (VS Code-style):**

1. Build `chatter` alongside the desktop app (`cargo build --release -p chatter-cli`)
2. Include it as a Tauri `resources` entry — bundled inside the `.app`
3. Add a menu item "Install CLI command" that symlinks the bundled binary
   to `/usr/local/bin/chatter` (macOS/Linux) or adds to PATH (Windows)
4. On macOS: `Chatter.app/Contents/Resources/chatter` → `/usr/local/bin/chatter`

This is a Phase 3 item — requires the build pipeline to produce both binaries.

## Release Hardening

Public macOS releases should ship as a signed and notarized `Chatter.app` with
bundle identifier `org.talkbank.chatter`. Reuse the private signing playbook in
`../../book/src/operations/code-signing-and-distribution.md`, which already extracted the
certificate and notarytool flow from
`../../java-chatter-stable/build-mac-app.sh` and `../../java-chatter-stable/notarize.sh`.

- This is a Rust/Tauri app, so the Java 25 JIT entitlements used by legacy Java
  Chatter do **not** apply here. Start with hardened runtime + timestamp and
  only add entitlements if a future Tauri capability requires them.
- Raw unsigned `.app` bundles are acceptable for local development only, not for
  end-user release artifacts.
- If the bundled-CLI plan lands, sign the nested `chatter` resource before
  sealing the `.app` and notarizing the outer DMG/zip. A signed app does not
  retroactively cover an unsigned separately shipped CLI artifact.

## Coding Standards

Follow the root `CLAUDE.md` for all Rust code. Additional rules for the desktop app:

- **No mutex** — use `ArcSwapOption`, atomics, or channels. See the mutex policy.
- **serde field names** — every enum variant with fields needs `#[serde(rename_all = "camelCase")]`. The enum-level `rename_all` only affects tag names, not field names.
- **TypeScript types must mirror Rust types** — when changing `events.rs`, update `types.ts` and run the integration tests to verify.
- **Reference the TUI source** when implementing display features — `crates/chatter-cli/src/ui/validation_tui/` is the reference implementation for error rendering, file list behavior, and navigation.
