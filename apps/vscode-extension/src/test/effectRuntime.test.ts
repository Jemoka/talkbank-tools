/**
 * Tests for the shared Effect runtime foundation.
 */

import type * as vscode from 'vscode';
import { Effect } from 'effect';
import { describe, expect, it } from 'vitest';

import {
    AsyncOperationError,
    ExecuteCommandClientService,
    ExtensionContextService,
    ExtensionRuntimeContextService,
    ExecutableServiceService,
    runWithExtensionServices,
    tryAsync,
} from '../effectRuntime';
import type { ExtensionServices } from '../activation/commands';
import { createMockExtensionContext } from './mocks/vscode';

function createTestServices(): {
    readonly context: vscode.ExtensionContext;
    readonly services: ExtensionServices;
} {
    const context = createMockExtensionContext() as unknown as vscode.ExtensionContext;
    const services = {
        client: { service: 'client' } as unknown as ExtensionServices['client'],
        commands: { service: 'commands' } as unknown as ExtensionServices['commands'],
        executableService: {
            service: 'executable',
        } as unknown as ExtensionServices['executableService'],
        cacheManager: { service: 'cache' } as unknown as ExtensionServices['cacheManager'],
        clanIntegration: { service: 'clan' } as unknown as ExtensionServices['clanIntegration'],
        runtimeContext: {
            service: 'runtime',
        } as unknown as ExtensionServices['runtimeContext'],
    } satisfies ExtensionServices;

    return {
        context,
        services,
    };
}

describe('effect runtime foundation', () => {
    it('provides extension services through the shared Layer', async () => {
        const { context, services } = createTestServices();

        const program = Effect.flatMap(ExtensionContextService, extensionContext =>
            Effect.flatMap(ExecuteCommandClientService, commands =>
                Effect.flatMap(ExecutableServiceService, executableService =>
                    Effect.flatMap(ExtensionRuntimeContextService, runtimeContext => Effect.succeed({
                        extensionPath: extensionContext.extensionPath,
                        commands,
                        executableService,
                        runtimeContext,
                    })),
                ),
            ),
        );

        await expect(runWithExtensionServices(program, context, services)).resolves.toEqual({
            extensionPath: context.extensionPath,
            commands: services.commands,
            executableService: services.executableService,
            runtimeContext: services.runtimeContext,
        });
    });

    it('wraps rejected async work in a tagged Effect error', async () => {
        await expect(
            Effect.runPromise(Effect.either(tryAsync('load participants', async () => {
                throw new Error('boom');
            }))),
        ).resolves.toMatchObject({
            _tag: 'Left',
            left: {
                _tag: 'AsyncOperationError',
                operation: 'load participants',
            },
        } satisfies {
            _tag: 'Left';
            left: Pick<AsyncOperationError, '_tag' | 'operation'>;
        });
    });
});
