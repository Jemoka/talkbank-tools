import { describe, expect, it } from 'vitest';

import executeCommandContract from './fixtures/executeCommandContract.json';
import { talkbankExecuteCommandList } from '../lsp/executeCommandClient';

describe('execute-command contract manifest', () => {
    it('matches the TypeScript command list exactly', () => {
        expect([...talkbankExecuteCommandList]).toEqual(executeCommandContract);
    });

    it('contains each command only once', () => {
        expect(new Set(executeCommandContract).size).toBe(executeCommandContract.length);
    });
});
