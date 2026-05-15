import * as vscode from 'vscode';
import { Effect } from 'effect';

import { showDependencyGraph } from '../../commands/dependencyGraph';
import { showPicture } from '../../commands/picture';
import {
    ExtensionCommandRunner,
    VSCodeWindowService,
    registerEffectCommand,
} from '../../effectCommandRuntime';
import {
    CacheManagerService,
    ClanIntegrationService,
    tryAsync,
} from '../../effectRuntime';
import { cancelCompose, enterComposeMode } from '../../specialChars';
import type { ExtensionServices } from './types';

/**
 * Register cache, CLAN, and other utility commands.
 *
 * @param context - Extension activation context.
 * @param services - Shared runtime services.
 * @returns Utility-related disposables.
 */
export function registerUtilityCommands(
    _context: vscode.ExtensionContext,
    _services: ExtensionServices,
    runner: ExtensionCommandRunner,
): vscode.Disposable[] {
    return [
        registerEffectCommand('talkbank.showDependencyGraph', runner, () => showDependencyGraph()),
        registerEffectCommand('talkbank.viewCacheStatistics', runner, () => Effect.gen(function*() {
            const cacheManager = yield* CacheManagerService;
            yield* Effect.asVoid(tryAsync('view cache statistics', () => cacheManager.viewCacheStatistics()));
        })),
        registerEffectCommand('talkbank.clearAllCache', runner, () => Effect.gen(function*() {
            const cacheManager = yield* CacheManagerService;
            yield* Effect.asVoid(tryAsync('clear cache', () => cacheManager.clearCache()));
        })),
        registerEffectCommand('talkbank.openInClan', runner, (fileUri?: vscode.Uri) => Effect.gen(function*() {
            const window = yield* VSCodeWindowService;
            const clanIntegration = yield* ClanIntegrationService;
            const uri = fileUri ?? window.activeTextEditor?.document.uri;
            if (!uri) {
                yield* Effect.asVoid(tryAsync('show missing file error', () => Promise.resolve(
                    window.showErrorMessage('No file selected'),
                )));
                return;
            }

            const editor = window.activeTextEditor;
            const line = editor ? editor.selection.active.line + 1 : undefined;
            const column = editor ? editor.selection.active.character + 1 : undefined;
            yield* Effect.asVoid(tryAsync(
                'open in clan',
                () => clanIntegration.openInClan(uri.fsPath, line, column),
            ));
        })),
        registerEffectCommand('talkbank.showPicture', runner, () => showPicture()),
        registerEffectCommand('talkbank.composeCA', runner, () => Effect.sync(() => {
            enterComposeMode('ca');
        })),
        registerEffectCommand('talkbank.composeChat', runner, () => Effect.sync(() => {
            enterComposeMode('nonca');
        })),
        registerEffectCommand('talkbank.cancelCompose', runner, () => Effect.sync(() => {
            cancelCompose();
        })),
    ];
}
