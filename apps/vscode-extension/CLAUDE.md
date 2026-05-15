# VS Code Extension — TalkBank CHAT Editor

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

All user, developer, and integrator documentation lives in **`book/`**
(mdBook, hosted alongside the other TalkBank books — see
`../../docs/inventory.md` §2). The book is the single source of truth for
this extension: installation, every mode (transcription, walker, coder,
review), every feature, architecture, developer on-ramp, and custom LSP
RPC contract reference. `book/src/introduction.md` is the entry point;
`book/src/SUMMARY.md` is the table of contents.

`README.md` is the marketplace-facing summary (what a user sees on the
VS Code Marketplace listing); it is a short feature overview plus a
pointer into the book. Do not grow it into a second manual — every
non-trivial addition belongs in a book chapter. Keep this directory
free of loose `.md` docs; if the book doesn't yet cover a topic, add
a book chapter.

## Overview

VS Code extension providing real-time validation, CLAN analysis, media playback,
transcription mode, waveform visualization, dependency graphs, and speaker filtering
for CHAT format files. Backed by the `chatter-lsp` Rust binary over stdio.

## The hard rule: no domain logic in this extension

This extension is a **presentation and workflow layer** over `chatter-lsp`.
The Rust side owns parsing, validation, alignment, CHAT serialization, CLAN
command semantics, and every other piece of CHAT domain knowledge. The
TypeScript side owns webview layout, command wiring, keybindings, state
machines for coder/review/transcription/walker modes, and VS Code API
integration.

**Concretely, this extension must NOT:**

- Re-parse CHAT text (`bulletParser.ts` is a narrow stopgap for timing
  bullets only, documented as such — do not expand it).
- Re-implement any alignment between tiers. Main ↔ `%mor`, `%mor` ↔ `%gra`,
  `%pho`, `%sin`, `%wor` alignment all live in `talkbank-model`. The LSP
  exposes the computed alignment via hover, highlights, alignment sidecar
  JSON (`alignmentSidecar.ts` just decodes the sidecar into segment
  objects — it does not derive alignment). If a webview panel needs
  cross-tier information, request it from the LSP; never derive it from
  the document text on the client.
- Classify CHAT constructs by pattern-matching on serialized strings.
  Routinely, this is a bug: the CHAT grammar has subtle context (CA
  markers, retraces, overlap groups, compound words) that defeats regex.
  If a feature needs to know what kind of thing is under the cursor, the
  LSP can answer via semantic tokens, hover, or a custom RPC endpoint —
  add the endpoint rather than re-implement the classification in TS.
- Hold persistent domain state (open files, validation results, parsed
  ASTs) outside the LSP. Per-session VS Code state (panel positions,
  review ratings, coder progress) is fine; CHAT semantics is not.

The TypeScript side is one more layer away from the grammar and model
source of truth than the LSP itself. The cost of reconstructing domain
semantics here is higher still — always route through the LSP.

## Three-layer architecture

```
┌──────────────────────────────────────────────────┐
│  VS Code Extension (TypeScript)                  │
│  commands · webviews · tree views · keybindings  │  ← this repo
│  state machines: coder / review / transcribe     │
│  LSP client (vscode-languageclient)              │
└───────────────────┬──────────────────────────────┘
                    │ LSP stdio + `talkbank/*` RPC
┌───────────────────┴──────────────────────────────┐
│  chatter-lsp (Rust binary)                      │
│  protocol routing · document state · caches      │  ← crates/chatter-lsp/
│  LSP features + custom talkbank/* RPC endpoints  │
│  hover/highlight/graph rendering (presentation)  │
└───────────────────┬──────────────────────────────┘
                    │ crate-internal calls
┌───────────────────┴──────────────────────────────┐
│  talkbank-{model,parser,transform,clan}          │
│  grammar · model types · validation · alignment  │  ← crates/talkbank-*/
│  CLAN command semantics · CHAT serialization     │
└──────────────────────────────────────────────────┘
```

## Project Structure

```
apps/vscode-extension/
├── src/
│   ├── extension.ts           # Entry point: LSP client, command registrations
│   ├── analysisPanel.ts       # Webview: CLAN analysis JSON → styled tables
│   ├── graphPanel.ts          # Webview: Graphviz DOT → SVG
│   ├── mediaPanel.ts          # Webview: audio/video playback with segment tracking
│   ├── waveformPanel.ts       # Webview: Web Audio API waveform visualization
│   ├── validationExplorer.ts  # Tree view: bulk directory validation
│   ├── cacheManager.ts        # Status bar: cache stats (polls chatter CLI)
│   ├── clanIntegration.ts     # Open in CLAN via send2clan FFI
│   └── utils/                 # alignmentSidecar, bulletParser (fallback), mediaResolver, speakerFilter, cliLocator
├── syntaxes/chat.tmLanguage.json  # TextMate grammar (fallback highlighting)
├── package.json               # Extension manifest (commands, keybindings, menus)
├── book/                      # ← canonical documentation (mdBook)
├── README.md                  # Marketplace summary (keep short; book is the manual)
└── CLAUDE.md                  # This file — AI-assistant guidance for the extension
```

## Key Commands

```bash
cd vscode && npm install && npm run compile
cd vscode && npm test          # vitest
cd vscode && npm run lint      # eslint
```

## Features & Shortcuts

| Feature | Shortcut | Description |
|---------|----------|-------------|
| Dependency Graph | Cmd+Shift+G | DOT → SVG via Graphviz WASM |
| Play at Cursor | Cmd+Shift+Enter | Single segment playback |
| Continuous Play | Cmd+Shift+/ | Play all segments from cursor |
| Rewind | F8 | Rewind 2 seconds |
| Loop Segment | F5 | Toggle loop |
| Walker | Alt+Down/Up | Step through utterances with playback |
| CLAN Analysis | Context menu | CLAN commands → analysis panel |
| Transcription | Command palette | F4 to stamp timing bullets |
| Speaker Filter | Context menu | Virtual document by speaker |
| Waveform | Cmd+Shift+W | Web Audio waveform visualization |

## LSP Binary Discovery

`findLspBinary()` searches: system PATH (via `which`) → `target/debug/` → `target/release/`.

## Webview Pattern

All panels use singleton pattern (`createOrShow`). Communication via PostMessage JSON
protocol (rewind, setLoop, seekTo, segmentChanged).

## Analysis Commands

All CLAN commands are wired: freq, mlu, mlt, wdlen, wdsize, maxwd, freqpos, timedur,
gemlist, cooccur, dist, chip, phonfreq, modrep, vocd, codes, complexity, corelex, chains,
dss, eval, flucalc, ipsyn, kideval, sugar, trnfix, uniq, plus those requiring user input
(kwal, combo, keymap, mortable, script, rely).

## Dependencies

(Versions live in `package.json`.)

- `vscode-languageclient` — LSP client
- `@hpcc-js/wasm` — Graphviz WASM
- TypeScript, ESLint, Vitest

## Detailed Documentation

All user and developer documentation lives in **`book/`** (mdBook). Read
`book/src/introduction.md` as the entry point; `book/src/SUMMARY.md`
lists every chapter.

High-traffic sections for newcomers:

- `book/src/getting-started/installation.md` — install a platform VSIX
- `book/src/developer/architecture.md` — three-layer design
- `book/src/developer/lsp-protocol.md` — LSP surface + custom RPC
- `book/src/navigation/alignment.md` — cross-tier alignment behavior
- `book/src/developer/clan-parity.md` — CLAN feature parity
