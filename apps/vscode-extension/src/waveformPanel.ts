/**
 * waveformPanel.ts
 *
 * VS Code WebviewPanel for displaying an interactive waveform alongside the
 * CHAT transcript (CLAN's "Sonic view" equivalent).
 *
 * Architecture:
 * - The webview fetches the media file via its webview-safe URI, decodes the
 *   audio using the Web Audio API, and renders peak amplitude per pixel column
 *   as a canvas waveform.
 * - Each •beg_end• segment is overlaid as a semi-transparent coloured region.
 * - Clicking the canvas computes the click time in ms and posts a `seek` message
 *   to the extension, which moves the editor cursor to the nearest utterance line
 *   and forwards a `seekTo` message to the MediaPanel if it is open.
 * - When the MediaPanel advances to a new segment, the extension posts a
 *   `highlightSegment` message here to draw a bright playhead indicator.
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
import { Segment, TimestampMs } from './utils/bulletParser';
import {
    WaveformPanelExtensionMessage,
    decodeWaveformPanelWebviewMessage,
} from './webviewMessageContracts';

export class WaveformPanel {
    public static currentPanel: WaveformPanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private readonly _disposables: vscode.Disposable[] = [];
    private readonly _assetPaths: WebviewPanelAssetPaths;
    private readonly _panelDependencies: WebviewPanelDependencies;

    /** Callback invoked when the user clicks the waveform to seek. */
    private readonly _onSeek: (ms: TimestampMs) => void;

    // -----------------------------------------------------------------------
    // Static factory
    // -----------------------------------------------------------------------

    /**
     * Creates a new WaveformPanel or updates the existing one.
     *
     * @param context   - Extension context (for resource URIs).
     * @param segments  - All segments parsed from the document's bullets.
     * @param mediaPath - Absolute path to the audio/video file.
     * @param docUri    - URI of the source .cha document (for the title).
     * @param onSeek    - Called when the user clicks the waveform; receives ms.
     */
    public static createOrShow(
        context: vscode.ExtensionContext,
        segments: Segment[],
        mediaPath: string,
        docUri: vscode.Uri,
        onSeek: (ms: TimestampMs) => void,
        dependencies: WebviewPanelDependencies = {},
    ): void {
        const column = vscode.ViewColumn.Beside;
        const mediaDir = vscode.Uri.file(path.dirname(mediaPath));
        const assetPaths = resolveWebviewPanelAssetPaths(context, 'waveformPanel');

        reuseOrCreatePanel(
            WaveformPanel.currentPanel,
            currentPanel => {
                currentPanel._update(segments, mediaPath);
                currentPanel._panel.reveal(column);
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'talkbankWaveform',
                        title: `Waveform – ${path.basename(docUri.fsPath)}`,
                        column,
                        localResourceRoots: [mediaDir],
                    },
                    dependencies,
                );

                const currentPanel = new WaveformPanel(
                    panel,
                    assetPaths,
                    dependencies,
                    segments,
                    mediaPath,
                    onSeek,
                );
                WaveformPanel.currentPanel = currentPanel;
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
        segments: Segment[],
        mediaPath: string,
        onSeek: (ms: TimestampMs) => void,
    ) {
        this._panel = panel;
        this._assetPaths = assetPaths;
        this._panelDependencies = panelDependencies;
        this._onSeek = onSeek;

        this._panel.onDidDispose(() => this._dispose(), null, this._disposables);

        this._panel.webview.onDidReceiveMessage(
            (message: unknown) => {
                const decoded = decodePanelMessage(
                    message,
                    'waveform panel',
                    decodeWaveformPanelWebviewMessage,
                );
                if (decoded === undefined) {
                    return;
                }
                this._onSeek(decoded.ms);
            },
            null,
            this._disposables,
        );

        this._update(segments, mediaPath);
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /**
     * Send a message from the extension to the webview's JavaScript.
     *
     * Primary use: `{ command: 'highlightSegment', index: N }` to draw a
     * playhead on the currently-playing segment.
     *
     * @param msg - Typed waveform-panel control message.
     */
    public postMessage(msg: WaveformPanelExtensionMessage): void {
        this._panel.webview.postMessage(msg);
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    private _update(segments: Segment[], mediaPath: string): void {
        const mediaWebviewUri = this._panel.webview.asWebviewUri(vscode.Uri.file(mediaPath));
        this._panel.webview.html = this._getHtmlForWebview(
            segments,
            mediaWebviewUri.toString(),
        );
    }

    /**
     * Generates the webview HTML for the waveform panel.
     *
     * Rendering pipeline (JS side):
     *  1. `fetch(mediaUri)` → `audioContext.decodeAudioData()` → Float32Array.
     *  2. Downsample: one canvas column = (totalSamples / canvasWidth) samples.
     *     Compute the peak amplitude of each column's sample window.
     *  3. Draw the waveform using `fillRect` (peak value mapped to pixel height).
     *  4. Overlay segment regions as semi-transparent coloured rectangles.
     *  5. Canvas `click` → compute ms → post `{ command: 'seek', ms }`.
     *  6. On `{ command: 'highlightSegment', index }` → draw bright vertical
     *     line at the segment's centre x-position.
     */
    private _getHtmlForWebview(segments: Segment[], mediaUri: string): string {
        const webview = this._panel.webview;

        // Read the HTML template from disk.
        let html = readWebviewPanelTemplate(this._assetPaths.htmlPath, this._panelDependencies);

        // Build the webview-safe URI for the external JS file.
        const jsUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.scriptPath),
        );

        // Build the data injection script block.
        const dataScript = `<script>
    const SEGMENTS  = ${JSON.stringify(segments)};
    const MEDIA_URI = ${JSON.stringify(mediaUri)};
</script>`;

        // Replace placeholders.
        html = html.replace('<!--INJECT_DATA-->', dataScript);
        html = html.replace('<!--INJECT_SCRIPT-->', `<script src="${jsUri}"></script>`);

        return html;
    }

    // -----------------------------------------------------------------------
    // Disposal
    // -----------------------------------------------------------------------

    private _dispose(): void {
        disposePanelResources(this._panel, this._disposables, () => {
            WaveformPanel.currentPanel = undefined;
        });
    }
}
