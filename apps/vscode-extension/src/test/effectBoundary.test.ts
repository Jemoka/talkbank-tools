/**
 * Tests for shared Effect Schema boundary helpers.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Schema } from 'effect';
import { describe, expect, it } from 'vitest';

import {
    PanelMessageDecodeError,
    StructuredPayloadDecodeError,
    decodePanelMessageWithSchema,
    decodeStructuredWithSchema,
    parseWithSchema,
} from '../effectBoundary';

const ExampleSchema = Schema.Struct({
    name: Schema.String,
});

describe('effect boundary helpers', () => {
    it('parses valid payloads and returns undefined on schema mismatch', () => {
        expect(parseWithSchema(ExampleSchema, { name: 'TalkBank' })).toEqual({
            name: 'TalkBank',
        });
        expect(parseWithSchema(ExampleSchema, { name: 42 })).toBeUndefined();
    });

    it('wraps malformed structured payloads in a tagged decode error', () => {
        expect(() => decodeStructuredWithSchema(
            'talkbank/example',
            { name: 42 },
            ExampleSchema,
        )).toThrow(StructuredPayloadDecodeError);
    });

    it('wraps malformed webview payloads in a tagged panel error', () => {
        expect(() => decodePanelMessageWithSchema(
            'example panel',
            { name: 42 },
            ExampleSchema,
        )).toThrow(PanelMessageDecodeError);
    });
});
