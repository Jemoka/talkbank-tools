import * as vscode from 'vscode';
import { Effect } from 'effect';

import { playBullet, playContinuous } from '../../commands/media';
import { startTranscription, stampBullet, stopTranscription } from '../../commands/transcription';
import { clearWalkerStateForDocument, walkerStep } from '../../commands/walker';
import { showWaveform } from '../../commands/waveform';
import { MediaPanel } from '../../mediaPanel';
import { createMediaRewindMessage, createMediaSetLoopMessage } from '../../webviewMessageContracts';
import {
    ExtensionCommandRunner,
    PanelNotOpenError,
    VSCodeCommandsService,
    registerEffectCommand,
} from '../../effectCommandRuntime';
import { ExtensionRuntimeContextService, tryAsync } from '../../effectRuntime';
import type { ExtensionServices } from './types';

/**
 * Register media, waveform, and traversal commands.
 *
 * @param context - Extension activation context.
 * @param services - Shared runtime services.
 * @returns Media-related disposables.
 */
export function registerMediaCommands(
    context: vscode.ExtensionContext,
    _services: ExtensionServices,
    runner: ExtensionCommandRunner,
): vscode.Disposable[] {
    return [
        registerEffectCommand('talkbank.playBullet', runner, () => playBullet()),
        registerEffectCommand('talkbank.playContinuous', runner, () => playContinuous()),
        registerEffectCommand('talkbank.stopPlayback', runner, () => Effect.sync(() => {
            MediaPanel.stop();
        })),
        registerEffectCommand('talkbank.rewindMedia', runner, () => Effect.gen(function*() {
            const runtimeContext = yield* ExtensionRuntimeContextService;
            if (!MediaPanel.currentPanel) {
                return yield* Effect.fail(new PanelNotOpenError({ panel: 'media panel' }));
            }

            const seconds = runtimeContext.getTranscriptionRewindSeconds();
            yield* Effect.sync(() => {
                MediaPanel.currentPanel?.postMessage(createMediaRewindMessage(seconds));
            });
        })),
        registerEffectCommand('talkbank.loopSegment', runner, () => Effect.try({
            try: () => {
                if (!MediaPanel.currentPanel) {
                    throw new PanelNotOpenError({ panel: 'media panel' });
                }
                MediaPanel.currentPanel.postMessage(createMediaSetLoopMessage());
            },
            catch: cause => cause,
        })),
        registerEffectCommand('talkbank.walkerNext', runner, () => walkerStep(+1)),
        registerEffectCommand('talkbank.walkerPrev', runner, () => walkerStep(-1)),
        vscode.workspace.onDidCloseTextDocument(doc => {
            runner.fireAndForget(
                'clear walker state on document close',
                clearWalkerStateForDocument(doc.uri.toString()),
            );
        }),
        vscode.workspace.onDidChangeTextDocument(event => {
            runner.fireAndForget(
                'clear walker state on document change',
                clearWalkerStateForDocument(event.document.uri.toString()),
            );
        }),
        registerEffectCommand('talkbank.startTranscription', runner, () => startTranscription()),
        registerEffectCommand('talkbank.stampBullet', runner, () => stampBullet()),
        registerEffectCommand('talkbank.stopTranscription', runner, () => stopTranscription()),
        registerEffectCommand('talkbank.showWaveform', runner, () => showWaveform()),
        registerEffectCommand('talkbank.configureTranscriptionKeys', runner, () => Effect.gen(function*() {
            const commands = yield* VSCodeCommandsService;
            yield* Effect.asVoid(tryAsync('open transcription keybindings', () => Promise.resolve(
                commands.executeCommand(
                    'workbench.action.openGlobalKeybindings',
                    'talkbank stamp rewind loop transcription',
                ),
            )));
        })),
    ];
}
