/**
 * Validation-explorer activation helpers.
 */

import type * as vscode from 'vscode';

import type { ExtensionRuntimeServices } from '../runtimeServices';
import { ValidationExecutor } from '../validation/executor';
import { FileFinder } from '../validation/fileFinder';
import {
    ValidationExplorerProvider,
    type ValidationExplorerWindow,
} from '../validationExplorer';

export interface ValidationExplorerWindowHost extends ValidationExplorerWindow {
    createTreeView<T>(
        viewId: string,
        options: vscode.TreeViewOptions<T>,
    ): vscode.TreeView<T>;
}

export interface ValidationExplorerCommandsHost {
    registerCommand(
        command: string,
        callback: (...args: any[]) => unknown,
    ): vscode.Disposable;
}

export interface ValidationExplorerActivationServices extends ExtensionRuntimeServices {
    readonly window: ValidationExplorerWindowHost;
    readonly commands: ValidationExplorerCommandsHost;
}

export interface ValidationExplorerActivationDependencies {
    readonly executor?: ValidationExecutor;
    readonly fileFinder?: FileFinder;
}

/**
 * Create the validation explorer and return the provider plus disposables.
 *
 * @returns Provider and disposables required for registration.
 */
export function activateValidationExplorer(
    services: ValidationExplorerActivationServices,
    dependencies: ValidationExplorerActivationDependencies = {},
): {
    provider: ValidationExplorerProvider;
    disposables: vscode.Disposable[];
} {
    const workspaceRoot = services.runtimeContext.getPrimaryWorkspaceRoot();
    const provider = new ValidationExplorerProvider({
        workspaceRoot,
        executor: dependencies.executor ?? new ValidationExecutor({
            workspaceRoot,
            executableService: services.executableService,
        }),
        fileFinder: dependencies.fileFinder ?? new FileFinder(),
        window: services.window,
    });
    const treeView = services.window.createTreeView('talkbank-validation-explorer', {
        treeDataProvider: provider,
        showCollapseAll: true
    });

    return {
        provider,
        disposables: [
            treeView,
            services.commands.registerCommand('talkbank.validateFile', (item: any) => {
                if (item && item.itemPath) {
                    void provider.validateFile(item.itemPath);
                }
            }),
            services.commands.registerCommand('talkbank.validateDirectory', (item: any) => {
                const dirPath = item && item.itemPath
                    ? item.itemPath
                    : services.runtimeContext.getPrimaryWorkspaceRoot();
                if (dirPath) {
                    void provider.validateDirectory(dirPath);
                }
            }),
            services.commands.registerCommand('talkbank.clearCache', (item: any) => {
                const itemPath = item && item.itemPath ? item.itemPath : undefined;
                void provider.clearCache(itemPath);
            }),
            services.commands.registerCommand(
                'talkbank.refreshValidation',
                () => provider.refresh(),
            ),
        ]
    };
}
