import * as vscode from 'vscode';

import { scopedFind } from '../../commands/scopedFind';
import { filterBySpeaker } from '../../commands/speakerFilter';
import {
    coderInsertCode,
    coderNext,
    startCoderMode,
    stopCoderMode,
} from '../../coderPanel';
import {
    startReview,
    stopReview,
    reviewNext,
    reviewPrev,
    rateUtterance,
    skipUtterance,
} from '../../commands/review';
import {
    ExtensionCommandRunner,
    registerEffectCommand,
} from '../../effectCommandRuntime';
import type { ExtensionServices } from './types';

/**
 * Register editing and navigation commands.
 *
 * @param context - Extension activation context.
 * @param services - Shared runtime services.
 * @returns Editing-related disposables.
 */
export function registerEditorCommands(
    _context: vscode.ExtensionContext,
    _services: ExtensionServices,
    runner: ExtensionCommandRunner,
): vscode.Disposable[] {
    return [
        registerEffectCommand('talkbank.filterBySpeaker', runner, () => filterBySpeaker()),
        registerEffectCommand('talkbank.scopedFind', runner, () => scopedFind()),
        registerEffectCommand('talkbank.startCoder', runner, () => startCoderMode()),
        registerEffectCommand('talkbank.stopCoder', runner, () => stopCoderMode()),
        registerEffectCommand('talkbank.coderNext', runner, () => coderNext()),
        registerEffectCommand('talkbank.coderInsertCode', runner, () => coderInsertCode()),
        // Review mode commands
        registerEffectCommand('talkbank.startReview', runner, () => startReview()),
        registerEffectCommand('talkbank.stopReview', runner, () => stopReview()),
        registerEffectCommand('talkbank.reviewNext', runner, () => reviewNext()),
        registerEffectCommand('talkbank.reviewPrev', runner, () => reviewPrev()),
        registerEffectCommand('talkbank.reviewGood', runner, () => rateUtterance('ok')),
        registerEffectCommand('talkbank.reviewEarly', runner, () => rateUtterance('early')),
        registerEffectCommand('talkbank.reviewLate', runner, () => rateUtterance('late')),
        registerEffectCommand('talkbank.reviewWrong', runner, () => rateUtterance('wrong')),
        registerEffectCommand('talkbank.reviewSkip', runner, () => skipUtterance()),
    ];
}
