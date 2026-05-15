/**
 * Shared lifecycle helpers for singleton VS Code webview panels.
 *
 * Keep the mechanics for "reuse existing or create new", inbound message
 * decoding, and disposable cleanup here so individual panel modules can focus
 * on their feature-specific behavior.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { PanelMessageDecodeError } from './effectBoundary';

/**
 * Minimal disposable shape used by panel lifecycle helpers.
 */
export interface DisposableLike {
    dispose(): void;
}

/**
 * Reuse the existing singleton panel when present; otherwise create a new one.
 */
export function reuseOrCreatePanel<T>(
    currentPanel: T | undefined,
    reuseExisting: (panel: T) => void,
    createPanel: () => T,
): T {
    if (currentPanel !== undefined) {
        reuseExisting(currentPanel);
        return currentPanel;
    }

    return createPanel();
}

/**
 * Decode one inbound webview message and log a consistent error on failure.
 */
export function decodePanelMessage<T>(
    message: unknown,
    label: string,
    decode: (message: unknown) => T | undefined,
): T | undefined {
    try {
        const decoded = decode(message);
        if (decoded === undefined) {
            console.error(`Unexpected ${label} message`, message);
            return undefined;
        }

        return decoded;
    } catch (error) {
        if (error instanceof PanelMessageDecodeError) {
            console.error(`Unexpected ${label} message`, message, error.details);
            return undefined;
        }

        throw error;
    }
}

/**
 * Clear the singleton reference, dispose the panel, and drain the disposable
 * stack owned by that panel instance.
 */
export function disposePanelResources(
    panel: DisposableLike,
    disposables: DisposableLike[],
    clearCurrentPanel: () => void,
): void {
    clearCurrentPanel();
    panel.dispose();

    while (disposables.length > 0) {
        disposables.pop()?.dispose();
    }
}
