/**
 * bulletParser.ts
 *
 * Pure TypeScript utilities for scanning CHAT document text for media headers
 * and bullet time markers (•beg_end•, U+2022) without requiring an LSP roundtrip.
 *
 * CLAN bullet format: \u2022<beg_ms>_<end_ms>\u2022
 * Example:  *CHI:  hello there . •12500_14300•
 */

/** Millisecond timestamp branded type — prevents mixing with other numbers. */
export type TimestampMs = number & { readonly __brand: 'TimestampMs' };

/** A timed segment associated with an utterance line in the CHAT document. */
export interface Segment {
    /** 0-indexed line number of the utterance's leading '*' line. */
    line: number;
    /** Segment begin time in milliseconds. */
    beg: TimestampMs;
    /** Segment end time in milliseconds. */
    end: TimestampMs;
}

/**
 * Extracts the media filename from a CHAT document's @Media: header.
 *
 * Handles both tab-separated (canonical) and space-separated variants:
 *   @Media:\t<name>, audio|video
 *   @Media: <name>, audio|video
 *
 * @param text - Full document text.
 * @returns The media name (no extension change applied), or null if absent.
 */
export function parseMediaHeader(text: string): string | null {
    // Tab-separated is the canonical CHAT format
    const tabMatch = text.match(/^@Media:\t([^,\n]+)/m);
    if (tabMatch) {
        return tabMatch[1].trim();
    }

    // Space-separated variant for robustness
    const spaceMatch = text.match(/^@Media: +([^,\n]+)/m);
    if (spaceMatch) {
        return spaceMatch[1].trim();
    }

    return null;
}

/**
 * Scans every line in the document for CLAN bullet markers (•beg_end•) and
 * associates each one with the nearest preceding utterance line (a line that
 * starts with '*').
 *
 * Notes:
 * - Old-style %snd: tier bullets are skipped (out of scope).
 * - A single utterance may span continuation lines; the bullet appears on
 *   whichever physical line ends the utterance, but is attributed to the
 *   opening '*' line.
 * - Segments are returned in document order.
 *
 * @param text - Full document text.
 * @returns Array of Segment objects in document order.
 */
export function parseBullets(text: string): Segment[] {
    const lines = text.split('\n');
    const segments: Segment[] = [];

    // 0-indexed line of the last '*' utterance opener seen so far.
    let lastUtteranceLine = -1;

    // Regex for •beg_end• (U+2022 ... U+2022); global to find multiple per line.
    const bulletRegex = /\u2022(\d+)_(\d+)\u2022/g;

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // Track the most recent utterance opener.
        if (line.startsWith('*')) {
            lastUtteranceLine = i;
        }

        // Skip old-style %snd: tier — those bullets are a different format.
        if (line.startsWith('%snd:')) {
            continue;
        }

        // Reset lastIndex so the regex rescans from the start of each line.
        bulletRegex.lastIndex = 0;
        let match: RegExpExecArray | null;
        while ((match = bulletRegex.exec(line)) !== null) {
            if (lastUtteranceLine >= 0) {
                segments.push({
                    line: lastUtteranceLine,
                    beg: parseInt(match[1], 10) as TimestampMs,
                    end: parseInt(match[2], 10) as TimestampMs,
                });
            }
        }
    }

    return segments;
}

/**
 * Returns the segment whose utterance line is closest to (and not after)
 * cursorLine — i.e. the last segment whose line index ≤ cursorLine.
 *
 * Assumes segments are in document order (as produced by parseBullets).
 *
 * @param segments - Array of segments from parseBullets().
 * @param cursorLine - The current cursor line (0-indexed).
 * @returns The matching segment, or undefined if none precedes the cursor.
 */
export function findNearestBullet(segments: Segment[], cursorLine: number): Segment | undefined {
    let result: Segment | undefined;
    for (const seg of segments) {
        if (seg.line <= cursorLine) {
            result = seg;
        } else {
            // Segments are ordered; no later segment can qualify.
            break;
        }
    }
    return result;
}
