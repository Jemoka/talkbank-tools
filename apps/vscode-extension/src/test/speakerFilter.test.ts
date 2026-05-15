/**
 * Tests for speakerFilter.ts utilities.
 *
 * parseSpeakers and filterBySpeakers are pure functions with no external
 * dependencies, so these tests need no mocks.
 */

import { describe, it, expect } from 'vitest';
import * as fc from 'fast-check';
import { parseSpeakers, filterBySpeakers, SpeakerCode } from '../utils/speakerFilter';

/** Cast string literals to SpeakerCode Set for tests. */
function speakerSet(...codes: string[]): Set<SpeakerCode> {
    return new Set(codes as SpeakerCode[]);
}

// ---------------------------------------------------------------------------
// parseSpeakers
// ---------------------------------------------------------------------------

describe('parseSpeakers', () => {
    it('returns empty array when no @Participants header present', () => {
        expect(parseSpeakers('@UTF8\n@Begin\n@End')).toEqual([]);
    });

    it('extracts a single speaker code', () => {
        const text = '@UTF8\n@Begin\n@Participants:\tCHI Ruth Target_Child\n@End';
        expect(parseSpeakers(text)).toEqual(['CHI']);
    });

    it('extracts multiple speaker codes in order', () => {
        const text = '@Participants:\tCHI Ruth Target_Child, MOT Mary Mother, FAT Fred Father';
        expect(parseSpeakers(text)).toEqual(['CHI', 'MOT', 'FAT']);
    });

    it('handles space-separated header (non-canonical but accepted)', () => {
        const text = '@Participants: CHI Ruth Target_Child, MOT Mary Mother';
        expect(parseSpeakers(text)).toEqual(['CHI', 'MOT']);
    });

    it('ignores trailing spaces and empty entries', () => {
        const text = '@Participants:\tCHI Ruth Target_Child,  , MOT Mary Mother';
        expect(parseSpeakers(text)).toEqual(['CHI', 'MOT']);
    });

    it('works when @Participants is not the first header', () => {
        const text = '@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Ruth Target_Child\n@ID:\teng|corpus|CHI\n@End';
        expect(parseSpeakers(text)).toEqual(['CHI']);
    });

    it('returns codes that may be longer than three characters', () => {
        // CHAT allows non-standard codes in some corpora
        const text = '@Participants:\tCHILD Ruth Target_Child';
        expect(parseSpeakers(text)).toEqual(['CHILD']);
    });

    // Property: always returns an array (never throws)
    it('never throws for arbitrary string input', () => {
        fc.assert(fc.property(fc.string(), (s) => {
            expect(() => parseSpeakers(s)).not.toThrow();
        }));
    });

    // Property: result is always an array of non-empty strings
    it('all returned codes are non-empty strings', () => {
        fc.assert(fc.property(fc.string(), (s) => {
            const codes = parseSpeakers(s);
            expect(Array.isArray(codes)).toBe(true);
            for (const code of codes) {
                expect(typeof code).toBe('string');
                expect(code.length).toBeGreaterThan(0);
            }
        }));
    });
});

// ---------------------------------------------------------------------------
// filterBySpeakers
// ---------------------------------------------------------------------------

const SAMPLE_CHAT = `@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Ruth Target_Child, MOT Mary Mother
@ID:\teng|corpus|CHI|||||Target_Child|||
@ID:\teng|corpus|MOT|||||Mother|||
*CHI:\thello world .
%mor:\tn|hello n|world .
*MOT:\thi there .
*CHI:\twant cookie .
@End`;

describe('filterBySpeakers', () => {
    it('returns all utterances when all speakers selected', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('CHI', 'MOT'));
        expect(result).toContain('*CHI:\thello world .');
        expect(result).toContain('*MOT:\thi there .');
        expect(result).toContain('*CHI:\twant cookie .');
    });

    it('keeps only CHI utterances when only CHI selected', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('CHI'));
        expect(result).toContain('*CHI:\thello world .');
        expect(result).toContain('*CHI:\twant cookie .');
        expect(result).not.toContain('*MOT:\thi there .');
    });

    it('keeps only MOT utterances when only MOT selected', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('MOT'));
        expect(result).not.toContain('*CHI:');
        expect(result).toContain('*MOT:\thi there .');
    });

    it('always includes all file-level headers', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('CHI'));
        expect(result).toContain('@UTF8');
        expect(result).toContain('@Begin');
        expect(result).toContain('@End');
        expect(result).toContain('@Participants:');
        expect(result).toContain('@ID:');
    });

    it('includes dependent tiers of selected utterances', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('CHI'));
        // %mor dependent tier of the first CHI utterance should be kept
        expect(result).toContain('%mor:\tn|hello n|world .');
    });

    it('excludes dependent tiers of non-selected utterances', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet('MOT'));
        // %mor belongs to CHI utterance, should be excluded
        expect(result).not.toContain('%mor:');
    });

    it('returns empty-utterance document when no speakers selected', () => {
        const result = filterBySpeakers(SAMPLE_CHAT, speakerSet());
        expect(result).not.toContain('*CHI:');
        expect(result).not.toContain('*MOT:');
        // Headers are still present
        expect(result).toContain('@Begin');
    });

    it('discards @Bg when its immediately following utterance is not selected', () => {
        const text = `@UTF8
@Begin
@Participants:\tCHI Ruth Target_Child, MOT Mary Mother
*CHI:\thello .
@Bg:\tgem1
*MOT:\thi .
@Eg:\tgem1
*CHI:\tbye .
@End`;
        // @Bg is pending when MOT (not selected) is flushed → discarded.
        // @Eg is pending when CHI (selected) is flushed → included.
        // Note: this produces an unbalanced gem (@Eg without @Bg) — a known
        // limitation of the single-pass streaming approach.
        const chiOnly = filterBySpeakers(text, speakerSet('CHI'));
        expect(chiOnly).not.toContain('@Bg:');
        expect(chiOnly).toContain('*CHI:\thello .');
        expect(chiOnly).toContain('*CHI:\tbye .');
    });

    it('never throws for empty input', () => {
        expect(() => filterBySpeakers('', speakerSet('CHI'))).not.toThrow();
        expect(filterBySpeakers('', speakerSet('CHI'))).toBe('');
    });

    // Property: result always contains @Begin if source does
    it('preserves @Begin when present', () => {
        fc.assert(fc.property(
            fc.constantFrom('CHI', 'MOT', 'FAT'),
            (code) => {
                const text = `@Begin\n*${code}:\thello .\n@End`;
                const result = filterBySpeakers(text, speakerSet(code));
                expect(result).toContain('@Begin');
            }
        ));
    });

    // Property: filtering by all speakers ≥ filtering by one speaker (more lines)
    it('selecting more speakers never produces fewer lines', () => {
        fc.assert(fc.property(
            fc.constantFrom(['CHI'], ['MOT'], ['CHI', 'MOT']),
            (codes) => {
                const result = filterBySpeakers(SAMPLE_CHAT, new Set(codes as unknown as SpeakerCode[]));
                const lineCount = result.split('\n').length;
                const chiOnly = filterBySpeakers(SAMPLE_CHAT, speakerSet('CHI'));
                const chiLines = chiOnly.split('\n').length;
                if (codes.length === 2) {
                    expect(lineCount).toBeGreaterThanOrEqual(chiLines);
                }
            }
        ));
    });
});
