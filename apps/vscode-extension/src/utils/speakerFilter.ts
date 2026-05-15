/**
 * speakerFilter.ts
 *
 * Pure utilities for extracting speaker codes from a CHAT document's
 * @Participants: header and for filtering utterance blocks by speaker code.
 *
 * Used by the `talkbank.filterBySpeaker` command (Phase 7 — speaker filtering).
 */

/** Three-letter CHAT speaker code branded type — prevents mixing with other strings. */
export type SpeakerCode = string & { readonly __brand: 'SpeakerCode' };

/**
 * Extracts all speaker codes declared in a CHAT document's @Participants: header.
 *
 * @Participants format (canonical):
 *   @Participants:\tCHI Ruth Target_Child, MOT Mary Mother
 *
 * Each entry is `CODE Name Role`, separated by commas.
 *
 * @param text - Full document text.
 * @returns Array of three-letter speaker codes in declaration order.
 */
export function parseSpeakers(text: string): SpeakerCode[] {
    // Match tab-separated (canonical) and space-separated variants.
    const match = text.match(/^@Participants:[\t ]+(.+)/m);
    if (!match) {
        return [];
    }

    const entries = match[1].split(',');
    const codes: SpeakerCode[] = [];

    for (const entry of entries) {
        const trimmed = entry.trim();
        if (!trimmed) {
            continue;
        }
        // Each entry starts with the speaker code (first whitespace-delimited token).
        const code = trimmed.split(/\s+/)[0];
        if (code) {
            codes.push(code as SpeakerCode);
        }
    }

    return codes;
}

/**
 * Returns a filtered copy of the document containing only the headers and
 * utterance blocks whose leading `*SPEAKER:` code is in `selectedCodes`.
 *
 * Filtering rules:
 * - All file-level headers (@UTF8, @Begin, @Participants, @ID, @End, …)
 *   are always included.
 * - An utterance block consists of the main tier (`*SPEAKER: …`) and all
 *   immediately following dependent tiers (`%xxx: …`).
 * - Inter-utterance comment headers (@Comment, @Bg, @Eg, …) that appear
 *   between utterances are included if the *next* utterance belongs to a
 *   selected speaker, or if there is no next utterance (end of file).
 *
 * @param text         - Full document text.
 * @param selectedCodes - Set of speaker codes to retain.
 * @returns Filtered document text suitable for opening as a virtual document.
 */
export function filterBySpeakers(text: string, selectedCodes: Set<SpeakerCode>): string {
    const lines = text.split('\n');
    const outputLines: string[] = [];

    // Utterance block accumulator: collects lines for the current utterance
    // (main tier + dependent tiers) before we know whether to keep it.
    let utteranceLines: string[] = [];
    // Inter-utterance headers seen since the last utterance ended.
    let pendingHeaders: string[] = [];
    // Whether the current utterance belongs to a selected speaker.
    let currentSpeakerSelected = false;

    /**
     * Flush a completed utterance block. If the speaker is selected, append
     * any pending inter-utterance headers then the utterance lines to output.
     * Otherwise discard both.
     */
    function flushUtterance(): void {
        if (utteranceLines.length === 0) {
            return;
        }
        if (currentSpeakerSelected) {
            outputLines.push(...pendingHeaders);
            outputLines.push(...utteranceLines);
            pendingHeaders = [];
        } else {
            // Discard pending headers and the unselected utterance.
            pendingHeaders = [];
        }
        utteranceLines = [];
        currentSpeakerSelected = false;
    }

    for (const line of lines) {
        if (line.startsWith('*')) {
            // New utterance main tier — flush previous utterance first.
            flushUtterance();

            // Determine if this speaker is selected.
            const speakerMatch = line.match(/^\*([^:]+):/);
            const speakerCode = speakerMatch ? speakerMatch[1] : '';
            currentSpeakerSelected = selectedCodes.has(speakerCode as SpeakerCode);
            utteranceLines = [line];
        } else if (line.startsWith('%')) {
            // Dependent tier — belongs to the current utterance.
            utteranceLines.push(line);
        } else if (line.startsWith('@')) {
            // Header line: flush any open utterance, then decide placement.
            flushUtterance();

            // @Begin and @End plus file-level headers always go to output.
            // @Comment, @Bg, @Eg, @G, etc. are held as pending until we see
            // the next utterance (so we can decide based on that speaker).
            const isFileHeader =
                line.startsWith('@UTF8') ||
                line.startsWith('@Begin') ||
                line.startsWith('@End') ||
                line.startsWith('@Languages') ||
                line.startsWith('@Participants') ||
                line.startsWith('@ID') ||
                line.startsWith('@Birth') ||
                line.startsWith('@Media') ||
                line.startsWith('@Options') ||
                line.startsWith('@Transcriber') ||
                line.startsWith('@Location') ||
                line.startsWith('@Recording') ||
                line.startsWith('@Room') ||
                line.startsWith('@Situation') ||
                line.startsWith('@Date') ||
                line.startsWith('@Time ') ||
                line.startsWith('@Warning') ||
                line.startsWith('@Font') ||
                line.startsWith('@New Episode') ||
                line.startsWith('@Color');

            if (isFileHeader) {
                // File-level headers flush pending headers (kept or dropped already)
                // and are unconditionally included.
                pendingHeaders = [];
                outputLines.push(line);
            } else {
                // Inter-utterance header: hold until next utterance decision.
                pendingHeaders.push(line);
            }
        } else if (line === '') {
            // Blank line: include only if currently inside an utterance block.
            if (utteranceLines.length > 0) {
                utteranceLines.push(line);
            } else {
                // Outside utterances: forward to output as-is.
                outputLines.push(line);
            }
        } else {
            // Continuation lines or unknown content: append to current utterance
            // if one is open, else add to output directly.
            if (utteranceLines.length > 0) {
                utteranceLines.push(line);
            } else {
                outputLines.push(line);
            }
        }
    }

    // Flush any open utterance at end of file.
    flushUtterance();
    // Any remaining pending headers have no following utterance — discard them
    // (they would be orphaned inter-utterance comments).

    return outputLines.join('\n');
}
