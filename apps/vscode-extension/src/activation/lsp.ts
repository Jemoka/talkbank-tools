/**
 * Language-server activation helpers for the TalkBank VS Code extension.
 */

import * as vscode from 'vscode';
import {
    LanguageClient,
    TransportKind
} from 'vscode-languageclient/node';
import type {
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';
import {
    type ValidationSeverity,
} from '../runtimeContext';
import type { ExtensionRuntimeServices } from '../runtimeServices';

const LSP_BINARY_PATH_SETTING = 'talkbank.lsp.binaryPath';
const OPEN_SETTINGS_ACTION = 'Open Settings';
const LSP_BINARY_NOT_FOUND_MESSAGE =
    'TalkBank LSP binary not found. Install the platform-specific VSIX, '
    + `place chatter-lsp on PATH, or set "${LSP_BINARY_PATH_SETTING}" to an absolute path.`;

export interface LanguageServerWorkspace {
    createFileSystemWatcher(globPattern: string): vscode.FileSystemWatcher;
}

export type LanguageClientFactory = (
    id: string,
    name: string,
    serverOptions: ServerOptions,
    clientOptions: LanguageClientOptions,
) => LanguageClient;

export interface LanguageServerActivationServices extends ExtensionRuntimeServices {
    readonly workspace: LanguageServerWorkspace;
    readonly createLanguageClient?: LanguageClientFactory;
}

/**
 * Filter diagnostics according to the current runtime validation-severity setting.
 */
export function filterDiagnosticsBySeverity<T extends { severity?: vscode.DiagnosticSeverity }>(
    diagnostics: readonly T[],
    severity: ValidationSeverity,
): T[] {
    if (severity === 'errorsOnly') {
        return diagnostics.filter(d => d.severity === vscode.DiagnosticSeverity.Error);
    }

    if (severity === 'errorsAndWarnings') {
        return diagnostics.filter(d =>
            d.severity === vscode.DiagnosticSeverity.Error ||
            d.severity === vscode.DiagnosticSeverity.Warning
        );
    }

    return [...diagnostics];
}

/**
 * Build language-client options from injected runtime services.
 */
export function createLanguageClientOptions(
    services: Pick<LanguageServerActivationServices, 'workspace' | 'runtimeContext'>,
): LanguageClientOptions {
    const { workspace, runtimeContext } = services;

    return {
        documentSelector: [{ scheme: 'file', language: 'chat' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.cha')
        },
        initializationOptions: {
            inlayHintsEnabled: runtimeContext.getInlayHintsEnabled(),
        },
        middleware: {
            handleDiagnostics(uri, diagnostics, next) {
                next(uri, filterDiagnosticsBySeverity(
                    diagnostics,
                    runtimeContext.getValidationSeverity(),
                ));
            },
            provideInlayHints(document, range, token, next) {
                if (!runtimeContext.getInlayHintsEnabled()) {
                    return [];
                }

                return next(document, range, token);
            },
        },
    };
}

/**
 * Start and return the TalkBank language client.
 *
 * Returns `undefined` when no `chatter-lsp` binary can be located — in that
 * case an actionable error message is shown to the user instead of spawning a
 * process that would fail with a cryptic ENOENT. Callers must treat the
 * absence of a client as "LSP features unavailable" and proceed accordingly.
 *
 * @param context - Extension activation context.
 * @returns Started language client instance, or `undefined` when no LSP binary
 *   was discovered.
 */
export async function activateLanguageServer(
    context: vscode.ExtensionContext,
    services: LanguageServerActivationServices,
): Promise<LanguageClient | undefined> {
    const lspBinary = services.executableService.findTalkbankLspBinary(
        context,
        services.runtimeContext.getConfiguredLspBinaryPath(),
    );
    if (!lspBinary) {
        const choice = await vscode.window.showErrorMessage(
            LSP_BINARY_NOT_FOUND_MESSAGE,
            OPEN_SETTINGS_ACTION,
        );
        if (choice === OPEN_SETTINGS_ACTION) {
            await vscode.commands.executeCommand(
                'workbench.action.openSettings',
                LSP_BINARY_PATH_SETTING,
            );
        }
        return undefined;
    }

    // The standalone `chatter-lsp` binary takes no subcommand — it speaks LSP
    // over stdio as soon as it starts.
    const serverArgs: string[] = [];
    const debugOptions = { execArgv: ['--nolazy', '--inspect=6009'] };

    const serverOptions: ServerOptions = {
        run: { command: lspBinary, args: serverArgs, transport: TransportKind.stdio } as any,
        debug: {
            command: lspBinary,
            args: serverArgs,
            transport: TransportKind.stdio,
            options: debugOptions
        } as any
    };

    const clientOptions = createLanguageClientOptions(services);
    const createLanguageClient =
        services.createLanguageClient ??
        ((id, name, server, client) => new LanguageClient(id, name, server, client));
    const client = createLanguageClient(
        'chatter-lsp',
        'TalkBank Language Server',
        serverOptions,
        clientOptions,
    );
    client.start();
    return client;
}
