/**
 * CLAN analysis commands — QuickPick-driven analysis dispatch via LSP,
 * plus KidEval/Eval panel and participant editor commands.
 */

import * as path from 'path';
import * as vscode from 'vscode';
import { Effect } from 'effect';

import { AnalysisCommandName, AnalysisPanel } from '../analysisPanel';
import {
    AsyncOperationError,
    ExecuteCommandClientService,
    ExtensionContextService,
    tryAsync,
} from '../effectRuntime';
import {
    ExtensionCommandRequirements,
    ExtensionCommandRunner,
    LanguageServerNotRunningError,
    VSCodeWindowService,
    requireActiveChatEditor,
    requireRunningLanguageServer,
} from '../effectCommandRuntime';
import { KidevalPanel, EvalMode } from '../kidevalPanel';
import { IdEditorPanel } from '../idEditorPanel';
import { AnalysisOptions, ExecuteCommandServerError } from '../lsp/executeCommandClient';

/** Descriptions shown in the QuickPick for each analysis command. */
const analysisCommands = [
    { label: 'freq', description: 'Word/morpheme frequency counts and type-token ratio' },
    { label: 'mlu', description: 'Mean length of utterance (morphemes)' },
    { label: 'mlt', description: 'Mean length of turn (utterances/words per turn)' },
    { label: 'wdlen', description: 'Word length distribution' },
    { label: 'wdsize', description: 'Word size (character lengths from %mor stems)' },
    { label: 'maxwd', description: 'Longest words per speaker' },
    { label: 'freqpos', description: 'Frequency by part-of-speech from %mor tier' },
    { label: 'timedur', description: 'Time duration from bullet timing marks' },
    { label: 'kwal', description: 'Keyword-in-context search' },
    { label: 'combo', description: 'Boolean keyword search (AND/OR)' },
    { label: 'gemlist', description: 'List @Bg/@Eg gem segments' },
    { label: 'cooccur', description: 'Word co-occurrence counting' },
    { label: 'dist', description: 'Word distribution/dispersion analysis' },
    { label: 'chip', description: 'Child/parent interaction profile' },
    { label: 'phonfreq', description: 'Phonological segment frequency from %pho tier' },
    { label: 'modrep', description: 'Model and replica analysis of imitations' },
    { label: 'vocd', description: 'Vocabulary diversity (D statistic)' },
    { label: 'codes', description: 'Frequency of coding tier codes' },
    { label: 'complexity', description: 'Syntactic complexity ratio from %gra tier' },
    { label: 'corelex', description: 'Core vocabulary analysis (frequent words)' },
    { label: 'chains', description: 'Code chains and sequences on %cod tier' },
    { label: 'dss', description: 'Developmental Sentence Scoring' },
    { label: 'eval', description: 'Combined language evaluation measures' },
    { label: 'flucalc', description: 'Fluency calculation (disfluency measures)' },
    { label: 'ipsyn', description: 'Index of Productive Syntax' },
    { label: 'keymap', description: 'Keyword-based contingency mapping' },
    { label: 'kideval', description: 'Child language evaluation (DSS+IPSyn+MLU)' },
    { label: 'mortable', description: 'Morpheme frequency table from script file' },
    { label: 'rely', description: 'Inter-rater reliability between two files' },
    { label: 'script', description: 'Compare transcript against template' },
    { label: 'sugar', description: 'Sampling Utterances and Grammatical Analysis' },
    { label: 'trnfix', description: 'Compare two dependent tiers for mismatches' },
    { label: 'uniq', description: 'Find repeated/unique utterances' },
];

function withAnalysisProgress<success, error>(
    runner: ExtensionCommandRunner,
    title: string,
    program: Effect.Effect<success, error, ExtensionCommandRequirements>,
): Effect.Effect<success, error | AsyncOperationError, ExtensionCommandRequirements> {
    return Effect.flatMap(VSCodeWindowService, window => Effect.async<success, error | AsyncOperationError>((resume) => {
        void Promise.resolve(window.withProgress(
            {
                location: vscode.ProgressLocation.Notification,
                title,
            },
            async () => {
                const result = await runner.runEither(program);
                if (result._tag === 'Left') {
                    resume(Effect.fail(result.left as error));
                    return;
                }
                resume(Effect.succeed(result.right as success));
            },
        )).catch(cause => {
            resume(Effect.fail(new AsyncOperationError({
                operation: 'show analysis progress',
                cause,
            })));
        });
    }));
}

function promptAnalysisOptions(
    targetUri: string,
    cmdName: AnalysisCommandName,
): Effect.Effect<AnalysisOptions | undefined, AsyncOperationError, typeof vscode.window> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const options: AnalysisOptions = {};

        if (cmdName === 'kwal') {
            const keywords = yield* tryAsync('show keyword input', () => Promise.resolve(
                window.showInputBox({
                    prompt: 'Enter keyword(s) to search (space-separated)',
                }),
            ));
            if (!keywords) {
                return undefined;
            }
            options.keywords = keywords.trim().split(/\s+/);
        } else if (cmdName === 'combo') {
            const expression = yield* tryAsync('show combo input', () => Promise.resolve(
                window.showInputBox({
                    prompt: 'Enter combo search expression (e.g. "want+cookie" or "want,milk")',
                }),
            ));
            if (!expression) {
                return undefined;
            }
            options.search = [expression];
        } else if (cmdName === 'keymap') {
            const keywords = yield* tryAsync('show keymap keyword input', () => Promise.resolve(
                window.showInputBox({
                    prompt: 'Enter keyword codes (space-separated)',
                }),
            ));
            if (!keywords) {
                return undefined;
            }
            options.keywords = keywords.trim().split(/\s+/);
        } else if (cmdName === 'mortable') {
            const scriptUri = yield* tryAsync('show morpheme script picker', () => Promise.resolve(
                window.showOpenDialog({
                    canSelectFiles: true,
                    canSelectMany: false,
                    filters: { 'Script files': ['cut'] },
                    openLabel: 'Select morpheme script file',
                }),
            ));
            if (!scriptUri?.[0]) {
                return undefined;
            }
            options.scriptPath = scriptUri[0].fsPath;
        } else if (cmdName === 'script') {
            const templateUri = yield* tryAsync('show template picker', () => Promise.resolve(
                window.showOpenDialog({
                    canSelectFiles: true,
                    canSelectMany: false,
                    filters: { 'CHAT files': ['cha'] },
                    openLabel: 'Select template CHAT file',
                }),
            ));
            if (!templateUri?.[0]) {
                return undefined;
            }
            options.templatePath = templateUri[0].fsPath;
        } else if (cmdName === 'rely') {
            if (targetUri.endsWith('/') || !targetUri.endsWith('.cha')) {
                yield* Effect.asVoid(tryAsync('show rely directory warning', () => Promise.resolve(
                    window.showWarningMessage(
                        'The "rely" command compares two files and is not supported for directories.',
                    ),
                )));
                return undefined;
            }

            const secondUri = yield* tryAsync('show second CHAT picker', () => Promise.resolve(
                window.showOpenDialog({
                    canSelectFiles: true,
                    canSelectMany: false,
                    filters: { 'CHAT files': ['cha'] },
                    openLabel: 'Select second CHAT file for comparison',
                }),
            ));
            if (!secondUri?.[0]) {
                return undefined;
            }
            options.secondFile = secondUri[0].toString();
        }

        return options;
    });
}

function runAnalysisOnTarget(
    runner: ExtensionCommandRunner,
    targetUri: string,
    displayName: string,
): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;

        const picked = yield* tryAsync('show analysis picker', () => Promise.resolve(
            window.showQuickPick(analysisCommands, {
                placeHolder: 'Select an analysis command',
                matchOnDescription: true,
            }),
        ));
        if (!picked) {
            return;
        }

        const cmdName = picked.label as AnalysisCommandName;
        const options = yield* promptAnalysisOptions(targetUri, cmdName);
        if (options === undefined) {
            return;
        }

        yield* withAnalysisProgress(
            runner,
            `Running ${cmdName.toUpperCase()}…`,
            Effect.gen(function*() {
                const commands = yield* ExecuteCommandClientService;
                const context = yield* ExtensionContextService;
                const result = yield* commands.analyze(cmdName, targetUri, options);
                if (typeof result === 'string' && result.startsWith('Analysis error:')) {
                    return yield* Effect.fail(new ExecuteCommandServerError({
                        command: 'talkbank/analyze',
                        details: result,
                    }));
                }

                yield* Effect.sync(() => {
                    AnalysisPanel.createOrShow(context, runner, result, cmdName, displayName);
                });
            }),
        );
    });
}

export function runAnalysis(
    runner: ExtensionCommandRunner,
): Effect.Effect<void, LanguageServerNotRunningError | unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveChatEditor();
        const commands = yield* ExecuteCommandClientService;
        yield* requireRunningLanguageServer(commands.isRunning());

        yield* runAnalysisOnTarget(
            runner,
            editor.document.uri.toString(),
            editor.document.uri.fsPath,
        );
    });
}

export function runAnalysisOnDirectory(
    runner: ExtensionCommandRunner,
): Effect.Effect<void, LanguageServerNotRunningError | AsyncOperationError | unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const window = yield* VSCodeWindowService;
        const commands = yield* ExecuteCommandClientService;
        yield* requireRunningLanguageServer(commands.isRunning());

        const folders = yield* tryAsync('show analysis directory picker', () => Promise.resolve(
            window.showOpenDialog({
                canSelectFiles: false,
                canSelectFolders: true,
                canSelectMany: false,
                openLabel: 'Select directory with .cha files',
            }),
        ));
        if (!folders?.[0]) {
            return;
        }

        yield* runAnalysisOnTarget(
            runner,
            folders[0].toString(),
            path.basename(folders[0].fsPath),
        );
    });
}

export function editParticipants(
    runner: ExtensionCommandRunner,
): Effect.Effect<void, LanguageServerNotRunningError | unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveChatEditor();
        const context = yield* ExtensionContextService;
        const commands = yield* ExecuteCommandClientService;
        yield* requireRunningLanguageServer(commands.isRunning());

        yield* Effect.sync(() => {
            IdEditorPanel.createOrShow(context, runner, editor.document, commands);
        });
    });
}

export function runEvalPanel(
    runner: ExtensionCommandRunner,
    mode: EvalMode,
): Effect.Effect<void, LanguageServerNotRunningError | unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const editor = yield* requireActiveChatEditor();
        const context = yield* ExtensionContextService;
        const commands = yield* ExecuteCommandClientService;
        yield* requireRunningLanguageServer(commands.isRunning());

        yield* Effect.sync(() => {
            KidevalPanel.createOrShow(
                context,
                runner,
                editor.document.uri.toString(),
                editor.document.uri.fsPath,
                commands,
                mode,
            );
        });
    });
}
