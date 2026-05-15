import { describe, expect, it } from 'vitest';

import { createCoderCommandStateStore } from '../coderState';

describe('coderState', () => {
    it('tracks activation, current utterance line, and reset state', () => {
        const store = createCoderCommandStateStore();

        expect(store.snapshot()).toEqual({
            active: false,
            codesTree: [],
            codesFilePath: undefined,
            currentUtteranceLine: -1,
        });

        store.activate(
            [
                {
                    code: '$PRA',
                    depth: 0,
                    children: [],
                },
            ],
            '/tmp/codes.cut',
        );
        store.setCurrentUtteranceLine(42);

        expect(store.snapshot()).toEqual({
            active: true,
            codesTree: [
                {
                    code: '$PRA',
                    depth: 0,
                    children: [],
                },
            ],
            codesFilePath: '/tmp/codes.cut',
            currentUtteranceLine: 42,
        });

        store.reset();

        expect(store.snapshot()).toEqual({
            active: false,
            codesTree: [],
            codesFilePath: undefined,
            currentUtteranceLine: -1,
        });
    });
});
