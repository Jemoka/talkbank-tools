import { Segment, TimestampMs } from './bulletParser';
import { AlignmentSidecarDocument } from '../lsp/executeCommandPayloads';

/**
 * Convert an LSP alignment sidecar payload to media playback segments.
 *
 * Returns `null` when no sidecar payload is available so callers can safely
 * fall back to legacy bullet parsing.
 */
export function segmentsFromAlignmentSidecar(payload: AlignmentSidecarDocument | null): Segment[] | null {
    if (payload === null) {
        return null;
    }

    const segments: Segment[] = [];
    for (const utterance of payload.utterances) {
        const line = utterance.range?.start.line;
        const timing = utterance.timing;
        if (line === undefined || timing === null || timing.skip) {
            continue;
        }
        if (timing.end_ms < timing.start_ms) {
            continue;
        }

        segments.push({
            line,
            beg: timing.start_ms as TimestampMs,
            end: timing.end_ms as TimestampMs,
        });
    }

    return segments;
}
