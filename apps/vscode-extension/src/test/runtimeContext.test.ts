import { describe, expect, it } from 'vitest';
import {
    DefaultExtensionRuntimeContext,
    normalizeBulletDisplayMode,
    normalizeValidationSeverity,
    resolveDefaultCachePath,
    type RuntimeWorkspaceFacade,
} from '../runtimeContext';

function createWorkspace(
    values: Record<string, Record<string, unknown>>,
    workspaceRoot = '/workspace',
): RuntimeWorkspaceFacade {
    return {
        workspaceFolders: [{ uri: { fsPath: workspaceRoot } }],
        getConfiguration(section = '') {
            const sectionValues = values[section] ?? {};
            return {
                get<T>(key: string, defaultValue?: T): T {
                    return (key in sectionValues
                        ? sectionValues[key]
                        : defaultValue) as T;
                },
            };
        },
    };
}

describe('resolveDefaultCachePath', () => {
    it('builds the default TalkBank cache path under the user cache dir', () => {
        expect(resolveDefaultCachePath('/Users/test')).toBe(
            '/Users/test/.cache/talkbank-tools/talkbank-cache.db'
        );
    });
});

describe('normalizeValidationSeverity', () => {
    it('accepts known severity values', () => {
        expect(normalizeValidationSeverity('all')).toBe('all');
        expect(normalizeValidationSeverity('errorsOnly')).toBe('errorsOnly');
        expect(normalizeValidationSeverity('errorsAndWarnings')).toBe(
            'errorsAndWarnings'
        );
    });

    it('falls back to all for unknown values', () => {
        expect(normalizeValidationSeverity('mystery')).toBe('all');
    });
});

describe('normalizeBulletDisplayMode', () => {
    it('accepts known bullet display values', () => {
        expect(normalizeBulletDisplayMode('dim')).toBe('dim');
        expect(normalizeBulletDisplayMode('hidden')).toBe('hidden');
        expect(normalizeBulletDisplayMode('normal')).toBe('normal');
    });

    it('falls back to dim for unknown values', () => {
        expect(normalizeBulletDisplayMode('mystery')).toBe('dim');
    });
});

describe('DefaultExtensionRuntimeContext', () => {
    it('reads workspace-root and configuration values through the shared boundary', () => {
        const runtimeContext = new DefaultExtensionRuntimeContext({
            workspace: createWorkspace({
                'talkbank.lsp': { binaryPath: '/tmp/chatter' },
                'talkbank.inlayHints': { enabled: false },
                'talkbank.validation': { severity: 'errorsOnly' },
                'talkbank.bullets': { display: 'hidden' },
                talkbank: {
                    'walker.autoPlay': false,
                    'walker.loopCount': 3,
                    'walker.pauseSeconds': 1.5,
                    'walker.walkLength': 12,
                    'media.defaultSpeed': 75,
                    'transcription.defaultSpeaker': 'MOT',
                    'transcription.rewindSeconds': 4,
                },
            }, '/tmp/workspace'),
            homedir: () => '/Users/test',
        });

        expect(runtimeContext.getPrimaryWorkspaceRoot()).toBe('/tmp/workspace');
        expect(runtimeContext.getConfiguredLspBinaryPath()).toBe('/tmp/chatter');
        expect(runtimeContext.getInlayHintsEnabled()).toBe(false);
        expect(runtimeContext.getValidationSeverity()).toBe('errorsOnly');
        expect(runtimeContext.getBulletDisplayMode()).toBe('hidden');
        expect(runtimeContext.getWalkerAutoPlay()).toBe(false);
        expect(runtimeContext.getWalkerLoopCount()).toBe(3);
        expect(runtimeContext.getWalkerPauseSeconds()).toBe(1.5);
        expect(runtimeContext.getWalkerWalkLength()).toBe(12);
        expect(runtimeContext.getMediaDefaultSpeed()).toBe(75);
        expect(runtimeContext.getTranscriptionDefaultSpeaker()).toBe('MOT');
        expect(runtimeContext.getTranscriptionRewindSeconds()).toBe(4);
        expect(runtimeContext.getCachePath()).toBe(
            '/Users/test/.cache/talkbank-tools/talkbank-cache.db'
        );
    });

    it('uses safe defaults when configuration values are absent', () => {
        const runtimeContext = new DefaultExtensionRuntimeContext({
            workspace: createWorkspace({}, '/tmp/workspace'),
            homedir: () => '/Users/test',
        });

        expect(runtimeContext.getConfiguredLspBinaryPath()).toBe('');
        expect(runtimeContext.getInlayHintsEnabled()).toBe(true);
        expect(runtimeContext.getValidationSeverity()).toBe('all');
        expect(runtimeContext.getBulletDisplayMode()).toBe('dim');
        expect(runtimeContext.getWalkerAutoPlay()).toBe(true);
        expect(runtimeContext.getWalkerLoopCount()).toBe(1);
        expect(runtimeContext.getWalkerPauseSeconds()).toBe(0);
        expect(runtimeContext.getWalkerWalkLength()).toBe(0);
        expect(runtimeContext.getMediaDefaultSpeed()).toBe(100);
        expect(runtimeContext.getTranscriptionDefaultSpeaker()).toBe('CHI');
        expect(runtimeContext.getTranscriptionRewindSeconds()).toBe(2);
    });

    it('normalizes unknown validation severity values', () => {
        const runtimeContext = new DefaultExtensionRuntimeContext({
            workspace: createWorkspace({
                'talkbank.validation': { severity: 'not-valid' },
                'talkbank.bullets': { display: 'barely-there' },
            }),
        });

        expect(runtimeContext.getValidationSeverity()).toBe('all');
        expect(runtimeContext.getBulletDisplayMode()).toBe('dim');
    });
});
