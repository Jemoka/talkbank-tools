/**
 * Command-registration sanity test.
 *
 * Every command declared in `package.json` `contributes.commands`
 * must be registered somewhere that VS Code will actually route to
 * at runtime. The registration can live on one of three paths:
 *
 * 1. `registerEffectCommand(id, runner, ...)` in one of the
 *    `src/activation/commands/*.ts` modules (the typed effect
 *    runtime — default home for new commands).
 * 2. `vscode.commands.registerCommand('id', ...)` in
 *    `src/activation/validation.ts` (the Validation Explorer tree
 *    view's own command surface).
 * 3. The LSP server's `workspace/executeCommand` RPC family — but
 *    those use `talkbank/` command IDs, not `talkbank.`, and so
 *    never appear in `contributes.commands` at all.
 *
 * A `talkbank.*` command in `package.json` that is not registered
 * via #1 or #2 is a dead button: VS Code will surface it in the
 * palette and the menus, the user will click, and nothing will
 * happen. This test prevents that regression by walking both
 * sources at test time.
 *
 * See `developer/adding-features.md` and `reference/commands.md` in
 * the book for the full architecture.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { describe, expect, test } from 'vitest';

const PACKAGE_JSON_PATH = path.join(__dirname, '..', '..', 'package.json');
const ACTIVATION_ROOT = path.join(__dirname, '..', 'activation');

interface PackageCommand {
    command: string;
    title?: string;
}

function readContributedCommands(): string[] {
    const raw = fs.readFileSync(PACKAGE_JSON_PATH, 'utf8');
    const parsed = JSON.parse(raw) as {
        contributes?: { commands?: PackageCommand[] };
    };
    const list = parsed.contributes?.commands ?? [];
    return list.map(c => c.command);
}

function readFilesRecursively(root: string): string[] {
    const results: string[] = [];
    const entries = fs.readdirSync(root, { withFileTypes: true });
    for (const entry of entries) {
        const full = path.join(root, entry.name);
        if (entry.isDirectory()) {
            results.push(...readFilesRecursively(full));
        } else if (entry.isFile() && entry.name.endsWith('.ts')) {
            results.push(full);
        }
    }
    return results;
}

function readRegisteredCommands(): Set<string> {
    const registered = new Set<string>();
    // Match `registerEffectCommand('talkbank.foo',` and
    // `vscode.commands.registerCommand('talkbank.foo',`. We match
    // both single- and double-quoted literals.
    const pattern =
        /(?:registerEffectCommand|registerCommand)\(\s*['"](talkbank\.[A-Za-z0-9]+)['"]/g;
    for (const file of readFilesRecursively(ACTIVATION_ROOT)) {
        const source = fs.readFileSync(file, 'utf8');
        for (const match of source.matchAll(pattern)) {
            registered.add(match[1]);
        }
    }
    return registered;
}

describe('contributes.commands registration sanity', () => {
    const declared = readContributedCommands();
    const registered = readRegisteredCommands();

    test('every contributes.commands entry has a handler', () => {
        const orphans = declared.filter(id => !registered.has(id));
        expect(
            orphans,
            `commands declared in package.json but not registered in src/activation/: ${orphans.join(', ')}`,
        ).toHaveLength(0);
    });

    test('no command handler registers an ID that is not declared', () => {
        const declaredSet = new Set(declared);
        const stray = [...registered].filter(id => !declaredSet.has(id));
        expect(
            stray,
            `commands registered in src/activation/ but not declared in package.json: ${stray.join(', ')}`,
        ).toHaveLength(0);
    });

    test('declared commands use the talkbank.* prefix', () => {
        const misprefixed = declared.filter(id => !id.startsWith('talkbank.'));
        expect(
            misprefixed,
            `commands without talkbank.* prefix: ${misprefixed.join(', ')}`,
        ).toHaveLength(0);
    });
});
