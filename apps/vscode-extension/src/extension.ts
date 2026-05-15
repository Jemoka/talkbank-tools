import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { CacheManager } from './cacheManager';
import { ClanIntegration } from './clanIntegration';
import { disposeSpecialChars } from './specialChars';
import { activateLanguageServer } from './activation/lsp';
import { registerBulletDecorations } from './activation/bullets';
import { registerReviewDecorations } from './activation/reviewDecorations';
import { registerReviewStatusBarCleanup } from './commands/review';
import { activateValidationExplorer } from './activation/validation';
import { registerExtensionCommands } from './activation/commands';
import { createExtensionCommandRunner } from './effectCommandRuntime';
import { TalkbankExecuteCommandClient } from './lsp/executeCommandClient';
import { createExtensionRuntimeServices } from './runtimeServices';

let activeClient: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext) {
    console.log('TalkBank extension activated');

    const runtimeServices = createExtensionRuntimeServices(vscode.workspace);
    const client = await activateLanguageServer(context, {
        ...runtimeServices,
        workspace: vscode.workspace,
    });
    if (!client) {
        // Every feature below depends on a live language client.
        return;
    }
    activeClient = client;
    const commandClient = new TalkbankExecuteCommandClient(client);
    context.subscriptions.push(...registerBulletDecorations(runtimeServices.runtimeContext));
    context.subscriptions.push(...registerReviewDecorations());
    context.subscriptions.push(registerReviewStatusBarCleanup());
    context.subscriptions.push(
        ...activateValidationExplorer({
            ...runtimeServices,
            window: vscode.window,
            commands: vscode.commands,
        }).disposables
    );

    const cacheManager = new CacheManager({
        ...runtimeServices,
        window: vscode.window,
        statusBarItem: vscode.window.createStatusBarItem(
            'talkbank.cache-stats',
            vscode.StatusBarAlignment.Right,
            100,
        ),
    });
    cacheManager.initialize();
    const clanIntegration = new ClanIntegration({
        ...runtimeServices,
        window: vscode.window,
    });
    context.subscriptions.push(cacheManager);
    const services = {
        ...runtimeServices,
        client,
        commands: commandClient,
        cacheManager,
        clanIntegration,
    };
    const runner = createExtensionCommandRunner(context, services);
    context.subscriptions.push(
        ...registerExtensionCommands(context, services, runner),
    );
}

export function deactivate(): Thenable<void> | undefined {
    disposeSpecialChars();
    const client = activeClient;
    activeClient = undefined;
    return client?.stop();
}
