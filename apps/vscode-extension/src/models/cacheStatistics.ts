/**
 * Cache statistics types and manager.
 *
 * Provides TypeScript interfaces for cache statistics returned by the
 * talkbank CLI `cache stats --json` command and a manager class for
 * querying and formatting these statistics.
 */

import type { TalkBankCliExecutableService } from '../executableService';

/**
 * Cache statistics from the database.
 *
 * Maps to Rust CacheStatistics from chatter-cli/src/commands/cache/stats.rs
 */
export interface CacheStatistics {
    /** Total number of cached file entries */
    totalEntries: number;

    /** Number of files that have been roundtrip tested */
    roundtripTested: number;

    /** Cache directory path */
    cacheDir: string;

    /** Cache database size in bytes */
    cacheSizeBytes: number;

    /** Last modification timestamp (ISO 8601) */
    lastModified: string;
}

/**
 * Per-corpus statistics from the cache.
 *
 * Maps to Rust CorpusStatistics from chatter-cli/src/commands/cache/stats.rs
 */
export interface CorpusStatistics {
    /** Corpus ID in the database */
    corpusId: number;

    /** Corpus name */
    name: string;

    /** Corpus root directory path */
    path: string;

    /** Total files in the corpus */
    fileCount: number;

    /** Files that passed validation */
    passed: number;

    /** Files that failed validation */
    failed: number;

    /** Files not yet tested */
    notTested: number;
}

/**
 * Complete response structure from `talkbank cache stats --json`.
 *
 * Maps to Rust CacheStatsResponse from chatter-cli/src/commands/cache/stats.rs
 */
export interface CacheStatsResponse {
    /** Overall cache statistics */
    statistics: CacheStatistics;

    /** Per-corpus breakdown */
    corpora: CorpusStatistics[];
}

/**
 * Manages cache statistics queries and formatting.
 *
 * @example
 * ```typescript
 * const manager = new CacheStatisticsManager('/path/to/talkbank');
 * const stats = await manager.getCacheStatistics();
 *
 * if (stats) {
 *   console.log(`Cache has ${stats.statistics.totalEntries} entries`);
 *   console.log(`Size: ${CacheStatisticsManager.formatBytes(stats.statistics.cacheSizeBytes)}`);
 * }
 * ```
 */
export class CacheStatisticsManager {
    /**
     * Creates a new cache statistics manager.
     *
     * @param cliPath - Path to the talkbank CLI binary
     * @param executableService - Shared executable service boundary
     */
    constructor(
        private cliPath: string,
        private readonly executableService: Pick<TalkBankCliExecutableService, 'runTalkBankCli'>,
    ) {}

    /**
     * Queries cache statistics via CLI --json output.
     *
     * Calls `talkbank cache stats --json` and parses the response.
     * Returns null if the CLI is not available, fails to execute, or
     * returns invalid JSON.
     *
     * @returns Parsed cache statistics, or null on error
     * @throws Never throws - returns null on any error
     *
     * @example
     * ```typescript
     * const stats = await manager.getCacheStatistics();
     * if (stats) {
     *   console.log(`Total entries: ${stats.statistics.totalEntries}`);
     *   for (const corpus of stats.corpora) {
     *     console.log(`${corpus.name}: ${corpus.passed}/${corpus.fileCount} passed`);
     *   }
     * } else {
     *   console.log('Failed to get cache statistics');
     * }
     * ```
     */
    async getCacheStatistics(): Promise<CacheStatsResponse | null> {
        try {
            const jsonOutput = this.executableService.runTalkBankCli(this.cliPath, [
                'cache',
                'stats',
                '--json'
            ]);

            // Parse JSON response
            return this.parseCacheStats(jsonOutput);
        } catch (error) {
            // CLI not found, execution failed, or other error
            return null;
        }
    }

    /**
     * Parses cache statistics JSON from CLI output.
     *
     * Converts snake_case from Rust to camelCase for TypeScript.
     *
     * @param jsonOutput - Raw JSON string from CLI
     * @returns Parsed statistics, or null if parsing fails
     * @throws Never throws - returns null on invalid input
     *
     * @example
     * ```typescript
     * const json = '{"statistics": {...}, "corpora": [...]}';
     * const stats = manager['parseCacheStats'](json);
     * ```
     */
    private parseCacheStats(jsonOutput: string): CacheStatsResponse | null {
        try {
            const raw = JSON.parse(jsonOutput);

            // Type guard validation
            if (!this.isValidCacheStatsResponse(raw)) {
                return null;
            }

            // Convert snake_case to camelCase, falling back to camelCase
            // if the key is already camelCase (e.g. in tests or future CLI versions).
            return {
                statistics: {
                    totalEntries: raw.statistics.totalEntries ?? raw.statistics.total_entries,
                    roundtripTested: raw.statistics.roundtripTested ?? raw.statistics.roundtrip_tested,
                    cacheDir: raw.statistics.cacheDir ?? raw.statistics.cache_dir,
                    cacheSizeBytes: raw.statistics.cacheSizeBytes ?? raw.statistics.cache_size_bytes,
                    lastModified: raw.statistics.lastModified ?? raw.statistics.last_modified
                },
                corpora: raw.corpora.map((c: any) => ({
                    corpusId: c.corpusId ?? c.corpus_id,
                    name: c.name,
                    path: c.path,
                    fileCount: c.fileCount ?? c.file_count,
                    passed: c.passed,
                    failed: c.failed,
                    notTested: c.notTested ?? c.not_tested
                }))
            };
        } catch {
            return null;
        }
    }

    /**
     * Type guard for runtime validation of cache stats response.
     *
     * @param obj - Object to validate
     * @returns True if object matches expected structure
     */
    private isValidCacheStatsResponse(obj: unknown): obj is any {
        if (typeof obj !== 'object' || obj === null) {
            return false;
        }

        const o = obj as any;

        return (
            'statistics' in o &&
            'corpora' in o &&
            typeof o.statistics === 'object' &&
            Array.isArray(o.corpora)
        );
    }

    /**
     * Formats byte count as human-readable string.
     *
     * @param bytes - Number of bytes
     * @returns Formatted string (e.g., "5.0 MB", "1.2 GB")
     *
     * @example
     * ```typescript
     * CacheStatisticsManager.formatBytes(0)         // "0 B"
     * CacheStatisticsManager.formatBytes(1024)      // "1.0 KB"
     * CacheStatisticsManager.formatBytes(1048576)   // "1.0 MB"
     * CacheStatisticsManager.formatBytes(5242880)   // "5.0 MB"
     * ```
     */
    static formatBytes(bytes: number): string {
        if (bytes === 0) return '0 B';

        const units = ['B', 'KB', 'MB', 'GB', 'TB'];
        const k = 1024;
        const i = Math.floor(Math.log(bytes) / Math.log(k));

        if (i === 0) {
            return `${bytes} B`;
        }

        return `${(bytes / Math.pow(k, i)).toFixed(1)} ${units[i]}`;
    }

    /**
     * Calculates hit rate percentage.
     *
     * @param hits - Number of hits
     * @param total - Total number of attempts
     * @returns Hit rate as percentage (0-100)
     *
     * @example
     * ```typescript
     * CacheStatisticsManager.calculateHitRate(50, 100)  // 50
     * CacheStatisticsManager.calculateHitRate(1, 3)     // 33.3
     * CacheStatisticsManager.calculateHitRate(0, 0)     // 0
     * ```
     */
    static calculateHitRate(hits: number, total: number): number {
        if (total === 0) return 0;
        return Math.round((hits / total) * 1000) / 10;  // Round to 1 decimal place
    }
}
