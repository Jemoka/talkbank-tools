/**
 * Message contract for the media playback webview panel.
 *
 * Split out from the umbrella `webviewMessageContracts.ts` so each
 * panel owns its own module. `webviewMessageContracts.ts` now
 * re-exports these names for backward compatibility — existing
 * consumers do not need to change their import paths.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Schema } from 'effect';

import { decodePanelMessageWithSchema } from '../effectBoundary';
import type { TimestampMs } from '../utils/bulletParser';

/**
 * Segment-change notification sent from the media webview.
 */
export interface MediaPanelSegmentChangedMessage {
    /**
     * Discriminant identifying a segment-change notification.
     */
    command: 'segmentChanged';
    /**
     * Zero-based segment index currently playing.
     */
    index: number;
}

/**
 * Timestamp response sent from the media webview.
 */
export interface MediaPanelTimestampMessage {
    /**
     * Discriminant identifying a timestamp response.
     */
    command: 'timestamp';
    /**
     * Current playback position in milliseconds.
     */
    ms: TimestampMs;
}

/**
 * Playback-stop notification sent from the media webview.
 */
export interface MediaPanelStoppedMessage {
    /**
     * Discriminant identifying stopped playback.
     */
    command: 'stopped';
}

/**
 * Message sent from the media webview to the extension.
 */
export type MediaPanelWebviewMessage =
    | MediaPanelSegmentChangedMessage
    | MediaPanelTimestampMessage
    | MediaPanelStoppedMessage;

/**
 * Rewind request sent from the extension to the media webview.
 */
export interface MediaPanelRewindMessage {
    /**
     * Discriminant identifying rewind.
     */
    command: 'rewind';
    /**
     * Number of seconds to rewind.
     */
    seconds: number;
}

/**
 * Loop-toggle request sent from the extension to the media webview.
 */
export interface MediaPanelSetLoopMessage {
    /**
     * Discriminant identifying loop toggling.
     */
    command: 'setLoop';
}

/**
 * Timestamp request sent from the extension to the media webview.
 */
export interface MediaPanelRequestTimestampMessage {
    /**
     * Discriminant identifying timestamp requests.
     */
    command: 'requestTimestamp';
}

/**
 * Absolute-seek request sent from the extension to the media webview.
 */
export interface MediaPanelSeekToMessage {
    /**
     * Discriminant identifying absolute seeks.
     */
    command: 'seekTo';
    /**
     * Target playback position in milliseconds.
     */
    ms: TimestampMs;
}

/**
 * Message sent from the extension to the media webview.
 */
export type MediaPanelExtensionMessage =
    | MediaPanelRewindMessage
    | MediaPanelSetLoopMessage
    | MediaPanelRequestTimestampMessage
    | MediaPanelSeekToMessage;

const MediaPanelSegmentChangedMessageSchema = Schema.Struct({
    command: Schema.Literal('segmentChanged'),
    index: Schema.NonNegativeInt,
});

const MediaPanelTimestampMessageSchema = Schema.Struct({
    command: Schema.Literal('timestamp'),
    ms: Schema.NonNegativeInt,
});

const MediaPanelStoppedMessageSchema = Schema.Struct({
    command: Schema.Literal('stopped'),
});

const MediaPanelWebviewMessageSchema = Schema.Union(
    MediaPanelSegmentChangedMessageSchema,
    MediaPanelTimestampMessageSchema,
    MediaPanelStoppedMessageSchema,
);

function toTimestampMs(value: number): TimestampMs {
    return value as TimestampMs;
}

/**
 * Decode one message sent from `mediaPanel.js` to `mediaPanel.ts`.
 */
export function decodeMediaPanelWebviewMessage(
    value: unknown,
): MediaPanelWebviewMessage {
    const decoded = decodePanelMessageWithSchema(
        'media panel',
        value,
        MediaPanelWebviewMessageSchema,
    );
    if (decoded.command === 'timestamp') {
        return {
            command: 'timestamp',
            ms: toTimestampMs(decoded.ms),
        };
    }

    return decoded;
}

/**
 * Build a rewind request for the media webview.
 */
export function createMediaRewindMessage(seconds: number): MediaPanelRewindMessage {
    return {
        command: 'rewind',
        seconds,
    };
}

/**
 * Build a loop-toggle request for the media webview.
 */
export function createMediaSetLoopMessage(): MediaPanelSetLoopMessage {
    return {
        command: 'setLoop',
    };
}

/**
 * Build a timestamp request for the media webview.
 */
export function createMediaRequestTimestampMessage(): MediaPanelRequestTimestampMessage {
    return {
        command: 'requestTimestamp',
    };
}

/**
 * Build an absolute-seek request for the media webview.
 */
export function createMediaSeekToMessage(ms: TimestampMs): MediaPanelSeekToMessage {
    return {
        command: 'seekTo',
        ms,
    };
}
