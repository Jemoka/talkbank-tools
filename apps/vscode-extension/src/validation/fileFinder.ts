/**
 * Filesystem discovery helpers for the validation explorer.
 */

import * as fs from 'fs';
import * as path from 'path';

/**
 * One directory entry shown by the validation explorer tree.
 */
export interface ValidationDirectoryEntry {
    name: string;
    fullPath: string;
    itemType: 'directory' | 'file';
}

interface DirentLike {
    name: string;
    isDirectory(): boolean;
    isFile(): boolean;
}

/**
 * Minimal filesystem contract used by `FileFinder`.
 */
export interface FileFinderFs {
    readdirSync(dirPath: string, options: { withFileTypes: true }): DirentLike[];
}

/**
 * Lists tree-visible entries and recursively discovers `.cha` files.
 */
export class FileFinder {
    private readonly fsOps: FileFinderFs;

    constructor(fsOps: FileFinderFs = fs) {
        this.fsOps = fsOps;
    }

    /**
     * Get visible subdirectories and `.cha` files for one directory.
     */
    async getDirectoryEntries(dirPath: string): Promise<ValidationDirectoryEntry[]> {
        try {
            const entries = this.fsOps.readdirSync(dirPath, { withFileTypes: true });
            const items: ValidationDirectoryEntry[] = [];

            for (const entry of entries) {
                if (entry.isDirectory() && !entry.name.startsWith('.')) {
                    items.push({
                        name: entry.name,
                        fullPath: path.join(dirPath, entry.name),
                        itemType: 'directory'
                    });
                }
            }

            for (const entry of entries) {
                if (entry.isFile() && entry.name.endsWith('.cha')) {
                    items.push({
                        name: entry.name,
                        fullPath: path.join(dirPath, entry.name),
                        itemType: 'file'
                    });
                }
            }

            return items.sort(compareDirectoryEntries);
        } catch (error) {
            console.error(`Error reading directory ${dirPath}:`, error);
            return [];
        }
    }

    /**
     * Recursively collect `.cha` files under a directory.
     */
    async findChatFiles(dirPath: string): Promise<string[]> {
        const files: string[] = [];

        const traverse = (dir: string) => {
            try {
                const entries = this.fsOps.readdirSync(dir, { withFileTypes: true });
                for (const entry of entries) {
                    if (entry.isDirectory() && !entry.name.startsWith('.')) {
                        traverse(path.join(dir, entry.name));
                    } else if (entry.isFile() && entry.name.endsWith('.cha')) {
                        files.push(path.join(dir, entry.name));
                    }
                }
            } catch (error) {
                console.error(`Error traversing ${dir}:`, error);
            }
        };

        traverse(dirPath);
        return files.sort();
    }
}

function compareDirectoryEntries(
    a: ValidationDirectoryEntry,
    b: ValidationDirectoryEntry
): number {
    if (a.itemType !== b.itemType) {
        return a.itemType === 'directory' ? -1 : 1;
    }

    return a.name.localeCompare(b.name);
}
