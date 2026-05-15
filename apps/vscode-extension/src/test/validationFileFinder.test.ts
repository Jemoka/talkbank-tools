import { describe, expect, it, vi } from 'vitest';
import {
    FileFinder,
    type FileFinderFs
} from '../validation/fileFinder';

type DirentKind = 'file' | 'directory';

function createDirent(name: string, kind: DirentKind) {
    return {
        name,
        isDirectory: () => kind === 'directory',
        isFile: () => kind === 'file'
    };
}

function createFileFinderFs(
    entriesByPath: Record<string, ReturnType<typeof createDirent>[]>
): FileFinderFs {
    return {
        readdirSync: vi.fn((dirPath: string) => {
            const entries = entriesByPath[dirPath];
            if (!entries) {
                throw new Error(`ENOENT: ${dirPath}`);
            }
            return entries;
        })
    };
}

describe('FileFinder', () => {
    it('lists visible directories before .cha files', async () => {
        const finder = new FileFinder(
            createFileFinderFs({
                '/workspace': [
                    createDirent('zeta.cha', 'file'),
                    createDirent('alpha', 'directory'),
                    createDirent('.git', 'directory'),
                    createDirent('notes.txt', 'file'),
                    createDirent('beta', 'directory'),
                    createDirent('aardvark.cha', 'file')
                ]
            })
        );

        const entries = await finder.getDirectoryEntries('/workspace');

        expect(entries).toEqual([
            {
                name: 'alpha',
                fullPath: '/workspace/alpha',
                itemType: 'directory'
            },
            {
                name: 'beta',
                fullPath: '/workspace/beta',
                itemType: 'directory'
            },
            {
                name: 'aardvark.cha',
                fullPath: '/workspace/aardvark.cha',
                itemType: 'file'
            },
            {
                name: 'zeta.cha',
                fullPath: '/workspace/zeta.cha',
                itemType: 'file'
            }
        ]);
    });

    it('finds .cha files recursively and skips hidden directories', async () => {
        const finder = new FileFinder(
            createFileFinderFs({
                '/workspace': [
                    createDirent('nested', 'directory'),
                    createDirent('.git', 'directory'),
                    createDirent('root.cha', 'file')
                ],
                '/workspace/nested': [
                    createDirent('deep.cha', 'file'),
                    createDirent('notes.txt', 'file')
                ]
            })
        );

        const files = await finder.findChatFiles('/workspace');

        expect(files).toEqual([
            '/workspace/nested/deep.cha',
            '/workspace/root.cha'
        ]);
    });
});
