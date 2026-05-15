/**
 * kidevalPanel.ts
 *
 * VS Code WebviewPanel for KidEval and Eval analysis with normative database
 * comparison. Both commands share the same UI pattern: database selector,
 * age/gender filtering, comparison table with z-scores.
 *
 * The `mode` parameter controls which LSP command is invoked and which
 * default database directory is used.
 *
 * Communication is bidirectional via PostMessage:
 *   Extension → Webview: databases, results, errors, config
 *   Webview → Extension: discoverDatabases, runAnalysis
 */

import * as vscode from 'vscode';
import * as path from 'path';
import { Effect } from 'effect';
import {
    AnalysisOptions,
    DatabaseDiscoveryCommand,
    ExecuteCommandServerError,
    TalkbankExecuteCommandClient,
} from './lsp/executeCommandClient';
import { decodePanelMessage, disposePanelResources, reuseOrCreatePanel } from './panelLifecycle';
import {
    KidevalPanelExtensionMessage,
    KidevalPanelRunAnalysisMessage,
    createKidevalDatabasesMessage,
    createKidevalErrorMessage,
    createKidevalFileInfoMessage,
    createKidevalResultsMessage,
    decodeKidevalPanelWebviewMessage,
} from './webviewMessageContracts';
import { resolveWebviewPanelAssetPaths, type WebviewPanelAssetPaths } from './panelAssets';
import {
    ExtensionCommandRequirements,
    ExtensionCommandRunner,
    VSCodeWindowService,
    VSCodeWorkspaceService,
    runPanelEffect,
} from './effectCommandRuntime';
import { tryAsync } from './effectRuntime';
import {
    createExtensionWebviewPanel,
    readWebviewPanelTemplate,
    type WebviewPanelDependencies,
} from './panelHost';

/** Analysis mode — determines command name, lib dir, and display labels. */
export type EvalMode = 'kideval' | 'eval' | 'evald';

interface ModeConfig {
    title: string;
    description: string;
    lspDiscoverCommand: DatabaseDiscoveryCommand;
    analyzeCommand: string;
    defaultLibDir: string;
}

const MODE_CONFIGS: Record<EvalMode, ModeConfig> = {
    kideval: {
        title: 'Child Evaluation',
        description: 'DSS + IPSyn + MLU combined child language measures with normative database comparison',
        lspDiscoverCommand: 'talkbank/kidevalDatabases',
        analyzeCommand: 'kideval',
        defaultLibDir: '/Users/Shared/CLAN/lib/kideval',
    },
    eval: {
        title: 'Language Evaluation',
        description: 'Comprehensive morphosyntactic evaluation with normative database comparison',
        lspDiscoverCommand: 'talkbank/evalDatabases',
        analyzeCommand: 'eval',
        defaultLibDir: '/Users/Shared/CLAN/lib/eval',
    },
    evald: {
        title: 'Dementia Evaluation',
        description: 'Language evaluation with DementiaBank normative database comparison',
        lspDiscoverCommand: 'talkbank/evalDatabases',
        analyzeCommand: 'eval-d',
        defaultLibDir: '/Users/Shared/CLAN/lib/eval',
    },
};

export class KidevalPanel {
    public static currentPanel: KidevalPanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private readonly _disposables: vscode.Disposable[] = [];
    private readonly _assetPaths: WebviewPanelAssetPaths;
    private readonly _panelDependencies: WebviewPanelDependencies;
    private readonly _mode: EvalMode;
    private _fileUri: string;
    private _filePath: string;

    // -----------------------------------------------------------------------
    // Static factory
    // -----------------------------------------------------------------------

    public static createOrShow(
        context: vscode.ExtensionContext,
        runner: ExtensionCommandRunner,
        fileUri: string,
        filePath: string,
        commands: TalkbankExecuteCommandClient,
        mode: EvalMode = 'kideval',
        dependencies: WebviewPanelDependencies = {},
    ): void {
        const column = vscode.ViewColumn.Beside;
        const modeConfig = MODE_CONFIGS[mode];
        const assetPaths = resolveWebviewPanelAssetPaths(context, 'kidevalPanel');

        reuseOrCreatePanel(
            KidevalPanel.currentPanel,
            currentPanel => {
                currentPanel._fileUri = fileUri;
                currentPanel._filePath = filePath;
                currentPanel._panel.reveal(column);
                currentPanel._sendFileInfo();
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'talkbankKideval',
                        title: `${modeConfig.title} – ${path.basename(filePath)}`,
                        column,
                    },
                    dependencies,
                );

                const currentPanel = new KidevalPanel(
                    panel,
                    runner,
                    assetPaths,
                    dependencies,
                    fileUri,
                    filePath,
                    commands,
                    mode,
                );
                KidevalPanel.currentPanel = currentPanel;
                return currentPanel;
            },
        );
    }

    // -----------------------------------------------------------------------
    // Constructor (private — use createOrShow)
    // -----------------------------------------------------------------------

    private constructor(
        panel: vscode.WebviewPanel,
        runner: ExtensionCommandRunner,
        assetPaths: WebviewPanelAssetPaths,
        panelDependencies: WebviewPanelDependencies,
        fileUri: string,
        filePath: string,
        commands: TalkbankExecuteCommandClient,
        mode: EvalMode,
    ) {
        this._panel = panel;
        this._assetPaths = assetPaths;
        this._panelDependencies = panelDependencies;
        this._fileUri = fileUri;
        this._filePath = filePath;
        this._mode = mode;

        this._panel.webview.html = this._getHtml();
        this._panel.onDidDispose(() => this._dispose(), null, this._disposables);

        // Handle messages from webview.
        this._panel.webview.onDidReceiveMessage(
            (message: unknown) => {
                const msg = decodePanelMessage(
                    message,
                    'kideval panel',
                    decodeKidevalPanelWebviewMessage,
                );
                if (msg === undefined) {
                    return;
                }

                switch (msg.command) {
                    case 'discoverDatabases':
                        runPanelEffect(
                            'kideval database discovery',
                            runner,
                            this._handleDiscoverDatabases(msg.libDir, commands),
                            errorMessage => {
                                this._postMessage(createKidevalErrorMessage(errorMessage));
                            },
                        );
                        break;
                    case 'runAnalysis':
                        runPanelEffect(
                            'kideval analysis',
                            runner,
                            this._handleRunAnalysis(msg, commands),
                            errorMessage => {
                                this._postMessage(createKidevalErrorMessage(errorMessage));
                            },
                        );
                        break;
                    case 'exportCsv': {
                        runPanelEffect(
                            'kideval export CSV',
                            runner,
                            this._exportCsv(msg.csv),
                            errorMessage => {
                                this._postMessage(createKidevalErrorMessage(errorMessage));
                            },
                        );
                        break;
                    }
                }
            },
            null,
            this._disposables,
        );

        this._sendFileInfo();
    }

    // -----------------------------------------------------------------------
    // Message handlers
    // -----------------------------------------------------------------------

    private _postMessage(message: KidevalPanelExtensionMessage): void {
        this._panel.webview.postMessage(message);
    }

    private _sendFileInfo(): void {
        const modeConfig = MODE_CONFIGS[this._mode];
        this._panel.title = `${modeConfig.title} – ${path.basename(this._filePath)}`;
        this._postMessage(createKidevalFileInfoMessage(path.basename(this._filePath)));
    }

    private _handleDiscoverDatabases(
        libDir: string,
        commands: TalkbankExecuteCommandClient,
    ): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
        const panel = this;
        const modeConfig = MODE_CONFIGS[this._mode];
        return Effect.gen(function*() {
            const databases = yield* commands.discoverDatabases(
                modeConfig.lspDiscoverCommand,
                libDir,
            );
            panel._postMessage(createKidevalDatabasesMessage(databases));
        });
    }

    private _handleRunAnalysis(
        msg: KidevalPanelRunAnalysisMessage,
        commands: TalkbankExecuteCommandClient,
    ): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
        const panel = this;
        const modeConfig = MODE_CONFIGS[this._mode];
        const options: AnalysisOptions = {};
        if (msg.databasePath) {
            options.databasePath = msg.databasePath;
        }
        if (msg.databaseFilter) {
            options.databaseFilter = msg.databaseFilter;
        }

        return Effect.gen(function*() {
            const result = yield* commands.analyze(
                modeConfig.analyzeCommand,
                panel._fileUri,
                options,
            );

            if (typeof result === 'string' && result.startsWith('Analysis error:')) {
                return yield* Effect.fail(new ExecuteCommandServerError({
                    command: 'talkbank/analyze',
                    details: result,
                }));
            }

            panel._postMessage(createKidevalResultsMessage(result));
        });
    }

    private _exportCsv(
        csv: string,
    ): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
        return Effect.gen(function*() {
            const window = yield* VSCodeWindowService;
            const workspace = yield* VSCodeWorkspaceService;
            const uri = yield* tryAsync('show kideval CSV save dialog', () => Promise.resolve(
                window.showSaveDialog({
                    filters: { 'CSV files': ['csv'] },
                    defaultUri: vscode.Uri.file('analysis.csv'),
                }),
            ));
            if (!uri) {
                return;
            }

            yield* Effect.asVoid(tryAsync('write kideval CSV', () => Promise.resolve(
                workspace.fs.writeFile(
                    uri,
                    Buffer.from(csv, 'utf-8'),
                ),
            )));
            yield* Effect.asVoid(tryAsync('show kideval CSV export message', () => Promise.resolve(
                window.showInformationMessage(`Exported to ${uri.fsPath}`),
            )));
        });
    }

    // -----------------------------------------------------------------------
    // HTML generation
    // -----------------------------------------------------------------------

    private _getHtml(): string {
        const webview = this._panel.webview;
        const modeConfig = MODE_CONFIGS[this._mode];

        let html = readWebviewPanelTemplate(this._assetPaths.htmlPath, this._panelDependencies);

        const jsUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.scriptPath),
        );

        // Inject mode configuration so the webview JS knows its context.
        const configScript = `<script>
    const MODE_CONFIG = ${JSON.stringify(modeConfig)};
</script>`;

        html = html.replace('<!--INJECT_CONFIG-->', configScript);
        html = html.replace('<!--INJECT_SCRIPT-->', `<script src="${jsUri}"></script>`);
        return html;
    }

    // -----------------------------------------------------------------------
    // Disposal
    // -----------------------------------------------------------------------

    private _dispose(): void {
        disposePanelResources(this._panel, this._disposables, () => {
            KidevalPanel.currentPanel = undefined;
        });
    }
}
