import * as path from 'path';

export interface ExtensionAssetPathContext {
    asAbsolutePath(relativePath: string): string;
}

export interface WebviewPanelAssetPaths {
    htmlPath: string;
    scriptPath: string;
}

export function resolveExtensionAssetPath(
    context: ExtensionAssetPathContext,
    relativePath: string,
): string {
    return context.asAbsolutePath(relativePath);
}

export function resolveWebviewPanelAssetPaths(
    context: ExtensionAssetPathContext,
    panelName: string,
): WebviewPanelAssetPaths {
    return {
        htmlPath: resolveExtensionAssetPath(
            context,
            path.join('out', 'webview', `${panelName}.html`),
        ),
        scriptPath: resolveExtensionAssetPath(
            context,
            path.join('out', 'webview', `${panelName}.js`),
        ),
    };
}
