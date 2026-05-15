/**
 * Media playback commands and segment loading.
 *
 * Provides the shared `loadMediaSegments()` helper (used by walker,
 * transcription, and waveform modules) plus the single-bullet and
 * continuous playback command handlers.
 */

import * as path from 'path';
import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionContextService,
    ExecuteCommandClientService,
    ExtensionRuntimeContextService,
} from '../effectRuntime';
import {
    ExtensionCommandRequirements,
    MediaFileNotFoundError,
    MissingMediaHeaderError,
    NoBulletMarkerNearCursorError,
    NoBulletMarkersError,
    requireActiveEditor,
} from '../effectCommandRuntime';
import { parseMediaHeader, parseBullets, findNearestBullet, Segment } from '../utils/bulletParser';
import { segmentsFromAlignmentSidecar } from '../utils/alignmentSidecar';
import { resolveMediaPath } from '../utils/mediaResolver';
import { MediaPanel } from '../mediaPanel';

/**
 * Load media segments for a CHAT document.
 *
 * Prefers LSP alignment sidecar timings and falls back to local bullet parsing
 * if sidecar data is unavailable.
 */
export function loadMediaSegments(
    document: vscode.TextDocument,
): Effect.Effect<Segment[], never, ExtensionCommandRequirements> {
    return Effect.flatMap(loadMediaSegmentsFromSidecar(document.uri), sidecarSegments => sidecarSegments !== null
        ? Effect.succeed(sidecarSegments)
        : Effect.sync(() => parseBullets(document.getText())));
}

/**
 * Request alignment/timing sidecar from LSP and convert it to playback segments.
 */
function loadMediaSegmentsFromSidecar(
    uri: vscode.Uri,
): Effect.Effect<Segment[] | null, never, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const commands = yield* ExecuteCommandClientService;
        if (!commands.isRunning()) {
            return null;
        }

        const sidecar = yield* commands.getAlignmentSidecar(uri.toString()).pipe(
            Effect.catchAll(error => Effect.sync(() => {
                console.warn('Failed to load alignment sidecar:', error);
                return null;
            })),
        );
        return segmentsFromAlignmentSidecar(sidecar);
    });
}

/**
 * Plays the single bullet segment nearest to (and not after) the cursor.
 */
export function playBullet(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const context = yield* ExtensionContextService;
        const runtimeContext = yield* ExtensionRuntimeContextService;

        const text = editor.document.getText();
        const cursorLine = editor.selection.active.line;
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
        const nearest = findNearestBullet(segments, cursorLine);
        if (!nearest) {
            return yield* Effect.fail(new NoBulletMarkerNearCursorError());
        }

        yield* Effect.sync(() => {
            MediaPanel.createOrShow(
                context,
                runtimeContext,
                [nearest],
                0,
                mediaPath,
                editor.document.uri,
            );
        });
    });
}

/**
 * Continuously plays all bullet segments from the one nearest the cursor
 * through to the end of the file.
 */
export function playContinuous(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const context = yield* ExtensionContextService;
        const runtimeContext = yield* ExtensionRuntimeContextService;

        const text = editor.document.getText();
        const cursorLine = editor.selection.active.line;
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
        if (segments.length === 0) {
            return yield* Effect.fail(new NoBulletMarkersError());
        }

        const nearest = findNearestBullet(segments, cursorLine);
        const startIndex = nearest ? segments.indexOf(nearest) : 0;

        yield* Effect.sync(() => {
            MediaPanel.createOrShow(
                context,
                runtimeContext,
                segments,
                startIndex,
                mediaPath,
                editor.document.uri,
            );
        });
    });
}
