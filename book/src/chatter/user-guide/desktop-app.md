# Chatter Desktop (Experimental)

**Status:** Current
**Last updated:** 2026-06-01 00:50 PDT

**⚠️ IMPORTANT: Chatter Desktop is an experimental in-repo evaluation app, NOT a supported end-user surface.** Use the `chatter` CLI for production CHAT validation, or use the VS Code extension for interactive editing with live validation. 

## When to use Chatter Desktop

Chatter Desktop (`apps/chatter/chatter-gui/`) is intended **only for**:
- Contributors and maintainers evaluating the native GUI implementation
- Local development and testing of the desktop surface
- Experimental internal workflows

**For supported end-user workflows:**
- **Validate CHAT from the command line:** use `chatter validate`
- **Edit CHAT with live diagnostics:** use the [VS Code extension](vscode-extension.md)

This is also **not** the Batchalign desktop app. The two experimental desktop surfaces in this repository are:

- **Chatter Desktop** (`apps/chatter/chatter-gui/`) — CHAT validation GUI only (experimental)
- **Batchalign Desktop** (`apps/batchalign/batchalign-gui/`) — Batchalign processing UI (experimental)

## Current status

- **Release contract:** Experimental (see repo-root `book/src/operations/release-contract.md`)
- **Distribution:** No supported DMG/MSI/AppImage release channel
- **CI:** Not included in main PR CI gates
- **Recommendation:** Contributors and maintainers only; **do not use for production work**

## Getting Started (Development Only)

### From source

```bash
cd apps/chatter/chatter-gui
npm install
cargo tauri dev       # launches the app with hot reload
cargo tauri build     # produces a distributable app bundle
```

Requires: Rust (stable, edition 2024), Node.js, and npm.

## Using the App

### Opening files

Chatter validates **one target at a time**: a single `.cha` file or one folder.

Three ways to start validating:

1. **Choose File** — opens a file picker filtered to `.cha` files
2. **Choose Folder** — opens a folder picker; validates all `.cha` files recursively
3. **Drag and drop** — drag one `.cha` file or one folder onto the app window

When idle, if you've previously validated a target, the drop zone shows
**"Last: resources/corpus/reference/ — Re-validate?"** as a clickable shortcut.

### Reading results

The main window has three areas:

```text
┌──────────────────────────────────────────────────────────────┐
│  [Choose File] [Choose Folder] or drag here  [System|Light|Dark] │
├──────────────────┬───────────────────────────────────────────┤
│ 3 FILES WITH     │  Filter by code… [All|Errors|Warnings]    │
│ ERRORS / 120     │                                           │
│                  │  ▾ [E302] Missing @End header              │
│  📁 corpus/      │  ┌───────────────────────┐                │
│    ✗ file1 (3)   │  │ 41 │ *CHI: hello .    │                │
│    ✗ file3 (1)   │  │ 42 │                   │                │
│                  │  │    │ ^                 │                │
│                  │  └───────────────────────┘                │
│                  │  💡 Add @End on the last line             │
│                  │  [Copy] [Open in CLAN]                    │
├──────────────────┴───────────────────────────────────────────┤
│  Progress: 45/120 │ 4 errors │ ~2m 30s remaining │ [Cancel]  │
└──────────────────────────────────────────────────────────────┘
```

- **File tree** (left) — collapsible directory tree showing **only files with
  errors** (valid files are hidden to reduce clutter). A header shows "N files
  with errors / M total". Files are sorted alphabetically.

- **Error panel** (right) — for the selected file, shows each error with its
  code in `[E001]` format, severity color, message, source snippet with caret
  underlines, and multi-span labels for complex errors (e.g., alignment
  mismatches across tiers). CHAT-specific formatting is handled: tabs expanded
  to 8-column boundaries, `\x15` bullets rendered as `•`, underline markers
  shown as styled underlined text. Suggestions prefixed with 💡.

- **Status bar** (bottom) — streaming progress during validation, ETA after 5+
  files, total error count, and action buttons.

### Filtering errors

A compact filter bar appears above the error cards when a file has diagnostics:

- **Code filter** — type "E7" to show only alignment errors, "W" for warnings, etc.
- **Severity toggle** — switch between All / Errors / Warnings

The file header updates to show filtered vs. total count (e.g., "3 errors (7 total)").

### Collapsible error cards

Each error card has a clickable header that toggles between expanded and
collapsed view. Collapsed cards show only the error code and first line of the
message. When a file has 5 or more errors, an **Expand All / Collapse All**
button appears.

### Dark mode

Chatter follows your system appearance by default. A **System / Light / Dark**
toggle in the drop zone area lets you override. Your preference is remembered
across sessions.

The dark palette uses muted Apple-style colors — readable miette error
highlighting on dark backgrounds.

### Clickable file paths

Click the file name in the error panel heading to **reveal the file in Finder**
(macOS), Explorer (Windows), or the default file manager (Linux).

### Copy errors

Each error card has a **Copy** button that copies the full miette-rendered error
text (plain text, not HTML) to your clipboard for pasting into issue reports or
messages.

### Actions

| Action | Where | What it does |
|--------|-------|--------------|
| **Re-validate** | Status bar / last-target hint | Re-run validation on the same target (picks up edits) |
| **Cancel** | Status bar (during validation) | Stop the current run |
| **Export** | Status bar | Save results as JSON or plain text via a save dialog |
| **Open in CLAN** | Per-error button | Opens the file at the error location in the CLAN editor |
| **Copy** | Per-error button | Copies the plain-text error to clipboard |
| **Reveal in file manager** | File name heading | Opens the file's parent directory |

"Open in CLAN" only appears when the CLAN application is detected on your
system (macOS and Windows only). It adjusts line numbers to account for headers
that CLAN hides (`@UTF8`, `@PID`, `@Font`, `@ColorWords`, `@Window`).

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+R / Cmd+R | Re-validate |
| Escape | Cancel running validation |

All other navigation is mouse-driven (click files, scroll errors).

### Window title

The window title updates to reflect the current state:

- **Idle:** "Chatter"
- **Discovering:** "Chatter — Discovering files…"
- **Running:** "Chatter — Validating (45/120)"
- **Finished:** "Chatter — 14 errors in 3 files" or "Chatter — All 74 files valid"

### ETA

After 5 or more files have been processed, the status bar shows an estimated
time remaining (e.g., "~2m 30s remaining"). The estimate updates every second.

### Notifications

When validation finishes while the app is not focused, a system notification
shows the summary ("Validation complete — 14 errors in 3 files").

### First launch

On first launch, an onboarding overlay explains the four main interactions: drag
files, error panel, keyboard shortcuts, and export. Dismiss with "Got it" — it
won't appear again.

## CLI Bundling

The desktop app can bundle the `chatter` CLI binary so power users who download
the GUI can also run the CLI from their terminal (like VS Code ships the `code`
command).

An **Install CLI Command** menu item (when available) symlinks the bundled
binary to `/usr/local/bin/chatter` (macOS/Linux) or copies it to a PATH
directory (Windows).

To build with the bundled CLI:

```bash
cargo build --release -p chatter-cli
mkdir -p apps/chatter/chatter-gui/src-tauri/resources
cp target/release/chatter apps/chatter/chatter-gui/src-tauri/resources/
cargo tauri build
```

## Architecture

The desktop app lives in `apps/chatter/chatter-gui/` as a sibling to `apps/vscode-extension/`:

```text
apps/chatter/chatter-gui/
  src-tauri/          Rust backend (Tauri v2)
    src/
      main.rs         Bin entry — calls chatter_gui_lib::run()
      lib.rs          Tauri app setup (Builder + module wiring)
      protocol.rs     Shared command/event names + request types
      commands.rs     validate, cancel, open_in_clan, export, reveal, install_cli
      events.rs       ValidationEvent → frontend event bridge
      validation.rs   Desktop validation orchestration for one target
  src/                React + TypeScript frontend
    components/       DropZone, FileTree, ErrorPanel, ProgressBar, OnboardingOverlay
    hooks/            useValidation, validationState, useTheme
    protocol/         Command/event names + TypeScript transport mirrors
    runtime/          Tauri transport + capability-focused runtime seam
```

The Rust backend calls `validate_directory_streaming()` from
`talkbank-transform` directly — the same streaming validation pipeline used by
the TUI. Events flow over crossbeam channels to the Rust side, then are
serialized to JSON and emitted to the frontend via Tauri's event bridge.

Cancellation uses `ArcSwapOption` for lock-free atomic swap of the cancel
sender — no mutex.

The frontend keeps Tauri-specific code confined to `src/runtime/tauriTransport.ts`.
React components and hooks consume narrower capabilities (`validationRunner`,
`validationTarget`, `clan`, `exports`) instead of reaching for one broad
desktop service object.

## Comparison with TUI

| Feature | TUI (`chatter validate`) | Desktop app |
|---------|--------------------------|-------------|
| File selection | CLI arguments | Drag-and-drop, file picker |
| Navigation | Keyboard (Tab, arrows) | Mouse click |
| Error display | Two-pane terminal UI | Scrollable panels with source snippets |
| Error filtering | — | Code filter + severity toggle |
| Copy error | — | Copy button per error |
| Open in CLAN | `c` key | Button per error |
| Export | `--format json --audit` | Save dialog (JSON or text) |
| Streaming progress | Progress bar | Progress bar + ETA |
| Dark mode | Terminal theme | System/Light/Dark toggle |
| Caching | Same engine | Same engine |
| Who it's for | Power users, CI | Researchers, linguists |

Both use the identical validation engine and produce the same error codes.

## When to Use Which Tool

The TalkBank toolchain offers validation through four interfaces. Each serves a
different workflow:

| Tool | Audience | Use when |
|------|----------|----------|
| **Chatter Desktop (experimental)** | Evaluators, contributors | You are specifically trying the experimental `apps/chatter/chatter-gui/` validation GUI from source. |
| **VS Code extension** | Editors, annotators | You're *editing* CHAT files and want live diagnostics, quick fixes, CLAN analysis, and media playback. |
| **`chatter validate` (TUI)** | Power users | You're comfortable in a terminal and want keyboard-driven navigation. |
| **`chatter validate` (CLI)** | CI, scripts | You need machine-readable output (`--format json`) or batch audits (`--audit`). |

Chatter Desktop focuses on **validation only** — it does not run CLAN analysis
commands or play media. If you need those features, use the VS Code extension.
