import type { LanguageClient } from 'vscode-languageclient/node';

import type { CacheManager } from '../../cacheManager';
import type { ClanIntegration } from '../../clanIntegration';
import type { TalkbankExecuteCommandClient } from '../../lsp/executeCommandClient';
import type { ExtensionRuntimeServices } from '../../runtimeServices';

/**
 * Shared extension services passed into feature registrars.
 *
 * This aggregate remains the current activation-time bridge into the
 * Effect-based service Layer in `effectRuntime.ts`.
 */
export interface ExtensionServices extends ExtensionRuntimeServices {
    /**
     * Raw language client for panels and direct LSP integrations that still need it.
     */
    client: LanguageClient;
    /**
     * Typed execute-command facade used by extension features.
     */
    commands: TalkbankExecuteCommandClient;
    /**
     * Cache manager service.
     */
    cacheManager: CacheManager;
    /**
     * CLAN integration service.
     */
    clanIntegration: ClanIntegration;
    /**
     * Shared runtime context for workspace/config/path lookups.
     */
    runtimeContext: ExtensionRuntimeServices['runtimeContext'];
}
