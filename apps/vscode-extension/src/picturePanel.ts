/**
 * picturePanel.ts
 *
 * Simple webview panel for displaying elicitation pictures (Cookie Theft,
 * picture description tasks, etc.). CLAN's PictController equivalent.
 *
 * Pictures are discovered by scanning the document for %pic references or
 * image files in the same directory as the CHAT file.
 */

import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { disposePanelResources, reuseOrCreatePanel } from './panelLifecycle';
import {
    createExtensionWebviewPanel,
    type WebviewPanelDependencies,
} from './panelHost';

const IMAGE_EXTENSIONS = ['.jpg', '.jpeg', '.png', '.gif', '.bmp', '.webp'];

export class PicturePanel {
    public static currentPanel: PicturePanel | undefined;

    private readonly _panel: vscode.WebviewPanel;
    private readonly _disposables: vscode.Disposable[] = [];

    public static createOrShow(
        context: vscode.ExtensionContext,
        imagePath: string,
        dependencies: WebviewPanelDependencies = {},
    ): void {
        const column = vscode.ViewColumn.Beside;

        reuseOrCreatePanel(
            PicturePanel.currentPanel,
            currentPanel => {
                currentPanel._update(imagePath);
                currentPanel._panel.reveal(column);
            },
            () => {
                const panel = createExtensionWebviewPanel(
                    context,
                    {
                        viewType: 'talkbankPicture',
                        title: 'Picture',
                        column,
                        enableScripts: false,
                        includeExtensionRoot: false,
                        localResourceRoots: [vscode.Uri.file('/')],
                    },
                    dependencies,
                );

                const currentPanel = new PicturePanel(panel, imagePath);
                PicturePanel.currentPanel = currentPanel;
                return currentPanel;
            },
        );
    }

    private constructor(panel: vscode.WebviewPanel, imagePath: string) {
        this._panel = panel;
        this._update(imagePath);

        this._panel.onDidDispose(() => this._dispose(), null, this._disposables);
    }

    private _update(imagePath: string): void {
        const imageUri = this._panel.webview.asWebviewUri(vscode.Uri.file(imagePath));
        const fileName = path.basename(imagePath);
        this._panel.title = `Picture: ${fileName}`;
        this._panel.webview.html = `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<style>
  body {
    margin: 0;
    padding: 16px;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    background: var(--vscode-editor-background);
  }
  img {
    max-width: 100%;
    max-height: 90vh;
    object-fit: contain;
    border-radius: 4px;
  }
</style>
</head>
<body>
  <img src="${imageUri}" alt="${fileName}" />
</body>
</html>`;
    }

    private _dispose(): void {
        disposePanelResources(this._panel, this._disposables, () => {
            PicturePanel.currentPanel = undefined;
        });
    }
}

/**
 * Find picture files associated with a CHAT document.
 *
 * Searches for:
 * 1. %pic references in the document text (e.g., %pic:"path/image.jpg")
 * 2. Image files in the same directory with a matching base name
 */
export function findPictures(docPath: string, docText: string): string[] {
    const docDir = path.dirname(docPath);
    const docBase = path.basename(docPath, path.extname(docPath));
    const found: string[] = [];
    const seen = new Set<string>();

    // 1. Scan for %pic references.
    const picRegex = /%pic[:\s]+"?([^"\s]+)"?/g;
    let match: RegExpExecArray | null;
    while ((match = picRegex.exec(docText)) !== null) {
        const ref = match[1];
        const abs = path.isAbsolute(ref) ? ref : path.resolve(docDir, ref);
        if (fs.existsSync(abs) && !seen.has(abs)) {
            seen.add(abs);
            found.push(abs);
        }
    }

    // 2. Look for images with the same base name as the .cha file.
    for (const ext of IMAGE_EXTENSIONS) {
        const candidate = path.join(docDir, docBase + ext);
        if (fs.existsSync(candidate) && !seen.has(candidate)) {
            seen.add(candidate);
            found.push(candidate);
        }
    }

    // 3. If nothing found, look for any image files in the directory.
    if (found.length === 0) {
        try {
            const entries = fs.readdirSync(docDir);
            for (const entry of entries) {
                if (IMAGE_EXTENSIONS.includes(path.extname(entry).toLowerCase())) {
                    const abs = path.join(docDir, entry);
                    if (!seen.has(abs)) {
                        seen.add(abs);
                        found.push(abs);
                    }
                }
            }
        } catch {
            // Directory read failed — no pictures.
        }
    }

    return found;
}
