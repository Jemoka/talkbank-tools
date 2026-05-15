#!/usr/bin/env node
// Copies a prebuilt chatter-lsp binary into apps/vscode-extension/server/ before packaging.
// Usage: node scripts/prepare-server.mjs --source <path> [--windows]
import { copyFileSync, chmodSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const { values } = parseArgs({
    options: {
        source: { type: 'string' },
        windows: { type: 'boolean', default: false },
    },
});

if (!values.source) {
    console.error('error: --source <path-to-chatter-lsp> is required');
    process.exit(1);
}

const source = resolve(values.source);
if (!existsSync(source)) {
    console.error(`error: source binary does not exist: ${source}`);
    process.exit(1);
}

const extensionRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const serverDir = resolve(extensionRoot, 'server');
const destName = values.windows ? 'chatter-lsp.exe' : 'chatter-lsp';
const dest = resolve(serverDir, destName);

mkdirSync(serverDir, { recursive: true });
copyFileSync(source, dest);
if (!values.windows) {
    chmodSync(dest, 0o755);
}

console.log(`staged ${source} -> ${dest}`);
