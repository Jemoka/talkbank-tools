/**
 * Tests for the Effect-native command runtime helpers.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Effect } from 'effect';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('vscode', () => ({
    window: {
        activeTextEditor: undefined,
    },
    workspace: {},
    commands: {
        registerCommand: vi.fn(),
    },
}));

import {
    createExtensionCommandRunner,
    MediaFileNotFoundError,
    NoActiveChatEditorError,
    NoActiveEditorError,
    VSCodeWindowService,
    formatEffectError,
    requireActiveChatEditor,
    requireActiveEditor,
} from '../effectCommandRuntime';
import { ExecutableServiceService } from '../effectRuntime';
import { TranscriptionCommandStateService } from '../mediaCommandState';

describe('effect command runtime', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('requires an active editor', async () => {
        const result = await Effect.runPromise(Effect.either(
            requireActiveEditor().pipe(
                Effect.provideService(VSCodeWindowService, {
                    activeTextEditor: undefined,
                } as never),
            ),
        ));

        expect(result._tag).toBe('Left');
        if (result._tag === 'Left') {
            expect(result.left).toBeInstanceOf(NoActiveEditorError);
        }
    });

    it('rejects non-chat editors for chat-only commands', async () => {
        const result = await Effect.runPromise(Effect.either(
            requireActiveChatEditor().pipe(
                Effect.provideService(VSCodeWindowService, {
                    activeTextEditor: {
                        document: {
                            languageId: 'plaintext',
                        },
                    },
                } as never),
            ),
        ));

        expect(result._tag).toBe('Left');
        if (result._tag === 'Left') {
            expect(result.left).toBeInstanceOf(NoActiveChatEditorError);
        }
    });

    it('formats domain errors for user-facing display', () => {
        expect(formatEffectError(new MediaFileNotFoundError({
            mediaName: 'sample.mov',
        }))).toBe('Media file not found: sample.mov');
    });

    it('keeps persistent command state across runner invocations', async () => {
        const runner = createExtensionCommandRunner({} as never, {
            client: {} as never,
            commands: {} as never,
            executableService: {
                service: 'executable',
            } as never,
            cacheManager: {} as never,
            clanIntegration: {} as never,
            runtimeContext: {} as never,
        });

        await runner.run(Effect.flatMap(
            TranscriptionCommandStateService,
            state => Effect.sync(() => {
                state.activate(1200 as never);
            }),
        ));

        const snapshot = await runner.run(Effect.flatMap(
            TranscriptionCommandStateService,
            state => Effect.sync(() => state.snapshot()),
        ));

        expect(snapshot).toEqual({
            active: true,
            previousTimestamp: 1200,
        });
    });

    it('provides the shared executable service through the command layer', async () => {
        const runner = createExtensionCommandRunner({} as never, {
            client: {} as never,
            commands: {} as never,
            executableService: {
                service: 'executable',
            } as never,
            cacheManager: {} as never,
            clanIntegration: {} as never,
            runtimeContext: {} as never,
        });

        const executableService = await runner.run(Effect.flatMap(
            ExecutableServiceService,
            service => Effect.succeed(service),
        ));

        expect(executableService).toEqual({
            service: 'executable',
        });
    });
});
