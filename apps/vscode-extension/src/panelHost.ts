import * as fs from 'fs';
import * as vscode from 'vscode';

import type { ExtensionAssetPathContext } from './panelAssets';

/**
 * Narrow window-side boundary for creating webview panels.
 *
 * Keeping this injectable prevents singleton panel modules from reaching
 * directly into `vscode.window` and makes their creation behavior testable.
 */
export interface WebviewPanelWindowHost {
    /**
     * Create one VS Code webview panel with the provided options.
     */
    createWebviewPanel(
        viewType: string,
        title: string,
        showOptions: vscode.ViewColumn,
        options?: vscode.WebviewPanelOptions & vscode.WebviewOptions,
    ): vscode.WebviewPanel;
}

/**
 * Narrow filesystem boundary for reading webview HTML templates.
 */
export interface WebviewPanelTemplateReader {
    /**
     * Read one UTF-8 template file from disk.
     */
    readUtf8(filePath: string): string;
}

/**
 * Shared optional dependencies for singleton panel modules.
 */
export interface WebviewPanelDependencies {
    /**
     * Optional host used to create new webview panels.
     */
    readonly windowHost?: WebviewPanelWindowHost;
    /**
     * Optional reader used to load HTML templates.
     */
    readonly templateReader?: WebviewPanelTemplateReader;
}

/**
 * Extension context subset needed to create a webview panel with extension
 * resource roots.
 */
export interface ExtensionWebviewPanelContext extends ExtensionAssetPathContext {
    /**
     * Extension URI used as the default local resource root.
     */
    readonly extensionUri: vscode.Uri;
}

/**
 * Options for creating one extension-owned webview panel.
 */
export interface CreateExtensionWebviewPanelOptions {
    /**
     * Stable VS Code panel id.
     */
    readonly viewType: string;
    /**
     * User-facing panel title.
     */
    readonly title: string;
    /**
     * Preferred editor column for the panel.
     */
    readonly column: vscode.ViewColumn;
    /**
     * Whether to enable scripts in the webview.
     */
    readonly enableScripts?: boolean;
    /**
     * Whether to keep the webview state alive while hidden.
     */
    readonly retainContextWhenHidden?: boolean;
    /**
     * Whether the extension root should be included automatically as a local
     * resource root. Defaults to `true`.
     */
    readonly includeExtensionRoot?: boolean;
    /**
     * Additional local resource roots required by the panel.
     */
    readonly localResourceRoots?: readonly vscode.Uri[];
}

const defaultWindowHost: WebviewPanelWindowHost = {
    createWebviewPanel(viewType, title, showOptions, options) {
        return vscode.window.createWebviewPanel(viewType, title, showOptions, options);
    },
};

const defaultTemplateReader: WebviewPanelTemplateReader = {
    readUtf8(filePath) {
        return fs.readFileSync(filePath, 'utf-8');
    },
};

/**
 * Create one extension-owned webview panel through the shared injected host.
 */
export function createExtensionWebviewPanel(
    context: ExtensionWebviewPanelContext,
    options: CreateExtensionWebviewPanelOptions,
    dependencies: WebviewPanelDependencies = {},
): vscode.WebviewPanel {
    const windowHost = dependencies.windowHost ?? defaultWindowHost;
    const localResourceRoots = [
        ...(options.includeExtensionRoot === false ? [] : [context.extensionUri]),
        ...(options.localResourceRoots ?? []),
    ];

    return windowHost.createWebviewPanel(
        options.viewType,
        options.title,
        options.column,
        {
            enableScripts: options.enableScripts ?? true,
            retainContextWhenHidden: options.retainContextWhenHidden ?? true,
            localResourceRoots,
        },
    );
}

/**
 * Read one webview HTML template through the shared template-reader boundary.
 */
export function readWebviewPanelTemplate(
    htmlPath: string,
    dependencies: WebviewPanelDependencies = {},
): string {
    const templateReader = dependencies.templateReader ?? defaultTemplateReader;
    return templateReader.readUtf8(htmlPath);
}
