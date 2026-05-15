/**
 * Tests for the typed execute-command payload boundary and its shared fixtures.
 */

/* eslint-disable @typescript-eslint/naming-convention */
import { describe, expect, it } from 'vitest';

import { StructuredPayloadDecodeError } from '../effectBoundary';
import analyzeCommandPayloadFixture from './fixtures/analyzeCommandPayload.json';
import {
    buildAnalyzeCommandRequest,
    decodeAvailableDatabaseList,
    decodeFormattedBulletLine,
    decodeParticipantEntryList,
    decodeScopedFindMatchList,
    decodeSpeakerInfoList,
    decodeUtteranceInfoList,
    parseAlignmentSidecarDocument,
} from '../lsp/executeCommandPayloads';

describe('execute-command payload decoders', () => {
    it('decodes structured speaker and participant payloads', () => {
        expect(decodeSpeakerInfoList([
            { code: 'CHI', name: 'Child', role: 'Target_Child' },
            { code: 'MOT', name: 'Mother', role: 'Mother' },
        ])).toEqual([
            { code: 'CHI', name: 'Child', role: 'Target_Child' },
            { code: 'MOT', name: 'Mother', role: 'Mother' },
        ]);

        expect(decodeParticipantEntryList([
            {
                line: 3,
                fields: {
                    language: 'eng',
                    corpus: 'Example',
                    speaker: 'CHI',
                    age: '2;03',
                    sex: 'male',
                    group: '',
                    ses: '',
                    role: 'Target_Child',
                    education: '',
                    custom: '',
                },
            },
        ])).toEqual([
            {
                line: 3,
                fields: {
                    language: 'eng',
                    corpus: 'Example',
                    speaker: 'CHI',
                    age: '2;03',
                    sex: 'male',
                    group: '',
                    ses: '',
                    role: 'Target_Child',
                    education: '',
                    custom: '',
                },
            },
        ]);
    });

    it('decodes structured chat-op payloads', () => {
        expect(decodeFormattedBulletLine({
            bullet: ' \u0015 100_200 \u0015',
            new_line: '*CHI:\t',
        })).toEqual({
            bullet: ' \u0015 100_200 \u0015',
            new_line: '*CHI:\t',
        });

        expect(decodeUtteranceInfoList([
            { line: 10, speaker: 'CHI', has_cod: false },
            { line: 12, speaker: 'MOT', has_cod: true },
        ])).toEqual([
            { line: 10, speaker: 'CHI', has_cod: false },
            { line: 12, speaker: 'MOT', has_cod: true },
        ]);

        expect(decodeScopedFindMatchList([
            {
                line: 4,
                character: 8,
                length: 6,
                tier: 'mor',
                speaker: 'CHI',
                line_text: '%mor:\tpro|I v|want n|cookie',
            },
        ])).toEqual([
            {
                line: 4,
                character: 8,
                length: 6,
                tier: 'mor',
                speaker: 'CHI',
                line_text: '%mor:\tpro|I v|want n|cookie',
            },
        ]);

        expect(decodeAvailableDatabaseList([
            {
                path: '/Users/Shared/CLAN/lib/kideval/eng_toyplay_db.cut',
                language: 'eng',
                corpus_type: 'toyplay',
                display_name: 'ENG (toyplay)',
            },
            {
                path: '/Users/Shared/CLAN/lib/kideval/jpn_db.cut',
                language: 'jpn',
                corpus_type: null,
                display_name: 'JPN',
            },
        ])).toEqual([
            {
                path: '/Users/Shared/CLAN/lib/kideval/eng_toyplay_db.cut',
                language: 'eng',
                corpus_type: 'toyplay',
                display_name: 'ENG (toyplay)',
            },
            {
                path: '/Users/Shared/CLAN/lib/kideval/jpn_db.cut',
                language: 'jpn',
                corpus_type: null,
                display_name: 'JPN',
            },
        ]);
    });

    it('builds the canonical analyze request fixture', () => {
        expect(buildAnalyzeCommandRequest('eval-d', 'file:///tmp/test.cha', {
            maxUtterances: 100,
            databasePath: '/Users/Shared/CLAN/lib/kideval/eng_toyplay_db.cut',
            databaseFilter: {
                language: 'eng',
                group: 'TD',
                gender: 'Female',
                age_from_months: 24,
                age_to_months: 48,
                speaker_codes: ['CHI', 'INV'],
            },
            dssMaxUtterances: 75,
            ipsynMaxUtterances: 80,
            sortByFrequency: true,
        })).toEqual(analyzeCommandPayloadFixture);
    });

    it('throws when a structured payload arrives as a string error', () => {
        expect(() => decodeSpeakerInfoList('Speaker error: Document not found')).toThrow(
            'Speaker error: Document not found',
        );
        expect(() => decodeParticipantEntryList('Participant error: Failed to parse document')).toThrow(
            'Participant error: Failed to parse document',
        );
    });

    it('wraps malformed structured objects in a tagged decode error', () => {
        expect(() => decodeAvailableDatabaseList([
            {
                path: '/Users/Shared/CLAN/lib/kideval/eng.cut',
                language: 'eng',
                corpus_type: 12,
                display_name: 'ENG',
            },
        ])).toThrow(StructuredPayloadDecodeError);
    });

    it('parses the playback-facing sidecar envelope and drops malformed utterances', () => {
        expect(parseAlignmentSidecarDocument('Parse error')).toBeNull();
        expect(parseAlignmentSidecarDocument({ utterances: [] })).toBeNull();

        expect(parseAlignmentSidecarDocument({
            schema_version: 1,
            uri: 'file:///tmp/sample.cha',
            utterances: [
                {
                    utterance_index: 0,
                    speaker: 'CHI',
                    range: { start: { line: 5, character: 0 }, end: { line: 5, character: 8 } },
                    timing: { start_ms: 100, end_ms: 240, skip: false },
                },
                {
                    utterance_index: 1,
                    speaker: 'MOT',
                    range: { start: { line: 6, character: 0 } },
                    timing: { start_ms: 250, end_ms: 300, skip: false },
                },
            ],
        })).toEqual({
            schema_version: 1,
            uri: 'file:///tmp/sample.cha',
            utterances: [
                {
                    utterance_index: 0,
                    speaker: 'CHI',
                    range: { start: { line: 5, character: 0 }, end: { line: 5, character: 8 } },
                    timing: { start_ms: 100, end_ms: 240, skip: false },
                },
            ],
        });
    });
});
