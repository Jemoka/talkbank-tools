/**
 * Tests for the Effect-native execute-command client facade.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Effect } from 'effect';
import { describe, expect, it } from 'vitest';

import {
    ExecuteCommandRequestError,
    ExecuteCommandResponseError,
    TalkbankExecuteCommandClient,
} from '../lsp/executeCommandClient';

function createClient(sendRequest: (method: string, payload: unknown) => Promise<unknown>) {
    return new TalkbankExecuteCommandClient({
        isRunning: () => true,
        sendRequest,
    } as never);
}

describe('execute command client', () => {
    it('decodes a dot variant from the dependency-graph response', async () => {
        const client = createClient(async () => ({ kind: 'dot', source: 'digraph G {}' }));

        await expect(Effect.runPromise(client.showDependencyGraph(
            'file:///tmp/sample.cha',
            { line: 4, character: 2 } as never,
        ))).resolves.toEqual({ kind: 'dot', source: 'digraph G {}' });
    });

    it('decodes an unavailable variant without ever passing text as DOT', async () => {
        const client = createClient(async () => ({ kind: 'unavailable', reason: 'No %mor tier found' }));

        await expect(Effect.runPromise(client.showDependencyGraph(
            'file:///tmp/sample.cha',
            { line: 4, character: 2 } as never,
        ))).resolves.toEqual({ kind: 'unavailable', reason: 'No %mor tier found' });
    });

    it('rejects a bare string response that does not carry a kind discriminant', async () => {
        // Guards against a regression to the old stringly-typed protocol where
        // the Graphviz renderer could receive "No %mor tier found" as DOT.
        const client = createClient(async () => 'digraph G {}');

        const result = await Effect.runPromise(Effect.either(client.showDependencyGraph(
            'file:///tmp/sample.cha',
            { line: 4, character: 2 } as never,
        )));
        expect(result._tag).toBe('Left');
    });

    it('wraps transport failures in a tagged request error', async () => {
        const client = createClient(async () => {
            throw new Error('socket closed');
        });

        const result = await Effect.runPromise(Effect.either(client.getAlignmentSidecar(
            'file:///tmp/sample.cha',
        )));
        expect(result._tag).toBe('Left');
        if (result._tag === 'Left') {
            expect(result.left).toBeInstanceOf(ExecuteCommandRequestError);
            expect(result.left.command).toBe('talkbank/getAlignmentSidecar');
        }
    });

    it('wraps unexpected string commands responses in a tagged response error', async () => {
        const client = createClient(async () => ({ ok: false }));

        const result = await Effect.runPromise(Effect.either(client.formatIdLine({
            language: 'eng',
            corpus: 'Example',
            speaker: 'CHI',
            age: '2;03',
            sex: 'male',
            group: '',
            ses: '',
            role: 'Target_Child',
            education: '',
            custom: '',
        })));
        expect(result._tag).toBe('Left');
        if (result._tag === 'Left') {
            expect(result.left).toBeInstanceOf(ExecuteCommandResponseError);
            expect(result.left.command).toBe('talkbank/formatIdLine');
        }
    });
});
