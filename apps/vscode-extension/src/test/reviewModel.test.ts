import { describe, it, expect } from 'vitest';
import {
    parseReviewItems,
    countReviewed,
    nextUnreviewed,
    prevUnreviewed,
    formatXrevContent,
} from '../reviewModel';

const SAMPLE_CHAT = `@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
@ID:\teng|test|MOT|||||Mother|||
*CHI:\thello . \u00151000_2000\u0015
%xalign:\tfa_aligned words_timed=1/1 coverage=100%
*MOT:\thi sweetie . \u00152100_3500\u0015
%xalign:\tboundary_averaged overlap=155ms machine=2100_3500 adjacent=CHI:5
%xrev:\t[?]
*CHI:\tcookie . \u00154000_5000\u0015
%xalign:\tgap_filled gap=500ms same_speaker machine=4500_5000 snapped_start=4000
%xrev:\t[ok] sounds right
*MOT:\tno more .
%xalign:\tlis_removal same_speaker_non_monotonic machine=3200_4800
%xrev:\t[wrong] completely off
@End
`;

describe('parseReviewItems', () => {
    it('finds all %xrev tiers', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items).toHaveLength(3);
    });

    it('extracts speaker codes', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items[0].speaker).toBe('MOT');
        expect(items[1].speaker).toBe('CHI');
        expect(items[2].speaker).toBe('MOT');
    });

    it('extracts review statuses', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items[0].status).toBe('?');
        expect(items[1].status).toBe('ok');
        expect(items[2].status).toBe('wrong');
    });

    it('extracts notes', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items[0].notes).toBe('');
        expect(items[1].notes).toBe('sounds right');
        expect(items[2].notes).toBe('completely off');
    });

    it('extracts xalign reasons', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items[0].xalignReason).toContain('boundary_averaged');
        expect(items[1].xalignReason).toContain('gap_filled');
        expect(items[2].xalignReason).toContain('lis_removal');
    });

    it('extracts bullet timestamps', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(items[0].begMs).toBe(2100);
        expect(items[0].endMs).toBe(3500);
        expect(items[1].begMs).toBe(4000);
        expect(items[1].endMs).toBe(5000);
        // MOT's last utterance has no bullet
        expect(items[2].begMs).toBe(-1);
        expect(items[2].endMs).toBe(-1);
    });

    it('returns empty for file with no %xrev tiers', () => {
        const items = parseReviewItems('@UTF8\n@Begin\n*CHI:\thello .\n@End\n');
        expect(items).toHaveLength(0);
    });
});

describe('countReviewed', () => {
    it('counts non-? items', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        expect(countReviewed(items)).toBe(2); // ok + wrong
    });
});

describe('nextUnreviewed', () => {
    it('finds next ? item wrapping around', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        // From index 0 (which is ?), next ? wraps to 0
        expect(nextUnreviewed(items, 0)).toBe(0);
        // From index 1 (ok), next ? is 0
        expect(nextUnreviewed(items, 1)).toBe(0);
        // From index 2 (wrong), next ? is 0
        expect(nextUnreviewed(items, 2)).toBe(0);
    });

    it('returns -1 when all reviewed', () => {
        const items = parseReviewItems(SAMPLE_CHAT).map(i => ({ ...i, status: 'ok' as const }));
        expect(nextUnreviewed(items, 0)).toBe(-1);
    });
});

describe('prevUnreviewed', () => {
    it('finds previous ? item wrapping around', () => {
        const items = parseReviewItems(SAMPLE_CHAT);
        // From index 1, prev ? is 0
        expect(prevUnreviewed(items, 1)).toBe(0);
    });
});

describe('formatXrevContent', () => {
    it('formats status only', () => {
        expect(formatXrevContent('ok', '')).toBe('[ok]');
    });

    it('formats status with notes', () => {
        expect(formatXrevContent('early', 'about 200ms before speech')).toBe(
            '[early] about 200ms before speech',
        );
    });

    it('formats unreviewed marker', () => {
        expect(formatXrevContent('?', '')).toBe('[?]');
    });
});
