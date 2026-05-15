import * as vscode from 'vscode';
import * as path from 'path';
import { disposePanelResources, reuseOrCreatePanel } from './panelLifecycle';
import {
    resolveExtensionAssetPath,
    resolveWebviewPanelAssetPaths,
    type WebviewPanelAssetPaths,
} from './panelAssets';
import {
    createExtensionWebviewPanel,
    readWebviewPanelTemplate,
    type WebviewPanelDependencies,
} from './panelHost';

interface GraphPanelAssetPaths extends WebviewPanelAssetPaths {
    graphvizPath: string;
}

export class GraphPanel {
    public static currentPanel: GraphPanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private _disposables: vscode.Disposable[] = [];
    private readonly _assetPaths: GraphPanelAssetPaths;
    private readonly _panelDependencies: WebviewPanelDependencies;
    private _dotSource: string;

    public static createOrShow(
        context: vscode.ExtensionContext,
        dotSource: string,
        fileName: string,
        dependencies: WebviewPanelDependencies = {},
    ) {
        const column = vscode.ViewColumn.Beside;
        const assetPaths: GraphPanelAssetPaths = {
            ...resolveWebviewPanelAssetPaths(context, 'graphPanel'),
            graphvizPath: resolveExtensionAssetPath(
                context,
                path.join('node_modules', '@hpcc-js', 'wasm', 'dist', 'graphviz.js'),
            ),
        };

        reuseOrCreatePanel(
            GraphPanel.currentPanel,
            currentPanel => {
                currentPanel.update(dotSource, fileName);
                currentPanel._panel.reveal(column);
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'dependencyGraph',
                        title: `Dependency Graph - ${path.basename(fileName)}`,
                        column,
                    },
                    dependencies,
                );

                const currentPanel = new GraphPanel(
                    panel,
                    assetPaths,
                    dependencies,
                    dotSource,
                    fileName,
                );
                GraphPanel.currentPanel = currentPanel;
                return currentPanel;
            },
        );
    }

    private constructor(
        panel: vscode.WebviewPanel,
        assetPaths: GraphPanelAssetPaths,
        panelDependencies: WebviewPanelDependencies,
        dotSource: string,
        _fileName: string,
    ) {
        this._panel = panel;
        this._assetPaths = assetPaths;
        this._panelDependencies = panelDependencies;
        this._dotSource = dotSource;

        this._panel.onDidDispose(() => this.dispose(), null, this._disposables);
        this._update();
    }

    public update(dotSource: string, fileName: string) {
        this._dotSource = dotSource;
        this._panel.title = `Dependency Graph - ${path.basename(fileName)}`;
        this._update();
    }

    private _update() {
        this._panel.webview.html = this._getHtmlForWebview();
    }

    private _getHtmlForWebview(): string {
        const webview = this._panel.webview;

        // Read the HTML template from disk.
        let html = readWebviewPanelTemplate(this._assetPaths.htmlPath, this._panelDependencies);

        // Build the webview-safe URI for the external JS file.
        const jsUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.scriptPath),
        );

        // Build the webview-safe URI for the bundled Graphviz WASM module.
        const graphvizUri = webview.asWebviewUri(
            vscode.Uri.file(this._assetPaths.graphvizPath),
        );

        // Build the data injection script block.
        const dataScript = `<script>
    const dotSource = ${JSON.stringify(this._dotSource)};
    const GRAPHVIZ_URI = ${JSON.stringify(graphvizUri.toString())};
</script>`;

        // Replace placeholders.
        html = html.replace('<!--INJECT_DATA-->', dataScript);
        html = html.replace('<!--INJECT_SCRIPT-->', `<script src="${jsUri}"></script>`);

        return html;
    }

    private dispose() {
        disposePanelResources(this._panel, this._disposables, () => {
            GraphPanel.currentPanel = undefined;
        });
    }
}
