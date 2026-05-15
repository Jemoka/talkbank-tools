/**
 * Shared persistent state for media-related extension commands.
 *
 * The VS Code command runner now owns these stores instead of relying on
 * module-level mutable variables in individual command files.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Context } from 'effect';

import type { Segment, TimestampMs } from './utils/bulletParser';

/**
 * Snapshot of the transcription command state.
 */
export interface TranscriptionCommandState {
    readonly active: boolean;
    readonly previousTimestamp: TimestampMs;
}

/**
 * Persistent store for transcription command state.
 */
export interface TranscriptionCommandStateStore {
    snapshot(): TranscriptionCommandState;
    activate(startMs: TimestampMs): void;
    updatePreviousTimestamp(timestamp: TimestampMs): void;
    reset(): void;
}

/**
 * Effect service for persistent transcription command state.
 */
export const TranscriptionCommandStateService = Context.GenericTag<TranscriptionCommandStateStore>(
    'talkbank/TranscriptionCommandState',
);

/**
 * Per-document walker state.
 */
export interface WalkerCommandState {
    readonly segments: readonly Segment[];
    readonly index: number;
}

/**
 * Persistent store for walker command state.
 */
export interface WalkerCommandStateStore {
    get(documentUri: string): WalkerCommandState | undefined;
    initialize(documentUri: string, segments: readonly Segment[]): WalkerCommandState;
    advance(documentUri: string, delta: number): WalkerCommandState | undefined;
    clear(documentUri: string): void;
    snapshot(): ReadonlyMap<string, WalkerCommandState>;
}

/**
 * Effect service for persistent walker command state.
 */
export const WalkerCommandStateService = Context.GenericTag<WalkerCommandStateStore>(
    'talkbank/WalkerCommandState',
);

/**
 * Create the persistent transcription state store used by the command runner.
 */
export function createTranscriptionCommandStateStore(): TranscriptionCommandStateStore {
    let active = false;
    let previousTimestamp = 0 as TimestampMs;

    return {
        snapshot: () => ({
            active,
            previousTimestamp,
        }),
        activate: startMs => {
            active = true;
            previousTimestamp = startMs;
        },
        updatePreviousTimestamp: timestamp => {
            previousTimestamp = timestamp;
        },
        reset: () => {
            active = false;
            previousTimestamp = 0 as TimestampMs;
        },
    };
}

/**
 * Create the persistent walker state store used by the command runner.
 */
export function createWalkerCommandStateStore(): WalkerCommandStateStore {
    const states = new Map<string, WalkerCommandState>();

    return {
        get: documentUri => states.get(documentUri),
        initialize: (documentUri, segments) => {
            const state: WalkerCommandState = {
                segments: [...segments],
                index: -1,
            };
            states.set(documentUri, state);
            return state;
        },
        advance: (documentUri, delta) => {
            const current = states.get(documentUri);
            if (!current) {
                return undefined;
            }

            const next: WalkerCommandState = {
                ...current,
                index: Math.max(0, Math.min(current.segments.length - 1, current.index + delta)),
            };
            states.set(documentUri, next);
            return next;
        },
        clear: documentUri => {
            states.delete(documentUri);
        },
        snapshot: () => new Map(states),
    };
}
