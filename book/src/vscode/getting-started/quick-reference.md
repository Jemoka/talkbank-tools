# Quick Reference

**Status:** Current
**Last updated:** 2026-05-11 20:18 EDT

A comprehensive reference card for every keyboard shortcut, command, and
setting in the TalkBank CHAT extension. Print this page or keep it open in a
tab while working.

For installation, see [Installation](installation.md). For a guided tour, see
[Your First CHAT File](first-file.md).

---

## Keyboard Shortcuts

All shortcuts below use macOS key names. On Windows and Linux, substitute
`Ctrl` for `Cmd`.

### Always Active (in any CHAT file)

| Shortcut | macOS | Action |
|----------|-------|--------|
| `Cmd+Shift+G` | `Cmd+Shift+G` | Show Dependency Graph |
| `Cmd+Shift+Enter` | `Cmd+Shift+Enter` | Play Media at Cursor |
| `Cmd+Shift+/` | `Cmd+Shift+/` | Play Media Continuously |
| `Cmd+Shift+W` | `Cmd+Shift+W` | Show Waveform View |
| `F8` | `F8` | Rewind Media (configurable seconds) |
| `Shift+F5` | `Shift+F5` | Toggle Segment Loop |
| `Alt+Down` | `Alt+Down` | Walker: Next Utterance |
| `Alt+Up` | `Alt+Up` | Walker: Previous Utterance |
| `Cmd+Shift+1` | `Cmd+Shift+1` | Insert CA Special Character (compose mode) |
| `Cmd+Shift+2` | `Cmd+Shift+2` | Insert CHAT Special Character (compose mode) |

### Transcription Mode Active

These shortcuts are only active when transcription mode has been started via
the Command Palette.

| Shortcut | macOS | Action |
|----------|-------|--------|
| `F4` | `F4` | Stamp Timestamp Bullet and advance to new line |

### Coder Mode Active

These shortcuts are only active after starting Coder Mode.

| Shortcut | macOS | Action |
|----------|-------|--------|
| `Cmd+Enter` | `Cmd+Enter` | Coder: Next Utterance (advance to next uncoded) |
| `Cmd+Shift+C` | `Cmd+Shift+C` | Coder: Insert Code without advancing |

### Review Mode Active

These shortcuts are only active after starting Review Mode. Number keys work
only when the editor is not focused (i.e., focus is in the review panel).

| Shortcut | macOS | Action |
|----------|-------|--------|
| `Alt+]` | `Alt+]` | Review: Next Flagged utterance |
| `Alt+[` | `Alt+[` | Review: Previous Flagged utterance |
| `1` | `1` | Review: Rate Good |
| `2` | `2` | Review: Rate Early |
| `3` | `3` | Review: Rate Late |
| `4` | `4` | Review: Rate Wrong |
| `5` | `5` | Review: Skip |

### Standard VS Code Shortcuts (enhanced for CHAT)

These are built-in VS Code shortcuts that the extension enhances with
CHAT-specific intelligence:

| Shortcut | Action |
|----------|--------|
| `Cmd+.` | Quick Fix (automatic corrections for 21 error codes) |
| `F2` | Rename Speaker Code (across all declarations and occurrences) |
| `F12` / `Cmd+Click` | Go to Definition (speaker to `@Participants`, tier to main word) |
| `Shift+F12` | Find All References (every occurrence of a speaker code) |
| `Shift+Alt+F` | Format Document (canonical CHAT style) |
| `Cmd+T` | Workspace Symbol Search (across all open CHAT files) |
| `Cmd+Shift+O` | Document Symbols (outline view by speaker and utterance) |
| `Shift+Ctrl+Right` | Smart Selection: Expand (word, utterance, tier block, transcript) |
| `Shift+Ctrl+Left` | Smart Selection: Shrink |

---

## Command Palette Commands

Open the Command Palette with `Cmd+Shift+P` and type "TalkBank" to see all
commands. Commands are listed below organized by category.

### Media

| Command | Description |
|---------|-------------|
| TalkBank: Play Media at Cursor | Play the audio/video segment at the cursor position |
| TalkBank: Play Media Continuously | Play from cursor to end, tracking current utterance |
| TalkBank: Stop Media Playback | Stop any active media playback |
| TalkBank: Rewind Media (2 s) | Rewind by the configured number of seconds |
| TalkBank: Toggle Segment Loop | Loop the current segment on/off |
| TalkBank: Show Waveform View | Open waveform visualization of linked media |
| TalkBank: Show Elicitation Picture | Display associated elicitation image (Cookie Theft, etc.) |

### Navigation

| Command | Description |
|---------|-------------|
| TalkBank: Show Dependency Graph | Render `%gra` dependency tree as a color-coded graph |
| TalkBank: Walker: Next Utterance | Step to the next utterance and play its segment |
| TalkBank: Walker: Previous Utterance | Step to the previous utterance and play its segment |
| TalkBank: Filter by Speaker... | Show only selected speakers in a side-by-side panel |
| TalkBank: Find in Tier... | Search within a specific CHAT tier, optionally by speaker |

### Analysis

| Command | Description |
|---------|-------------|
| TalkBank: Run CLAN Analysis... | Choose from 33 CLAN commands and view results in a styled panel |
| TalkBank: Run CLAN Analysis on Directory... | Run any CLAN command across all `.cha` files in a folder |
| TalkBank: Run KidEval... | Compare child language measures against normative databases |
| TalkBank: Run Eval... | General assessment normative comparison |
| TalkBank: Run Eval-D (Dementia)... | Dementia evaluation with DementiaBank databases |

### Editing

| Command | Description |
|---------|-------------|
| TalkBank: Edit Participants... | Edit `@ID` headers in an interactive table |
| TalkBank: Insert CA Special Character... | Enter compose mode for Conversation Analysis symbols |
| TalkBank: Insert CHAT Special Character... | Enter compose mode for CHAT/IPA symbols |
| TalkBank: Cancel Special Character Input | Cancel active compose mode |

### Transcription

| Command | Description |
|---------|-------------|
| TalkBank: Start Transcription Mode | Begin transcribing with media playback |
| TalkBank: Stamp Timestamp Bullet | Insert timing bullet at current playback position, advance to new line |
| TalkBank: Stop Transcription Mode | End transcription mode |
| TalkBank: Configure Transcription Keybindings... | Open keybinding editor for transcription keys (F4, F5, F8) |

### Coder

| Command | Description |
|---------|-------------|
| TalkBank: Start Coder Mode... | Load a `.cut` codes file and begin coding utterances |
| TalkBank: Stop Coder Mode | End coder mode |
| TalkBank: Coder: Next Utterance | Advance to the next uncoded utterance |
| TalkBank: Coder: Insert Code... | Select and insert a code from the loaded codes file |

### Review

| Command | Description |
|---------|-------------|
| TalkBank: Start Review Mode | Begin reviewing flagged utterances |
| TalkBank: Stop Review Mode | End review mode |
| TalkBank: Review: Next Flagged | Jump to the next flagged utterance |
| TalkBank: Review: Previous Flagged | Jump to the previous flagged utterance |
| TalkBank: Review: Rate Good | Rate the current utterance as Good |
| TalkBank: Review: Rate Early | Rate the current utterance as Early (boundary too early) |
| TalkBank: Review: Rate Late | Rate the current utterance as Late (boundary too late) |
| TalkBank: Review: Rate Wrong | Rate the current utterance as Wrong |
| TalkBank: Review: Skip | Skip the current utterance without rating |

### Validation

| Command | Description |
|---------|-------------|
| TalkBank: Validate File | Run validation on the current file |
| TalkBank: Validate Directory | Run validation on a directory tree |
| TalkBank: Refresh Validation | Re-run validation for the Validation Explorer |
| TalkBank: Clear Cache | Clear cached validation results for the current file |
| TalkBank: Clear All Validation Cache | Clear the entire validation cache |
| TalkBank: View Cache Statistics | Show cache size and hit/miss statistics |

### Integration

| Command | Description |
|---------|-------------|
| TalkBank: Open in CLAN | Open the current file in the CLAN macOS application at the cursor position |

---

## Settings Reference

All settings are under the `talkbank.*` namespace. Open VS Code Settings
(`Cmd+,`) and search for "talkbank" to see them all.

| Setting | Default | Range / Options | Description |
|---------|---------|-----------------|-------------|
| `talkbank.transcription.defaultSpeaker` | `"CHI"` | Any valid speaker code | Default speaker code inserted on new utterance lines during transcription mode |
| `talkbank.transcription.rewindSeconds` | `2` | 0.5 -- 30 | Seconds to rewind when using the Rewind Media command (`F8`) |
| `talkbank.walker.autoPlay` | `true` | `true` / `false` | Automatically play the media segment when stepping with the walker (`Alt+Down`/`Alt+Up`) |
| `talkbank.walker.loopCount` | `1` | 0 -- 50 | Times to loop each segment during walker playback (0 = loop indefinitely until next step) |
| `talkbank.walker.pauseSeconds` | `0` | 0 -- 10 | Seconds to pause between segments during continuous playback or walker stepping |
| `talkbank.walker.walkLength` | `0` | 0 -- 100 | Utterances to play during continuous walker mode (0 = play all remaining segments) |
| `talkbank.media.defaultSpeed` | `100` | 25, 50, 75, 100, 125, 150, 175, 200 | Default playback speed as a percentage (100 = normal, 50 = half, 200 = double) |
| `talkbank.bullets.display` | `"dim"` | `"dim"`, `"hidden"`, `"normal"` | How timing bullets are displayed: dim (35% opacity), hidden (invisible), or normal (full visibility) |
| `talkbank.lsp.binaryPath` | (auto-detect) | Absolute file path | Override path to the `chatter-lsp` binary. Leave empty to auto-detect from PATH or `target/` directory |
| `talkbank.inlayHints.enabled` | `true` | `true` / `false` | Show inline annotations for alignment mismatches (e.g., `[alignment: 3 main <-> 2 mor]`) |
| `talkbank.validation.severity` | `"all"` | `"all"`, `"errorsOnly"`, `"errorsAndWarnings"` | Filter which diagnostics are displayed: all, errors only, or errors and warnings |

---

## CLAN Analysis Commands (33 total)

These commands are available via **TalkBank: Run CLAN Analysis...** in the
Command Palette or the right-click context menu. Commands marked with an
asterisk (*) prompt for additional input before running.

### Frequency and Length

| Command | Description |
|---------|-------------|
| `freq` | Word frequency counts |
| `freqpos` | Word frequency by part of speech |
| `mlu` | Mean length of utterance (in morphemes) |
| `mlt` | Mean length of turn |
| `wdlen` | Word length distribution |
| `wdsize` | Word size (character count) distribution |
| `maxwd` | Longest words |

### Search

| Command | Description |
|---------|-------------|
| `kwal` * | Keyword and line search |
| `combo` * | Boolean combination search |
| `cooccur` | Co-occurrence analysis |
| `dist` | Distribution analysis |
| `uniq` | Unique word list |

### Developmental Measures

| Command | Description |
|---------|-------------|
| `dss` | Developmental Sentence Score |
| `ipsyn` | Index of Productive Syntax |
| `kideval` | Child language normative comparison |
| `eval` | General assessment normative comparison |
| `sugar` | Syntactic Utterances and Grammatical Analysis Revised |
| `vocd` | Vocabulary diversity (D statistic) |

### Coding and Reliability

| Command | Description |
|---------|-------------|
| `codes` | Code frequency analysis |
| `chains` | Chain analysis |
| `keymap` * | Key remapping analysis |
| `rely` * | Inter-rater reliability |

### Fluency and Phonology

| Command | Description |
|---------|-------------|
| `flucalc` | Fluency calculation |
| `phonfreq` | Phoneme frequency |

### Interaction

| Command | Description |
|---------|-------------|
| `chip` | Child imitation and practice |
| `modrep` | Model and repetition analysis |

### Morphology and Tiers

| Command | Description |
|---------|-------------|
| `mortable` * | Morphology table |
| `trnfix` | Transcript repair |

### Structure

| Command | Description |
|---------|-------------|
| `gemlist` | Gem (activity) listing |
| `timedur` | Timed duration analysis |
| `script` * | Script execution |
| `complexity` | Syntactic complexity |
| `corelex` | Core lexicon analysis |

---

## Snippets

Type any of the listed prefixes and press `Tab` to expand. Each snippet has
multiple equivalent prefixes — use whichever feels most natural.

| Prefixes | Expands To |
|----------|-----------|
| `@UTF8`, `header`, `newfile` | Complete new CHAT file template (`@UTF8`, `@Begin`, `@Languages`, `@Participants`, two `@ID` lines, an initial `*CHI:` line, `@End`) |
| `@Participants`, `participant` | `@Participants:` header with two-speaker placeholder |
| `@ID`, `id` | `@ID:` header with 10 pipe-delimited fields |
| `*`, `utterance` | Main tier line with speaker placeholder and trailing `.` |
| `%mor` | `%mor:` dependent tier |
| `%gra` | `%gra:` dependent tier |
| `@Comment`, `comment` | `@Comment:` header |
| `@Bg`, `@Eg`, `gem` | Gem block (`@Bg` / `@Eg` pair around the cursor) |
