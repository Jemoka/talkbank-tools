/**
 * mediaPanel.ts
 *
 * VS Code WebviewPanel for CHAT media playback — audio and video both supported.
 * Follows the same structural pattern as GraphPanel.
 *
 * Responsibilities:
 * - Open a panel beside the editor with an <audio> or <video> element.
 * - Play a single segment or continuously play from a start index to EOF.
 * - During continuous play, poll currentTime every 100 ms (matching CLAN's
 *   approach) and advance to the next segment when the current one ends.
 * - Post `segmentChanged` messages back to the extension so the extension can
 *   move the editor cursor to the currently-playing utterance line.
 * - Accept inbound messages from the extension (rewind, setLoop, seekTo,
 *   requestTimestamp) and dispatch responses.
 */

import * as vscode from 'vscode';
import * as path from 'path';
import { decodePanelMessage, disposePanelResources, reuseOrCreatePanel } from './panelLifecycle';
import { resolveWebviewPanelAssetPaths, type WebviewPanelAssetPaths } from './panelAssets';
import {
    createExtensionWebviewPanel,
    readWebviewPanelTemplate,
    type WebviewPanelDependencies,
} from './panelHost';
import type { ExtensionRuntimeContext } from './runtimeContext';
import { Segment } from './utils/bulletParser';
import {
    MediaPanelExtensionMessage,
    MediaPanelWebviewMessage,
    decodeMediaPanelWebviewMessage,
} from './webviewMessageContracts';

/** Extensions treated as video (require <video> element; rest use <audio>). */
const VIDEO_EXTENSIONS = new Set(['.mov', '.mp4', '.m4v', '.avi', '.wmv', '.mpg']);

/** Callback type for one-shot message listeners (used by transcription mode). */
type WebviewMessageCallback = (msg: MediaPanelWebviewMessage) => void;

export class MediaPanel {
    public static currentPanel: MediaPanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private readonly _disposables: vscode.Disposable[] = [];
    private readonly _assetPaths: WebviewPanelAssetPaths;
    private readonly _panelDependencies: WebviewPanelDependencies;
    private readonly _runtimeContext: ExtensionRuntimeContext;

    /** All segments that were passed in at panel creation/update time. */
    private _segments: Segment[];

    /**
     * URI of the .cha document that triggered playback.
     * Used to locate the right text editor for cursor sync even when the
     * webview panel has focus (which un-sets activeTextEditor on older VS Code
     * versions).
     */
    private readonly _docUri: vscode.Uri;

    /**
     * One-shot message listeners registered via onNextMessage().
     * Each callback is invoked once for the next message from the webview,
     * then removed. Used by transcription mode to await timestamp responses.
     */
    private _messageCallbacks: Array<WebviewMessageCallback> = [];

    // -----------------------------------------------------------------------
    // Static factory
    // -----------------------------------------------------------------------

    /**
     * Creates a new MediaPanel or updates the existing one.
     *
     * @param context      - Extension context (for resource URIs).
     * @param segments     - Timed segments to play.
     * @param startIndex   - Index into `segments` at which playback begins.
     * @param mediaPath    - Absolute path to the audio/video file.
     * @param docUri       - URI of the source .cha document (for cursor sync).
     */
    public static createOrShow(
        context: vscode.ExtensionContext,
        runtimeContext: ExtensionRuntimeContext,
        segments: Segment[],
        startIndex: number,
        mediaPath: string,
        docUri: vscode.Uri,
        dependencies: WebviewPanelDependencies = {},
    ): void {
        const column = vscode.ViewColumn.Beside;
        const mediaDir = vscode.Uri.file(path.dirname(mediaPath));
        const assetPaths = resolveWebviewPanelAssetPaths(context, 'mediaPanel');

        reuseOrCreatePanel(
            MediaPanel.currentPanel,
            currentPanel => {
                currentPanel._segments = segments;
                currentPanel._update(segments, startIndex, mediaPath);
                currentPanel._panel.reveal(column);
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'talkbankMedia',
                        title: `Media – ${path.basename(docUri.fsPath)}`,
                        column,
                        // The media directory must be in localResourceRoots so that
                        // asWebviewUri() can convert the file:// path to a webview-safe URI.
                        localResourceRoots: [mediaDir],
                    },
                    dependencies,
                );

                const currentPanel = new MediaPanel(
                    panel,
                    assetPaths,
                    dependencies,
                    runtimeContext,
                    segments,
                    startIndex,
                    mediaPath,
                    docUri,
                );
                MediaPanel.currentPanel = currentPanel;
                return currentPanel;
            },
        );
    }

    // -----------------------------------------------------------------------
    // Constructor (private — use createOrShow)
    // -----------------------------------------------------------------------

    private constructor(
        panel: vscode.WebviewPanel,
        assetPaths: WebviewPanelAssetPaths,
        panelDependencies: WebviewPanelDependencies,
        runtimeContext: ExtensionRuntimeContext,
        segments: Segment[],
        startIndex: number,
        mediaPath: string,
        docUri: vscode.Uri,
    ) {
        this._panel = panel;
        this._assetPaths = assetPaths;
        this._panelDependencies = panelDependencies;
        this._runtimeContext = runtimeContext;
        this._segments = segments;
        this._docUri = docUri;

        // Clean up on panel close.
        this._panel.onDidDispose(() => this._dispose(), null, this._disposables);

        // Handle messages posted by the webview's JS (e.g. segmentChanged, timestamp).
        this._panel.webview.onDidReceiveMessage(
            (message: unknown) => {
                this._handleWebviewMessage(message);
            },
            null,
            this._disposables,
        );

        this._update(segments, startIndex, mediaPath);
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /** Stop playback and close the panel. */
    public static stop(): void {
        MediaPanel.currentPanel?._dispose();
    }

    /**
     * Send a message from the extension to the webview's JavaScript.
     *
     * Used by commands such as `talkbank.rewindMedia`, `talkbank.loopSegment`,
     * and `talkbank.startTranscription` to control playback state.
     *
     * @param msg - Typed media-panel control message.
     */
    public postMessage(msg: MediaPanelExtensionMessage): void {
        this._panel.webview.postMessage(msg);
    }

    /**
     * Register a one-shot callback for the next message received from the webview.
     *
     * The callback fires at most once — it is removed after the first invocation.
     * Returns a `vscode.Disposable` that, when disposed, removes the callback
     * before it fires (cancellation).
     *
     * Usage (transcription timestamp request):
     * ```typescript
     * const d = MediaPanel.currentPanel!.onNextMessage(msg => {
     *     if (msg.command === 'timestamp') { d.dispose(); resolve(msg.ms); }
     * });
     * MediaPanel.currentPanel!.postMessage({ command: 'requestTimestamp' });
     * ```
     *
     * @param cb - Callback invoked with the next webview message.
     * @returns Disposable that cancels the one-shot listener.
     */
    public onNextMessage(cb: WebviewMessageCallback): vscode.Disposable {
        this._messageCallbacks.push(cb);
        return new vscode.Disposable(() => {
            const idx = this._messageCallbacks.indexOf(cb);
            if (idx >= 0) {
                this._messageCallbacks.splice(idx, 1);
            }
        });
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /**
     * Handles messages sent from the webview's JavaScript to the extension.
     *
     * `segmentChanged`: The webview has advanced to a new segment during
     * continuous playback.  Move the editor cursor to the utterance line so
     * the active utterance stays visible in the transcript.
     *
     * `timestamp`: Response to a `requestTimestamp` message from the extension.
     * Dispatched to any registered one-shot message callbacks.
     *
     * All messages are also dispatched to one-shot callbacks registered via
     * `onNextMessage()`, enabling the promise-based timestamp request pattern
     * used by transcription mode.
     */
    private _handleWebviewMessage(message: unknown): void {
        const decoded = decodePanelMessage(
            message,
            'media panel',
            decodeMediaPanelWebviewMessage,
        );
        if (decoded === undefined) {
            return;
        }

        if (decoded.command === 'segmentChanged') {
            const seg = this._segments[decoded.index];
            if (!seg) {
                return;
            }

            // Find the .cha editor even when the webview has keyboard focus.
            const editor = vscode.window.visibleTextEditors.find(
                (e: vscode.TextEditor) => e.document.uri.toString() === this._docUri.toString(),
            );
            if (!editor) {
                return;
            }

            const pos = new vscode.Position(seg.line, 0);
            editor.selection = new vscode.Selection(pos, pos);
            editor.revealRange(
                new vscode.Range(pos, pos),
                vscode.TextEditorRevealType.InCenterIfOutsideViewport,
            );
        }

        // Dispatch to any registered one-shot callbacks (e.g. for 'timestamp').
        // Each callback is invoked once and then removed.
        const callbacks = this._messageCallbacks.splice(0);
        for (const cb of callbacks) {
            cb(decoded);
        }
    }

    /** Re-renders the webview HTML with new playback parameters. */
    private _update(segments: Segment[], startIndex: number, mediaPath: string): void {
        const isVideo = VIDEO_EXTENSIONS.has(path.extname(mediaPath).toLowerCase());
        const mediaWebviewUri = this._panel.webview.asWebviewUri(vscode.Uri.file(mediaPath));
        this._panel.webview.html = this._getHtmlForWebview(
            segments,
            startIndex,
            mediaWebviewUri.toString(),
            isVideo,
        );
    }

    /**
     * Builds the complete HTML document for the webview.
     *
     * The segments array and startIndex are embedded directly as JSON (same
     * technique as GraphPanel's dotSource) to avoid a separate init message.
     *
     * Playback loop (JS side):
     *  - Seek to seg.beg / 1000, call media.play().
     *  - Poll currentTime every 100 ms.
     *  - When currentTime >= seg.end / 1000:
     *      - If looping: re-seek to same segment start and continue.
     *      - If more segments remain: advance index, post segmentChanged, repeat.
     *      - Otherwise: pause and post stopped.
     *
     * Inbound messages from the extension (via window.addEventListener('message')):
     *  - `{ command: 'rewind', seconds }` — seek backwards by N seconds.
     *  - `{ command: 'setLoop' }` — toggle segment loop on/off.
     *  - `{ command: 'requestTimestamp' }` — respond with current time in ms.
     *  - `{ command: 'seekTo', ms }` — seek to absolute time in ms.
     */
    private _getHtmlForWebview(
        segments: Segment[],
        startIndex: number,
        mediaUri: string,
        isVideo: boolean,
    ): string {
        const webview = this._panel.webview;

        // Read the HTML template from disk.
        let html = readWebviewPanelTemplate(this._assetPaths.htmlPath, this._panelDependencies);

        // Build the webview-safe URI for the external JS file.
        const jsUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.scriptPath),
        );

        // Build the media tag (audio vs video).
        const mediaTag = isVideo
            ? `<video id="media" controls></video>`
            : `<audio id="media" controls></audio>`;

        // Read user configuration for media defaults.
        const defaultSpeed = this._runtimeContext.getMediaDefaultSpeed();
        const loopCount = this._runtimeContext.getWalkerLoopCount();
        const pauseSeconds = this._runtimeContext.getWalkerPauseSeconds();
        const walkLength = this._runtimeContext.getWalkerWalkLength();

        // Build the data injection script block.
        const dataScript = `<script>
    const SEGMENTS      = ${JSON.stringify(segments)};
    const MEDIA_URI     = ${JSON.stringify(mediaUri)};
    const START_IDX     = ${JSON.stringify(startIndex)};
    const IS_VIDEO      = ${JSON.stringify(isVideo)};
    const DEFAULT_SPEED = ${JSON.stringify(defaultSpeed)};
    const LOOP_COUNT    = ${JSON.stringify(loopCount)};
    const PAUSE_MS      = ${JSON.stringify(Math.round(pauseSeconds * 1000))};
    const WALK_LENGTH   = ${JSON.stringify(walkLength)};
</script>`;

        // Replace placeholders.
        html = html.replace('<!--INJECT_MEDIA_TAG-->', mediaTag);
        html = html.replace('<!--INJECT_DATA-->', dataScript);
        html = html.replace('<!--INJECT_SCRIPT-->', `<script src="${jsUri}"></script>`);

        return html;
    }

    // -----------------------------------------------------------------------
    // Disposal
    // -----------------------------------------------------------------------

    private _dispose(): void {
        disposePanelResources(this._panel, this._disposables, () => {
            MediaPanel.currentPanel = undefined;
        });
    }
}
