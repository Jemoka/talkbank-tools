/**
 * Tests for persistent media-command state stores.
 */

import { describe, expect, it } from 'vitest';

import {
    createTranscriptionCommandStateStore,
    createWalkerCommandStateStore,
} from '../mediaCommandState';
import type { Segment, TimestampMs } from '../utils/bulletParser';

describe('media command state', () => {
    it('tracks transcription activation and reset state', () => {
        const state = createTranscriptionCommandStateStore();

        expect(state.snapshot()).toEqual({
            active: false,
            previousTimestamp: 0,
        });

        state.activate(1200 as TimestampMs);
        expect(state.snapshot()).toEqual({
            active: true,
            previousTimestamp: 1200,
        });

        state.updatePreviousTimestamp(2400 as TimestampMs);
        expect(state.snapshot()).toEqual({
            active: true,
            previousTimestamp: 2400,
        });

        state.reset();
        expect(state.snapshot()).toEqual({
            active: false,
            previousTimestamp: 0,
        });
    });

    it('tracks per-document walker segments and index changes', () => {
        const state = createWalkerCommandStateStore();
        const segments: Segment[] = [
            { line: 1, beg: 1000 as TimestampMs, end: 1500 as TimestampMs },
            { line: 2, beg: 2000 as TimestampMs, end: 2500 as TimestampMs },
        ];

        expect(state.get('doc')).toBeUndefined();

        expect(state.initialize('doc', segments)).toEqual({
            segments,
            index: -1,
        });
        expect(state.advance('doc', +1)).toEqual({
            segments,
            index: 0,
        });
        expect(state.advance('doc', +1)).toEqual({
            segments,
            index: 1,
        });
        expect(state.advance('doc', +1)).toEqual({
            segments,
            index: 1,
        });

        state.clear('doc');
        expect(state.get('doc')).toBeUndefined();
        expect(state.snapshot().size).toBe(0);
    });
});
