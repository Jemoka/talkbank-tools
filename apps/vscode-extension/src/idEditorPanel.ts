/**
 * idEditorPanel.ts
 *
 * VS Code WebviewPanel for editing @ID participant headers in CHAT files.
 * Delegates parsing and serialization to the LSP via `talkbank/getParticipants`
 * and `talkbank/formatIdLine` commands, keeping TypeScript as a thin UI layer.
 *
 * @ID format: @ID:\tlang|corpus|code|age|sex|group|ses|role|education|custom|
 */

import * as vscode from 'vscode';
import * as path from 'path';
import { Effect } from 'effect';
import { ParticipantEntry, TalkbankExecuteCommandClient } from './lsp/executeCommandClient';
import { resolveWebviewPanelAssetPaths, type WebviewPanelAssetPaths } from './panelAssets';
import {
    createExtensionWebviewPanel,
    readWebviewPanelTemplate,
    type WebviewPanelDependencies,
} from './panelHost';
import { decodePanelMessage, disposePanelResources, reuseOrCreatePanel } from './panelLifecycle';
import {
    IdEditorPanelExtensionMessage,
    createIdEditorEntriesMessage,
    createIdEditorErrorMessage,
    createIdEditorSavedMessage,
    decodeIdEditorPanelWebviewMessage,
} from './webviewMessageContracts';
import {
    DocumentClosedError,
    ExtensionCommandRequirements,
    ExtensionCommandRunner,
    VSCodeWorkspaceService,
    runPanelEffect,
} from './effectCommandRuntime';
import { tryAsync } from './effectRuntime';

/** Parsed representation of a single @ID line (matches LSP `ParticipantEntry`). */
export type IdEntry = ParticipantEntry;

export class IdEditorPanel {
    public static currentPanel: IdEditorPanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private readonly _disposables: vscode.Disposable[] = [];
    private readonly _assetPaths: WebviewPanelAssetPaths;
    private readonly _panelDependencies: WebviewPanelDependencies;
    private readonly _commands: TalkbankExecuteCommandClient;
    private _docUri: vscode.Uri;

    // -----------------------------------------------------------------------
    // Static factory
    // -----------------------------------------------------------------------

    public static createOrShow(
        context: vscode.ExtensionContext,
        runner: ExtensionCommandRunner,
        document: vscode.TextDocument,
        commands: TalkbankExecuteCommandClient,
        dependencies: WebviewPanelDependencies = {},
    ): void {
        const column = vscode.ViewColumn.Beside;
        const assetPaths = resolveWebviewPanelAssetPaths(context, 'idEditorPanel');

        reuseOrCreatePanel(
            IdEditorPanel.currentPanel,
            currentPanel => {
                currentPanel._docUri = document.uri;
                currentPanel._panel.reveal(column);
                runPanelEffect(
                    'ID editor refresh',
                    runner,
                    currentPanel._sendEntries(document),
                    errorMessage => {
                        currentPanel._postMessage(createIdEditorErrorMessage(errorMessage));
                    },
                );
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'talkbankIdEditor',
                        title: `Participants – ${path.basename(document.fileName)}`,
                        column,
                    },
                    dependencies,
                );

                const currentPanel = new IdEditorPanel(
                    panel,
                    runner,
                    assetPaths,
                    dependencies,
                    document,
                    commands,
                );
                IdEditorPanel.currentPanel = currentPanel;
                return currentPanel;
            },
        );
    }

    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    private constructor(
        panel: vscode.WebviewPanel,
        runner: ExtensionCommandRunner,
        assetPaths: WebviewPanelAssetPaths,
        panelDependencies: WebviewPanelDependencies,
        document: vscode.TextDocument,
        commands: TalkbankExecuteCommandClient,
    ) {
        this._panel = panel;
        this._assetPaths = assetPaths;
        this._panelDependencies = panelDependencies;
        this._docUri = document.uri;
        this._commands = commands;

        this._panel.webview.html = this._getHtml();
        this._panel.onDidDispose(() => this._dispose(), null, this._disposables);

        this._panel.webview.onDidReceiveMessage(
            (message: unknown) => {
                const msg = decodePanelMessage(
                    message,
                    'ID editor panel',
                    decodeIdEditorPanelWebviewMessage,
                );
                if (msg === undefined) {
                    return;
                }

                runPanelEffect(
                    'ID editor panel',
                    runner,
                    this._applyEdits(msg.entries),
                    errorMessage => {
                        this._postMessage(createIdEditorErrorMessage(errorMessage));
                    },
                );
            },
            null,
            this._disposables,
        );

        runPanelEffect(
            'ID editor initial load',
            runner,
            this._sendEntries(document),
            errorMessage => {
                this._postMessage(createIdEditorErrorMessage(errorMessage));
            },
        );
    }

    // -----------------------------------------------------------------------
    // Fetch participants from LSP and send to webview
    // -----------------------------------------------------------------------

    private _sendEntries(
        document: vscode.TextDocument,
    ): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
        const panel = this;
        panel._panel.title = `Participants – ${path.basename(document.fileName)}`;

        return Effect.gen(function*() {
            const entries = yield* panel._commands.getParticipants(document.uri.toString());
            panel._postMessage(createIdEditorEntriesMessage(entries, path.basename(document.fileName)));
        });
    }

    // -----------------------------------------------------------------------
    // Apply edits back to document via LSP-formatted @ID lines
    // -----------------------------------------------------------------------

    private _applyEdits(
        entries: IdEntry[],
    ): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
        const panel = this;
        return Effect.gen(function*() {
            const workspace = yield* VSCodeWorkspaceService;
            const doc = workspace.textDocuments.find(
                document => document.uri.toString() === panel._docUri.toString(),
            );
            if (!doc) {
                return yield* Effect.fail(new DocumentClosedError());
            }

            const edit = new vscode.WorkspaceEdit();

            for (const entry of entries) {
                if (entry.line < 0 || entry.line >= doc.lineCount) {
                    continue;
                }

                const line = doc.lineAt(entry.line);
                const formatted = yield* panel._commands.formatIdLine(entry.fields);
                edit.replace(panel._docUri, line.range, formatted);
            }

            const success = yield* tryAsync('apply participant edits', () => Promise.resolve(
                workspace.applyEdit(edit),
            ));
            if (!success) {
                panel._postMessage(createIdEditorErrorMessage('Failed to apply edits.'));
                return;
            }

            panel._postMessage(createIdEditorSavedMessage());
            const updatedDoc = workspace.textDocuments.find(
                document => document.uri.toString() === panel._docUri.toString(),
            );
            if (updatedDoc) {
                yield* panel._sendEntries(updatedDoc);
            }
        });
    }

    /**
     * Send one typed extension-originated message to the ID editor webview.
     */
    private _postMessage(message: IdEditorPanelExtensionMessage): void {
        this._panel.webview.postMessage(message);
    }

    // -----------------------------------------------------------------------
    // HTML
    // -----------------------------------------------------------------------

    private _getHtml(): string {
        const webview = this._panel.webview;
        let html = readWebviewPanelTemplate(this._assetPaths.htmlPath, this._panelDependencies);

        const jsUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.scriptPath),
        );

        html = html.replace('<!--INJECT_SCRIPT-->', `<script src="${jsUri}"></script>`);
        return html;
    }

    // -----------------------------------------------------------------------
    // Disposal
    // -----------------------------------------------------------------------

    private _dispose(): void {
        disposePanelResources(this._panel, this._disposables, () => {
            IdEditorPanel.currentPanel = undefined;
        });
    }
}
