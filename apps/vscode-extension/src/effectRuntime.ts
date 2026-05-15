/**
 * Shared Effect foundation for the VS Code extension.
 *
 * This is the bridge from the current aggregate service bag used by extension
 * activation into the Layer / Context world that future Effect-native command,
 * panel, and client code will consume.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import type * as vscode from 'vscode';
import { Context, Data, Effect, Either, Layer } from 'effect';
import type { LanguageClient } from 'vscode-languageclient/node';

import type { ExtensionServices } from './activation/commands';
import type { CacheManager } from './cacheManager';
import type { ClanIntegration } from './clanIntegration';
import type { TalkbankExecuteCommandClient } from './lsp/executeCommandClient';
import type { ExtensionExecutableService } from './runtimeServices';
import type { ExtensionRuntimeContext } from './runtimeContext';

/**
 * Tagged error for one rejected or thrown async boundary operation.
 */
export class AsyncOperationError extends Data.TaggedError('AsyncOperationError')<{
    readonly operation: string;
    readonly cause: unknown;
}> {}

/**
 * Effect service for the VS Code extension activation context.
 */
export const ExtensionContextService = Context.GenericTag<vscode.ExtensionContext>(
    'talkbank/ExtensionContext',
);

/**
 * Effect service for the raw LSP client.
 */
export const LanguageClientService = Context.GenericTag<LanguageClient>(
    'talkbank/LanguageClient',
);

/**
 * Effect service for the typed execute-command client facade.
 */
export const ExecuteCommandClientService = Context.GenericTag<TalkbankExecuteCommandClient>(
    'talkbank/ExecuteCommandClient',
);

/**
 * Effect service for the shared executable boundary.
 */
export const ExecutableServiceService = Context.GenericTag<ExtensionExecutableService>(
    'talkbank/ExecutableService',
);

/**
 * Effect service for the cache manager.
 */
export const CacheManagerService = Context.GenericTag<CacheManager>('talkbank/CacheManager');

/**
 * Effect service for CLAN integration helpers.
 */
export const ClanIntegrationService = Context.GenericTag<ClanIntegration>(
    'talkbank/ClanIntegration',
);

/**
 * Effect service for the shared runtime context boundary.
 */
export const ExtensionRuntimeContextService = Context.GenericTag<ExtensionRuntimeContext>(
    'talkbank/ExtensionRuntimeContext',
);

/**
 * Current Effect environment provided by extension activation.
 */
export type ExtensionEffectRequirements =
    | vscode.ExtensionContext
    | LanguageClient
    | TalkbankExecuteCommandClient
    | ExtensionExecutableService
    | CacheManager
    | ClanIntegration
    | ExtensionRuntimeContext;

/**
 * Build one Layer from the current extension activation context plus service bag.
 */
export function makeExtensionServicesLayer(
    context: vscode.ExtensionContext,
    services: ExtensionServices,
) {
    return Layer.mergeAll(
        Layer.succeed(ExtensionContextService, context),
        Layer.succeed(LanguageClientService, services.client),
        Layer.succeed(ExecuteCommandClientService, services.commands),
        Layer.succeed(ExecutableServiceService, services.executableService),
        Layer.succeed(CacheManagerService, services.cacheManager),
        Layer.succeed(ClanIntegrationService, services.clanIntegration),
        Layer.succeed(ExtensionRuntimeContextService, services.runtimeContext),
    );
}

/**
 * Run one Effect program with the current extension service Layer.
 */
export function runWithExtensionServices<A, E>(
    program: Effect.Effect<A, E, ExtensionEffectRequirements>,
    context: vscode.ExtensionContext,
    services: ExtensionServices,
): Promise<A> {
    return runWithLayer(program, makeExtensionServicesLayer(context, services));
}

/**
 * Run one fully-provided Effect program.
 */
export function runWithLayer<A, E, R>(
    program: Effect.Effect<A, E, R>,
    layer: Layer.Layer<R>,
): Promise<A> {
    return Effect.runPromise(Effect.provide(program, layer));
}

/**
 * Run one fully-provided Effect program and return the typed success/error result.
 */
export function runEitherWithLayer<A, E, R>(
    program: Effect.Effect<A, E, R>,
    layer: Layer.Layer<R>,
): Promise<Either.Either<A, E>> {
    return Effect.runPromise(Effect.either(Effect.provide(program, layer)));
}

/**
 * Wrap one async boundary operation in a tagged Effect error.
 */
export function tryAsync<A>(
    operation: string,
    evaluate: () => Promise<A>,
): Effect.Effect<A, AsyncOperationError> {
    return Effect.tryPromise({
        try: evaluate,
        catch: cause => new AsyncOperationError({
            operation,
            cause,
        }),
    });
}
