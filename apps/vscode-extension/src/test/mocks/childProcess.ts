/**
 * Mock implementation of child_process for testing CLI calls.
 *
 * Provides utilities to mock execSync calls used to invoke talkbank CLI.
 */

import { vi } from 'vitest';

/**
 * Options for mocking execSync behavior.
 */
export interface MockExecSyncOptions {
    /** Standard output to return */
    stdout?: string;
    /** Standard error to write */
    stderr?: string;
    /** Whether the command should throw an error */
    shouldThrow?: boolean;
    /** Error message if shouldThrow is true */
    errorMessage?: string;
    /** Exit code for the error */
    exitCode?: number;
}

/**
 * Creates a mock for child_process.execSync.
 *
 * This function returns a Vitest mock function that simulates execSync behavior,
 * allowing tests to control CLI command responses without spawning processes.
 *
 * @param options - Configuration for mock behavior
 * @returns Vitest mock function that simulates execSync
 *
 * @example
 * ```typescript
 * // Success case
 * const execSync = createMockExecSync({
 *   stdout: '{"total_entries": 100}'
 * });
 * const result = execSync('talkbank cache stats --json');
 * expect(result.toString()).toBe('{"total_entries": 100}');
 *
 * // Error case
 * const execSync = createMockExecSync({
 *   shouldThrow: true,
 *   errorMessage: 'Command failed',
 *   exitCode: 1
 * });
 * expect(() => execSync('bad-command')).toThrow('Command failed');
 * ```
 */
export function createMockExecSync(options: MockExecSyncOptions = {}) {
    const {
        stdout = '',
        stderr = '',
        shouldThrow = false,
        errorMessage = 'Command execution failed',
        exitCode = 1
    } = options;

    return vi.fn((command: string, execOptions?: any) => {
        if (shouldThrow) {
            const error: any = new Error(errorMessage);
            error.status = exitCode;
            error.stderr = Buffer.from(stderr);
            error.stdout = Buffer.from('');
            throw error;
        }

        // Return stdout as Buffer (matching real execSync behavior)
        return Buffer.from(stdout);
    });
}

/**
 * Creates a command-specific mock for execSync that responds differently
 * based on the command string.
 *
 * @param commandMap - Map of command patterns to mock responses
 * @returns Vitest mock function that matches commands
 *
 * @example
 * ```typescript
 * const execSync = createCommandSpecificMock({
 *   'cache stats --json': { stdout: '{"total_entries": 100}' },
 *   'cache clear --all': { stdout: 'Cache cleared' },
 *   'invalid': { shouldThrow: true, errorMessage: 'Not found' }
 * });
 *
 * const stats = execSync('talkbank cache stats --json');
 * expect(stats.toString()).toContain('total_entries');
 * ```
 */
export function createCommandSpecificMock(
    commandMap: Record<string, MockExecSyncOptions>
) {
    return vi.fn((command: string, execOptions?: any) => {
        // Find matching command in map (supports partial matches)
        for (const [pattern, options] of Object.entries(commandMap)) {
            if (command.includes(pattern)) {
                if (options.shouldThrow) {
                    const error: any = new Error(options.errorMessage || 'Command failed');
                    error.status = options.exitCode || 1;
                    error.stderr = Buffer.from(options.stderr || '');
                    error.stdout = Buffer.from('');
                    throw error;
                }
                return Buffer.from(options.stdout || '');
            }
        }

        // No match - throw error
        const error: any = new Error(`Unexpected command: ${command}`);
        error.status = 127;
        throw error;
    });
}

/**
 * Mock for filesystem operations that CLI might perform.
 */
export interface MockFsOperations {
    existsSync: (path: string) => boolean;
    readFileSync: (path: string) => string | Buffer;
    statSync: (path: string) => { isFile: () => boolean; isDirectory: () => boolean; size: number };
}

/**
 * Creates a mock filesystem for testing file-based operations.
 *
 * @param files - Map of file paths to file contents
 * @returns Mock fs operations
 *
 * @example
 * ```typescript
 * const fs = createMockFileSystem({
 *   '/path/to/talkbank': 'binary',
 *   '/path/to/Cargo.toml': '[package]\nname = "chatter-cli"'
 * });
 *
 * expect(fs.existsSync('/path/to/talkbank')).toBe(true);
 * expect(fs.existsSync('/nonexistent')).toBe(false);
 * ```
 */
export function createMockFileSystem(
    files: Record<string, string | Buffer> = {}
): MockFsOperations {
    return {
        existsSync: vi.fn((path: string) => path in files),
        readFileSync: vi.fn((path: string) => {
            if (path in files) {
                return files[path];
            }
            throw new Error(`ENOENT: no such file or directory, open '${path}'`);
        }),
        statSync: vi.fn((path: string) => {
            if (path in files) {
                return {
                    isFile: () => true,
                    isDirectory: () => false,
                    size: files[path].toString().length
                };
            }
            throw new Error(`ENOENT: no such file or directory, stat '${path}'`);
        })
    };
}
