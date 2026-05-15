import { afterEach, describe, expect, it, vi } from 'vitest';

import { CacheManager } from '../cacheManager';
import type { ExtensionExecutableService } from '../runtimeServices';
import type { ExtensionRuntimeContext } from '../runtimeContext';
import { createMockStatusBarItem } from './mocks/vscode';

function createRuntimeContext(cachePath = '/cache/talkbank-cache.db'): ExtensionRuntimeContext {
    return {
        getPrimaryWorkspaceRoot: () => '/workspace',
        getConfiguredLspBinaryPath: () => '',
        getInlayHintsEnabled: () => true,
        getValidationSeverity: () => 'all',
        getBulletDisplayMode: () => 'dim',
        getWalkerAutoPlay: () => true,
        getWalkerLoopCount: () => 1,
        getWalkerPauseSeconds: () => 0,
        getWalkerWalkLength: () => 0,
        getMediaDefaultSpeed: () => 100,
        getTranscriptionDefaultSpeaker: () => 'CHI',
        getTranscriptionRewindSeconds: () => 2,
        getCachePath: () => cachePath,
    };
}

function createExecutableService(
    overrides: Partial<ExtensionExecutableService> = {},
): ExtensionExecutableService {
    return {
        findTalkBankCli: vi.fn(async () => null),
        runTalkBankCli: vi.fn(() => ''),
        findSend2ClanBinary: vi.fn(async () => null),
        spawnDetached: vi.fn(),
        findTalkbankLspBinary: vi.fn(() => '/tmp/chatter'),
        ...overrides,
    };
}

describe('CacheManager', () => {
    afterEach(() => {
        vi.useRealTimers();
    });

    it('initializes the injected status bar explicitly', () => {
        vi.useFakeTimers();

        const statusBarItem = createMockStatusBarItem();
        const manager = new CacheManager({
            executableService: createExecutableService(),
            runtimeContext: createRuntimeContext(),
            window: {
                showInformationMessage: vi.fn(),
                showWarningMessage: vi.fn(),
                showErrorMessage: vi.fn(),
            },
            statusBarItem: statusBarItem as any,
        });

        manager.initialize();

        expect(statusBarItem.command).toBe('talkbank.viewCacheStatistics');
        expect(statusBarItem.tooltip).toBe('Click to view cache statistics');
        expect(statusBarItem.show).toHaveBeenCalled();

        manager.dispose();
    });

    it('clears the runtime-context cache path through injected filesystem services', async () => {
        const statusBarItem = createMockStatusBarItem();
        const window = {
            showInformationMessage: vi.fn(),
            showWarningMessage: vi.fn(async () => 'Clear All'),
            showErrorMessage: vi.fn(),
        };
        const fsOps = {
            existsSync: vi.fn(() => true),
            unlinkSync: vi.fn(),
        };
        const manager = new CacheManager({
            executableService: createExecutableService(),
            runtimeContext: createRuntimeContext('/tmp/talkbank-cache.db'),
            window,
            statusBarItem: statusBarItem as any,
            fsOps,
        });

        await manager.clearCache();

        expect(fsOps.unlinkSync).toHaveBeenCalledWith('/tmp/talkbank-cache.db');
        expect(window.showInformationMessage).toHaveBeenCalledWith('Validation cache cleared.');
        manager.dispose();
    });
});
