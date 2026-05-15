/**
 * Effect-based state store for the alignment review mode.
 *
 * Follows the `coderState.ts` pattern: private mutable state behind an
 * immutable snapshot interface, registered as an Effect service via
 * `Context.GenericTag`.
 */

import { Context } from 'effect';
import type { ReviewItem, ReviewStatus } from './reviewModel';

// ---------------------------------------------------------------------------
// Snapshot (immutable public view)
// ---------------------------------------------------------------------------

/** Read-only snapshot of review mode state. */
export interface ReviewCommandState {
    /** Whether review mode is currently active. */
    readonly active: boolean;
    /** All review items found in the current file. */
    readonly items: readonly ReviewItem[];
    /** Index of the currently selected review item. */
    readonly currentIndex: number;
    /** URI of the document being reviewed. */
    readonly documentUri: string;
}

// ---------------------------------------------------------------------------
// Store (mutable interface)
// ---------------------------------------------------------------------------

/** Mutable state store for review mode (exported for Effect layer typing). */
export interface ReviewCommandStateStore {
    /** Return an immutable snapshot of current state. */
    snapshot(): ReviewCommandState;
    /** Enter review mode with the given items and document. */
    activate(items: readonly ReviewItem[], documentUri: string): void;
    /** Update the current item index (navigation). */
    setCurrentIndex(index: number): void;
    /** Update a specific item's status and notes after rating. */
    updateItemStatus(index: number, status: ReviewStatus, notes: string): void;
    /** Exit review mode and clear all state. */
    reset(): void;
}

// ---------------------------------------------------------------------------
// Effect service tag
// ---------------------------------------------------------------------------

/**
 * Effect service tag for injecting the review state store into commands.
 *
 * Usage in commands:
 * ```ts
 * const reviewState = yield* ReviewCommandStateService;
 * const snapshot = reviewState.snapshot();
 * ```
 */
export const ReviewCommandStateService = Context.GenericTag<ReviewCommandStateStore>(
    'talkbank/ReviewCommandState',
);

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a fresh review state store instance.
 *
 * Called once during extension activation and registered in the Effect layer
 * via `Layer.succeed(ReviewCommandStateService, store)`.
 */
export function createReviewCommandStateStore(): ReviewCommandStateStore {
    let active = false;
    let items: ReviewItem[] = [];
    let currentIndex = 0;
    let documentUri = '';

    return {
        snapshot: () => ({
            active,
            items: [...items],
            currentIndex,
            documentUri,
        }),

        activate: (newItems, uri) => {
            active = true;
            items = [...newItems];
            currentIndex = 0;
            documentUri = uri;
        },

        setCurrentIndex: (index) => {
            currentIndex = index;
        },

        updateItemStatus: (index, status, notes) => {
            if (index >= 0 && index < items.length) {
                items[index] = { ...items[index], status, notes };
            }
        },

        reset: () => {
            active = false;
            items = [];
            currentIndex = 0;
            documentUri = '';
        },
    };
}
