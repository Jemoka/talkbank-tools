/**
 * coderPanel.ts
 *
 * Coder mode for structured annotation of CHAT transcripts.
 * CLAN's Coder mode equivalent (ced_codes.cpp).
 *
 * Workflow:
 * 1. User loads a .cut codes file (hierarchical code tree)
 * 2. Extension steps through utterances one at a time
 * 3. User selects codes from a QuickPick tree
 * 4. Selected code is inserted on the utterance's %cod tier
 * 5. Advance to next uncoded utterance
 *
 * Utterance detection and coded-status are delegated to the LSP via
 * `talkbank/getUtterances` — no CHAT parsing in TypeScript.
 *
 * The .cut format uses tab-indented hierarchies:
 *   $PRA
 *   	$PRA:request
 *   	$PRA:demand
 *   $ACT
 *   	$ACT:play
 *   	$ACT:read
 */

import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    CoderCommandStateService,
    type CoderCommandState,
} from './coderState';
import { CodeNode, parseCodesFile } from './coderModel';
import {
    ExtensionCommandRequirements,
    VSCodeCommandsService,
    VSCodeWindowService,
    requireActiveChatEditor,
} from './effectCommandRuntime';
import {
    ExecuteCommandClientService,
    tryAsync,
} from './effectRuntime';
import { TextFileService } from './textFileService';
import { UtteranceInfo } from './lsp/executeCommandClient';

// -------------------------------------------------------------------------
// Codes file parser (.cut format — NOT CHAT, so local parsing is fine)
// -------------------------------------------------------------------------

/** Flatten a code tree into a list of QuickPick items with indentation. */
function flattenCodes(nodes: readonly CodeNode[], indent: string = ''): vscode.QuickPickItem[] {
    const items: vscode.QuickPickItem[] = [];
    for (const node of nodes) {
        items.push({
            label: `${indent}${node.code}`,
            description: node.children.length > 0 ? `(${node.children.length} children)` : '',
            detail: undefined,
        });
        if (node.children.length > 0) {
            items.push(...flattenCodes(node.children, indent + '    '));
        }
    }
    return items;
}

// -------------------------------------------------------------------------
// Utterance info from LSP
// -------------------------------------------------------------------------

function requireActiveCoderState(snapshot: CoderCommandState): CoderCommandState | undefined {
    return snapshot.active ? snapshot : undefined;
}

function currentUtteranceLine(snapshot: CoderCommandState): number {
    return snapshot.currentUtteranceLine;
}

function currentCodesTree(snapshot: CoderCommandState): readonly CodeNode[] {
    return snapshot.codesTree;
}

/** Get utterance info from the LSP (model-based, no regex). */
function getUtterances(
    doc: vscode.TextDocument,
): Effect.Effect<UtteranceInfo[], unknown, ExtensionCommandRequirements> {
    return Effect.flatMap(
        ExecuteCommandClientService,
        commands => commands.getUtterances(doc.uri.toString()),
    );
}

/** Find the next uncoded utterance line after a given line. */
function findNextUncoded(
    doc: vscode.TextDocument,
    afterLine: number,
): Effect.Effect<number | undefined, unknown, ExtensionCommandRequirements> {
    return Effect.map(getUtterances(doc), utterances => {
        for (const utterance of utterances) {
            if (utterance.line <= afterLine) {
                continue;
            }
            if (!utterance.has_cod) {
                return utterance.line;
            }
        }
        return undefined;
    });
}

/** Find the insertion point for a %cod tier after a given utterance line.
 *  Uses LSP utterance data to find the end of the utterance block. */
function findCodInsertLine(
    doc: vscode.TextDocument,
    utteranceLine: number,
): Effect.Effect<number, unknown, ExtensionCommandRequirements> {
    return Effect.map(getUtterances(doc), utterances => {
        const idx = utterances.findIndex(utterance => utterance.line === utteranceLine);
        if (idx === -1) {
            return utteranceLine + 1;
        }

        if (idx + 1 < utterances.length) {
            const nextLine = utterances[idx + 1].line;
            let insertLine = nextLine;
            while (
                insertLine > utteranceLine + 1
                && doc.lineAt(insertLine - 1).text.trim() === ''
            ) {
                insertLine--;
            }
            return insertLine;
        }

        let insertLine = utteranceLine + 1;
        for (let lineIndex = utteranceLine + 1; lineIndex < doc.lineCount; lineIndex++) {
            const text = doc.lineAt(lineIndex).text;
            if (text.startsWith('@') || text.trim() === '') {
                break;
            }
            insertLine = lineIndex + 1;
        }
        return insertLine;
    });
}

// -------------------------------------------------------------------------
// Commands
// -------------------------------------------------------------------------

/** Start coder mode: load a codes file and begin stepping through utterances. */
export function startCoderMode(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        yield* requireActiveChatEditor();
        const window = yield* VSCodeWindowService;
        const commands = yield* VSCodeCommandsService;
        const coderState = yield* CoderCommandStateService;
        const textFiles = yield* TextFileService;

        const files = yield* tryAsync('show codes file picker', () => Promise.resolve(
            window.showOpenDialog({
                canSelectMany: false,
                filters: { 'Codes files': ['cut', 'txt'] },
                title: 'Select a codes file (.cut)',
            }),
        ));
        if (!files || files.length === 0) {
            return;
        }

        const codesPath = files[0].fsPath;
        const codesText = yield* tryAsync('read codes file', () => textFiles.readUtf8(codesPath)).pipe(
            Effect.catchAll(cause => Effect.gen(function*() {
            yield* Effect.asVoid(tryAsync('show codes file read error', () => Promise.resolve(
                window.showErrorMessage(`Failed to read codes file: ${String(cause)}`),
            )));
            return '';
            })),
        );
        if (codesText === '') {
            return;
        }

        const tree = parseCodesFile(codesText);
        if (tree.length === 0) {
            yield* Effect.asVoid(tryAsync('show empty codes warning', () => Promise.resolve(
                window.showWarningMessage('Codes file is empty or has no valid codes.'),
            )));
            return;
        }

        yield* Effect.sync(() => {
            coderState.activate(tree, codesPath);
        });

        yield* Effect.asVoid(tryAsync('set coder active context', () => Promise.resolve(
            commands.executeCommand(
                'setContext',
                'talkbank.coderActive',
                true,
            ),
        )));
        yield* Effect.asVoid(tryAsync('show coder started message', () => Promise.resolve(
            window.showInformationMessage(
                `Coder mode started with ${tree.length} top-level codes. Use "Coder: Next" to step through utterances.`,
            ),
        )));

        yield* coderNext();
    });
}

/** Stop coder mode. */
export function stopCoderMode(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const commands = yield* VSCodeCommandsService;
        const coderState = yield* CoderCommandStateService;

        yield* Effect.sync(() => {
            coderState.reset();
        });
        yield* Effect.asVoid(tryAsync('set coder inactive context', () => Promise.resolve(
            commands.executeCommand(
                'setContext',
                'talkbank.coderActive',
                false,
            ),
        )));
        yield* Effect.asVoid(tryAsync('show coder stopped message', () => Promise.resolve(
            window.showInformationMessage('Coder mode stopped.'),
        )));
    });
}

/** Advance to the next uncoded utterance and prompt for a code. */
export function coderNext(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const coderState = yield* CoderCommandStateService;
        const editor = window.activeTextEditor;
        const snapshot = requireActiveCoderState(coderState.snapshot());
        if (!editor || !snapshot) {
            yield* Effect.asVoid(tryAsync('show coder inactive warning', () => Promise.resolve(
                window.showWarningMessage('Coder mode is not active.'),
            )));
            return;
        }

        const nextLine = yield* findNextUncoded(
            editor.document,
            currentUtteranceLine(snapshot),
        );
        if (nextLine === undefined) {
            yield* Effect.asVoid(tryAsync('show coder finished message', () => Promise.resolve(
                window.showInformationMessage('All utterances have been coded!'),
            )));
            yield* stopCoderMode();
            return;
        }

        yield* Effect.sync(() => {
            coderState.setCurrentUtteranceLine(nextLine);
        });

        const pos = new vscode.Position(nextLine, 0);
        yield* Effect.sync(() => {
            editor.selection = new vscode.Selection(pos, pos);
            editor.revealRange(
                new vscode.Range(pos, pos),
                vscode.TextEditorRevealType.InCenterIfOutsideViewport,
            );
        });

        const utterances = yield* getUtterances(editor.document);
        const coded = utterances.filter(utterance => utterance.has_cod).length;
        const total = utterances.length;
        const utteranceText = editor.document.lineAt(nextLine).text;
        const items = flattenCodes(currentCodesTree(snapshot));
        const picked = yield* tryAsync('show coder quick pick', () => Promise.resolve(
            window.showQuickPick(items, {
                placeHolder: `Select code for: ${utteranceText.substring(0, 80)}… (${coded}/${total} coded)`,
                matchOnDescription: true,
            }),
        ));

        if (picked) {
            const code = picked.label.trim();
            const insertLine = yield* findCodInsertLine(editor.document, nextLine);

            yield* Effect.asVoid(tryAsync('insert coder code', () => Promise.resolve(
                editor.edit(editBuilder => {
                    editBuilder.insert(
                        new vscode.Position(insertLine, 0),
                        `%cod:\t${code}\n`,
                    );
                }),
            )));
            yield* coderNext();
        }
    });
}

/** Insert a code on the current utterance without advancing. */
export function coderInsertCode(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const coderState = yield* CoderCommandStateService;
        const editor = window.activeTextEditor;
        const snapshot = requireActiveCoderState(coderState.snapshot());
        if (!editor || !snapshot) {
            return;
        }

        const items = flattenCodes(currentCodesTree(snapshot));
        const picked = yield* tryAsync('show coder insert quick pick', () => Promise.resolve(
            window.showQuickPick(items, {
                placeHolder: 'Select code to insert',
                matchOnDescription: true,
            }),
        ));
        if (!picked) {
            return;
        }

        const code = picked.label.trim();
        const insertLine = yield* findCodInsertLine(
            editor.document,
            currentUtteranceLine(snapshot),
        );

        yield* Effect.asVoid(tryAsync('insert coder code without advance', () => Promise.resolve(
            editor.edit(editBuilder => {
                editBuilder.insert(
                    new vscode.Position(insertLine, 0),
                    `%cod:\t${code}\n`,
                );
            }),
        )));
    });
}
