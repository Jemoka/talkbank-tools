import { describe, expect, it } from 'vitest';
import { ExecutableService } from '../executableService';
import { createMockExecSync, createMockFileSystem } from './mocks/childProcess';

describe('ExecutableService — bundled-binary discovery', () => {
    it('prefers the bundled extension/server/chatter-lsp over everything else', () => {
        const mockFs = createMockFileSystem({
            '/extension/server/chatter-lsp': 'binary',
            '/usr/local/bin/chatter-lsp': 'also-binary',
            '/extension/../target/release/chatter-lsp': 'dev-binary',
        });
        const execSync = createMockExecSync({
            stdout: '/usr/local/bin/chatter-lsp\n',
        });
        const service = new ExecutableService({ fs: mockFs, execSync });
        const context = {
            asAbsolutePath: (rel: string) => `/extension/${rel}`,
        } as any;

        expect(service.findTalkbankLspBinary(context)).toBe(
            '/extension/server/chatter-lsp',
        );
        // PATH lookup must not happen when bundled binary exists
        expect(execSync).not.toHaveBeenCalled();
    });

    it('uses server/chatter-lsp.exe on Windows', () => {
        const mockFs = createMockFileSystem({
            '/extension/server/chatter-lsp.exe': 'binary',
        });
        const execSync = createMockExecSync({ shouldThrow: true, errorMessage: 'which failed' });
        const service = new ExecutableService({ fs: mockFs, execSync });
        const context = {
            asAbsolutePath: (rel: string) => `/extension/${rel}`,
        } as any;
        const originalPlatform = process.platform;
        Object.defineProperty(process, 'platform', { value: 'win32' });
        try {
            expect(service.findTalkbankLspBinary(context)).toBe(
                '/extension/server/chatter-lsp.exe',
            );
        } finally {
            Object.defineProperty(process, 'platform', { value: originalPlatform });
        }
    });

    it('falls through to PATH when no bundled binary', () => {
        const mockFs = createMockFileSystem({
            '/usr/local/bin/chatter-lsp': 'binary',
        });
        const execSync = createMockExecSync({ stdout: '/usr/local/bin/chatter-lsp\n' });
        const service = new ExecutableService({ fs: mockFs, execSync });
        const context = { asAbsolutePath: (rel: string) => `/extension/${rel}` } as any;

        expect(service.findTalkbankLspBinary(context)).toBe('/usr/local/bin/chatter-lsp');
    });
});
