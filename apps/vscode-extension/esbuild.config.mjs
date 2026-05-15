// esbuild config for VS Code extension bundling.
//
// The extension entrypoint `src/extension.ts` and its entire reachable module
// graph are bundled into `dist/extension.js`, so the shipped VSIX does not
// need to carry `node_modules/` at runtime. Webview JS (sandboxed, separate
// context) is copied verbatim — it is not part of this bundle.
//
// `vscode` is always resolved at runtime by the host; never bundle it.

import { build, context } from 'esbuild';

const watchMode = process.argv.includes('--watch');
const production = process.argv.includes('--production');

const buildOptions = {
    entryPoints: ['src/extension.ts'],
    bundle: true,
    outfile: 'dist/extension.js',
    external: ['vscode'],
    platform: 'node',
    target: 'node20',
    format: 'cjs',
    sourcemap: !production,
    minify: production,
    logLevel: 'info',
};

if (watchMode) {
    const ctx = await context(buildOptions);
    await ctx.watch();
    console.error('esbuild watching...');
} else {
    await build(buildOptions);
}
