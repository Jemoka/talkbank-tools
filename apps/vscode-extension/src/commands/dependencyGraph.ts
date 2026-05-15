/**
 * Command handler for dependency-graph generation.
 */

import * as vscode from 'vscode';
import { Effect } from 'effect';

import { GraphPanel } from '../graphPanel';
import {
    ExtensionCommandRequirements,
    requireActiveEditor,
} from '../effectCommandRuntime';
import {
    ExecuteCommandClientService,
    ExtensionContextService,
} from '../effectRuntime';

/**
 * Generate and show the dependency graph for the current cursor position.
 *
 * The LSP returns a discriminated response. A `dot` variant is rendered in a
 * graph webview; an `unavailable` variant (e.g., "No %mor tier found") is
 * surfaced as a plain information message so the Graphviz renderer never sees
 * non-DOT text.
 */
export function showDependencyGraph(): Effect.Effect<
    void,
    unknown,
    ExtensionCommandRequirements
> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveEditor();
        const commands = yield* ExecuteCommandClientService;
        const context = yield* ExtensionContextService;
        const document = editor.document;
        const position = editor.selection.active;

        const response = yield* commands.showDependencyGraph(
            document.uri.toString(),
            position,
        );

        yield* Effect.sync(() => {
            if (response.kind === 'dot') {
                GraphPanel.createOrShow(context, response.source, document.fileName);
            } else {
                vscode.window.showInformationMessage(
                    `Dependency graph unavailable: ${response.reason}`,
                );
            }
        });
    });
}
