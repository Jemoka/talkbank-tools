/**
 * Bullet-decoration activation for CHAT media timing markers.
 */

import * as vscode from 'vscode';
import {
    DefaultExtensionRuntimeContext,
    type ExtensionRuntimeContext,
} from '../runtimeContext';

/**
 * Register bullet decoration listeners and return the resulting disposables.
 *
 * @returns Disposable resources for bullet decorations.
 */
export function registerBulletDecorations(
    runtimeContext: ExtensionRuntimeContext = new DefaultExtensionRuntimeContext({
        workspace: vscode.workspace,
    }),
): vscode.Disposable[] {
    let bulletDecorationType = createBulletDecoration(runtimeContext);

    function updateBulletDecorations(editor: vscode.TextEditor): void {
        if (editor.document.languageId !== 'chat') {
            editor.setDecorations(bulletDecorationType, []);
            return;
        }
        const mode = runtimeContext.getBulletDisplayMode();
        if (mode === 'normal') {
            editor.setDecorations(bulletDecorationType, []);
            return;
        }

        const text = editor.document.getText();
        const ranges: vscode.Range[] = [];
        const bulletRegex = /\u2022\d+_\d+\u2022/g;
        let match: RegExpExecArray | null;
        while ((match = bulletRegex.exec(text)) !== null) {
            const start = editor.document.positionAt(match.index);
            const end = editor.document.positionAt(match.index + match[0].length);
            ranges.push(new vscode.Range(start, end));
        }
        editor.setDecorations(bulletDecorationType, ranges);
    }

    function refreshAllBulletDecorations(): void {
        for (const editor of vscode.window.visibleTextEditors) {
            updateBulletDecorations(editor);
        }
    }

    refreshAllBulletDecorations();

    return [
        bulletDecorationType,
        vscode.window.onDidChangeActiveTextEditor(editor => {
            if (editor) {
                updateBulletDecorations(editor);
            }
        }),
        vscode.workspace.onDidChangeTextDocument(event => {
            const editor = vscode.window.visibleTextEditors.find(
                candidate => candidate.document.uri.toString() === event.document.uri.toString()
            );
            if (editor) {
                updateBulletDecorations(editor);
            }
        }),
        vscode.window.onDidChangeVisibleTextEditors(editors => {
            for (const editor of editors) {
                updateBulletDecorations(editor);
            }
        }),
        vscode.workspace.onDidChangeConfiguration(event => {
            if (event.affectsConfiguration('talkbank.bullets.display')) {
                bulletDecorationType.dispose();
                bulletDecorationType = createBulletDecoration(runtimeContext);
                refreshAllBulletDecorations();
            }
        }),
    ];
}

/**
 * Create a decoration type based on the configured bullet display mode.
 *
 * @returns Decoration type for bullet spans.
 */
function createBulletDecoration(
    runtimeContext: ExtensionRuntimeContext,
): vscode.TextEditorDecorationType {
    const mode = runtimeContext.getBulletDisplayMode();
    if (mode === 'hidden') {
        return vscode.window.createTextEditorDecorationType({
            opacity: '0',
            textDecoration: 'none; font-size: 0',
        });
    }
    if (mode === 'dim') {
        return vscode.window.createTextEditorDecorationType({
            opacity: '0.35',
            color: new vscode.ThemeColor('editorLineNumber.foreground'),
        });
    }
    return vscode.window.createTextEditorDecorationType({});
}
