/**
 * Tests for shared utility functions.
 *
 * Following TDD methodology - tests written before implementation.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as fc from 'fast-check';
import { createMockFileSystem, createMockExecSync } from './mocks/childProcess';

// Will be implemented in src/utils/cliLocator.ts
import { findTalkBankCli, CliLocatorOptions } from '../utils/cliLocator';
import { findProjectRoot, ProjectSearchResult } from '../utils/projectLocator';

describe('findTalkBankCli', () => {
    let mockFs: any;
    let mockExecSync: any;

    beforeEach(() => {
        // Reset mocks before each test
        vi.clearAllMocks();
    });

    afterEach(() => {
        // Restore mocks
        vi.restoreAllMocks();
    });

    it('should find chatter in debug directory when it exists', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/project/target/debug/chatter': 'binary'
        });

        const options: CliLocatorOptions = {
            projectRoot: '/project',
            fs: mockFs
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBe('/project/target/debug/chatter');
        expect(mockFs.existsSync).toHaveBeenCalledWith('/project/target/debug/chatter');
    });

    it('should find chatter in release directory when debug does not exist', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/project/target/release/chatter': 'binary'
        });

        const options: CliLocatorOptions = {
            projectRoot: '/project',
            fs: mockFs
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBe('/project/target/release/chatter');
        expect(mockFs.existsSync).toHaveBeenCalledWith('/project/target/debug/chatter');
        expect(mockFs.existsSync).toHaveBeenCalledWith('/project/target/release/chatter');
    });

    it('should fall back to PATH when not found in project', async () => {
        // Arrange
        mockFs = createMockFileSystem({});
        mockExecSync = createMockExecSync({
            stdout: '/usr/local/bin/chatter\n'
        });

        const options: CliLocatorOptions = {
            projectRoot: '/project',
            fs: mockFs,
            execSync: mockExecSync
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBe('/usr/local/bin/chatter');
        expect(mockExecSync).toHaveBeenCalledWith(
            'which chatter',
            expect.objectContaining({ encoding: 'utf-8' })
        );
    });

    it('should return null when chatter is not found anywhere', async () => {
        // Arrange
        mockFs = createMockFileSystem({});
        mockExecSync = createMockExecSync({
            shouldThrow: true,
            errorMessage: 'Command failed: which chatter',
            exitCode: 1
        });

        const options: CliLocatorOptions = {
            projectRoot: '/project',
            fs: mockFs,
            execSync: mockExecSync
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBeNull();
    });

    it('should handle undefined projectRoot', async () => {
        // Arrange
        mockExecSync = createMockExecSync({
            stdout: '/usr/local/bin/chatter\n'
        });

        const options: CliLocatorOptions = {
            projectRoot: undefined,
            execSync: mockExecSync
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBe('/usr/local/bin/chatter');
    });

    it('should prefer debug over release build', async () => {
        // Arrange - both debug and release exist
        mockFs = createMockFileSystem({
            '/project/target/debug/chatter': 'debug-binary',
            '/project/target/release/chatter': 'release-binary'
        });

        const options: CliLocatorOptions = {
            projectRoot: '/project',
            fs: mockFs
        };

        // Act
        const result = await findTalkBankCli(options);

        // Assert
        expect(result).toBe('/project/target/debug/chatter');
    });
});

describe('findProjectRoot', () => {
    let mockFs: any;

    beforeEach(() => {
        vi.clearAllMocks();
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('should find project root in current directory', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/workspace/Cargo.toml': '[package]\nname = "test"'
        });

        const options = {
            startPath: '/workspace',
            fs: mockFs
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(true);
        expect(result.path).toBe('/workspace');
    });

    it('should traverse up to parent directories', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/workspace/project/Cargo.toml': '[package]\nname = "test"'
        });

        const options = {
            startPath: '/workspace/project/src/subdir',
            fs: mockFs
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(true);
        expect(result.path).toBe('/workspace/project');
    });

    it('should respect max depth limit', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/Cargo.toml': '[package]\nname = "test"'
        });

        const options = {
            startPath: '/a/b/c/d/e/f/g/h/i/j/k',
            fs: mockFs,
            maxDepth: 5
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(false);
        expect(result.path).toBeNull();
    });

    it('should return not found when no Cargo.toml exists', async () => {
        // Arrange
        mockFs = createMockFileSystem({});

        const options = {
            startPath: '/workspace',
            fs: mockFs
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(false);
        expect(result.path).toBeNull();
    });

    it('should stop at filesystem root', async () => {
        // Arrange
        mockFs = createMockFileSystem({});

        const options = {
            startPath: '/workspace/project',
            fs: mockFs
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(false);
        expect(result.path).toBeNull();
    });

    it('should use default max depth of 10', async () => {
        // Arrange
        mockFs = createMockFileSystem({
            '/Cargo.toml': '[package]'
        });

        // Mock path.dirname to track depth
        let depth = 0;
        const originalDirname = require('path').dirname;
        vi.mock('path', async () => {
            const actual = await vi.importActual('path');
            return {
                ...actual,
                dirname: (p: string) => {
                    depth++;
                    return originalDirname(p);
                }
            };
        });

        const options = {
            startPath: '/a/b/c/d/e/f/g/h/i/j/k',
            fs: mockFs
            // maxDepth not specified - should default to 10
        };

        // Act
        const result = await findProjectRoot(options);

        // Assert
        expect(result.found).toBe(false);
    });
});

describe('findProjectRoot - property-based tests', () => {
    it('should never throw errors regardless of path', () => {
        fc.assert(
            fc.asyncProperty(
                fc.string(),
                async (randomPath) => {
                    const mockFs = createMockFileSystem({});
                    const options = {
                        startPath: randomPath || '/default',
                        fs: mockFs
                    };

                    // Should not throw
                    const result = await findProjectRoot(options);

                    // Result should always be valid
                    expect(result).toHaveProperty('found');
                    expect(result).toHaveProperty('path');
                    expect(typeof result.found).toBe('boolean');

                    if (result.found) {
                        expect(typeof result.path).toBe('string');
                    } else {
                        expect(result.path).toBeNull();
                    }
                }
            ),
            { numRuns: 50 }
        );
    });

    it('should always find Cargo.toml when it exists in path', () => {
        fc.assert(
            fc.asyncProperty(
                fc.array(fc.string().filter(s => s.length > 0 && !s.includes('/')), { minLength: 1, maxLength: 5 }),
                async (pathSegments) => {
                    const fullPath = '/' + pathSegments.join('/');
                    const mockFs = createMockFileSystem({
                        [fullPath + '/Cargo.toml']: '[package]'
                    });

                    const options = {
                        startPath: fullPath + '/subdir',
                        fs: mockFs
                    };

                    const result = await findProjectRoot(options);

                    expect(result.found).toBe(true);
                    expect(result.path).toBe(fullPath);
                }
            ),
            { numRuns: 50 }
        );
    });
});

describe('findTalkBankCli - property-based tests', () => {
    it('should never throw errors regardless of inputs', () => {
        fc.assert(
            fc.asyncProperty(
                fc.option(fc.string(), { nil: undefined }),
                async (randomRoot) => {
                    const mockFs = createMockFileSystem({});
                    const mockExecSync = createMockExecSync({ shouldThrow: true });

                    const options: CliLocatorOptions = {
                        projectRoot: randomRoot,
                        fs: mockFs,
                        execSync: mockExecSync
                    };

                    // Should not throw
                    const result = await findTalkBankCli(options);

                    // Result should be string or null
                    expect(result === null || typeof result === 'string').toBe(true);
                }
            ),
            { numRuns: 50 }
        );
    });
});
