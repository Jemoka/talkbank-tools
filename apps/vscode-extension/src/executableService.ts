import { execSync, spawn, type SpawnOptions } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import type * as vscode from 'vscode';

/**
 * Return the on-disk filename of `binaryName` for the current platform — adds
 * `.exe` on Windows. Used wherever a single platform-native binary is looked up
 * (the dev-tree both-suffix search intentionally does not use this).
 */
function platformBinaryName(binaryName: string): string {
    return process.platform === 'win32' ? `${binaryName}.exe` : binaryName;
}

export type ExecSyncFn = (command: string, options: any) => Buffer | string;
export type SpawnFn = (
    command: string,
    args: readonly string[],
    options: SpawnOptions,
) => {
    unref: () => void;
};

export interface ExecutableServiceOptions {
    fs?: {
        existsSync: (path: string) => boolean;
    };
    execSync?: ExecSyncFn;
    spawn?: SpawnFn;
}

export interface TalkBankCliLocatorOptions {
    projectRoot?: string;
}

export interface TalkBankCliExecutableService {
    findTalkBankCli(options?: TalkBankCliLocatorOptions): Promise<string | null>;
    runTalkBankCli(cliPath: string, args: string[]): string;
}

export interface ClanExecutableService {
    findSend2ClanBinary(startPath?: string | null): Promise<string | null>;
    spawnDetached(command: string, args: string[]): void;
}

export interface LspExecutableService {
    findTalkbankLspBinary(context: vscode.ExtensionContext, configuredPath?: string): string | null;
}

/**
 * Shared boundary for all external executable discovery and invocation used by
 * the VS Code extension runtime.
 */
export class ExecutableService implements
    TalkBankCliExecutableService,
    ClanExecutableService,
    LspExecutableService
{
    private readonly fsOps: {
        existsSync: (path: string) => boolean;
    };

    private readonly execSyncFn: ExecSyncFn;
    private readonly spawnFn: SpawnFn;

    constructor(options: ExecutableServiceOptions = {}) {
        this.fsOps = options.fs ?? fs;
        this.execSyncFn = options.execSync ?? execSync;
        this.spawnFn = options.spawn ?? ((command, args, spawnOptions) =>
            spawn(command, args, spawnOptions));
    }

    async findTalkBankCli(
        options: TalkBankCliLocatorOptions = {},
    ): Promise<string | null> {
        const { projectRoot } = options;

        if (projectRoot) {
            const localBuild = this.findTargetBinary(projectRoot, 'chatter');
            if (localBuild) {
                return localBuild;
            }
        }

        return this.findOnPath('chatter');
    }

    runTalkBankCli(cliPath: string, args: string[]): string {
        const command = [this.quoteExecutable(cliPath), ...args.map(arg => this.quoteArgument(arg))]
            .join(' ');
        const output = this.execSyncFn(command, {
            encoding: 'utf-8',
            stdio: ['pipe', 'pipe', 'pipe'],
        });
        return output.toString().trim();
    }

    findTalkbankLspBinary(context: vscode.ExtensionContext, configuredPath = ''): string | null {
        if (configuredPath && this.fsOps.existsSync(configuredPath)) {
            return configuredPath;
        }

        const bundled = this.findBundledBinary(context, 'chatter-lsp');
        if (bundled) {
            return bundled;
        }

        const pathBinary = this.findOnPath('chatter-lsp');
        if (pathBinary && this.fsOps.existsSync(pathBinary)) {
            return pathBinary;
        }

        const localBuild = this.findExtensionTargetBinary(context, 'chatter-lsp');
        if (localBuild) {
            return localBuild;
        }

        return null;
    }

    async findSend2ClanBinary(startPath?: string | null): Promise<string | null> {
        const pathBinary = this.findOnPath('send2clan');
        if (pathBinary && this.fsOps.existsSync(pathBinary)) {
            return pathBinary;
        }

        if (!startPath) {
            return null;
        }

        let current = startPath;
        for (let depth = 0; depth < 10; depth += 1) {
            const candidate = this.findTargetBinary(current, 'send2clan');
            if (candidate) {
                return candidate;
            }

            const parent = path.dirname(current);
            if (parent === current) {
                break;
            }
            current = parent;
        }

        return null;
    }

    spawnDetached(command: string, args: string[]): void {
        this.spawnFn(command, args, {
            detached: true,
            stdio: 'ignore',
        }).unref();
    }

    private findOnPath(command: string): string | null {
        // Use platform-appropriate path discovery: `where.exe` on Windows,
        // `which` on Unix. Both return the full path to the executable.
        const whichCommand = process.platform === 'win32'
            ? `where.exe ${command}`
            : `which ${command}`;
        try {
            const result = this.execSyncFn(whichCommand, {
                encoding: 'utf-8',
                stdio: ['pipe', 'pipe', 'pipe'],
            });
            // `where.exe` may return multiple lines; take the first match.
            const found = result.toString().trim().split(/\r?\n/)[0];
            return found || null;
        } catch {
            return null;
        }
    }

    private findTargetBinary(root: string, binaryName: string): string | null {
        for (const profile of ['debug', 'release']) {
            for (const candidateName of [binaryName, `${binaryName}.exe`]) {
                const candidate = path.join(root, 'target', profile, candidateName);
                if (this.fsOps.existsSync(candidate)) {
                    return candidate;
                }
            }
        }

        return null;
    }

    private findExtensionTargetBinary(
        context: vscode.ExtensionContext,
        binaryName: string,
    ): string | null {
        for (const profile of ['debug', 'release']) {
            for (const candidateName of [binaryName, `${binaryName}.exe`]) {
                const candidate = context.asAbsolutePath(
                    path.join('..', 'target', profile, candidateName),
                );
                if (this.fsOps.existsSync(candidate)) {
                    return candidate;
                }
            }
        }

        return null;
    }

    /**
     * Look for a platform-specific binary shipped inside the installed VSIX at
     * `<extensionPath>/server/<binary>[.exe]`. Returns null when not present,
     * which is the expected case for the generic (non-platform) VSIX or a
     * dev-tree checkout that has not staged a binary.
     */
    private findBundledBinary(
        context: vscode.ExtensionContext,
        binaryName: string,
    ): string | null {
        const candidate = context.asAbsolutePath(
            path.join('server', platformBinaryName(binaryName)),
        );
        return this.fsOps.existsSync(candidate) ? candidate : null;
    }

    private quoteExecutable(value: string): string {
        return JSON.stringify(value);
    }

    private quoteArgument(value: string): string {
        if (value.startsWith('/') || value.includes('\\') || /\s|"/.test(value) || value === '') {
            return JSON.stringify(value);
        }
        return value;
    }
}
