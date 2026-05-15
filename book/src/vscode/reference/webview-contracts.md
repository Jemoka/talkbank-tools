# Webview Message Contracts

**Status:** Current
**Last updated:** 2026-04-16 21:54 EDT

Each webview panel exchanges JSON messages with the extension host
over VS Code's `Webview.postMessage` / `onDidReceiveMessage` channels.
This page is the per-panel contract reference: message unions each
direction, decoder location, HTML/JS renderer paths.

Contracts are defined via Effect `Schema` on the extension side so
incoming payloads decode through a typed boundary (see
[`effectBoundary.ts`][effectBoundary]). Panels that render DOT graphs
or static images do not need a full message contract and are listed at
the end.

[effectBoundary]: https://github.com/TalkBank/talkbank-tools/blob/main/apps/vscode-extension/src/effectBoundary.ts

## Seven panels

| Panel | Webview → Extension | Extension → Webview | Contract module |
|-------|----|----|----|
| analysis | `ExportCsvMessage` | `PanelErrorMessage` | `webviewMessageContracts.ts` |
| idEditor | `IdEditorPanelSaveMessage` | `IdEditorPanelEntriesMessage \| IdEditorPanelSavedMessage \| PanelErrorMessage` | `webviewMessageContracts.ts` |
| media | `{segmentChanged, timestamp, stopped}` | `{rewind, setLoop, requestTimestamp, seekTo}` | `webviewContracts/mediaPanelContract.ts` |
| waveform | `WaveformPanelSeekMessage` | `WaveformPanelHighlightSegmentMessage` | `webviewMessageContracts.ts` |
| kideval | `{discoverDatabases, runAnalysis}` | `{fileInfo, databases, results, error}` | `webviewMessageContracts.ts` |
| graph | (none — one-shot DOT) | (none — one-shot DOT render) | `graphPanel.ts` inline |
| picture | (none) | (none) | `picturePanel.ts` inline |

## Shared shape: PanelErrorMessage

Every panel that can surface an extension-side failure uses the same
error-envelope type:

```ts
interface PanelErrorMessage {
    command: 'error';
    message: string;
}
```

`createIdEditorErrorMessage(msg)` and `createKidevalErrorMessage(msg)`
are thin constructors in `webviewMessageContracts.ts`.

## Analysis panel

CLAN analysis results viewer. One message direction: webview asks the
extension to export a visible table as CSV.

```ts
// Webview → Extension
interface ExportCsvMessage {
    command: 'exportCsv';
    csv: string;      // Pre-rendered CSV text
    filename: string; // Suggested save-as filename
}
type AnalysisPanelWebviewMessage = ExportCsvMessage;

// Extension → Webview
type AnalysisPanelExtensionMessage = PanelErrorMessage;
```

- **Decoder:** `decodeAnalysisPanelWebviewMessage(value) → AnalysisPanelWebviewMessage` in `webviewMessageContracts.ts`.
- **Renderer:** `src/webview/analysisPanel.html` + `analysisPanel.js`.
- **Panel controller:** `src/analysisPanel.ts`.

## ID editor panel

Edit `@ID` header entries for all participants in a document.

```ts
// Webview → Extension
interface IdEditorPanelSaveMessage {
    command: 'save';
    entries: IdLineFields[]; // All participant entries
}

// Extension → Webview
interface IdEditorPanelEntriesMessage {
    command: 'entries';
    entries: ParticipantEntry[];
}
interface IdEditorPanelSavedMessage {
    command: 'saved';
}
type IdEditorPanelExtensionMessage =
    | IdEditorPanelEntriesMessage
    | IdEditorPanelSavedMessage
    | PanelErrorMessage;
```

- **Decoder:** `decodeIdEditorPanelWebviewMessage` in `webviewMessageContracts.ts`.
- **Constructors:** `createIdEditorEntriesMessage`, `createIdEditorSavedMessage`, `createIdEditorErrorMessage`.
- **Renderer:** `src/webview/idEditorPanel.{html,js}`.
- **Panel controller:** `src/idEditorPanel.ts`.
- **Upstream data:** `talkbank/getParticipants` (RPC); save translates entries back into `@ID` lines via `talkbank/formatIdLine`.

## Media panel

Audio/video playback tied to utterance segments. Most message-rich
panel. Contract lives in its own module
(`webviewContracts/mediaPanelContract.ts`) after KIB-006.

```ts
// Webview → Extension
type MediaPanelWebviewMessage =
    | { command: 'segmentChanged'; index: number }
    | { command: 'timestamp'; ms: TimestampMs }
    | { command: 'stopped' };

// Extension → Webview
type MediaPanelExtensionMessage =
    | { command: 'rewind'; seconds: number }
    | { command: 'setLoop' }
    | { command: 'requestTimestamp' }
    | { command: 'seekTo'; ms: TimestampMs };
```

- **Decoder:** `decodeMediaPanelWebviewMessage` in `webviewContracts/mediaPanelContract.ts`.
- **Constructors:** `createMediaRewindMessage`, `createMediaSetLoopMessage`, `createMediaRequestTimestampMessage`, `createMediaSeekToMessage`.
- **Renderer:** `src/webview/mediaPanel.{html,js}`.
- **Panel controller:** `src/mediaPanel.ts`.

`TimestampMs` is a branded `number` newtype from `utils/bulletParser.ts`;
the decoder brands validated incoming values through `toTimestampMs`.

## Waveform panel

Web Audio API waveform visualization; click-to-seek + segment
highlight.

```ts
// Webview → Extension
interface WaveformPanelSeekMessage {
    command: 'seek';
    ms: TimestampMs;
}
type WaveformPanelWebviewMessage = WaveformPanelSeekMessage;

// Extension → Webview
interface WaveformPanelHighlightSegmentMessage {
    command: 'highlightSegment';
    index: number;
}
type WaveformPanelExtensionMessage = WaveformPanelHighlightSegmentMessage;
```

- **Decoder:** `decodeWaveformPanelWebviewMessage`.
- **Constructor:** `createWaveformHighlightSegmentMessage(index)`.
- **Renderer:** `src/webview/waveformPanel.{html,js}`.
- **Panel controller:** `src/waveformPanel.ts`.

## KidEval panel

Normative-assessment comparison against CLAN databases.

```ts
// Webview → Extension
type KidevalPanelWebviewMessage =
    | { command: 'discoverDatabases'; libraryDir: string }
    | { command: 'runAnalysis'; databasePath: string; ...options };

// Extension → Webview
type KidevalPanelExtensionMessage =
    | { command: 'fileInfo'; fileName: string }
    | { command: 'databases'; databases: AvailableDatabase[]; filter: AnalysisDatabaseFilter }
    | { command: 'results'; data: unknown }
    | PanelErrorMessage;
```

- **Decoder:** `decodeKidevalPanelWebviewMessage`.
- **Constructors:** `createKidevalFileInfoMessage`, `createKidevalDatabasesMessage`, `createKidevalResultsMessage`, `createKidevalErrorMessage`.
- **Renderer:** `src/webview/kidevalPanel.{html,js}`.
- **Panel controller:** `src/kidevalPanel.ts`.
- **Upstream RPC:** `discoverDatabases` message triggers `talkbank/kidevalDatabases`; `runAnalysis` triggers `talkbank/analyze` with `kideval` command name.

## Graph panel (no contract)

Renders a Graphviz DOT string from the LSP's
`talkbank/showDependencyGraph` endpoint. One-shot: the extension
posts the DOT source into the webview, which renders it with
`@hpcc-js/wasm` Graphviz. No response channel; the webview has no
state the extension needs.

- **Panel controller:** `src/graphPanel.ts`.
- **Renderer:** `src/webview/graphPanel.{html,js}`.
- **Upstream:** `talkbank/showDependencyGraph` (see
  [RPC Contracts](rpc-contracts.md#talkbankshowdependencygraph)).

## Picture panel (no contract)

Static image viewer for elicitation pictures (Cookie Theft, picture
description tasks). No JSON messages; the extension builds an HTML
page inline with one `<img>` tag pointing at a webview resource URI.

- **Panel controller:** `src/picturePanel.ts`.
- **Renderer:** built inline from TypeScript (no external HTML file).

## Decoding pattern

Every decoder uses the shared `decodePanelMessageWithSchema` helper in
`effectBoundary.ts`, which runs the Effect `Schema` against the
payload and throws `StructuredPayloadDecodeError` with a
panel-labeled reason on mismatch. Panel controllers translate the
error into a `PanelErrorMessage` sent back to the webview.

## Adding a new panel

1. Add the panel's message types + schemas + decoder + constructors.
   New panels should live under `src/webviewContracts/<panel>PanelContract.ts`
   (not in the umbrella `webviewMessageContracts.ts`); the umbrella
   file still exists as a re-export facade for historical shapes but
   is not the right home for new contracts. See KIB-006.
2. Add the panel controller under `src/<panel>Panel.ts` — follow the
   singleton pattern from `mediaPanel.ts` (`createOrShow`, panel
   disposal hooks, asset URI resolution via `panelAssets.ts`).
3. Add the panel HTML + JS under `src/webview/<panel>Panel.{html,js}`.
4. Register the open-panel command in
   `src/activation/commands/<family>.ts` via `registerEffectCommand`.
5. Declare the command in `package.json` `contributes.commands`.
