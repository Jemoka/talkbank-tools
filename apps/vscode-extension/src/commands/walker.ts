/**
 * Walker mode — step through utterances one segment at a time.
 *
 * Maintains per-document state (segment list + current index) and
 * optionally auto-plays each segment via the MediaPanel.
 */

import * as path from 'path';
import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    NoBulletMarkersError,
    requireActiveEditor,
} from '../effectCommandRuntime';
import {
    ExtensionContextService,
    ExtensionRuntimeContextService,
} from '../effectRuntime';
import { WalkerCommandStateService } from '../mediaCommandState';
import { parseMediaHeader } from '../utils/bulletParser';
import { resolveMediaPath } from '../utils/mediaResolver';
import { MediaPanel } from '../mediaPanel';
import { loadMediaSegments } from './media';

/**
 * Steps through utterances one segment at a time.
 *
 * @param delta - +1 for next, -1 for previous.
 */
export function walkerStep(
    delta: number,
): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const context = yield* ExtensionContextService;
        const runtimeContext = yield* ExtensionRuntimeContextService;
        const walkerState = yield* WalkerCommandStateService;
        const docUri = editor.document.uri.toString();

        let state = yield* Effect.sync(() => walkerState.get(docUri));
        if (!state || state.segments.length === 0) {
            const segments = yield* loadMediaSegments(editor.document);
            state = yield* Effect.sync(() => walkerState.initialize(docUri, segments));
        }

        if (!state) {
            return yield* Effect.dieMessage('Walker state unavailable after initialization');
        }

        if (state.segments.length === 0) {
            return yield* Effect.fail(new NoBulletMarkersError());
        }

        const nextState = yield* Effect.sync(() => walkerState.advance(docUri, delta) ?? state);
        const segment = nextState.segments[nextState.index];

        const pos = new vscode.Position(segment.line, 0);
        yield* Effect.sync(() => {
            editor.selection = new vscode.Selection(pos, pos);
            editor.revealRange(
                new vscode.Range(pos, pos),
                vscode.TextEditorRevealType.InCenterIfOutsideViewport,
            );
        });

        const autoPlay = runtimeContext.getWalkerAutoPlay();
        if (!autoPlay) {
            return;
        }

        const mediaName = parseMediaHeader(editor.document.getText());
        if (!mediaName) {
            return;
        }

        const docDir = path.dirname(editor.document.uri.fsPath);
        const mediaPath = resolveMediaPath(docDir, mediaName);
        if (!mediaPath) {
            return;
        }

        yield* Effect.sync(() => {
            MediaPanel.createOrShow(
                context,
                runtimeContext,
                [segment],
                0,
                mediaPath,
                editor.document.uri,
            );
        });
    });
}

/**
 * Clear any remembered walker state for one document URI.
 */
export function clearWalkerStateForDocument(
    documentUri: string,
): Effect.Effect<void, never, ExtensionCommandRequirements> {
    return Effect.flatMap(WalkerCommandStateService, walkerState => Effect.sync(() => {
        walkerState.clear(documentUri);
    }));
}
