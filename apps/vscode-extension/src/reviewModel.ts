/**
 * Pure data types and parsers for the alignment review system.
 *
 * Parses `%xalign` (machine decisions) and `%xrev` (human review markers)
 * tiers from CHAT file text. No VS Code dependency — testable in isolation.
 *
 * Tier format:
 *   %xalign:\t<structured key=value decision metadata>
 *   %xrev:\t[?]              — unreviewed (needs human check)
 *   %xrev:\t[ok]             — timing confirmed correct
 *   %xrev:\t[early]          — bullet starts before speech
 *   %xrev:\t[late]           — bullet starts after speech
 *   %xrev:\t[wrong]          — completely wrong location
 *   %xrev:\t[corrected]      — reviewer fixed the bullet
 *   %xrev:\t[stamped]        — reviewer added timing from scratch
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Status markers for `%xrev` tiers. */
export type ReviewStatus =
    | '?'
    | 'ok'
    | 'early'
    | 'late'
    | 'wrong'
    | 'corrected'
    | 'stamped';

/** A single reviewable utterance with its associated tiers. */
export interface ReviewItem {
    /** 0-indexed line of the `*SPK:` main tier line. */
    readonly utteranceLine: number;
    /** 0-indexed line of the `%xrev:` tier (or -1 if absent). */
    readonly xrevLine: number;
    /** 0-indexed line of the `%xalign:` tier (or -1 if absent). */
    readonly xalignLine: number;
    /** Current review status parsed from `%xrev`. */
    readonly status: ReviewStatus;
    /** Raw `%xalign` content (the machine's reasoning). */
    readonly xalignReason: string;
    /** Speaker code (e.g., "CHI", "PIL"). */
    readonly speaker: string;
    /** First ~80 chars of the utterance text. */
    readonly utteranceText: string;
    /** Bullet start time in milliseconds (-1 if no bullet). */
    readonly begMs: number;
    /** Bullet end time in milliseconds (-1 if no bullet). */
    readonly endMs: number;
    /** User-appended notes (text after the status marker in `%xrev`). */
    readonly notes: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const XREV_STATUS_REGEX = /^\[([?\w]+)\]\s*(.*)/;
const BULLET_REGEX = /\u0015(\d+)_(\d+)\u0015/;
const SPEAKER_REGEX = /^\*(\w+):/;

/** All valid review status values. */
export const REVIEW_STATUSES: readonly ReviewStatus[] = [
    '?', 'ok', 'early', 'late', 'wrong', 'corrected', 'stamped',
];

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/**
 * Parse all reviewable items from a CHAT file's text.
 *
 * Scans for `%xrev:` tiers, associates each with its parent `*SPK:` utterance
 * and any adjacent `%xalign:` tier, extracts status markers and bullet
 * timestamps.
 */
export function parseReviewItems(text: string): ReviewItem[] {
    const lines = text.split('\n');
    const items: ReviewItem[] = [];

    // Track the most recent main tier line and its properties.
    let currentUtteranceLine = -1;
    let currentSpeaker = '';
    let currentUtteranceText = '';
    let currentBegMs = -1;
    let currentEndMs = -1;
    let currentXalignLine = -1;
    let currentXalignReason = '';

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // Main tier line: *SPK: text •beg_end•
        if (line.startsWith('*')) {
            currentUtteranceLine = i;
            const speakerMatch = SPEAKER_REGEX.exec(line);
            currentSpeaker = speakerMatch ? speakerMatch[1] : '';
            currentUtteranceText = line.slice(line.indexOf(':') + 1).trim();
            if (currentUtteranceText.length > 80) {
                currentUtteranceText = currentUtteranceText.slice(0, 80) + '...';
            }
            const bulletMatch = BULLET_REGEX.exec(line);
            currentBegMs = bulletMatch ? parseInt(bulletMatch[1], 10) : -1;
            currentEndMs = bulletMatch ? parseInt(bulletMatch[2], 10) : -1;
            currentXalignLine = -1;
            currentXalignReason = '';
        }

        // %xalign tier
        if (line.startsWith('%xalign:')) {
            currentXalignLine = i;
            currentXalignReason = line.slice('%xalign:'.length).trim();
        }

        // %xrev tier — this is a review item
        if (line.startsWith('%xrev:')) {
            const content = line.slice('%xrev:'.length).trim();
            const statusMatch = XREV_STATUS_REGEX.exec(content);
            const rawStatus = statusMatch ? statusMatch[1] : '?';
            const notes = statusMatch ? statusMatch[2].trim() : content;

            const status = isValidStatus(rawStatus) ? rawStatus : '?';

            items.push({
                utteranceLine: currentUtteranceLine,
                xrevLine: i,
                xalignLine: currentXalignLine,
                status,
                xalignReason: currentXalignReason,
                speaker: currentSpeaker,
                utteranceText: currentUtteranceText,
                begMs: currentBegMs,
                endMs: currentEndMs,
                notes,
            });
        }
    }

    return items;
}

/**
 * Count review progress: how many items have been reviewed (status !== '?').
 */
export function countReviewed(items: readonly ReviewItem[]): number {
    return items.filter(item => item.status !== '?').length;
}

/**
 * Find the index of the next unreviewed item (status === '?'),
 * wrapping around from the end. Returns -1 if all are reviewed.
 */
export function nextUnreviewed(items: readonly ReviewItem[], fromIndex: number): number {
    const n = items.length;
    for (let offset = 1; offset <= n; offset++) {
        const idx = (fromIndex + offset) % n;
        if (items[idx].status === '?') {
            return idx;
        }
    }
    return -1;
}

/**
 * Find the index of the previous unreviewed item, wrapping.
 */
export function prevUnreviewed(items: readonly ReviewItem[], fromIndex: number): number {
    const n = items.length;
    for (let offset = 1; offset <= n; offset++) {
        const idx = (fromIndex - offset + n) % n;
        if (items[idx].status === '?') {
            return idx;
        }
    }
    return -1;
}

/**
 * Format the `%xrev:` tier content from a status and optional notes.
 *
 * Returns the text AFTER the `%xrev:\t` prefix, e.g., `[ok] some note`.
 */
export function formatXrevContent(status: ReviewStatus, notes: string): string {
    const marker = `[${status}]`;
    return notes ? `${marker} ${notes}` : marker;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isValidStatus(s: string): s is ReviewStatus {
    return (REVIEW_STATUSES as readonly string[]).includes(s);
}
