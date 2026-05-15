import { describe, expect, it, vi } from 'vitest';

import { ClanIntegration } from '../clanIntegration';
import type { ExtensionExecutableService } from '../runtimeServices';
import type { ExtensionRuntimeContext } from '../runtimeContext';

function createRuntimeContext(root = '/workspace'): ExtensionRuntimeContext {
    return {
        getPrimaryWorkspaceRoot: () => root,
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
        getCachePath: () => '/cache/talkbank-cache.db',
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

describe('ClanIntegration', () => {
    it('reports missing send2clan binaries through the injected window service', async () => {
        const window = {
            showInformationMessage: vi.fn(),
            showErrorMessage: vi.fn(),
        };
        const executableService = createExecutableService({
            findSend2ClanBinary: vi.fn(async () => null),
        });
        const integration = new ClanIntegration({
            executableService,
            runtimeContext: createRuntimeContext('/workspace'),
            window,
        });

        await integration.openInClan('/workspace/sample.cha', 4, 2);

        expect(executableService.findSend2ClanBinary).toHaveBeenCalledWith('/workspace');
        expect(window.showErrorMessage).toHaveBeenCalledWith(
            'send2clan binary not found. Please build the project: cargo build -p send2clan-sys'
        );
    });

    it('spawns send2clan with line and column arguments through the shared executable service', async () => {
        const window = {
            showInformationMessage: vi.fn(),
            showErrorMessage: vi.fn(),
        };
        const executableService = createExecutableService({
            findSend2ClanBinary: vi.fn(async () => '/tmp/send2clan'),
        });
        const integration = new ClanIntegration({
            executableService,
            runtimeContext: createRuntimeContext('/workspace'),
            window,
        });

        await integration.openInClan('/workspace/sample.cha', 4, 2);

        expect(executableService.spawnDetached).toHaveBeenCalledWith(
            '/tmp/send2clan',
            ['/workspace/sample.cha', '4', '2'],
        );
        expect(window.showInformationMessage).toHaveBeenCalledWith(
            'Opening sample.cha in CLAN...'
        );
    });
});
