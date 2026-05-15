# TalkBank CHAT for VS Code

**Status:** Current
**Last updated:** 2026-04-13 20:34 EDT

Full language support for CHAT transcription files (`.cha`) — the format used by [TalkBank](https://talkbank.org), [CHILDES](https://childes.talkbank.org), and related corpora for linguistic research on conversational data.

This extension replaces the macOS CLAN application with a modern, cross-platform editing environment inside VS Code.

## Screenshots

| Feature | Preview |
|---------|---------|
| Real-time validation | ![Validation diagnostics](images/screenshots/validation.png) |
| Intelligent completion | ![Completion suggestions](images/screenshots/completion.png) |
| Dependency graph (`%gra`) | ![Dependency graph](images/screenshots/dependency-graph.png) |

## Features

### Real-Time Validation

Errors and warnings appear as you type. The language server validates syntax, required headers, participant declarations, tier alignment, morphology format, grammar relations, timing bullets, and terminators — then shows diagnostics in the editor and Problems panel.

### Cross-Tier Alignment

Hover over any word to see how it aligns across annotation layers. Click a word and all its aligned elements on `%mor`, `%gra`, `%pho`, and other tiers highlight simultaneously.

### Syntax Highlighting

Full TextMate grammar plus semantic tokens from the language server. Headers, speaker codes, dependent tiers, annotations, morphology, grammar relations, pauses, and timing bullets are all distinctly colored.

### Dependency Graph Visualization

Press `Cmd+Shift+G` to render the `%gra` dependency structure of the current utterance as a color-coded graph. Arcs are labeled with grammatical relations (SUBJ, OBJ, DET, MOD, ROOT, etc.) and colored by type. Export as SVG or PNG.

### CLAN Analysis Commands

Run analysis commands directly from the editor — right-click and choose **Run CLAN Analysis**, or use the Command Palette. Results display in a styled panel with stat cards, tables, and bar charts. Export results to CSV for use in spreadsheets or statistical software.

| Category | Commands |
|----------|----------|
| **Frequency & length** | freq, freqpos, mlu, mlt, wdlen, maxwd |
| **Search** | kwal, combo, cooccur, dist, uniq |
| **Developmental measures** | dss, ipsyn, kideval, eval, sugar, vocd |
| **Coding & reliability** | codes, chains, keymap, rely |
| **Fluency & phonology** | flucalc, phonfreq |
| **Interaction** | chip, modrep |
| **Morphology & tiers** | mortable, trnfix |
| **Structure** | gemlist, timedur, script |

Commands that need extra input (keywords, file paths) prompt you before running. Run analysis on an entire directory via the Explorer context menu.

### KidEval / Eval / Eval-D Normative Comparison

Compare a child's language measures against normative databases. Select a database, optionally filter by age and gender, and view z-scores in an interactive panel. Available for child development (KidEval), general assessment (Eval), and dementia evaluation (Eval-D with DementiaBank databases).

### Participant Editor

Edit `@ID` headers visually in a table with editable columns for all 10 pipe-delimited fields. Parsing and serialization are fully delegated to the language server.

### Media Playback

Play audio or video linked via `@Media:` headers. **Play at Cursor** (`Cmd+Shift+Enter`) plays the segment nearest the cursor. **Continuous Play** (`Cmd+Shift+/`) plays from cursor to end, tracking the current utterance in the editor. Adjustable playback speed (0.25x–2x). Segment timing is loaded from the language-server alignment sidecar when available, with local bullet parsing as fallback.

### Waveform View

Press `Cmd+Shift+W` to open a waveform visualization of the linked media. Colored overlays mark each timed utterance. Click anywhere on the waveform to seek both the audio and the editor cursor. Zoom in/out with toolbar buttons or mouse wheel; the view auto-scrolls during playback.

### Picture Display

Show elicitation pictures (Cookie Theft, etc.) alongside the transcript. The extension finds images from `%pic:` references, same-name files, or the document directory.

### Coder Mode

Load a `.cut` codes file and step through uncoded utterances. Select codes from a hierarchical QuickPick to insert `%cod:` tiers. `Cmd+Enter` advances to the next uncoded utterance.

### Transcription Mode

Start transcription mode from the Command Palette. Type while audio plays, then press `F4` to stamp a timing bullet and advance to a new utterance line. Configurable default speaker code and rewind interval.

### Walker Mode

Step through utterances one at a time with `Alt+Down` / `Alt+Up`. Each step moves the cursor and plays the corresponding audio segment.

### Speaker Filtering

Filter the transcript to show only selected speakers. Choose one or more speaker codes and a filtered view opens in a side-by-side panel.

### Special Character Input

Insert 50+ CA and CHAT special characters via compose-key mode. Press `Cmd+Shift+1` for Conversation Analysis symbols (intonation arrows, overlap brackets, creaky, whisper, etc.) or `Cmd+Shift+2` for CHAT symbols (IPA, diacritics, glottal stop, etc.). The next keystroke inserts the corresponding Unicode character.

### Code Completion

Context-aware suggestions for speaker codes (from `@Participants`), dependent tier prefixes (`%mor`, `%gra`, `%pho`, etc.), and postcode punctuation. Header completion triggers on `@` and bracket completion triggers on `[` for rapid annotation entry.

### Quick Fixes

Automatic fixes for common errors: replacing `xx` with `xxx`, adding missing terminators, appending trailing-off markers, adding undeclared speakers to `@Participants` (E308), and fixes for E501/E502/E503. Diagnostics use fade-out tags for deprecated or unnecessary code.

### On-Type Formatting

As you type, the extension auto-indents continuation lines with tabs, keeping dependent tiers properly aligned without manual intervention.

### Document Formatting

Format to canonical CHAT style with `Shift+Alt+F`. Normalizes whitespace, header ordering, and tier indentation.

### Rename Speaker

Press `F2` on a speaker code to rename it across `@Participants`, `@ID` headers, and all main tier lines in one operation. Linked editing keeps matching speaker codes in sync as you type.

### Workspace Symbols

Press `Cmd+T` to search across all open CHAT files by speaker code and utterance content.

### Document Links

`@Media:` header values are clickable links that open the referenced media file.

### Smart Selection

Use `Shift+Ctrl+Right` (expand) / `Shift+Ctrl+Left` (shrink) to grow or shrink the selection by syntactic units — word, utterance, tier block, and transcript.

### Find All References

`Shift+F12` on a speaker code lists every occurrence: declaration, `@ID` header, and all main tier lines.

### Code Lens

Utterance counts per speaker are shown above the `@Participants` header (e.g., `CHI: 42 utterances`).

### Snippets

8 CHAT snippets for common structures: header block (`@UTF8`/`header`/`newfile`), `@Participants`, `@ID`, main tier, `%mor`, `%gra`, `@Comment`, and Gem (`@Bg`/`@Eg`). Type the prefix and press `Tab`.

### Go to Definition

`F12` or `Cmd+Click` jumps from speaker codes to their `@Participants` declaration, and from `%mor` or `%gra` items to the aligned main tier word.

### Hover

Hover over headers (`@Participants`, `@ID`, etc.) and timing bullets to see contextual documentation and alignment details.

### Inlay Hints

Inline annotations show alignment count mismatches (e.g., `[alignment: 3 main <-> 2 mor]`) without modifying the file.

### Validation Explorer

A tree view in the Explorer sidebar for bulk validation of entire directories. Cached results in SQLite speed up repeated runs across large corpora.

### CLAN Integration

If the CLAN application is installed, open any `.cha` file directly in CLAN at the current cursor position via Apple Events (macOS) or Windows messaging.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+Shift+G` | Show Dependency Graph |
| `Cmd+Shift+Enter` | Play Media at Cursor |
| `Cmd+Shift+/` | Play Media Continuously |
| `Cmd+Shift+W` | Show Waveform View |
| `Alt+Down` / `Alt+Up` | Walker: Next / Previous Utterance |
| `F8` | Rewind Media |
| `Shift+F5` | Toggle Segment Loop |
| `F4` | Stamp Timestamp Bullet (transcription mode) |
| `Cmd+Shift+1` | Insert CA Special Character |
| `Cmd+Shift+2` | Insert CHAT Special Character |
| `F2` | Rename Speaker Code |
| `Shift+F12` | Find All References |
| `F12` / `Cmd+Click` | Go to Definition |
| `Cmd+.` | Quick Fix |
| `Shift+Alt+F` | Format Document |
| `Cmd+T` | Workspace Symbol Search |

On Windows/Linux, substitute `Ctrl` for `Cmd`.

## Beyond CLAN

The extension goes well beyond replicating the macOS CLAN application. Features with no CLAN equivalent include: corpus-scale validation across entire directory trees, real-time diagnostics as you type, quick-fix code actions, bidirectional cross-tier alignment highlighting, alignment mismatch inlay hints, go-to-definition, find all references, rename across all occurrences, linked editing, code lens utterance counts, smart selection by syntactic units, workspace symbol search across files, clickable document links, speaker filtering, code folding, CSV export, configurable severity filtering, and cross-platform support (macOS, Windows, Linux). See the [CLAN Feature Parity chapter](book/src/developer/clan-parity.md) for the full comparison.

## Requirements

- VS Code 1.85 or later
- The `chatter-lsp` binary (ships in the same release archive as `chatter`)

The extension searches for `chatter-lsp` on your system PATH first, then falls back to `target/debug/` and `target/release/` relative to the extension directory.

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `talkbank.bullets.display` | `"dim"` | Bullet display mode: `"dim"`, `"hidden"`, or `"normal"` |
| `talkbank.walker.autoPlay` | `true` | Auto-play media when stepping with walker |
| `talkbank.walker.loopCount` | `1` | Loop count per segment (0 = indefinite) |
| `talkbank.walker.pauseSeconds` | `0` | Pause between segments during continuous play |
| `talkbank.walker.walkLength` | `0` | Utterances to play continuously (0 = all) |
| `talkbank.media.defaultSpeed` | `100` | Playback speed percentage (25–200) |
| `talkbank.inlayHints.enabled` | `true` | Show inline alignment mismatch hints |
| `talkbank.validation.severity` | `"all"` | Filter diagnostics: `"all"`, `"errorsOnly"`, `"errorsAndWarnings"` |
| `talkbank.lsp.binaryPath` | (auto) | Override path to the `chatter-lsp` binary |

## Documentation

All documentation lives in the **[TalkBank CHAT Editor book](book/src/introduction.md)** (mdBook). Entry points:

- [Getting Started](book/src/getting-started/installation.md) — install a platform VSIX and open your first file
- [Editing](book/src/editing/syntax-highlighting.md) / [Navigation](book/src/navigation/alignment.md) / [Media & Transcription](book/src/media/playback.md) — feature chapters
- [Developer Guide](book/src/developer/architecture.md) — three-layer architecture, LSP internals, adding features
- [CLAN Feature Parity](book/src/developer/clan-parity.md) — comparison with the macOS CLAN application

## Links

- [TalkBank](https://talkbank.org) — home of the TalkBank project
- [CHILDES](https://childes.talkbank.org) — Child Language Data Exchange System
- [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.html) — official CHAT format specification
- [Issue Tracker](https://github.com/TalkBank/talkbank-tools/issues)
