/* eslint-disable @typescript-eslint/naming-convention */
/**
 * Shared webview message contracts and decoders for VS Code panels.
 *
 * The extension side receives `unknown` payloads from `webview.postMessage()`.
 * This module centralizes the remaining panel-specific message shapes so panel
 * adapters do not pattern-match on ad hoc `{ command: string }` bags.
 */

import { Schema } from 'effect';

import { decodePanelMessageWithSchema } from './effectBoundary';
import {
    AnalysisDatabaseFilter,
    AnalysisDatabaseFilterSchema,
    AvailableDatabase,
    ParticipantEntry,
    ParticipantEntryListSchema,
    toMutableAnalysisDatabaseFilter,
} from './lsp/executeCommandPayloads';
import type { TimestampMs } from './utils/bulletParser';

/**
 * Brand a validated non-negative number as a [`TimestampMs`].
 */
function toTimestampMs(value: number): TimestampMs {
    return value as TimestampMs;
}

/**
 * Base shape for panel-originated CSV export requests.
 */
export interface ExportCsvMessage {
    /**
     * Discriminant identifying CSV export.
     */
    command: 'exportCsv';
    /**
     * CSV payload to save to disk.
     */
    csv: string;
}

/**
 * Base shape for extension-originated error notifications.
 */
export interface PanelErrorMessage {
    /**
     * Discriminant identifying a panel error notification.
     */
    command: 'error';
    /**
     * Human-readable error text for the webview.
     */
    message: string;
}

const ExportCsvMessageSchema = Schema.Struct({
    command: Schema.Literal('exportCsv'),
    csv: Schema.String,
});

/**
 * Message from the analysis-results panel back to the extension.
 */
export type AnalysisPanelWebviewMessage = ExportCsvMessage;

/**
 * Decode one message sent from `analysisPanel.js` to `analysisPanel.ts`.
 */
export function decodeAnalysisPanelWebviewMessage(
    value: unknown,
): AnalysisPanelWebviewMessage {
    return decodePanelMessageWithSchema(
        'analysis panel',
        value,
        ExportCsvMessageSchema,
    );
}

/**
 * Save request emitted by the participant editor webview.
 */
export interface IdEditorPanelSaveMessage {
    /**
     * Discriminant identifying a save request.
     */
    command: 'save';
    /**
     * Full participant-entry list to write back into the document.
     */
    entries: ParticipantEntry[];
}

/**
 * Message sent from the participant editor webview to the extension.
 */
export type IdEditorPanelWebviewMessage = IdEditorPanelSaveMessage;

/**
 * Participant entries payload sent from the extension to the webview.
 */
export interface IdEditorPanelEntriesMessage {
    /**
     * Discriminant identifying fresh participant entries.
     */
    command: 'entries';
    /**
     * Participant rows returned by the LSP.
     */
    entries: ParticipantEntry[];
    /**
     * Display name of the active document.
     */
    fileName: string;
}

/**
 * Success notification after the extension writes updated participant entries.
 */
export interface IdEditorPanelSavedMessage {
    /**
     * Discriminant identifying a successful save.
     */
    command: 'saved';
}

/**
 * Message sent from the extension to the participant editor webview.
 */
export type IdEditorPanelExtensionMessage =
    | IdEditorPanelEntriesMessage
    | IdEditorPanelSavedMessage
    | PanelErrorMessage;

const IdEditorPanelSaveMessageSchema = Schema.Struct({
    command: Schema.Literal('save'),
    entries: ParticipantEntryListSchema,
});

/**
 * Decode one message sent from `idEditorPanel.js` to `idEditorPanel.ts`.
 */
export function decodeIdEditorPanelWebviewMessage(
    value: unknown,
): IdEditorPanelWebviewMessage {
    const decoded = decodePanelMessageWithSchema(
        'ID editor panel',
        value,
        IdEditorPanelSaveMessageSchema,
    );
    return {
        command: 'save',
        entries: [...decoded.entries],
    };
}

/**
 * Build the participant-entry message for the ID editor webview.
 */
export function createIdEditorEntriesMessage(
    entries: ParticipantEntry[],
    fileName: string,
): IdEditorPanelEntriesMessage {
    return {
        command: 'entries',
        entries,
        fileName,
    };
}

/**
 * Build the saved notification for the ID editor webview.
 */
export function createIdEditorSavedMessage(): IdEditorPanelSavedMessage {
    return {
        command: 'saved',
    };
}

/**
 * Build an error notification for the ID editor webview.
 */
export function createIdEditorErrorMessage(message: string): PanelErrorMessage {
    return {
        command: 'error',
        message,
    };
}

// Media panel contract was split into its own module (KIB-006). This
// file retains re-exports so existing consumers don't need to change
// their import paths; new code should import from
// `./webviewContracts/mediaPanelContract` directly.
export type {
    MediaPanelSegmentChangedMessage,
    MediaPanelTimestampMessage,
    MediaPanelStoppedMessage,
    MediaPanelWebviewMessage,
    MediaPanelRewindMessage,
    MediaPanelSetLoopMessage,
    MediaPanelRequestTimestampMessage,
    MediaPanelSeekToMessage,
    MediaPanelExtensionMessage,
} from './webviewContracts/mediaPanelContract';
export {
    decodeMediaPanelWebviewMessage,
    createMediaRewindMessage,
    createMediaSetLoopMessage,
    createMediaRequestTimestampMessage,
    createMediaSeekToMessage,
} from './webviewContracts/mediaPanelContract';

/**
 * Seek request emitted by the waveform webview.
 */
export interface WaveformPanelSeekMessage {
    /**
     * Discriminant identifying a waveform seek request.
     */
    command: 'seek';
    /**
     * Requested playback position in milliseconds.
     */
    ms: TimestampMs;
}

/**
 * Message sent from the waveform webview to the extension.
 */
export type WaveformPanelWebviewMessage = WaveformPanelSeekMessage;

/**
 * Segment-highlight request sent from the extension to the waveform webview.
 */
export interface WaveformPanelHighlightSegmentMessage {
    /**
     * Discriminant identifying waveform highlighting.
     */
    command: 'highlightSegment';
    /**
     * Zero-based segment index to highlight.
     */
    index: number;
}

/**
 * Message sent from the extension to the waveform webview.
 */
export type WaveformPanelExtensionMessage = WaveformPanelHighlightSegmentMessage;

const WaveformPanelSeekMessageSchema = Schema.Struct({
    command: Schema.Literal('seek'),
    ms: Schema.NonNegativeInt,
});

/**
 * Decode one message sent from `waveformPanel.js` to `waveformPanel.ts`.
 */
export function decodeWaveformPanelWebviewMessage(
    value: unknown,
): WaveformPanelWebviewMessage {
    const decoded = decodePanelMessageWithSchema(
        'waveform panel',
        value,
        WaveformPanelSeekMessageSchema,
    );
    return {
        command: 'seek',
        ms: toTimestampMs(decoded.ms),
    };
}

/**
 * Build a segment-highlight request for the waveform webview.
 */
export function createWaveformHighlightSegmentMessage(
    index: number,
): WaveformPanelHighlightSegmentMessage {
    return {
        command: 'highlightSegment',
        index,
    };
}

/**
 * Database-discovery request emitted by the KidEval/Eval webview.
 */
export interface KidevalPanelDiscoverDatabasesMessage {
    /**
     * Discriminant identifying database discovery.
     */
    command: 'discoverDatabases';
    /**
     * CLAN library directory to search.
     */
    libDir: string;
}

/**
 * Analysis-run request emitted by the KidEval/Eval webview.
 */
export interface KidevalPanelRunAnalysisMessage {
    /**
     * Discriminant identifying one analysis run.
     */
    command: 'runAnalysis';
    /**
     * Optional selected database path.
     */
    databasePath?: string;
    /**
     * Optional demographic filter payload.
     */
    databaseFilter?: AnalysisDatabaseFilter;
}

/**
 * Message sent from the KidEval/Eval webview to the extension.
 */
export type KidevalPanelWebviewMessage =
    | KidevalPanelDiscoverDatabasesMessage
    | KidevalPanelRunAnalysisMessage
    | ExportCsvMessage;

/**
 * File-information payload sent from the extension to the KidEval/Eval webview.
 */
export interface KidevalPanelFileInfoMessage {
    /**
     * Discriminant identifying active-file metadata.
     */
    command: 'fileInfo';
    /**
     * Display name of the active file.
     */
    fileName: string;
}

/**
 * Database-list payload sent from the extension to the KidEval/Eval webview.
 */
export interface KidevalPanelDatabasesMessage {
    /**
     * Discriminant identifying database results.
     */
    command: 'databases';
    /**
     * Decoded list of available normative databases.
     */
    databases: AvailableDatabase[];
}

/**
 * Analysis-result payload sent from the extension to the KidEval/Eval webview.
 */
export interface KidevalPanelResultsMessage {
    /**
     * Discriminant identifying analysis results.
     */
    command: 'results';
    /**
     * JSON-serializable result payload.
     */
    data: unknown;
}

/**
 * Message sent from the extension to the KidEval/Eval webview.
 */
export type KidevalPanelExtensionMessage =
    | KidevalPanelFileInfoMessage
    | KidevalPanelDatabasesMessage
    | KidevalPanelResultsMessage
    | PanelErrorMessage;

const KidevalPanelDiscoverDatabasesMessageSchema = Schema.Struct({
    command: Schema.Literal('discoverDatabases'),
    libDir: Schema.String,
});

const KidevalPanelRunAnalysisMessageSchema = Schema.Struct({
    command: Schema.Literal('runAnalysis'),
    databasePath: Schema.optional(Schema.String),
    databaseFilter: Schema.optional(AnalysisDatabaseFilterSchema),
});

const KidevalPanelWebviewMessageSchema = Schema.Union(
    KidevalPanelDiscoverDatabasesMessageSchema,
    KidevalPanelRunAnalysisMessageSchema,
    ExportCsvMessageSchema,
);

/**
 * Decode one message sent from `kidevalPanel.js` to `kidevalPanel.ts`.
 */
export function decodeKidevalPanelWebviewMessage(
    value: unknown,
): KidevalPanelWebviewMessage {
    const decoded = decodePanelMessageWithSchema(
        'kideval panel',
        value,
        KidevalPanelWebviewMessageSchema,
    );
    if (decoded.command === 'runAnalysis') {
        return {
            command: 'runAnalysis',
            databasePath: decoded.databasePath,
            databaseFilter: decoded.databaseFilter === undefined
                ? undefined
                : toMutableAnalysisDatabaseFilter(decoded.databaseFilter),
        };
    }

    return decoded;
}

/**
 * Build the active-file metadata message for the KidEval/Eval webview.
 */
export function createKidevalFileInfoMessage(fileName: string): KidevalPanelFileInfoMessage {
    return {
        command: 'fileInfo',
        fileName,
    };
}

/**
 * Build the database-results message for the KidEval/Eval webview.
 */
export function createKidevalDatabasesMessage(
    databases: AvailableDatabase[],
): KidevalPanelDatabasesMessage {
    return {
        command: 'databases',
        databases,
    };
}

/**
 * Build the analysis-results message for the KidEval/Eval webview.
 */
export function createKidevalResultsMessage(data: unknown): KidevalPanelResultsMessage {
    return {
        command: 'results',
        data,
    };
}

/**
 * Build an error notification for the KidEval/Eval webview.
 */
export function createKidevalErrorMessage(message: string): PanelErrorMessage {
    return {
        command: 'error',
        message,
    };
}
