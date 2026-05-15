/**
 * Command registration for the TalkBank VS Code extension.
 */

import * as vscode from 'vscode';

import { ExtensionCommandRunner } from '../../effectCommandRuntime';
import { registerAnalysisCommands } from './analysis';
import { registerEditorCommands } from './editor';
import { registerMediaCommands } from './media';
import type { ExtensionServices } from './types';
import { registerUtilityCommands } from './utility';

export type { ExtensionServices } from './types';

/**
 * Register all command groups for the extension.
 *
 * @param context - Extension activation context.
 * @param services - Shared runtime services.
 * @returns Disposable registrations.
 */
export function registerExtensionCommands(
    context: vscode.ExtensionContext,
    services: ExtensionServices,
    runner: ExtensionCommandRunner,
): vscode.Disposable[] {
    return [
        ...registerUtilityCommands(context, services, runner),
        ...registerMediaCommands(context, services, runner),
        ...registerAnalysisCommands(context, services, runner),
        ...registerEditorCommands(context, services, runner),
    ];
}
