/**
 * Persistent command-state store for coder mode.
 *
 * The command runner owns this store so coder workflows survive across command
 * invocations without depending on module-level mutable globals.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Context } from 'effect';

import type { CodeNode } from './coderModel';

/**
 * Snapshot of the current coder-mode session.
 */
export interface CoderCommandState {
    /**
     * Whether coder mode is currently active.
     */
    readonly active: boolean;
    /**
     * Loaded hierarchical code tree from the selected `.cut` file.
     */
    readonly codesTree: readonly CodeNode[];
    /**
     * Absolute path to the loaded `.cut` file.
     */
    readonly codesFilePath?: string;
    /**
     * Current utterance line tracked by coder mode.
     */
    readonly currentUtteranceLine: number;
}

/**
 * Persistent store for coder command state.
 */
export interface CoderCommandStateStore {
    /**
     * Return the current immutable snapshot.
     */
    snapshot(): CoderCommandState;
    /**
     * Activate coder mode with one loaded code tree and source path.
     */
    activate(codesTree: readonly CodeNode[], codesFilePath: string): void;
    /**
     * Update the tracked utterance line for the active coder session.
     */
    setCurrentUtteranceLine(line: number): void;
    /**
     * Clear all coder-mode session state.
     */
    reset(): void;
}

/**
 * Effect service for the persistent coder-mode store.
 */
export const CoderCommandStateService = Context.GenericTag<CoderCommandStateStore>(
    'talkbank/CoderCommandState',
);

/**
 * Create the persistent coder state store used by the command runner.
 */
export function createCoderCommandStateStore(): CoderCommandStateStore {
    let active = false;
    let codesTree: readonly CodeNode[] = [];
    let codesFilePath: string | undefined;
    let currentUtteranceLine = -1;

    return {
        snapshot: () => ({
            active,
            codesTree,
            codesFilePath,
            currentUtteranceLine,
        }),
        activate: (nextCodesTree, nextCodesFilePath) => {
            active = true;
            codesTree = [...nextCodesTree];
            codesFilePath = nextCodesFilePath;
            currentUtteranceLine = -1;
        },
        setCurrentUtteranceLine: line => {
            currentUtteranceLine = line;
        },
        reset: () => {
            active = false;
            codesTree = [];
            codesFilePath = undefined;
            currentUtteranceLine = -1;
        },
    };
}
