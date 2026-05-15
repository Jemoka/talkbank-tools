import {
    ExecutableService,
    type ClanExecutableService,
    type LspExecutableService,
    type TalkBankCliExecutableService,
} from './executableService';
import {
    DefaultExtensionRuntimeContext,
    type ExtensionRuntimeContext,
    type RuntimeWorkspaceFacade,
} from './runtimeContext';

/**
 * Shared executable boundary used across activation-time services.
 */
export type ExtensionExecutableService =
    TalkBankCliExecutableService & ClanExecutableService & LspExecutableService;

/**
 * Shared runtime services injected into activation-time features.
 */
export interface ExtensionRuntimeServices {
    readonly executableService: ExtensionExecutableService;
    readonly runtimeContext: ExtensionRuntimeContext;
}

/**
 * Create the shared runtime-service pair once during extension activation.
 */
export function createExtensionRuntimeServices(
    workspace: RuntimeWorkspaceFacade,
    executableService: ExtensionExecutableService = new ExecutableService(),
): ExtensionRuntimeServices {
    return {
        executableService,
        runtimeContext: new DefaultExtensionRuntimeContext({
            workspace,
        }),
    };
}
