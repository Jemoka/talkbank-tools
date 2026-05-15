import * as fs from 'fs';
import * as path from 'path';

import { CacheStatisticsManager } from './models/cacheStatistics';
import type { ExtensionRuntimeServices } from './runtimeServices';
import { findProjectRoot, type ProjectSearchResult } from './utils/projectLocator';

type Awaitable<T> = T | PromiseLike<T>;

/**
 * Cache statistics retrieved from the cache database
 */
interface CacheStats {
    cacheSize: string;  // e.g., "5.2 MB"
    totalEntries: number;
    validEntries: number;
    invalidEntries: number;
    expiredEntries: number;
    cacheHitRate: number; // percentage
    lastUpdated: string;  // ISO date string
}

export interface CacheManagerStatusBarItem {
    text: string;
    tooltip?: unknown;
    command?: unknown;
    show(...args: any[]): unknown;
    dispose(...args: any[]): unknown;
}

export interface CacheManagerWindow {
    showInformationMessage(message: string): Awaitable<unknown>;
    showWarningMessage(message: string, ...items: string[]): Awaitable<string | undefined>;
    showErrorMessage(message: string): Awaitable<unknown>;
}

export interface CacheManagerFs {
    existsSync(path: string): boolean;
    unlinkSync(path: string): void;
}

export interface CacheManagerDependencies extends ExtensionRuntimeServices {
    readonly window: CacheManagerWindow;
    readonly statusBarItem: CacheManagerStatusBarItem;
    readonly fsOps?: CacheManagerFs;
    readonly findProjectRootFn?: (
        options: { startPath: string }
    ) => Promise<ProjectSearchResult>;
}

/**
 * Manages the validation cache
 */
export class CacheManager {
    private readonly window: CacheManagerWindow;
    private readonly statusBarItem: CacheManagerStatusBarItem;
    private readonly fsOps: CacheManagerFs;
    private readonly findProjectRootFn: (
        options: { startPath: string }
    ) => Promise<ProjectSearchResult>;
    private cacheStatsRefreshInterval: NodeJS.Timeout | null = null;
    private projectRoot: string | undefined;

    constructor(
        private readonly dependencies: CacheManagerDependencies,
    ) {
        this.window = dependencies.window;
        this.statusBarItem = dependencies.statusBarItem;
        this.fsOps = dependencies.fsOps ?? fs;
        this.findProjectRootFn = dependencies.findProjectRootFn ?? findProjectRoot;
    }

    /**
     * Start status-bar updates and project-root discovery.
     */
    initialize(): void {
        if (this.cacheStatsRefreshInterval !== null) {
            return;
        }

        this.statusBarItem.command = 'talkbank.viewCacheStatistics';
        this.statusBarItem.tooltip = 'Click to view cache statistics';
        this.statusBarItem.show();

        void this.findProjectRootDir();
        this.startRefreshingStats();
    }

    /**
     * Find the project root by looking for Cargo.toml
     */
    private async findProjectRootDir(): Promise<void> {
        const workspaceRoot = this.dependencies.runtimeContext.getPrimaryWorkspaceRoot();
        if (!workspaceRoot) {
            return;
        }

        const result = await this.findProjectRootFn({
            startPath: workspaceRoot
        });

        if (result.found && result.path) {
            this.projectRoot = result.path;
        }
    }

    /**
     * Start auto-refreshing cache stats
     */
    private startRefreshingStats(): void {
        void this.updateCacheStats();
        this.cacheStatsRefreshInterval = setInterval(() => {
            void this.updateCacheStats();
        }, 5000);
    }

    /**
     * Update cache statistics in the status bar
     */
    private async updateCacheStats(): Promise<void> {
        try {
            const stats = await this.getCacheStats();
            if (stats) {
                this.statusBarItem.text = `$(database) Cache: ${stats.totalEntries} files`;
            } else {
                this.statusBarItem.text = '$(database) Cache: Initializing...';
            }
        } catch (error) {
            console.error('Error updating cache stats:', error);
            this.statusBarItem.text = '$(database) Cache: Error';
        }
    }

    /**
     * Get cache statistics from the unified cache database
     */
    private async getCacheStats(): Promise<CacheStats | null> {
        try {
            const cliPath = await this.findTalkBankCliPath();
            if (!cliPath) {
                return null;
            }

            const manager = new CacheStatisticsManager(
                cliPath,
                this.dependencies.executableService,
            );
            const response = await manager.getCacheStatistics();

            if (!response) {
                return null;
            }

            const stats = response.statistics;

            return {
                cacheSize: CacheStatisticsManager.formatBytes(stats.cacheSizeBytes),
                totalEntries: stats.totalEntries,
                validEntries: 0,
                invalidEntries: 0,
                expiredEntries: 0,
                cacheHitRate: 0,
                lastUpdated: stats.lastModified
            };
        } catch (error) {
            console.error('Error getting cache stats:', error);
            return null;
        }
    }

    /**
     * View detailed cache statistics
     */
    async viewCacheStatistics(): Promise<void> {
        const stats = await this.getCacheStats();

        if (!stats) {
            await this.window.showInformationMessage(
                'Cache not initialized yet. Validate some files first.',
            );
            return;
        }

        const message = `
Cache Statistics
================

Cache Size: ${stats.cacheSize}
Total Entries: ${stats.totalEntries}
Valid Entries: ${stats.validEntries}
Invalid Entries: ${stats.invalidEntries}
Expired Entries: ${stats.expiredEntries}
Cache Hit Rate: ${stats.cacheHitRate.toFixed(1)}%

Last Updated: ${new Date(stats.lastUpdated).toLocaleString()}
`.trim();

        await this.window.showInformationMessage(message);
    }

    /**
     * Clear entire validation cache
     */
    async clearCache(): Promise<void> {
        const selection = await this.window.showWarningMessage(
            'Clear all validation cache?',
            'Clear All',
            'Cancel'
        );

        if (selection !== 'Clear All') {
            return;
        }

        try {
            const cachePath = this.dependencies.runtimeContext.getCachePath();
            if (this.fsOps.existsSync(cachePath)) {
                this.fsOps.unlinkSync(cachePath);
                await this.window.showInformationMessage('Validation cache cleared.');
                void this.updateCacheStats();
            } else {
                await this.window.showInformationMessage('Cache file not found.');
            }
        } catch (error) {
            await this.window.showErrorMessage(`Failed to clear cache: ${error}`);
        }
    }

    /**
     * Clear cache for a specific file or directory
     */
    async clearCacheForPath(itemPath: string): Promise<void> {
        try {
            await this.window.showInformationMessage(
                `Cache clearing for ${path.basename(itemPath)} would require database query. ` +
                'For now, use "Clear All Cache" from the command palette.'
            );
        } catch (error) {
            await this.window.showErrorMessage(`Failed to clear cache: ${error}`);
        }
    }

    /**
     * Find the talkbank CLI binary
     */
    private async findTalkBankCliPath(): Promise<string | null> {
        return await this.dependencies.executableService.findTalkBankCli({
            projectRoot: this.projectRoot
        });
    }

    /**
     * Dispose resources
     */
    dispose(): void {
        if (this.cacheStatsRefreshInterval) {
            clearInterval(this.cacheStatsRefreshInterval);
            this.cacheStatsRefreshInterval = null;
        }
        this.statusBarItem.dispose();
    }
}
