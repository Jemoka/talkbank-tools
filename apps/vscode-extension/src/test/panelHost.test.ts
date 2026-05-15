import { describe, expect, it, vi } from 'vitest';

const { createWebviewPanelMock } = vi.hoisted(() => ({
    createWebviewPanelMock: vi.fn(() => ({})),
}));

vi.mock('vscode', () => ({
    window: {
        createWebviewPanel: createWebviewPanelMock,
    },
}));

import {
    createExtensionWebviewPanel,
    readWebviewPanelTemplate,
    type ExtensionWebviewPanelContext,
    type WebviewPanelDependencies,
} from '../panelHost';

describe('panelHost', () => {
    it('creates extension webviews with the extension root included by default', () => {
        const createWebviewPanel = vi.fn(() => ({}) as never);
        const context: ExtensionWebviewPanelContext = {
            extensionUri: { path: '/extension' } as never,
            asAbsolutePath(relativePath: string): string {
                return `/extension/${relativePath}`;
            },
        };
        const dependencies: WebviewPanelDependencies = {
            windowHost: {
                createWebviewPanel,
            },
        };
        const extraRoot = { path: '/extra' } as never;

        createExtensionWebviewPanel(
            context,
            {
                viewType: 'talkbankTest',
                title: 'Test Panel',
                column: 2 as never,
                localResourceRoots: [extraRoot],
            },
            dependencies,
        );

        expect(createWebviewPanel).toHaveBeenCalledWith(
            'talkbankTest',
            'Test Panel',
            2,
            expect.objectContaining({
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [context.extensionUri, extraRoot],
            }),
        );
    });

    it('can skip the extension root for panels that only need explicit roots', () => {
        const createWebviewPanel = vi.fn(() => ({}) as never);
        const rootOnlyContext: ExtensionWebviewPanelContext = {
            extensionUri: { path: '/extension' } as never,
            asAbsolutePath(relativePath: string): string {
                return `/extension/${relativePath}`;
            },
        };

        createExtensionWebviewPanel(
            rootOnlyContext,
            {
                viewType: 'talkbankPicture',
                title: 'Picture',
                column: 2 as never,
                includeExtensionRoot: false,
                localResourceRoots: [{ path: '/' } as never],
            },
            {
                windowHost: {
                    createWebviewPanel,
                },
            },
        );

        expect(createWebviewPanel).toHaveBeenCalledWith(
            'talkbankPicture',
            'Picture',
            2,
            expect.objectContaining({
                localResourceRoots: [{ path: '/' }],
            }),
        );
    });

    it('reads templates through the injected reader boundary', () => {
        const readUtf8 = vi.fn(() => '<html>template</html>');

        expect(
            readWebviewPanelTemplate('/extension/out/webview/mediaPanel.html', {
                templateReader: { readUtf8 },
            }),
        ).toBe('<html>template</html>');
        expect(readUtf8).toHaveBeenCalledWith('/extension/out/webview/mediaPanel.html');
    });
});
