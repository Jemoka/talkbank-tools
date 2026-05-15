/**
 * Utility for locating the talkbank CLI binary.
 *
 * Searches in project target directories (debug/release) first,
 * then falls back to system PATH.
 */

import {
    ExecutableService,
    type ExecutableServiceOptions,
    type TalkBankCliExecutableService,
} from '../executableService';

/**
 * Options for CLI location search.
 */
export interface CliLocatorOptions extends ExecutableServiceOptions {
    /**
     * Project root directory to search in.
     * If undefined, only searches PATH.
     */
    projectRoot?: string;
    /**
     * Optional shared executable service to avoid constructing a new runtime boundary.
     */
    executableService?: Pick<TalkBankCliExecutableService, 'findTalkBankCli'>;
}

/**
 * Finds the talkbank CLI binary.
 *
 * Search order:
 * 1. projectRoot/target/debug/talkbank (if projectRoot provided)
 * 2. projectRoot/target/release/talkbank (if projectRoot provided)
 * 3. System PATH (via `which talkbank`)
 *
 * @param options - Configuration for search behavior
 * @returns Path to talkbank binary, or null if not found
 * @throws Never throws - returns null on any error
 *
 * @example
 * ```typescript
 * // Search with project root
 * const cli = await findTalkBankCli({ projectRoot: '/workspace/talkbank-tools' });
 * if (cli) {
 *   console.log(`Found CLI at: ${cli}`);
 * }
 *
 * // Search only PATH
 * const cli = await findTalkBankCli({});
 * ```
 */
export async function findTalkBankCli(
    options: CliLocatorOptions = {}
): Promise<string | null> {
    const { projectRoot, executableService, ...serviceOptions } = options;
    const service = executableService ?? new ExecutableService(serviceOptions);
    return service.findTalkBankCli({ projectRoot });
}
