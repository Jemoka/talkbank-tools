/**
 * Tests for shared singleton-panel lifecycle helpers.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';

import { PanelMessageDecodeError } from '../effectBoundary';
import {
    decodePanelMessage,
    disposePanelResources,
    reuseOrCreatePanel,
} from '../panelLifecycle';

describe('panel lifecycle helpers', () => {
    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('reuses an existing singleton panel', () => {
        const currentPanel = { id: 'current' };
        const reuseExisting = vi.fn();
        const createPanel = vi.fn(() => ({ id: 'new' }));

        const returned = reuseOrCreatePanel(currentPanel, reuseExisting, createPanel);

        expect(returned).toBe(currentPanel);
        expect(reuseExisting).toHaveBeenCalledWith(currentPanel);
        expect(createPanel).not.toHaveBeenCalled();
    });

    it('creates a new singleton panel when none exists', () => {
        const reuseExisting = vi.fn();
        const createdPanel = { id: 'new' };
        const createPanel = vi.fn(() => createdPanel);

        const returned = reuseOrCreatePanel(undefined, reuseExisting, createPanel);

        expect(returned).toBe(createdPanel);
        expect(reuseExisting).not.toHaveBeenCalled();
        expect(createPanel).toHaveBeenCalledOnce();
    });

    it('logs and drops malformed panel messages', () => {
        const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);

        const decoded = decodePanelMessage(
            { command: 'nope' },
            'analysis panel',
            () => {
                throw new PanelMessageDecodeError({
                    label: 'analysis panel',
                    details: 'Expected exportCsv payload',
                });
            },
        );

        expect(decoded).toBeUndefined();
        expect(consoleError).toHaveBeenCalledWith(
            'Unexpected analysis panel message',
            { command: 'nope' },
            'Expected exportCsv payload',
        );
    });

    it('clears singleton state and drains panel disposables', () => {
        const disposed: string[] = [];
        const panel = {
            dispose: vi.fn(() => {
                disposed.push('panel');
            }),
        };
        const disposables = [
            {
                dispose: vi.fn(() => {
                    disposed.push('first');
                }),
            },
            {
                dispose: vi.fn(() => {
                    disposed.push('second');
                }),
            },
        ];
        const clearCurrentPanel = vi.fn(() => {
            disposed.push('clear');
        });

        disposePanelResources(panel, disposables, clearCurrentPanel);

        expect(clearCurrentPanel).toHaveBeenCalledOnce();
        expect(panel.dispose).toHaveBeenCalledOnce();
        expect(disposables).toHaveLength(0);
        expect(disposed).toEqual(['clear', 'panel', 'second', 'first']);
    });
});
