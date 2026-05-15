/**
 * Tests for cache statistics functionality.
 *
 * Following TDD methodology - tests written before implementation.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as fc from 'fast-check';
import { createCommandSpecificMock } from './mocks/childProcess';
import { ExecutableService } from '../executableService';

// Will be implemented in src/models/cacheStatistics.ts
import {
    CacheStatisticsManager,
    CacheStatsResponse,
    CacheStatistics,
    CorpusStatistics
} from '../models/cacheStatistics';

describe('CacheStatisticsManager', () => {
    let mockExecSync: any;

    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('getCacheStatistics', () => {
        it('should parse valid JSON from CLI', async () => {
            // Arrange
            const mockResponse: CacheStatsResponse = {
                statistics: {
                    totalEntries: 100,
                    roundtripTested: 50,
                    cacheDir: '/Users/test/.cache/talkbank-tools',
                    cacheSizeBytes: 5242880,
                    lastModified: '2026-01-17T10:00:00Z'
                },
                corpora: [
                    {
                        corpusId: 1,
                        name: 'CHILDES',
                        path: '/data/childes',
                        fileCount: 100,
                        passed: 90,
                        failed: 10,
                        notTested: 0
                    }
                ]
            };

            mockExecSync = createCommandSpecificMock({
                'cache stats --json': {
                    stdout: JSON.stringify(mockResponse)
                }
            });

            const manager = new CacheStatisticsManager(
                '/path/to/talkbank',
                new ExecutableService({ execSync: mockExecSync }),
            );

            // Act
            const result = await manager.getCacheStatistics();

            // Assert
            expect(result).not.toBeNull();
            expect(result!.statistics.totalEntries).toBe(100);
            expect(result!.statistics.roundtripTested).toBe(50);
            expect(result!.corpora).toHaveLength(1);
            expect(result!.corpora[0].name).toBe('CHILDES');
        });

        it('should return null when CLI is not found', async () => {
            // Arrange
            mockExecSync = createCommandSpecificMock({
                'cache stats --json': {
                    shouldThrow: true,
                    errorMessage: 'Command not found',
                    exitCode: 127
                }
            });

            const manager = new CacheStatisticsManager(
                '/path/to/talkbank',
                new ExecutableService({ execSync: mockExecSync }),
            );

            // Act
            const result = await manager.getCacheStatistics();

            // Assert
            expect(result).toBeNull();
        });

        it('should return null when JSON parsing fails', async () => {
            // Arrange
            mockExecSync = createCommandSpecificMock({
                'cache stats --json': {
                    stdout: 'invalid json{{'
                }
            });

            const manager = new CacheStatisticsManager(
                '/path/to/talkbank',
                new ExecutableService({ execSync: mockExecSync }),
            );

            // Act
            const result = await manager.getCacheStatistics();

            // Assert
            expect(result).toBeNull();
        });

        it('should handle empty corpora array', async () => {
            // Arrange
            const mockResponse: CacheStatsResponse = {
                statistics: {
                    totalEntries: 0,
                    roundtripTested: 0,
                    cacheDir: '/cache',
                    cacheSizeBytes: 0,
                    lastModified: '2026-01-17T10:00:00Z'
                },
                corpora: []
            };

            mockExecSync = createCommandSpecificMock({
                'cache stats --json': {
                    stdout: JSON.stringify(mockResponse)
                }
            });

            const manager = new CacheStatisticsManager(
                '/path/to/talkbank',
                new ExecutableService({ execSync: mockExecSync }),
            );

            // Act
            const result = await manager.getCacheStatistics();

            // Assert
            expect(result).not.toBeNull();
            expect(result!.corpora).toHaveLength(0);
        });

        it('should convert snake_case to camelCase', async () => {
            // Arrange
            const mockResponse = {
                statistics: {
                    total_entries: 100,  // snake_case
                    roundtrip_tested: 50,
                    cache_dir: '/cache',
                    cache_size_bytes: 1024,
                    last_modified: '2026-01-17T10:00:00Z'
                },
                corpora: [{
                    corpus_id: 1,
                    name: 'Test',
                    path: '/test',
                    file_count: 10,
                    passed: 8,
                    failed: 2,
                    not_tested: 0
                }]
            };

            mockExecSync = createCommandSpecificMock({
                'cache stats --json': {
                    stdout: JSON.stringify(mockResponse)
                }
            });

            const manager = new CacheStatisticsManager(
                '/path/to/talkbank',
                new ExecutableService({ execSync: mockExecSync }),
            );

            // Act
            const result = await manager.getCacheStatistics();

            // Assert
            expect(result).not.toBeNull();
            expect(result!.statistics.totalEntries).toBe(100);  // camelCase
            expect(result!.statistics.roundtripTested).toBe(50);
            expect(result!.statistics.cacheSizeBytes).toBe(1024);
            expect(result!.corpora[0].corpusId).toBe(1);
            expect(result!.corpora[0].fileCount).toBe(10);
            expect(result!.corpora[0].notTested).toBe(0);
        });
    });

    describe('formatBytes', () => {
        it('should format bytes as human-readable string', () => {
            expect(CacheStatisticsManager.formatBytes(0)).toBe('0 B');
            expect(CacheStatisticsManager.formatBytes(1023)).toBe('1023 B');
            expect(CacheStatisticsManager.formatBytes(1024)).toBe('1.0 KB');
            expect(CacheStatisticsManager.formatBytes(1536)).toBe('1.5 KB');
            expect(CacheStatisticsManager.formatBytes(1048576)).toBe('1.0 MB');
            expect(CacheStatisticsManager.formatBytes(5242880)).toBe('5.0 MB');
            expect(CacheStatisticsManager.formatBytes(1073741824)).toBe('1.0 GB');
        });
    });

    describe('calculateHitRate', () => {
        it('should calculate percentage correctly', () => {
            expect(CacheStatisticsManager.calculateHitRate(0, 100)).toBe(0);
            expect(CacheStatisticsManager.calculateHitRate(50, 100)).toBe(50);
            expect(CacheStatisticsManager.calculateHitRate(100, 100)).toBe(100);
            expect(CacheStatisticsManager.calculateHitRate(33, 100)).toBe(33);
        });

        it('should handle zero total', () => {
            expect(CacheStatisticsManager.calculateHitRate(0, 0)).toBe(0);
        });

        it('should round to 1 decimal place', () => {
            expect(CacheStatisticsManager.calculateHitRate(1, 3)).toBeCloseTo(33.3, 1);
            expect(CacheStatisticsManager.calculateHitRate(2, 3)).toBeCloseTo(66.7, 1);
        });
    });
});

describe('Property-based tests for cache statistics', () => {
    it('should never throw errors when parsing', () => {
        fc.assert(
            fc.asyncProperty(
                fc.string(),
                async (randomString) => {
                    const mockExecSync = createCommandSpecificMock({
                        'cache stats --json': {
                            stdout: randomString
                        }
                    });

                    const manager = new CacheStatisticsManager(
                        '/path/to/talkbank',
                        new ExecutableService({ execSync: mockExecSync }),
                    );
                    const result = await manager.getCacheStatistics();

                    // Should return null or valid result, never throw
                    expect(result === null || typeof result === 'object').toBe(true);

                    if (result !== null) {
                        expect(result).toHaveProperty('statistics');
                        expect(result).toHaveProperty('corpora');
                    }
                }
            ),
            { numRuns: 50 }
        );
    });

    it('should handle any byte value', () => {
        fc.assert(
            fc.property(
                fc.nat(),
                (bytes) => {
                    const formatted = CacheStatisticsManager.formatBytes(bytes);

                    // Should always return a string
                    expect(typeof formatted).toBe('string');

                    // Should contain a number and unit
                    expect(formatted).toMatch(/^\d+(\.\d+)?\s+[KMGT]?B$/);
                }
            ),
            { numRuns: 100 }
        );
    });
});
