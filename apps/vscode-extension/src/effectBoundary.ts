/**
 * Shared Effect Schema decoding helpers for extension boundary modules.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Data, Schema } from 'effect';

/**
 * Tagged error for one malformed structured LSP response payload.
 */
export class StructuredPayloadDecodeError extends Data.TaggedError('StructuredPayloadDecodeError')<{
    readonly command: string;
    readonly details: string;
}> {}

/**
 * Tagged error for one malformed inbound webview message.
 */
export class PanelMessageDecodeError extends Data.TaggedError('PanelMessageDecodeError')<{
    readonly label: string;
    readonly details: string;
}> {}

function decodeUnknown<decodedPayload>(
    schema: Schema.Schema<decodedPayload>,
    payload: unknown,
): decodedPayload | undefined {
    const decoded = Schema.decodeUnknownEither(schema)(payload);
    return decoded._tag === 'Right' ? decoded.right : undefined;
}

/**
 * Parse one payload with an Effect Schema, returning `undefined` on failure.
 */
export function parseWithSchema<decodedPayload>(
    schema: Schema.Schema<decodedPayload>,
    payload: unknown,
): decodedPayload | undefined {
    return decodeUnknown(schema, payload);
}

/**
 * Decode one structured LSP payload with an Effect Schema.
 */
export function decodeStructuredWithSchema<decodedPayload>(
    command: string,
    payload: unknown,
    schema: Schema.Schema<decodedPayload>,
): decodedPayload {
    if (typeof payload === 'string') {
        throw new Error(payload);
    }

    const decoded = Schema.decodeUnknownEither(schema)(payload);
    if (decoded._tag === 'Left') {
        throw new StructuredPayloadDecodeError({
            command,
            details: decoded.left.message,
        });
    }

    return decoded.right;
}

/**
 * Decode one inbound webview message with an Effect Schema.
 */
export function decodePanelMessageWithSchema<decodedPayload>(
    label: string,
    payload: unknown,
    schema: Schema.Schema<decodedPayload>,
): decodedPayload {
    const decoded = Schema.decodeUnknownEither(schema)(payload);
    if (decoded._tag === 'Left') {
        throw new PanelMessageDecodeError({
            label,
            details: decoded.left.message,
        });
    }

    return decoded.right;
}
