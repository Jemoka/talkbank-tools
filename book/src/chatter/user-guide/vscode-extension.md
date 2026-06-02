# VS Code Extension

**Status:** Current
**Last updated:** 2026-05-11 18:58 EDT

**Support tier:** Public preview. VSIX bundles are published on GitHub Releases, not yet available from the VS Code Marketplace.

A full-featured CHAT editor for VS Code built on the Rust-powered `chatter-lsp` language server. Edit, validate, analyze, transcribe, and play back audio — all inside your editor. Leverages the same CHAT engine as the `chatter` CLI with live real-time feedback.

**Platform support:** Windows, macOS, and Linux

## Installation

The extension is located in [`apps/vscode-extension/`](https://github.com/TalkBank/talkbank-tools/tree/main/vscode). Two install paths, depending on what you want to do.

**End users — install a released VSIX.** Pre-built, platform-specific VSIX bundles (each ships with the matching `chatter-lsp` binary) are published on [GitHub Releases](https://github.com/TalkBank/talkbank-tools/releases). Download the `.vsix` for your platform, then in VS Code: Extensions sidebar → **"…"** menu → **Install from VSIX…** and pick the downloaded file.

**Developers — run from source.** Open the `apps/vscode-extension/` folder in VS Code, install dependencies, compile, and launch the Extension Development Host:

```bash
cd vscode
npm install
npm run compile
# Then press F5 in VS Code to launch the dev host
```

The dev host runs the locally-compiled extension and talks to a `chatter-lsp` binary discovered via the resolver described in the [Configuration](#configuration) and [Architecture](#architecture) sections below.

---

## Validation & Diagnostics

### Live Validation

Errors and warnings appear inline as you type — the same error codes as `chatter validate` (currently 191; verify with `rg -c '#\[code\("[EW]' crates/core/talkbank-model/src/errors/codes/error_code.rs`), with source-level precision:

- Squiggly underlines on the exact error location
- Problems panel integration with error codes and descriptions
- Validates on every keystroke (250ms debounce), plus immediately on open and save
- Filter by severity: all (default), errors only, or errors and warnings

### Quick Fixes

Press **Cmd+.** (Mac) / **Ctrl+.** (Windows/Linux) to auto-fix common issues. Supports 21 error codes including:

- Add missing terminators, fix `xx` → `xxx`, insert undeclared speakers
- Swap reversed timestamps, close unclosed brackets, delete empty utterances
- Fix consecutive stress markers, consecutive commas, trailing-off markers

### Inlay Hints

Inline annotations show alignment mismatches (e.g., `[alignment: 3 main ↔ 2 mor]`) and timing durations directly in the editor. Toggle with `talkbank.inlayHints.enabled`.

### Validation Explorer

A tree view in the Explorer sidebar for bulk validation of entire directories. Runs the `chatter validate` CLI under the hood, caches results, and displays errors as navigable tree nodes. Right-click any folder to validate.

---

## Cross-Tier Alignment Visualization

### Hover

Hover over any word on the main tier to see its aligned morphological breakdown (`%mor`), grammatical relations (`%gra`), phonological form (`%pho`), and aligned items from `%sin` — all in a rich Markdown popup.

Hover works bidirectionally: hover a `%mor` item to see the aligned main-tier word.

### Highlight Linking

Click any word on the main tier and all aligned items on dependent tiers light up. Click a `%mor` or `%gra` item and the corresponding main-tier word highlights. Bidirectional across `%mor`, `%gra`, `%pho`, `%sin`, `%mod`.

### Dependency Graph

**Cmd+Shift+G** (Mac) / **Ctrl+Shift+G** (Windows/Linux) renders the `%gra` dependency tree for the utterance at your cursor. Words appear as labeled nodes with colored edges by relation type (subject, object, root, etc.). Rendered via Graphviz WASM — no internet required.

Toolbar: zoom, fit to window, export as SVG or PNG.

---

## Media Playback

### Single Segment

**Cmd+Shift+Enter** (Mac) / **Ctrl+Shift+Enter** (Windows/Linux) plays the timing bullet (`•beg_end•`) nearest the cursor. Audio/video resolves from the `@Media:` header.

### Continuous Playback

**Cmd+Shift+/** plays all segments from cursor to end of file. The editor cursor tracks the currently-playing utterance, auto-advancing through the transcript.

### Playback Controls

| Action | Shortcut | Description |
|--------|----------|-------------|
| Play segment | Cmd+Shift+Enter | Play nearest bullet |
| Continuous play | Cmd+Shift+/ | Play from cursor to EOF |
| Stop | — | Stop playback |
| Rewind | F8 | Jump back (default 2s, configurable 0.5–30s) |
| Loop segment | Shift+F5 | Toggle looping current segment |
| Speed | Toolbar slider | 0.25x–2x playback speed |

### Waveform View

**Cmd+Shift+W** opens an interactive waveform panel. Each timing bullet appears as a colored rectangle over the audio waveform. Click anywhere on the waveform to seek. Zoom in/out with buttons, slider, or mouse wheel. Auto-scrolls during playback.

---

## Transcription Mode

A dedicated workflow for transcribing audio into CHAT format.

1. **Start**: `talkbank.startTranscription` — opens the media panel with the full audio file
2. **Play**: Audio plays from your cursor position
3. **Stamp**: Press **F4** to insert a timing bullet at the current playback position and open a new `*SPEAKER:` line
4. **Rewind**: Press **F8** to jump back and re-listen
5. **Stop**: `talkbank.stopTranscription` — exits transcription mode

Configure the default speaker code (`talkbank.transcription.defaultSpeaker`, default: "CHI") and rewind duration.

---

## Walker Mode

Step through utterances one at a time with synchronized audio playback.

| Action | Shortcut |
|--------|----------|
| Next utterance | Alt+Down |
| Previous utterance | Alt+Up |

Each step moves the editor cursor and optionally plays the corresponding audio segment. Configure auto-play, loop count (0 = infinite), pause between segments, and walk length.

---

## CLAN Analysis Commands

Run any of the 34 CLAN analysis commands directly from VS Code — no terminal needed (see [`status-matrix.md`](../../clan-reference/appendices/status-matrix.md) for the authoritative list).

**Right-click** a `.cha` file or folder → **Run Analysis** → pick a command from the list:

- **Frequency/Count**: freq, mlu, mlt, wdlen, wdsize, maxwd, freqpos, timedur, vocd, ...
- **Syntax**: complexity, dss, ipsyn, sugar, ...
- **Search**: kwal, combo, ...
- **Assessment**: kideval, eval, eval-d

Results appear in a dedicated panel with styled tables, stat cards, bar charts, and a **CSV export** button.

### KidEval / Eval / Eval-D

Dedicated assessment panels for child language evaluation, general evaluation, and dementia assessment. Select language and normative database, filter by age and gender, and compare z-scores against population means.

---

## Navigation

| Feature | Shortcut | Description |
|---------|----------|-------------|
| **Outline** | Cmd+Shift+O | Two-level tree: speakers → utterances |
| **Go to Definition** | F12 / Cmd+Click | Speaker code → `@Participants`; dependent tier → aligned main-tier word |
| **Find All References** | Shift+F12 | All occurrences of a speaker code |
| **Rename Speaker** | F2 | Renames across `@Participants`, `@ID`, and all main-tier lines |
| **Workspace Symbols** | Cmd+T | Cross-file search by speaker and utterance |
| **Scoped Find** | Context menu | Search within specific tiers (`%mor`, `%gra`, etc.) with optional speaker filter |
| **Folding** | Standard | Utterance blocks (main + dependent tiers) and header blocks fold as units |
| **Smart Selection** | Shift+Ctrl+Right/Left | Expand/shrink by syntactic unit: word → content → tier → transcript |

---

## Editing

### Code Completion

- Speaker codes (from `@Participants`)
- Dependent tier prefixes (`%mor`, `%gra`, `%pho`, etc.)
- Header names (triggers on `@`)
- Bracket annotations (triggers on `[`)
- Postcode punctuation

### Snippets

8 built-in CHAT snippets: file header, `@Participants`, `@ID`, main tier, `%mor`, `%gra`, `@Comment`, Gem (`@Bg`/`@Eg`).

### Participant Editor

`talkbank.editParticipants` opens a table editor for all `@ID` headers. Edit the 10 pipe-delimited fields (language, corpus, speaker, age, sex, group, SES, role, education, custom) in a form view. Changes are written back as canonical `@ID` lines.

### Linked Editing

Edit a speaker code and all matching codes in the file update simultaneously.

### Auto-Formatting

Format document rewrites to canonical CHAT format. On-type formatting auto-indents after tier prefixes.

---

## Special Characters

### Compose Mode

Type CA (Conversation Analysis) and CHAT special symbols without memorizing Unicode:

| Shortcut | Mode | Symbols |
|----------|------|---------|
| Cmd+Shift+1 | CA symbols | 30+ conversation analysis markers |
| Cmd+Shift+2 | CHAT symbols | 20+ CHAT annotation marks |

Press a key to insert the symbol. Press **Escape** to cancel. Status bar shows the active mode.

---

## Coder Mode

Annotate transcripts with coding schemes from `.cut` files.

1. Start coder mode (context menu → "Start Coder Mode")
2. Load a `.cut` codes file with hierarchical codes
3. Step through uncoded utterances (**Cmd+Enter** / **Ctrl+Enter**)
4. Insert codes from a hierarchical picker (**Cmd+Shift+C** / **Ctrl+Shift+C**)
5. Codes appear as `%cod:` tiers

Progress tracking shows how many utterances remain uncoded.

---

## Speaker Filtering

**Context menu → "Filter by Speaker..."** opens a multi-select picker. Choose one or more speakers to create a filtered read-only view showing only those speakers' utterance blocks, with all file headers preserved.

### Code Lens

Utterance counts per speaker appear above the `@Participants` header (e.g., `CHI: 42 utterances`).

---

## Additional Features

| Feature | Description |
|---------|-------------|
| **Syntax highlighting** | Semantic tokens via tree-sitter: headers, speaker codes, tiers, timing, markers, errors |
| **Timing bullet display** | Configurable: dim (35% opacity, default), hidden, or normal visibility |
| **Show elicitation picture** | Display images referenced by `%pic:` tiers or same-name files |
| **Open in CLAN** | Send the current file to the CLAN application (macOS/Windows, requires CLAN installed) |
| **Cache management** | Status bar shows cache statistics; commands to clear per-file or all cached validation results |
| **Document links** | `@Media:` headers are clickable — opens the referenced media file |

---

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `talkbank.lsp.binaryPath` | `""` | Path to the `chatter-lsp` binary (empty = auto-detect: bundled extension binary, then PATH, then extension target dir) |
| `talkbank.inlayHints.enabled` | `true` | Show alignment and timing inlay hints |
| `talkbank.validation.severity` | `"all"` | Filter: `"all"`, `"errorsOnly"`, `"errorsAndWarnings"` |
| `talkbank.bullets.display` | `"dim"` | Bullet visibility: `"dim"`, `"hidden"`, `"normal"` |
| `talkbank.media.defaultSpeed` | `100` | Playback speed % (25–200) |
| `talkbank.transcription.defaultSpeaker` | `"CHI"` | Default speaker for new utterances |
| `talkbank.transcription.rewindSeconds` | `2` | Rewind duration in seconds (0.5–30) |
| `talkbank.walker.autoPlay` | `true` | Auto-play when stepping through utterances |
| `talkbank.walker.loopCount` | `1` | Loops per segment (0 = infinite) |
| `talkbank.walker.pauseSeconds` | `0` | Pause between segments (0–10s) |
| `talkbank.walker.walkLength` | `0` | Utterances to walk (0 = all remaining) |

---

## Architecture

The extension launches the dedicated `chatter-lsp` binary over
stdio using the Language Server Protocol. (`chatter-lsp` is a
separate Rust crate at `crates/chatter/chatter-lsp/`; the VS Code
extension's executable resolver at
`apps/vscode-extension/src/executableService.ts` looks for it in the bundled
extension binary, then on PATH, then under the extension's target
directory.) The LSP server provides:

- Incremental parsing via tree-sitter
- Full validation (currently 191 error codes; see live count above)
- Cross-tier alignment computation
- CLAN analysis command execution
- Document formatting and symbol extraction

Interactive features (media playback, waveform, graphs, analysis panels, assessment tools) run as VS Code Webview panels with HTML/CSS/JS. The Graphviz renderer uses WASM — no external dependencies.

## Development

```bash
cd vscode
npm install          # Install dependencies
npm run compile      # Build the extension
npm run watch        # Watch mode for development
npm test             # Run tests
npm run lint         # ESLint check
```

To debug: open the `apps/vscode-extension/` folder in VS Code and press **F5** to launch the Extension Development Host.
