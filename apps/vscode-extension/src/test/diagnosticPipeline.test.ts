/**
 * Focused unit tests for the LSP diagnostic display pipeline.
 *
 * The real pipeline is:
 *   chatter-lsp publishes diagnostics -> vscode-languageclient calls the
 *   `handleDiagnostics` middleware -> the middleware consults the runtime
 *   severity setting and forwards a (possibly filtered) array to `next`,
 *   which is what VS Code renders in the Problems view and as squiggles.
 *
 * The existing suite in `activationLsp.test.ts` covers a single happy path
 * for two severity modes. This file exercises the edge cases that shape
 * what a user actually sees: empty inputs, the `all` mode default, dynamic
 * severity changes, provenance of non-error diagnostics, and the initialize-
 * time plumbing (`initializationOptions`, `synchronize.fileEvents`) that
 * determines whether diagnostics can arrive at all.
 *
 * All tests run under vitest (unit-test mode). The VS Code extension has no
 * `@apps/vscode-extension/test-electron` integration harness wired up — the dep is listed
 * but there is no `.vscode-test.*` config and no `integration/` directory.
 * Adding one is out of scope for this file; the diagnostic display logic
 * here is plain TypeScript with no VS Code runtime dependency beyond the
 * `DiagnosticSeverity` enum, which we mock.
 */

import { describe, expect, it, vi } from 'vitest';

vi.mock('vscode', () => ({
    DiagnosticSeverity: {
        Error: 0,
        Warning: 1,
        Information: 2,
        Hint: 3,
    },
}));

vi.mock('vscode-languageclient/node', () => ({
    LanguageClient: class {},
    TransportKind: {
        stdio: 0,
    },
}));

import {
    createLanguageClientOptions,
    filterDiagnosticsBySeverity,
} from '../activation/lsp';
import type { ExtensionRuntimeContext, ValidationSeverity } from '../runtimeContext';

// Matches the VS Code DiagnosticSeverity enum as mocked above. Kept as
// named constants so test intent is obvious without cross-referencing
// the mock block.
const ERROR = 0;
const WARNING = 1;
const INFORMATION = 2;
const HINT = 3;

interface FakeDiagnostic {
    severity?: number;
    message: string;
}

function makeRuntimeContext(
    overrides: Partial<ExtensionRuntimeContext> = {},
): ExtensionRuntimeContext {
    return {
        getPrimaryWorkspaceRoot: () => '/workspace',
        getConfiguredLspBinaryPath: () => '',
        getInlayHintsEnabled: () => true,
        getValidationSeverity: () => 'all',
        getBulletDisplayMode: () => 'dim',
        getWalkerAutoPlay: () => true,
        getWalkerLoopCount: () => 1,
        getWalkerPauseSeconds: () => 0,
        getWalkerWalkLength: () => 0,
        getMediaDefaultSpeed: () => 100,
        getTranscriptionDefaultSpeaker: () => 'CHI',
        getTranscriptionRewindSeconds: () => 2,
        getCachePath: () => '/cache/talkbank-cache.db',
        ...overrides,
    };
}

describe('filterDiagnosticsBySeverity — severity filtering', () => {
    it('returns an empty array untouched in every severity mode', () => {
        for (const mode of ['all', 'errorsOnly', 'errorsAndWarnings'] as const) {
            expect(filterDiagnosticsBySeverity([], mode)).toEqual([]);
        }
    });

    it('returns a defensive copy in "all" mode so downstream mutation cannot leak back', () => {
        // The `all` branch is implemented with a spread to isolate the
        // middleware-owned array from the LSP-client-owned input. If that
        // ever regresses to `return diagnostics`, the copy assertion below
        // catches it.
        const input: readonly FakeDiagnostic[] = Object.freeze([
            { severity: ERROR, message: 'E001' },
            { severity: WARNING, message: 'W1' },
        ]);

        const output = filterDiagnosticsBySeverity(input, 'all');

        expect(output).toEqual(input);
        expect(output).not.toBe(input);
    });

    it('keeps Information and Hint diagnostics when the mode is "all"', () => {
        const input: FakeDiagnostic[] = [
            { severity: ERROR, message: 'E' },
            { severity: WARNING, message: 'W' },
            { severity: INFORMATION, message: 'I' },
            { severity: HINT, message: 'H' },
        ];

        const output = filterDiagnosticsBySeverity(input, 'all');

        expect(output.map(d => d.message)).toEqual(['E', 'W', 'I', 'H']);
    });

    it('drops Warning, Information, and Hint when the mode is "errorsOnly"', () => {
        const input: FakeDiagnostic[] = [
            { severity: HINT, message: 'H' },
            { severity: INFORMATION, message: 'I' },
            { severity: WARNING, message: 'W' },
            { severity: ERROR, message: 'E1' },
            { severity: ERROR, message: 'E2' },
        ];

        const output = filterDiagnosticsBySeverity(input, 'errorsOnly');

        expect(output.map(d => d.message)).toEqual(['E1', 'E2']);
    });

    it('keeps both Error and Warning in the order they appeared when the mode is "errorsAndWarnings"', () => {
        // Order matters for the VS Code Problems panel presentation: users
        // scan it top-to-bottom, so the filter must not reshuffle.
        const input: FakeDiagnostic[] = [
            { severity: WARNING, message: 'W1' },
            { severity: INFORMATION, message: 'I' },
            { severity: ERROR, message: 'E1' },
            { severity: HINT, message: 'H' },
            { severity: WARNING, message: 'W2' },
        ];

        const output = filterDiagnosticsBySeverity(input, 'errorsAndWarnings');

        expect(output.map(d => d.message)).toEqual(['W1', 'E1', 'W2']);
    });

    it('treats a diagnostic without a severity field as "not an error" and drops it in strict modes', () => {
        // chatter-lsp always sets severity, but the type lets it be
        // undefined. Documenting the behaviour avoids ambiguity if a
        // malformed publishDiagnostics ever arrives.
        const input: FakeDiagnostic[] = [
            { message: 'mystery-no-severity' },
            { severity: ERROR, message: 'E' },
        ];

        expect(filterDiagnosticsBySeverity(input, 'errorsOnly').map(d => d.message))
            .toEqual(['E']);
        expect(filterDiagnosticsBySeverity(input, 'errorsAndWarnings').map(d => d.message))
            .toEqual(['E']);
        // `all` keeps unknown-severity diagnostics, because the user asked
        // to see everything the server published.
        expect(filterDiagnosticsBySeverity(input, 'all').map(d => d.message))
            .toEqual(['mystery-no-severity', 'E']);
    });
});

describe('handleDiagnostics middleware — dynamic behaviour', () => {
    it('re-reads getValidationSeverity on each invocation so a settings change takes effect without reactivation', () => {
        // Users change talkbank.validation.severity through VS Code
        // settings; the middleware must honour the new value on the very
        // next publishDiagnostics, not the one captured at activation.
        let severity: ValidationSeverity = 'errorsOnly';
        const runtimeContext = makeRuntimeContext({
            getValidationSeverity: () => severity,
        });
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext,
        });
        const uri = { path: '/workspace/a.cha' } as never;
        const diagnostics = [
            { severity: ERROR, message: 'E' },
            { severity: WARNING, message: 'W' },
        ];
        const next = vi.fn();

        options.middleware?.handleDiagnostics?.(uri, diagnostics as never, next);
        expect(next).toHaveBeenLastCalledWith(uri, [diagnostics[0]]);

        severity = 'all';
        options.middleware?.handleDiagnostics?.(uri, diagnostics as never, next);
        expect(next).toHaveBeenLastCalledWith(uri, diagnostics);
    });

    it('forwards the incoming URI to next() unchanged, so diagnostics stay attached to the correct editor', () => {
        // VS Code clears diagnostics for a URI when the middleware forwards
        // an empty array under that same URI. Swapping the URI would leave
        // stale squiggles on the old file.
        const runtimeContext = makeRuntimeContext({
            getValidationSeverity: () => 'errorsOnly',
        });
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext,
        });
        const uri = { path: '/workspace/specific.cha', marker: Symbol('uri') } as never;
        const next = vi.fn();

        options.middleware?.handleDiagnostics?.(uri, [] as never, next);

        expect(next).toHaveBeenCalledTimes(1);
        expect(next.mock.calls[0][0]).toBe(uri);
        expect(next.mock.calls[0][1]).toEqual([]);
    });
});

describe('createLanguageClientOptions — initialize-time plumbing', () => {
    it('reports the inlay-hints runtime flag in initializationOptions so the server starts in the right mode', () => {
        // The server reads initializationOptions.inlayHintsEnabled on
        // `initialize`; getting this wrong means inlay hints never appear
        // even after the user toggles the setting and reloads.
        const runtimeContext = makeRuntimeContext({
            getInlayHintsEnabled: () => false,
        });

        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext,
        });

        expect(options.initializationOptions).toEqual({ inlayHintsEnabled: false });
    });

    it('registers a **/*.cha file-system watcher under synchronize so the LSP receives file-change events', () => {
        // Without this watcher, edits made outside the editor (git pull,
        // external tool writing a .cha file) would not retrigger
        // validation, and stale diagnostics would linger.
        const createFileSystemWatcher = vi.fn(() => 'watcher' as never);
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher },
            runtimeContext: makeRuntimeContext(),
        });

        expect(createFileSystemWatcher).toHaveBeenCalledWith('**/*.cha');
        expect(options.synchronize).toEqual({ fileEvents: 'watcher' });
    });

    it('activates only on file:// CHAT documents so diagnostics do not fire on untitled or remote buffers', () => {
        // `documentSelector` drives both client activation and which
        // documents the LSP will receive didOpen notifications for. A
        // looser selector would cause spurious ERROR diagnostics on
        // non-CHAT content.
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext: makeRuntimeContext(),
        });

        expect(options.documentSelector).toEqual([
            { scheme: 'file', language: 'chat' },
        ]);
    });
});

describe('provideInlayHints middleware — gating', () => {
    it('skips the downstream request entirely when inlay hints are disabled', () => {
        // Even when the server supports inlay hints (they were enabled at
        // init), the user can turn them off live. The middleware must
        // short-circuit before touching `next`, so disabled hints never
        // cost an LSP round-trip.
        const runtimeContext = makeRuntimeContext({
            getInlayHintsEnabled: () => false,
        });
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext,
        });
        const next = vi.fn(() => ['unreachable']);

        const result = options.middleware?.provideInlayHints?.(
            {} as never,
            {} as never,
            {} as never,
            next as never,
        );

        expect(result).toEqual([]);
        expect(next).not.toHaveBeenCalled();
    });

    it('delegates to next() when inlay hints are enabled, forwarding document/range/token arguments as given', () => {
        const runtimeContext = makeRuntimeContext({
            getInlayHintsEnabled: () => true,
        });
        const options = createLanguageClientOptions({
            workspace: { createFileSystemWatcher: vi.fn(() => 'watcher' as never) },
            runtimeContext,
        });
        const document = { uri: 'doc' } as never;
        const range = { start: 0, end: 1 } as never;
        const token = { isCancellationRequested: false } as never;
        const hints = [{ position: 0, label: '2.5s' }];
        const next = vi.fn(() => hints);

        const result = options.middleware?.provideInlayHints?.(document, range, token, next as never);

        expect(result).toBe(hints);
        expect(next).toHaveBeenCalledWith(document, range, token);
    });
});
