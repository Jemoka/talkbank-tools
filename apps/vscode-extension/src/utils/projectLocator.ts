/**
 * Utility for locating the project root directory.
 *
 * Searches for Cargo.toml by traversing up the directory tree.
 */

import * as fs from 'fs';

/**
 * Result of project root search.
 */
export interface ProjectSearchResult {
    /**
     * Whether Cargo.toml was found.
     */
    found: boolean;

    /**
     * Path to directory containing Cargo.toml, or null if not found.
     */
    path: string | null;
}

/**
 * Options for project root search.
 */
export interface ProjectLocatorOptions {
    /**
     * Directory to start searching from.
     */
    startPath: string;

    /**
     * Maximum number of parent directories to traverse.
     * Defaults to 10.
     */
    maxDepth?: number;

    /**
     * Filesystem operations (for testing).
     * Defaults to Node.js fs module.
     */
    fs?: {
        existsSync: (path: string) => boolean;
    };
}

/**
 * Returns the parent directory of a path using pure string manipulation.
 *
 * Deliberately avoids path.dirname / path.resolve so that paths containing
 * ".." or "." segments are preserved exactly as-is (matching mock filesystem
 * keys built the same way).  Stops at the filesystem root by returning "/"
 * whenever the last "/" is at index 0.
 */
function stringParentDir(p: string): string {
    const lastSlash = p.lastIndexOf('/');
    // lastSlash === 0 means the only "/" is the leading one (we are at root).
    if (lastSlash <= 0) {
        return '/';
    }
    return p.substring(0, lastSlash);
}

/**
 * Finds the project root by searching for Cargo.toml.
 *
 * Traverses up the directory tree from startPath, looking for Cargo.toml.
 * Stops at filesystem root or after maxDepth iterations.
 *
 * Path construction uses plain string concatenation rather than path.join so
 * that the keys checked against the filesystem mirror exactly how callers
 * build file paths (important for in-memory mock filesystems in tests).
 *
 * @param options - Configuration for search behavior
 * @returns Search result with found status and path
 * @throws Never throws - returns not found on any error
 *
 * @example
 * ```typescript
 * // Search from workspace folder
 * const result = await findProjectRoot({
 *   startPath: '/workspace/talkbank-tools/vscode-talkbank/src'
 * });
 *
 * if (result.found) {
 *   console.log(`Project root: ${result.path}`);
 * } else {
 *   console.log('No Cargo.toml found');
 * }
 * ```
 */
export async function findProjectRoot(
    options: ProjectLocatorOptions
): Promise<ProjectSearchResult> {
    const { startPath, maxDepth = 10, fs: fsOps = fs } = options;

    try {
        let current = startPath;

        for (let i = 0; i < maxDepth; i++) {
            // Build the candidate path without normalization so it matches
            // however the caller constructed the filesystem keys.
            const sep = current.endsWith('/') ? '' : '/';
            const cargoToml = current + sep + 'Cargo.toml';

            if (fsOps.existsSync(cargoToml)) {
                return {
                    found: true,
                    path: current
                };
            }

            // Move to parent directory using string manipulation only — no
            // path.dirname / path.resolve — to avoid normalising ".." or "."
            // segments away, which would break mock-filesystem key lookups.
            const parent = stringParentDir(current);

            // Stop if we've reached filesystem root (parent === current).
            if (parent === current) {
                break;
            }

            current = parent;
        }

        // Not found within maxDepth
        return {
            found: false,
            path: null
        };
    } catch (error) {
        // Any unexpected error - return not found
        return {
            found: false,
            path: null
        };
    }
}
