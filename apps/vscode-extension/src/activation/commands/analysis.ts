import * as vscode from 'vscode';

import {
    editParticipants,
    runAnalysis,
    runAnalysisOnDirectory,
    runEvalPanel,
} from '../../commands/analysis';
import {
    ExtensionCommandRunner,
    registerEffectCommand,
} from '../../effectCommandRuntime';
import type { ExtensionServices } from './types';

/**
 * Register analysis and evaluation commands.
 *
 * @param context - Extension activation context.
 * @param services - Shared runtime services.
 * @returns Analysis-related disposables.
 */
export function registerAnalysisCommands(
    context: vscode.ExtensionContext,
    _services: ExtensionServices,
    runner: ExtensionCommandRunner,
): vscode.Disposable[] {
    return [
        registerEffectCommand('talkbank.runAnalysis', runner, () => runAnalysis(runner)),
        registerEffectCommand('talkbank.runAnalysisOnDirectory', runner, () => runAnalysisOnDirectory(runner)),
        registerEffectCommand('talkbank.runKideval', runner, () => runEvalPanel(runner, 'kideval')),
        registerEffectCommand('talkbank.runEval', runner, () => runEvalPanel(runner, 'eval')),
        registerEffectCommand('talkbank.runEvald', runner, () => runEvalPanel(runner, 'evald')),
        registerEffectCommand('talkbank.editParticipants', runner, () => editParticipants(runner)),
    ];
}
