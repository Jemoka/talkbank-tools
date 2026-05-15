/* eslint-disable @typescript-eslint/naming-convention */
import { describe, expect, it } from 'vitest';
import { AlignmentSidecarDocument } from '../lsp/executeCommandPayloads';
import { segmentsFromAlignmentSidecar } from '../utils/alignmentSidecar';

describe('segmentsFromAlignmentSidecar', () => {
    it('returns null when no sidecar payload is available', () => {
        expect(segmentsFromAlignmentSidecar(null)).toBeNull();
    });

    it('builds segments from utterance timing and range lines', () => {
        const payload: AlignmentSidecarDocument = {
            schema_version: 1,
            uri: 'file:///tmp/sample.cha',
            utterances: [
                {
                    utterance_index: 0,
                    speaker: 'CHI',
                    range: { start: { line: 5, character: 0 }, end: { line: 5, character: 20 } },
                    timing: { start_ms: 100, end_ms: 240, skip: false },
                },
                {
                    utterance_index: 1,
                    speaker: 'MOT',
                    range: { start: { line: 7, character: 0 }, end: { line: 7, character: 18 } },
                    timing: { start_ms: 260, end_ms: 410, skip: false },
                },
            ],
        };

        expect(segmentsFromAlignmentSidecar(payload)).toEqual([
            { line: 5, beg: 100, end: 240 },
            { line: 7, beg: 260, end: 410 },
        ]);
    });

    it('skips timing entries marked with skip=true', () => {
        const payload: AlignmentSidecarDocument = {
            schema_version: 1,
            uri: 'file:///tmp/skip.cha',
            utterances: [
                {
                    utterance_index: 0,
                    speaker: 'CHI',
                    range: { start: { line: 2, character: 0 }, end: { line: 2, character: 10 } },
                    timing: { start_ms: 50, end_ms: 100, skip: true },
                },
                {
                    utterance_index: 1,
                    speaker: 'CHI',
                    range: { start: { line: 3, character: 0 }, end: { line: 3, character: 10 } },
                    timing: { start_ms: 120, end_ms: 190, skip: false },
                },
            ],
        };

        expect(segmentsFromAlignmentSidecar(payload)).toEqual([
            { line: 3, beg: 120, end: 190 },
        ]);
    });

    it('ignores incomplete timing rows but keeps valid rows', () => {
        const payload: AlignmentSidecarDocument = {
            schema_version: 1,
            uri: 'file:///tmp/incomplete.cha',
            utterances: [
                {
                    utterance_index: 0,
                    speaker: 'CHI',
                    range: null,
                    timing: { start_ms: 0, end_ms: 10, skip: false },
                },
                {
                    utterance_index: 1,
                    speaker: 'CHI',
                    range: { start: { line: 2, character: 0 }, end: { line: 2, character: 1 } },
                    timing: null,
                },
                {
                    utterance_index: 2,
                    speaker: 'CHI',
                    range: { start: { line: 3, character: 0 }, end: { line: 3, character: 1 } },
                    timing: { start_ms: 40, end_ms: 20, skip: false },
                },
                {
                    utterance_index: 3,
                    speaker: 'CHI',
                    range: { start: { line: 4, character: 0 }, end: { line: 4, character: 1 } },
                    timing: { start_ms: 60, end_ms: 90, skip: false },
                },
            ],
        };

        expect(segmentsFromAlignmentSidecar(payload)).toEqual([
            { line: 4, beg: 60, end: 90 },
        ]);
    });

    it('returns empty segment list for valid sidecar with no timing rows', () => {
        const payload: AlignmentSidecarDocument = {
            schema_version: 1,
            uri: 'file:///tmp/empty.cha',
            utterances: [],
        };

        expect(segmentsFromAlignmentSidecar(payload)).toEqual([]);
    });
});
