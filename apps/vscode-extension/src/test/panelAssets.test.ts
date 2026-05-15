import * as path from 'path';
import { describe, expect, it } from 'vitest';

import {
    resolveExtensionAssetPath,
    resolveWebviewPanelAssetPaths,
} from '../panelAssets';

const context = {
    asAbsolutePath(relativePath: string): string {
        return path.join('/mock/extension', relativePath);
    },
};

describe('panelAssets', () => {
    it('resolves webview panel assets through the injected extension context', () => {
        expect(resolveWebviewPanelAssetPaths(context, 'mediaPanel')).toEqual({
            htmlPath: path.join('/mock/extension', 'out', 'webview', 'mediaPanel.html'),
            scriptPath: path.join('/mock/extension', 'out', 'webview', 'mediaPanel.js'),
        });
    });

    it('resolves arbitrary extension assets through the injected extension context', () => {
        expect(
            resolveExtensionAssetPath(
                context,
                path.join('node_modules', '@hpcc-js', 'wasm', 'dist', 'graphviz.js'),
            ),
        ).toBe(
            path.join('/mock/extension', 'node_modules', '@hpcc-js', 'wasm', 'dist', 'graphviz.js'),
        );
    });
});
