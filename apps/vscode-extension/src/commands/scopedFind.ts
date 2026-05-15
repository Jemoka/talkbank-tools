/**
 * Scoped Find — semantically scoped search across CHAT tiers via LSP.
 *
 * Lets the user pick a tier scope, optionally filter by speaker, enter a
 * search query (plain text or /regex/), and navigate through matches.
 */

import * as vscode from 'vscode';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    VSCodeWindowService,
    requireActiveChatEditor,
} from '../effectCommandRuntime';
import { ExecuteCommandClientService, tryAsync } from '../effectRuntime';

const TIER_SCOPES = [
    { label: 'All Tiers', value: 'all' },
    { label: 'Main Tier (*SPEAKER:)', value: 'main' },
    { label: '%mor (Morphology)', value: 'mor' },
    { label: '%gra (Grammar Relations)', value: 'gra' },
    { label: '%pho (Phonology)', value: 'pho' },
    { label: '%sin (Sign/Gesture)', value: 'sin' },
    { label: '%com (Comment)', value: 'com' },
    { label: '%cod (Coding)', value: 'cod' },
    { label: '%act (Action)', value: 'act' },
    { label: '%exp (Explanation)', value: 'exp' },
];

interface TierScopePick extends vscode.QuickPickItem {
    value: string;
}

export function scopedFind(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveChatEditor();
        const window = yield* VSCodeWindowService;
        const commands = yield* ExecuteCommandClientService;

        const scopePick = yield* tryAsync('show tier scope picker', () => Promise.resolve(
            window.showQuickPick<TierScopePick>(
                TIER_SCOPES.map(scope => ({ label: scope.label, value: scope.value })),
                { placeHolder: 'Search in which tier?' },
            ),
        ));
        if (!scopePick) {
            return;
        }

        const uri = editor.document.uri.toString();
        let speakers: string[] = [];

        const speakerResult = yield* commands.getSpeakers(uri);
        if (speakerResult.length > 1) {
            const speakerPick = yield* tryAsync('show speaker filter picker', () => Promise.resolve(
                window.showQuickPick(
                    [
                        { label: 'All Speakers', code: '' },
                        ...speakerResult.map(speaker => ({
                            label: speaker.code,
                            description: speaker.name ? `${speaker.name} (${speaker.role})` : speaker.role,
                            code: speaker.code,
                        })),
                    ],
                    { placeHolder: 'Filter by speaker? (Esc = all speakers)' },
                ),
            ));
            if (speakerPick && speakerPick.code) {
                speakers = [speakerPick.code];
            }
        }

        const query = yield* tryAsync('show scoped find query input', () => Promise.resolve(
            window.showInputBox({
                placeHolder: 'Search text (plain text or /regex/)',
                prompt: `Searching in ${scopePick.label}${speakers.length ? ` for ${speakers[0]}` : ''}`,
            }),
        ));
        if (!query) {
            return;
        }

        let searchQuery = query;
        let isRegex = false;
        if (query.startsWith('/') && query.endsWith('/') && query.length > 2) {
            searchQuery = query.slice(1, -1);
            isRegex = true;
        }

        const results = yield* commands.scopedFind({
            uri,
            query: searchQuery,
            scope: scopePick.value,
            speakers,
            regex: isRegex,
        });

        if (results.length === 0) {
            yield* Effect.asVoid(tryAsync('show scoped find empty result message', () => Promise.resolve(
                window.showInformationMessage(`No matches for "${query}" in ${scopePick.label}`),
            )));
            return;
        }

        const items = results.map(match => ({
            label: `$(search) ${match.line_text.trim()}`,
            description: `${match.speaker} %${match.tier} L${match.line + 1}`,
            detail: `Line ${match.line + 1}, Col ${match.character + 1}`,
            match,
        }));

        const selected = yield* tryAsync('show scoped find results', () => Promise.resolve(
            window.showQuickPick(items, {
                placeHolder: `${results.length} match${results.length === 1 ? '' : 'es'} in ${scopePick.label}`,
                matchOnDescription: true,
                matchOnDetail: true,
            }),
        ));
        if (!selected) {
            return;
        }

        const pos = new vscode.Position(selected.match.line, selected.match.character);
        const endPos = new vscode.Position(
            selected.match.line,
            selected.match.character + selected.match.length,
        );
        yield* Effect.sync(() => {
            editor.selection = new vscode.Selection(pos, endPos);
            editor.revealRange(new vscode.Range(pos, endPos), vscode.TextEditorRevealType.InCenter);
        });
    });
}
