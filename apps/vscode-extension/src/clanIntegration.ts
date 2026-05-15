import * as path from 'path';

import type { ExtensionRuntimeServices } from './runtimeServices';

type Awaitable<T> = T | PromiseLike<T>;

export interface ClanIntegrationWindow {
    showInformationMessage(message: string): Awaitable<unknown>;
    showErrorMessage(message: string): Awaitable<unknown>;
}

export interface ClanIntegrationDependencies extends ExtensionRuntimeServices {
    readonly window: ClanIntegrationWindow;
}

/**
 * CLAN integration for opening files in the CLAN application
 *
 * **Uses send2clan FFI bindings** (from send2clan-sys crate) to programmatically
 * open CHAT files in CLAN with cursor positioning via platform-standard IPC:
 * - macOS: Apple Events
 * - Windows: Windows messages
 * - Linux: Not supported (CLAN doesn't run natively)
 *
 * Requires `send2clan`; when it is unavailable we surface a build/install error.
 * CLAN must be installed separately - this extension only opens files, doesn't require CLAN API access.
 *
 * **Optional fallback feature**: CLAN integration is NOT required for validation workflows.
 * The primary validation path uses VS Code native file navigation (instant, no external process).
 * This "Open in CLAN" option is for users who need CLAN-specific analysis features.
 */
export class ClanIntegration {
    constructor(
        private readonly dependencies: ClanIntegrationDependencies,
    ) {}

    /**
     * Open a CHAT file in CLAN at a specific line/column using send2clan FFI
     */
    async openInClan(filePath: string, line?: number, column?: number): Promise<void> {
        try {
            const send2clanBinary = await this.dependencies.executableService.findSend2ClanBinary(
                this.dependencies.runtimeContext.getPrimaryWorkspaceRoot()
            );
            if (!send2clanBinary) {
                await this.dependencies.window.showErrorMessage(
                    'send2clan binary not found. Please build the project: cargo build -p send2clan-sys'
                );
                return;
            }

            const args = [filePath];
            if (line !== undefined) {
                args.push(String(line));
                if (column !== undefined) {
                    args.push(String(column));
                }
            }

            this.dependencies.executableService.spawnDetached(send2clanBinary, args);
            await this.dependencies.window.showInformationMessage(
                `Opening ${path.basename(filePath)} in CLAN...`
            );
        } catch (error) {
            await this.dependencies.window.showErrorMessage(
                `Failed to open file in CLAN via send2clan: ${
                    error instanceof Error ? error.message : String(error)
                }`
            );
        }
    }
}
