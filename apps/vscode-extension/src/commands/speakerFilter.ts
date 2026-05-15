/**
 * Command handler for LSP-backed speaker filtering.
 */

import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    VSCodeWindowService,
    VSCodeWorkspaceService,
    requireActiveEditor,
} from '../effectCommandRuntime';
import { ExecuteCommandClientService, tryAsync } from '../effectRuntime';

/**
 * Open a filtered read-only document that includes only the selected speakers.
 */
export function filterBySpeaker(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const window = yield* VSCodeWindowService;
        const workspace = yield* VSCodeWorkspaceService;
        const commands = yield* ExecuteCommandClientService;
        const uri = editor.document.uri.toString();

        const speakerResult = yield* commands.getSpeakers(uri);
        if (speakerResult.length === 0) {
            yield* Effect.asVoid(tryAsync('show warning message', () => Promise.resolve(
                window.showWarningMessage('No @Participants: header found in this file'),
            )));
            return;
        }

        const picked = yield* tryAsync('show speaker picker', () => Promise.resolve(
            window.showQuickPick(
                speakerResult.map(speaker => ({
                    label: speaker.code,
                    description: speaker.name ? `${speaker.name} (${speaker.role})` : speaker.role,
                })),
                {
                    canPickMany: true,
                    placeHolder: 'Select speaker(s) to show',
                },
            ) as Thenable<readonly vscode.QuickPickItem[] | undefined>,
        ));
        if (!picked || picked.length === 0) {
            return;
        }

        const filtered = yield* commands.filterDocument(uri, picked.map(item => item.label));
        const document = yield* tryAsync('open filtered speaker document', () => Promise.resolve(
            workspace.openTextDocument({
                content: filtered,
                language: 'chat',
            }),
        ));
        yield* Effect.asVoid(tryAsync('show filtered speaker document', () => Promise.resolve(
            window.showTextDocument(
                document,
                {
                    viewColumn: vscode.ViewColumn.Beside,
                    preview: true,
                },
            ),
        )));
    });
}
