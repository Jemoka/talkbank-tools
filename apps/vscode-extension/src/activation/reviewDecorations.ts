/**
 * Editor decorations for `%xrev` review tiers in CHAT files.
 *
 * Color-codes `%xrev:` lines based on their review status:
 *   - Yellow left border: `[?]` (unreviewed, needs attention)
 *   - Green left border:  `[ok]` or `[corrected]` or `[stamped]` (confirmed)
 *   - Red left border:    `[wrong]` or `[early]` or `[late]` (problem found)
 *
 * Decorations are applied to any open CHAT file with `%xrev` tiers,
 * independent of whether review mode is active. This gives immediate
 * visual feedback when browsing aligned files.
 */

import * as vscode from 'vscode';

// ---------------------------------------------------------------------------
// Decoration types
// ---------------------------------------------------------------------------

const unreviewedDecoration = vscode.window.createTextEditorDecorationType({
    borderWidth: '0 0 0 3px',
    borderStyle: 'solid',
    borderColor: '#e6db74', // yellow — needs attention
    isWholeLine: true,
    overviewRulerColor: '#e6db74',
    overviewRulerLane: vscode.OverviewRulerLane.Left,
});

const confirmedDecoration = vscode.window.createTextEditorDecorationType({
    borderWidth: '0 0 0 3px',
    borderStyle: 'solid',
    borderColor: '#a6e22e', // green — confirmed good
    isWholeLine: true,
});

const problemDecoration = vscode.window.createTextEditorDecorationType({
    borderWidth: '0 0 0 3px',
    borderStyle: 'solid',
    borderColor: '#f92672', // red — problem found
    isWholeLine: true,
    overviewRulerColor: '#f92672',
    overviewRulerLane: vscode.OverviewRulerLane.Left,
});

// ---------------------------------------------------------------------------
// Regex patterns for %xrev status markers
// ---------------------------------------------------------------------------

const XREV_UNREVIEWED = /^%xrev:\s*\[\?\]/;
const XREV_CONFIRMED = /^%xrev:\s*\[(ok|corrected|stamped)\]/;
const XREV_PROBLEM = /^%xrev:\s*\[(wrong|early|late)\]/;

// ---------------------------------------------------------------------------
// Update logic
// ---------------------------------------------------------------------------

function updateReviewDecorations(editor: vscode.TextEditor): void {
    if (editor.document.languageId !== 'chat') {
        editor.setDecorations(unreviewedDecoration, []);
        editor.setDecorations(confirmedDecoration, []);
        editor.setDecorations(problemDecoration, []);
        return;
    }

    const text = editor.document.getText();
    const lines = text.split('\n');

    const unreviewed: vscode.Range[] = [];
    const confirmed: vscode.Range[] = [];
    const problem: vscode.Range[] = [];

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (!line.startsWith('%xrev:')) {
            continue;
        }

        const range = new vscode.Range(i, 0, i, line.length);

        if (XREV_UNREVIEWED.test(line)) {
            unreviewed.push(range);
        } else if (XREV_CONFIRMED.test(line)) {
            confirmed.push(range);
        } else if (XREV_PROBLEM.test(line)) {
            problem.push(range);
        }
    }

    editor.setDecorations(unreviewedDecoration, unreviewed);
    editor.setDecorations(confirmedDecoration, confirmed);
    editor.setDecorations(problemDecoration, problem);
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/**
 * Register review decoration listeners. Returns disposables for cleanup.
 *
 * Call during extension activation alongside `registerBulletDecorations()`.
 */
export function registerReviewDecorations(): vscode.Disposable[] {
    // Apply to all currently visible editors.
    for (const editor of vscode.window.visibleTextEditors) {
        updateReviewDecorations(editor);
    }

    return [
        unreviewedDecoration,
        confirmedDecoration,
        problemDecoration,

        vscode.window.onDidChangeActiveTextEditor(editor => {
            if (editor) {
                updateReviewDecorations(editor);
            }
        }),

        vscode.workspace.onDidChangeTextDocument(event => {
            const editor = vscode.window.visibleTextEditors.find(
                e => e.document === event.document,
            );
            if (editor) {
                updateReviewDecorations(editor);
            }
        }),
    ];
}
