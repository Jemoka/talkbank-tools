/**
 * Tests for shared webview/panel message contracts.
 */

/* eslint-disable @typescript-eslint/naming-convention */
import { describe, expect, it } from 'vitest';

import { PanelMessageDecodeError } from '../effectBoundary';
import type { TimestampMs } from '../utils/bulletParser';
import {
    createIdEditorEntriesMessage,
    createIdEditorErrorMessage,
    createIdEditorSavedMessage,
    createKidevalDatabasesMessage,
    createKidevalErrorMessage,
    createKidevalFileInfoMessage,
    createKidevalResultsMessage,
    createMediaRequestTimestampMessage,
    createMediaRewindMessage,
    createMediaSeekToMessage,
    createMediaSetLoopMessage,
    createWaveformHighlightSegmentMessage,
    decodeAnalysisPanelWebviewMessage,
    decodeIdEditorPanelWebviewMessage,
    decodeKidevalPanelWebviewMessage,
    decodeMediaPanelWebviewMessage,
    decodeWaveformPanelWebviewMessage,
} from '../webviewMessageContracts';

describe('webview message contracts', () => {
    it('decodes analysis CSV export requests', () => {
        expect(decodeAnalysisPanelWebviewMessage({
            command: 'exportCsv',
            csv: 'word,count\nwant,4\n',
        })).toEqual({
            command: 'exportCsv',
            csv: 'word,count\nwant,4\n',
        });

        expect(() => decodeAnalysisPanelWebviewMessage({
            command: 'exportCsv',
            csv: 4,
        })).toThrow(PanelMessageDecodeError);
    });

    it('decodes ID editor save requests and builds extension messages', () => {
        const entries = [
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
        ];

        expect(decodeIdEditorPanelWebviewMessage({
            command: 'save',
            entries,
        })).toEqual({
            command: 'save',
            entries,
        });

        expect(createIdEditorEntriesMessage(entries, 'sample.cha')).toEqual({
            command: 'entries',
            entries,
            fileName: 'sample.cha',
        });

        expect(createIdEditorSavedMessage()).toEqual({ command: 'saved' });
        expect(createIdEditorErrorMessage('boom')).toEqual({
            command: 'error',
            message: 'boom',
        });
    });

    it('decodes media panel messages and builds media control messages', () => {
        const timestamp = 1250 as TimestampMs;

        expect(decodeMediaPanelWebviewMessage({
            command: 'segmentChanged',
            index: 2,
        })).toEqual({
            command: 'segmentChanged',
            index: 2,
        });

        expect(decodeMediaPanelWebviewMessage({
            command: 'timestamp',
            ms: timestamp,
        })).toEqual({
            command: 'timestamp',
            ms: timestamp,
        });

        expect(decodeMediaPanelWebviewMessage({
            command: 'stopped',
        })).toEqual({
            command: 'stopped',
        });

        expect(() => decodeMediaPanelWebviewMessage({
            command: 'segmentChanged',
            index: -1,
        })).toThrow(PanelMessageDecodeError);

        expect(createMediaRewindMessage(2)).toEqual({
            command: 'rewind',
            seconds: 2,
        });
        expect(createMediaSetLoopMessage()).toEqual({ command: 'setLoop' });
        expect(createMediaRequestTimestampMessage()).toEqual({
            command: 'requestTimestamp',
        });
        expect(createMediaSeekToMessage(timestamp)).toEqual({
            command: 'seekTo',
            ms: timestamp,
        });
    });

    it('decodes waveform seek requests and builds highlight messages', () => {
        const timestamp = 900 as TimestampMs;

        expect(decodeWaveformPanelWebviewMessage({
            command: 'seek',
            ms: timestamp,
        })).toEqual({
            command: 'seek',
            ms: timestamp,
        });

        expect(() => decodeWaveformPanelWebviewMessage({
            command: 'seek',
            ms: -5,
        })).toThrow(PanelMessageDecodeError);

        expect(createWaveformHighlightSegmentMessage(4)).toEqual({
            command: 'highlightSegment',
            index: 4,
        });
    });

    it('decodes kideval messages and builds extension notifications', () => {
        expect(decodeKidevalPanelWebviewMessage({
            command: 'discoverDatabases',
            libDir: '/Users/Shared/CLAN/lib/kideval',
        })).toEqual({
            command: 'discoverDatabases',
            libDir: '/Users/Shared/CLAN/lib/kideval',
        });

        expect(decodeKidevalPanelWebviewMessage({
            command: 'runAnalysis',
            databasePath: '/Users/Shared/CLAN/lib/kideval/eng.cut',
            databaseFilter: {
                language: 'eng',
                group: 'TD',
                gender: 'Female',
                age_from_months: 24,
                age_to_months: 48,
                speaker_codes: ['CHI'],
            },
        })).toEqual({
            command: 'runAnalysis',
            databasePath: '/Users/Shared/CLAN/lib/kideval/eng.cut',
            databaseFilter: {
                language: 'eng',
                group: 'TD',
                gender: 'Female',
                age_from_months: 24,
                age_to_months: 48,
                speaker_codes: ['CHI'],
            },
        });

        expect(decodeKidevalPanelWebviewMessage({
            command: 'exportCsv',
            csv: 'measure,z\nmlu,-1.2\n',
        })).toEqual({
            command: 'exportCsv',
            csv: 'measure,z\nmlu,-1.2\n',
        });

        expect(() => decodeKidevalPanelWebviewMessage({
            command: 'runAnalysis',
            databaseFilter: {
                gender: 'Unknown',
            },
        })).toThrow(PanelMessageDecodeError);

        expect(createKidevalFileInfoMessage('sample.cha')).toEqual({
            command: 'fileInfo',
            fileName: 'sample.cha',
        });
        expect(createKidevalDatabasesMessage([
            {
                path: '/Users/Shared/CLAN/lib/kideval/eng.cut',
                language: 'eng',
                corpus_type: 'toyplay',
                display_name: 'ENG (toyplay)',
            },
        ])).toEqual({
            command: 'databases',
            databases: [
                {
                    path: '/Users/Shared/CLAN/lib/kideval/eng.cut',
                    language: 'eng',
                    corpus_type: 'toyplay',
                    display_name: 'ENG (toyplay)',
                },
            ],
        });
        expect(createKidevalResultsMessage({ ok: true })).toEqual({
            command: 'results',
            data: { ok: true },
        });
        expect(createKidevalErrorMessage('bad')).toEqual({
            command: 'error',
            message: 'bad',
        });
    });
});
