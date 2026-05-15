/**
 * Effect-native command and panel runtime for the VS Code extension.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import * as vscode from 'vscode';
import { Context, Data, Effect, Layer } from 'effect';

import type { ExtensionServices } from './activation/commands';
import { StructuredPayloadDecodeError } from './effectBoundary';
import {
    AsyncOperationError,
    CacheManagerService,
    ClanIntegrationService,
    ExecutableServiceService,
    ExecuteCommandClientService,
    ExtensionContextService,
    ExtensionEffectRequirements,
    ExtensionRuntimeContextService,
    LanguageClientService,
    runEitherWithLayer,
    runWithLayer,
    tryAsync,
} from './effectRuntime';
import {
    ExecuteCommandRequestError,
    ExecuteCommandResponseError,
    ExecuteCommandServerError,
} from './lsp/executeCommandClient';
import {
    createTranscriptionCommandStateStore,
    createWalkerCommandStateStore,
    TranscriptionCommandStateStore,
    TranscriptionCommandStateService,
    WalkerCommandStateStore,
    WalkerCommandStateService,
} from './mediaCommandState';
import {
    CoderCommandStateService,
    CoderCommandStateStore,
    createCoderCommandStateStore,
} from './coderState';
import {
    ReviewCommandStateService,
    ReviewCommandStateStore,
    createReviewCommandStateStore,
} from './reviewState';
import {
    NodeTextFileService,
    TextFileService as TextFileServiceTag,
    TextFileService,
} from './textFileService';

/**
 * Effect service exposing `vscode.window`.
 */
export const VSCodeWindowService = Context.GenericTag<typeof vscode.window>(
    'talkbank/VSCodeWindow',
);

/**
 * Effect service exposing `vscode.workspace`.
 */
export const VSCodeWorkspaceService = Context.GenericTag<typeof vscode.workspace>(
    'talkbank/VSCodeWorkspace',
);

/**
 * Effect service exposing `vscode.commands`.
 */
export const VSCodeCommandsService = Context.GenericTag<typeof vscode.commands>(
    'talkbank/VSCodeCommands',
);

/**
 * Effect environment used by extension commands and async panel callbacks.
 */
export type ExtensionCommandRequirements =
    | ExtensionEffectRequirements
    | typeof vscode.window
    | typeof vscode.workspace
    | typeof vscode.commands
    | TranscriptionCommandStateStore
    | WalkerCommandStateStore
    | CoderCommandStateStore
    | ReviewCommandStateStore
    | TextFileService;

/**
 * Tagged error for commands that require an active editor.
 */
export class NoActiveEditorError extends Data.TaggedError('NoActiveEditorError')<{}> {}

/**
 * Tagged error for commands that require an active CHAT editor.
 */
export class NoActiveChatEditorError extends Data.TaggedError('NoActiveChatEditorError')<{}> {}

/**
 * Tagged error for commands that require a running language server.
 */
export class LanguageServerNotRunningError extends Data.TaggedError('LanguageServerNotRunningError')<{}> {}

/**
 * Tagged error for transcript commands that require a `@Media:` header.
 */
export class MissingMediaHeaderError extends Data.TaggedError('MissingMediaHeaderError')<{}> {}

/**
 * Tagged error for transcript commands when the referenced media file is missing.
 */
export class MediaFileNotFoundError extends Data.TaggedError('MediaFileNotFoundError')<{
    readonly mediaName: string;
}> {}

/**
 * Tagged error for commands that expected at least one bullet segment.
 */
export class NoBulletMarkersError extends Data.TaggedError('NoBulletMarkersError')<{}> {}

/**
 * Tagged error for commands that expected one bullet at or before the cursor.
 */
export class NoBulletMarkerNearCursorError extends Data.TaggedError('NoBulletMarkerNearCursorError')<{}> {}

/**
 * Tagged error for operations that require an open media panel.
 */
export class PanelNotOpenError extends Data.TaggedError('PanelNotOpenError')<{
    readonly panel: string;
}> {}

/**
 * Tagged error for operations that require the source document to stay open.
 */
export class DocumentClosedError extends Data.TaggedError('DocumentClosedError')<{}> {}

/**
 * Tagged error for empty-but-required command responses.
 */
export class EmptyCommandResponseError extends Data.TaggedError('EmptyCommandResponseError')<{
    readonly command: string;
}> {}

/**
 * Shared runner that executes Effect programs with the extension command layer.
 */
export interface ExtensionCommandRunner {
    run<A, E>(program: Effect.Effect<A, E, ExtensionCommandRequirements>): Promise<A>;
    runEither<A, E>(program: Effect.Effect<A, E, ExtensionCommandRequirements>): Promise<{
        readonly _tag: 'Left' | 'Right';
        readonly left?: E;
        readonly right?: A;
    }>;
    fireAndForget<E>(
        label: string,
        program: Effect.Effect<unknown, E, ExtensionCommandRequirements>,
        onError?: (error: E) => void,
    ): void;
}

/**
 * Build the Effect layer used by command handlers and async panel work.
 */
export function makeExtensionCommandLayer(
    context: vscode.ExtensionContext,
    services: ExtensionServices,
): Layer.Layer<ExtensionCommandRequirements> {
    const transcriptionState = createTranscriptionCommandStateStore();
    const walkerState = createWalkerCommandStateStore();
    const coderState = createCoderCommandStateStore();
    const reviewState = createReviewCommandStateStore();
    const textFileService = new NodeTextFileService();

    return Layer.mergeAll(
        Layer.succeed(VSCodeWindowService, vscode.window),
        Layer.succeed(VSCodeWorkspaceService, vscode.workspace),
        Layer.succeed(VSCodeCommandsService, vscode.commands),
        Layer.succeed(ExtensionContextService, context),
        Layer.succeed(LanguageClientService, services.client),
        Layer.succeed(ExecuteCommandClientService, services.commands),
        Layer.succeed(ExecutableServiceService, services.executableService),
        Layer.succeed(CacheManagerService, services.cacheManager),
        Layer.succeed(ClanIntegrationService, services.clanIntegration),
        Layer.succeed(ExtensionRuntimeContextService, services.runtimeContext),
        Layer.succeed(TranscriptionCommandStateService, transcriptionState),
        Layer.succeed(WalkerCommandStateService, walkerState),
        Layer.succeed(CoderCommandStateService, coderState),
        Layer.succeed(ReviewCommandStateService, reviewState),
        Layer.succeed(TextFileServiceTag, textFileService),
    );
}

/**
 * Create one reusable Effect runner for the activated extension.
 */
export function createExtensionCommandRunner(
    context: vscode.ExtensionContext,
    services: ExtensionServices,
): ExtensionCommandRunner {
    const layer = makeExtensionCommandLayer(context, services);

    return {
        run: program => runWithLayer(program, layer),
        runEither: async program => {
            const result = await runEitherWithLayer(program, layer);
            return result._tag === 'Left'
                ? { _tag: 'Left', left: result.left }
                : { _tag: 'Right', right: result.right };
        },
        fireAndForget: (label, program, onError) => {
            void runEitherWithLayer(program, layer).then(result => {
                if (result._tag === 'Left') {
                    console.error(`Effect failure in ${label}`, result.left);
                    onError?.(result.left);
                }
            });
        },
    };
}

/**
 * Register one Effect-native extension command.
 */
export function registerEffectCommand<args extends unknown[] = []>(
    name: string,
    runner: ExtensionCommandRunner,
    handler: (...args: args) => Effect.Effect<void, unknown, ExtensionCommandRequirements>,
): vscode.Disposable {
    return vscode.commands.registerCommand(name, (...args: unknown[]) => {
        runner.fireAndForget(
            name,
            Effect.catchAll(
                Effect.suspend(() => handler(...args as args)),
                error => showEffectError(error),
            ),
        );
    });
}

/**
 * Run one async panel effect without blocking the VS Code message callback.
 */
export function runPanelEffect<E>(
    label: string,
    runner: ExtensionCommandRunner,
    program: Effect.Effect<void, E, ExtensionCommandRequirements>,
    onError?: (message: string) => void,
): void {
    runner.fireAndForget(
        label,
        program,
        error => {
            const message = formatEffectError(error);
            if (onError) {
                onError(message);
                return;
            }

            console.error(`Unhandled panel effect error in ${label}: ${message}`);
        },
    );
}

/**
 * Require that there is an active editor.
 */
export function requireActiveEditor(): Effect.Effect<
    vscode.TextEditor,
    NoActiveEditorError,
    typeof vscode.window
> {
    return Effect.flatMap(VSCodeWindowService, window => {
        const editor = window.activeTextEditor;
        return editor === undefined
            ? Effect.fail(new NoActiveEditorError())
            : Effect.succeed(editor);
    });
}

/**
 * Require that the active editor is a CHAT document.
 */
export function requireActiveChatEditor(): Effect.Effect<
    vscode.TextEditor,
    NoActiveEditorError | NoActiveChatEditorError,
    typeof vscode.window
> {
    return Effect.flatMap(requireActiveEditor(), editor => editor.document.languageId === 'chat'
        ? Effect.succeed(editor)
        : Effect.fail(new NoActiveChatEditorError()));
}

/**
 * Require that the Effect-backed execute-command client is currently running.
 */
export function requireRunningLanguageServer(
    isRunning: boolean,
): Effect.Effect<void, LanguageServerNotRunningError> {
    return isRunning
        ? Effect.void
        : Effect.fail(new LanguageServerNotRunningError());
}

/**
 * Render one domain error into a consistent user-facing message.
 */
export function formatEffectError(error: unknown): string {
    if (error instanceof NoActiveEditorError) {
        return 'No active editor';
    }
    if (error instanceof NoActiveChatEditorError) {
        return 'No active CHAT (.cha) file';
    }
    if (error instanceof LanguageServerNotRunningError) {
        return 'Language server is not running';
    }
    if (error instanceof MissingMediaHeaderError) {
        return 'No @Media: header found in this file';
    }
    if (error instanceof MediaFileNotFoundError) {
        return `Media file not found: ${error.mediaName}`;
    }
    if (error instanceof NoBulletMarkersError) {
        return 'No bullet markers found in this file';
    }
    if (error instanceof NoBulletMarkerNearCursorError) {
        return 'No bullet marker found at or before the cursor';
    }
    if (error instanceof PanelNotOpenError) {
        return `No ${error.panel} is currently open`;
    }
    if (error instanceof DocumentClosedError) {
        return 'Document is no longer open.';
    }
    if (error instanceof EmptyCommandResponseError) {
        return `No result returned for ${error.command}`;
    }
    if (error instanceof ExecuteCommandRequestError) {
        return `Failed to execute ${error.command}: ${error.details}`;
    }
    if (error instanceof ExecuteCommandResponseError) {
        return `Unexpected response for ${error.command}: ${error.details}`;
    }
    if (error instanceof ExecuteCommandServerError) {
        return error.details;
    }
    if (error instanceof StructuredPayloadDecodeError) {
        return `Failed to decode ${error.command}: ${error.details}`;
    }
    if (error instanceof AsyncOperationError) {
        return `Operation failed during ${error.operation}`;
    }
    if (error instanceof Error) {
        return error.message;
    }
    return String(error);
}

function effectErrorSeverity(error: unknown): 'error' | 'warning' | 'info' {
    if (
        error instanceof NoActiveEditorError
        || error instanceof NoActiveChatEditorError
        || error instanceof MissingMediaHeaderError
        || error instanceof MediaFileNotFoundError
        || error instanceof NoBulletMarkersError
        || error instanceof NoBulletMarkerNearCursorError
        || error instanceof PanelNotOpenError
        || error instanceof LanguageServerNotRunningError
    ) {
        return 'warning';
    }
    return 'error';
}

/**
 * Present one Effect failure through the VS Code window service.
 */
export function showEffectError(
    error: unknown,
): Effect.Effect<void, AsyncOperationError, typeof vscode.window> {
    const message = formatEffectError(error);
    return Effect.flatMap(VSCodeWindowService, window => {
        switch (effectErrorSeverity(error)) {
            case 'warning':
                return Effect.asVoid(tryAsync('show warning message', () => Promise.resolve(
                    window.showWarningMessage(message),
                )));
            case 'info':
                return Effect.asVoid(tryAsync('show information message', () => Promise.resolve(
                    window.showInformationMessage(message),
                )));
            case 'error':
            default:
                return Effect.asVoid(tryAsync('show error message', () => Promise.resolve(
                    window.showErrorMessage(message),
                )));
        }
    });
}
