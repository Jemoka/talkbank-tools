/**
 * Transcription mode — stamp timing bullets from live media playback.
 *
 * Manages transcription state (active flag + previous timestamp) and
 * coordinates with the MediaPanel for real-time timestamp requests.
 */

import * as path from 'path';
import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    MediaFileNotFoundError,
    MissingMediaHeaderError,
    PanelNotOpenError,
    VSCodeCommandsService,
    VSCodeWindowService,
    requireActiveEditor,
} from '../effectCommandRuntime';
import {
    ExecuteCommandClientService,
    ExtensionContextService,
    ExtensionRuntimeContextService,
    tryAsync,
} from '../effectRuntime';
import { TranscriptionCommandStateService } from '../mediaCommandState';
import { parseMediaHeader, Segment, TimestampMs } from '../utils/bulletParser';
import { resolveMediaPath } from '../utils/mediaResolver';
import { MediaPanel } from '../mediaPanel';
import { loadMediaSegments } from './media';
import { createMediaRequestTimestampMessage } from '../webviewMessageContracts';

export function stopTranscription(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const commands = yield* VSCodeCommandsService;
        const transcriptionState = yield* TranscriptionCommandStateService;

        yield* Effect.sync(() => {
            transcriptionState.reset();
        });

        yield* Effect.asVoid(tryAsync('set transcription inactive context', () => Promise.resolve(
            commands.executeCommand(
                'setContext',
                'talkbank.transcriptionActive',
                false,
            ),
        )));
        yield* Effect.sync(() => {
            MediaPanel.stop();
            window.setStatusBarMessage('Transcription stopped', 3000);
        });
    });
}

/**
 * Starts transcription mode: opens the MediaPanel with the entire file as one
 * segment (no auto-advance), sets `transcriptionActive = true`.
 */
export function startTranscription(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const context = yield* ExtensionContextService;
        const runtimeContext = yield* ExtensionRuntimeContextService;
        const window = yield* VSCodeWindowService;
        const commands = yield* VSCodeCommandsService;
        const transcriptionState = yield* TranscriptionCommandStateService;

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
        const startMs = segments.length > 0 ? segments[0].beg : 0 as TimestampMs;
        const fullSegment: Segment = {
            line: 0,
            beg: startMs,
            end: 999_999_999 as TimestampMs,
        };

        yield* Effect.sync(() => {
            MediaPanel.createOrShow(
                context,
                runtimeContext,
                [fullSegment],
                0,
                mediaPath,
                editor.document.uri,
            );
        });

        yield* Effect.sync(() => {
            transcriptionState.activate(startMs);
        });

        yield* Effect.asVoid(tryAsync('set transcription active context', () => Promise.resolve(
            commands.executeCommand(
                'setContext',
                'talkbank.transcriptionActive',
                true,
            ),
        )));
        yield* Effect.sync(() => {
            window.setStatusBarMessage(
                '$(record) Transcription active — press F4 to stamp a bullet',
                5000,
            );
        });
    });
}

/**
 * Stamps the current playback time as a bullet on the current editor line,
 * then inserts a new utterance line below.
 *
 * Requires an active MediaPanel with a loaded audio file.
 */
export function stampBullet(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const editor = yield* requireActiveEditor();
        const runtimeContext = yield* ExtensionRuntimeContextService;
        const transcriptionState = yield* TranscriptionCommandStateService;
        const state = yield* Effect.sync(() => transcriptionState.snapshot());

        if (!state.active) {
            yield* Effect.asVoid(tryAsync('show transcription inactive warning', () => Promise.resolve(
                window.showWarningMessage(
                    'Transcription mode is not active. Use Start Transcription first.',
                ),
            )));
            return;
        }

        const panel = MediaPanel.currentPanel;
        if (!panel) {
            return yield* Effect.fail(new PanelNotOpenError({ panel: 'media panel' }));
        }

        const currentMs = yield* requestTimestamp(panel, transcriptionState);
        const defaultSpeaker = runtimeContext.getTranscriptionDefaultSpeaker();
        const commandClient = yield* ExecuteCommandClientService;
        const formatted = yield* commandClient.formatBulletLine(
            state.previousTimestamp,
            currentMs,
            defaultSpeaker,
        );
        yield* Effect.sync(() => {
            transcriptionState.updatePreviousTimestamp(currentMs);
        });

        const cursorPos = editor.selection.active;
        yield* Effect.asVoid(tryAsync('stamp transcription bullet', () => Promise.resolve(
            editor.edit(editBuilder => {
                const lineEnd = editor.document.lineAt(cursorPos.line).range.end;
                editBuilder.insert(lineEnd, formatted.bullet);

                const eol = editor.document.eol === vscode.EndOfLine.CRLF ? '\r\n' : '\n';
                editBuilder.insert(lineEnd, eol + formatted.new_line);
            }),
        )));

        yield* Effect.sync(() => {
            const newLine = cursorPos.line + 1;
            const newPos = new vscode.Position(newLine, formatted.new_line.length);
            editor.selection = new vscode.Selection(newPos, newPos);
            editor.revealRange(new vscode.Range(newPos, newPos));
        });
    });
}

/**
 * Sends a `requestTimestamp` message to the MediaPanel webview and waits for
 * the `{ command: 'timestamp', ms }` response via a one-shot listener.
 */
function requestTimestamp(
    panel: MediaPanel,
    transcriptionState: import('../mediaCommandState').TranscriptionCommandStateStore,
): Effect.Effect<TimestampMs> {
    return Effect.async<TimestampMs>((resume) => {
        const disposable = panel.onNextMessage((message) => {
            if (message.command === 'timestamp') {
                clearTimeout(timeout);
                disposable.dispose();
                resume(Effect.succeed(message.ms));
            }
        });

        panel.postMessage(createMediaRequestTimestampMessage());
        const timeout = setTimeout(() => {
            disposable.dispose();
            resume(Effect.succeed(transcriptionState.snapshot().previousTimestamp));
        }, 1000);

        return Effect.sync(() => {
            clearTimeout(timeout);
            disposable.dispose();
        });
    });
}
