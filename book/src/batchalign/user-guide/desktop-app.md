# Batchalign Desktop (Experimental)

**Status:** Current
**Last updated:** 2026-04-29 10:24 EDT

Batchalign Desktop is the native Batchalign GUI shell in
`apps/batchalign/dashboard-desktop/`. It is **experimental** and is **not** currently a
supported public release surface.

This is also **not** the Chatter desktop app. The two desktop surfaces in this
repository are:

- **Batchalign Desktop** (`apps/batchalign/dashboard-desktop/`) — Batchalign processing UI
- **Chatter Desktop** (`apps/chatter/chatter-gui/`) — CHAT validation-only GUI

For supported end-user workflows today, install and run **`batchalign3`** via
the canonical CLI path in [Installation](installation.md). Treat this chapter as
orientation for the in-repo desktop shell, not as the primary installation path
for first-time users.

## Current status

- **Release contract:** Experimental (see repo-root `book/src/operations/release-contract.md`)
- **Distribution:** no supported public desktop bundle line today
- **Desktop shell location:** `apps/batchalign/dashboard-desktop/`
- **Supported product surface today:** `batchalign3` CLI, local server, and
  dashboard UI

## Evaluating from source

## Getting started

1. **Install Batchalign** — follow the [Installation](installation.md) guide.
   The desktop app needs `batchalign3` on your PATH.

2. **Launch the shell from source** — run `npm run dev` from
   `apps/batchalign/dashboard-desktop/`.

3. **First-time setup** — on first launch, a setup wizard asks you to choose
   your default speech-to-text engine:
   - **Rev.AI** — fast cloud service, requires a paid API key from
     [rev.ai/auth/signup](https://www.rev.ai/auth/signup)
   - **Whisper** — free local model, slower, downloads ~2 GB on first use

   This creates `~/.batchalign.ini`, the same config file that `batchalign3
   setup` writes from the terminal.

## What the shell is for

When run from source, the Batchalign desktop shell is meant to expose the same
high-level processing flows as the `batchalign3` CLI and web dashboard:

- pick a processing command
- choose files or folders
- launch work against a local Batchalign server
- monitor progress in a native window

## The home screen

After setup, you see the home screen with two zones:

**Command cards** — six tasks you can perform:

| Card | Command | What it does |
|------|---------|-------------|
| Transcribe Audio | `transcribe` | Turn audio or video recordings into written transcripts in CHAT format |
| Add Grammar | `morphotag` | Add part-of-speech tags and grammatical structure — needed for CLAN commands like MLU and DSS |
| Align to Audio | `align` | Link each word in a transcript to its exact moment in the audio, so you can click to play |
| Translate | `translate` | Add an English translation line under each utterance in a non-English transcript |
| Segment Utterances | `utseg` | Automatically break a long block of text into separate speaker turns |
| Score Accuracy | `benchmark` | Measure how closely a machine transcript matches a human-verified one |

**Recent tasks** — a compact list of your most recent processing jobs, with
status and file count.

## Processing a folder

1. **Pick a command** — click one of the six cards.

2. **Choose input files** — click the dashed area to open a native folder
   picker. The app scans the folder for relevant files (`.cha` for most
   commands, audio files for Transcribe). You'll see the file count and
   folder path.

3. **Choose output location** — by default, output goes to a separate folder
   (click to choose one). You can switch to "Modify in place" if you want to
   overwrite the originals — make backups first.

4. **Select language** — shown for commands that need it (Transcribe, Align,
   Segment Utterances, Score Accuracy). Defaults to English.

5. **Start processing** — click the full-width Start button. The app submits
   the job to the local server and switches to the progress view.

## Watching progress

The progress screen shows:

- **Summary bar** — command name, file count ("12 of 45 files")
- **Progress bar** — animated blue stripe while running, turns green on
  completion or red on failure
- **File list** — live-updating via server-sent events. Currently processing
  files appear at the top with a pulsing blue dot, followed by queued,
  completed, and errored files. Each row shows the filename, current stage
  (e.g., "Aligning", "Transcribing"), and duration.
- **Cancel** — click **Cancel** in the top-right to stop the job. Files that
  already finished processing are kept.

When processing finishes:

- **Success** — the bar turns green. Click **Open Output Folder** to view
  your results in Finder/Explorer. Click **Process More Files** to start
  another job.
- **Errors** — an error panel appears below the progress bar, grouping
  failed files by error type with plain-language explanations and suggested
  fixes. Common causes include invalid CHAT format, missing audio files, or
  low memory.

## Server status

The app automatically starts a local batchalign3 server when it launches
(on port 18000) and stops it when you quit. A status bar at the top of the
screen shows the connection state:

| Indicator | Meaning |
|-----------|---------|
| Green dot, "Server running" | Ready to accept jobs |
| Yellow pulsing dot, "Server starting..." | Server is booting (usually 1-3 seconds) |
| Red dot, "Server stopped" | Server crashed or was stopped — click **Start Server** to restart |
| Red dot, "batchalign3 not found" | The `batchalign3` binary isn't installed — follow the install instructions shown |

You can manually stop and restart the server from the status bar.

## Help

Click the **?** button in the top-right corner of the header to open a help
panel with descriptions of all six commands and answers to common questions.

## Dashboard (power users)

Click **Dashboard** in the header to switch to the fleet monitoring view.
This shows all jobs across servers with detailed file-level status, error
grouping, and algorithm trace visualizations. It's the same dashboard
available in a web browser at `http://localhost:18000/dashboard`.

## Settings and configuration

Click the **gear icon** in the top-right corner of the header to open
Settings. From there you can:

- Switch your default ASR engine between Rev.AI and Whisper
- Add or update your Rev.AI API key

Changes are saved to `~/.batchalign.ini`, the same config file that the CLI
uses. You can also edit this file directly if you prefer:

```ini
[asr]
engine = rev
engine.rev.key = YOUR_KEY_HERE
```

Valid engine values: `rev` (Rev.AI cloud) or `whisper` (local).

## Keyboard shortcuts

The app does not currently define custom keyboard shortcuts. Standard
platform shortcuts (Cmd+Q / Alt+F4 to quit, Cmd+W to close window) work
as expected.

## Troubleshooting

**"batchalign3 not found"** — the app can't find the CLI binary. Make sure
you've installed Batchalign (`uv tool install batchalign3`) and that your
terminal's PATH is available to GUI apps. On macOS, you may need to restart
after installing.

**Server won't start** — check that nothing else is using port 18000.
Try running `batchalign3 serve start --port 18000` in a terminal to see
the error output.

**Files not showing up** — the folder picker filters by file extension.
For most commands, only `.cha` files are shown. For Transcribe, only audio
files (`.wav`, `.mp3`, `.mp4`, `.m4a`, `.flac`) are shown.

**Processing is slow** — the first run downloads ML models (~2 GB) and may
take several minutes. Subsequent runs are much faster because models stay
cached and the server keeps them in memory. See [Performance](performance.md)
for tuning tips.

For other issues, see [Troubleshooting](troubleshooting.md).
