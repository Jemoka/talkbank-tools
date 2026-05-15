/**
 * Waveform view — Web Audio API visualization of the media file
 * with segment overlay and click-to-seek.
 */

import * as path from 'path';
import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    MediaFileNotFoundError,
    MissingMediaHeaderError,
    VSCodeWindowService,
    requireActiveEditor,
} from '../effectCommandRuntime';
import { ExtensionContextService } from '../effectRuntime';
import { parseMediaHeader, TimestampMs } from '../utils/bulletParser';
import { resolveMediaPath } from '../utils/mediaResolver';
import { MediaPanel } from '../mediaPanel';
import { WaveformPanel } from '../waveformPanel';
import {
    createMediaSeekToMessage,
    createWaveformHighlightSegmentMessage,
} from '../webviewMessageContracts';
import { loadMediaSegments } from './media';

export function showWaveform(): Effect.Effect<
    void,
    unknown,
    ExtensionCommandRequirements
> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const context = yield* ExtensionContextService;
        const window = yield* VSCodeWindowService;

        const text = editor.document.getText();
        const mediaName = parseMediaHeader(text);
        if (!mediaName) {
            return yield* Effect.fail(new MissingMediaHeaderError());
        }

        const docDir = path.dirname(editor.document.uri.fsPath);
        const mediaPath = resolveMediaPath(docDir, mediaName);
        if (!mediaPath) {
            return yield* Effect.fail(new MediaFileNotFoundError({ mediaName }));
        }

        const segments = yield* loadMediaSegments(editor.document);
        const docUriStr = editor.document.uri.toString();
        const docUri = editor.document.uri;

        function onWaveformSeek(ms: TimestampMs) {
            let nearestIdx = 0;
            let nearestDist = Infinity;
            for (let index = 0; index < segments.length; index++) {
                const midMs = (segments[index].beg + segments[index].end) / 2;
                const dist = Math.abs(midMs - ms);
                if (dist < nearestDist) {
                    nearestDist = dist;
                    nearestIdx = index;
                }
            }

            if (segments.length > 0) {
                const segment = segments[nearestIdx];
                const pos = new vscode.Position(segment.line, 0);
                const chatEditor = window.visibleTextEditors.find(
                    visibleEditor => visibleEditor.document.uri.toString() === docUriStr,
                );
                if (chatEditor) {
                    chatEditor.selection = new vscode.Selection(pos, pos);
                    chatEditor.revealRange(
                        new vscode.Range(pos, pos),
                        vscode.TextEditorRevealType.InCenterIfOutsideViewport,
                    );
                }

                WaveformPanel.currentPanel?.postMessage(
                    createWaveformHighlightSegmentMessage(nearestIdx),
                );
            }

            MediaPanel.currentPanel?.postMessage(createMediaSeekToMessage(ms));
        }

        yield* Effect.sync(() => {
            WaveformPanel.createOrShow(context, segments, mediaPath, docUri, onWaveformSeek);
        });
    });
}
