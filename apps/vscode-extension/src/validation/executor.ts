/**
 * Validation and cache-clear CLI execution for the validation explorer.
 */

import type { TalkBankCliExecutableService } from '../executableService';
import { findProjectRoot, type ProjectSearchResult } from '../utils/projectLocator';

/**
 * Validation status for a file.
 */
export enum ValidationStatus {
    Valid = 'valid',
    Invalid = 'invalid',
    Validating = 'validating',
    Cached = 'cached',
    ParseError = 'parse_error',
    ReadError = 'read_error'
}

/**
 * One validation issue returned by the CLI JSON output.
 */
export interface FileValidationIssue {
    code: string;
    severity: string;
    message: string;
    line?: number;
    column?: number;
}

/**
 * Validation result for one CHAT file.
 */
export interface FileValidationResult {
    path: string;
    status: ValidationStatus;
    errorCount?: number;
    isCacheHit?: boolean;
    errors?: FileValidationIssue[];
}

/**
 * Executor configuration and test seams.
 */
export interface ValidationExecutorOptions {
    workspaceRoot?: string;
    executableService: TalkBankCliExecutableService;
    locateCli?: () => Promise<string | null>;
    findProjectRoot?: (options: { startPath: string }) => Promise<ProjectSearchResult>;
}

interface ValidationCliResponse {
    status: string;
    errors?: FileValidationIssue[];
}

/**
 * Convert CLI JSON output into the explorer's validation-result shape.
 */
export function parseValidationResult(
    filePath: string,
    output: string
): FileValidationResult {
    const result = JSON.parse(output) as ValidationCliResponse;

    if (result.status === 'valid') {
        return {
            path: filePath,
            status: ValidationStatus.Valid,
            isCacheHit: false
        };
    }

    if (result.status === 'invalid') {
        return {
            path: filePath,
            status: ValidationStatus.Invalid,
            errorCount: result.errors ? result.errors.length : 0,
            errors: result.errors || []
        };
    }

    if (result.status === 'parse_error') {
        return {
            path: filePath,
            status: ValidationStatus.ParseError
        };
    }

    if (result.status === 'read_error') {
        return {
            path: filePath,
            status: ValidationStatus.ReadError
        };
    }

    throw new Error(`Unknown validation status: ${result.status}`);
}

/**
 * Runs validation-explorer CLI commands while keeping binary discovery local.
 */
export class ValidationExecutor {
    private readonly workspaceRoot: string | undefined;
    private readonly executableService: TalkBankCliExecutableService;
    private readonly locateCli?: () => Promise<string | null>;
    private readonly findProjectRootFn: (
        options: { startPath: string }
    ) => Promise<ProjectSearchResult>;

    constructor(options: ValidationExecutorOptions) {
        this.workspaceRoot = options.workspaceRoot;
        this.executableService = options.executableService;
        this.locateCli = options.locateCli;
        this.findProjectRootFn = options.findProjectRoot ?? findProjectRoot;
    }

    /**
     * Validate one file through the TalkBank CLI.
     */
    async validateFile(filePath: string): Promise<FileValidationResult> {
        const cliPath = await this.resolveCliPath();
        const output = this.executableService.runTalkBankCli(cliPath, [
            'validate',
            filePath,
            '--json'
        ]);
        return parseValidationResult(filePath, output);
    }

    /**
     * Preview cache clear results for a path.
     */
    async previewCacheClear(targetPath: string): Promise<string> {
        const cliPath = await this.resolveCliPath();
        return this.executableService.runTalkBankCli(cliPath, [
            'cache',
            'clear',
            '--prefix',
            targetPath,
            '--dry-run'
        ]);
    }

    /**
     * Clear cached validation results for a path.
     */
    async clearCache(targetPath: string): Promise<string> {
        const cliPath = await this.resolveCliPath();
        return this.executableService.runTalkBankCli(cliPath, [
            'cache',
            'clear',
            '--prefix',
            targetPath
        ]);
    }

    private async resolveCliPath(): Promise<string> {
        const cliPath = this.locateCli
            ? await this.locateCli()
            : await this.findTalkBankCliPath();

        if (!cliPath) {
            throw new Error('chatter CLI not found. Please build the project first.');
        }

        return cliPath;
    }

    private async findTalkBankCliPath(): Promise<string | null> {
        const projectRoot = await this.findProjectRootDir();

        return await this.executableService.findTalkBankCli({
            projectRoot: projectRoot || undefined
        });
    }

    private async findProjectRootDir(): Promise<string | null> {
        if (!this.workspaceRoot) {
            return null;
        }

        const result = await this.findProjectRootFn({
            startPath: this.workspaceRoot
        });

        return result.found ? result.path : null;
    }
}
