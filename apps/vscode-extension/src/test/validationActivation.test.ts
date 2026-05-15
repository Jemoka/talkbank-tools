import { describe, expect, it, vi } from 'vitest';

vi.mock('vscode', () => {
    class EventEmitter<T> {
        readonly event = vi.fn();
        readonly fire = vi.fn();
        readonly dispose = vi.fn();
    }

    class TreeItem {
        constructor(
            public readonly label?: string,
            public readonly collapsibleState?: number,
        ) {}
    }

    class ThemeIcon {
        constructor(
            public readonly id: string,
            public readonly color?: unknown,
        ) {}
    }

    class ThemeColor {
        constructor(public readonly id: string) {}
    }

    class Position {
        constructor(
            public readonly line: number,
            public readonly character: number,
        ) {}
    }

    class Range {
        constructor(
            public readonly start: Position,
            public readonly end: Position,
        ) {}
    }

    return {
        EventEmitter,
        Position,
        ProgressLocation: {
            Notification: 1,
        },
        Range,
        ThemeColor,
        ThemeIcon,
        TreeItem,
        TreeItemCollapsibleState: {
            None: 0,
            Collapsed: 1,
            Expanded: 2,
        },
        Uri: {
            file: (fsPath: string) => ({ fsPath }),
        },
    };
});

import { activateValidationExplorer } from '../activation/validation';
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

describe('validation activation', () => {
    it('uses the injected runtime-context root for directory validation commands', async () => {
        const handlers = new Map<string, (...args: any[]) => unknown>();
        const treeView = {
            dispose: vi.fn(),
        } as any;
        const window = {
            createTreeView: vi.fn(() => treeView),
            showErrorMessage: vi.fn(),
            showWarningMessage: vi.fn(),
            showInformationMessage: vi.fn(),
            withProgress: vi.fn(),
        } as any;
        const commands = {
            registerCommand: vi.fn((command: string, handler: (...args: any[]) => unknown) => {
                handlers.set(command, handler);
                return { dispose: vi.fn() };
            }),
        } as any;
        const executor = {
            validateFile: vi.fn(),
            previewCacheClear: vi.fn(),
            clearCache: vi.fn(),
        } as any;
        const fileFinder = {
            findChatFiles: vi.fn(),
            getDirectoryEntries: vi.fn(),
        } as any;

        const { provider } = activateValidationExplorer(
            {
                executableService: createExecutableService(),
                runtimeContext: createRuntimeContext('/workspace'),
                window,
                commands,
            },
            {
                executor,
                fileFinder,
            },
        );
        const validateDirectory = handlers.get('talkbank.validateDirectory');
        const validateDirectorySpy = vi
            .spyOn(provider, 'validateDirectory')
            .mockResolvedValue();

        await validateDirectory?.();

        expect(window.createTreeView).toHaveBeenCalledWith(
            'talkbank-validation-explorer',
            expect.objectContaining({
                treeDataProvider: provider,
                showCollapseAll: true,
            }),
        );
        expect(validateDirectorySpy).toHaveBeenCalledWith('/workspace');
    });
});
