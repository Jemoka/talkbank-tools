/**
 * Typed facade around `workspace/executeCommand` calls used by the extension.
 *
 * This centralizes command names and payload shapes so feature modules do not
 * construct ad hoc request objects inline.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import type * as vscode from 'vscode';
import { Effect } from 'effect';
import type { LanguageClient } from 'vscode-languageclient/node';
import {
    AnalyzeCommandRequest,
    AnalysisOptions,
    AlignmentSidecarDocument,
    AvailableDatabase,
    DependencyGraphResponse,
    FormattedBulletLine,
    IdLineFields,
    ParticipantEntry,
    ScopedFindMatch,
    ScopedFindRequest,
    SpeakerInfo,
    UtteranceInfo,
    buildAnalyzeCommandRequest,
    decodeAvailableDatabaseList,
    decodeDependencyGraphResponse,
    decodeFormattedBulletLine,
    decodeParticipantEntryList,
    decodeScopedFindMatchList,
    decodeSpeakerInfoList,
    decodeUtteranceInfoList,
    parseAlignmentSidecarDocument,
} from './executeCommandPayloads';
import {
    ExecuteCommandRequestError,
    ExecuteCommandResponseError,
    ExecuteCommandServerError,
    ExecuteCommandStructuredError,
} from './executeCommandErrors';
import { StructuredPayloadDecodeError } from '../effectBoundary';
import { tryAsync } from '../effectRuntime';

// Re-export the error family so existing consumers that import from
// this module continue to work unchanged. New code should prefer
// `./executeCommandErrors`.
export {
    ExecuteCommandRequestError,
    ExecuteCommandResponseError,
    ExecuteCommandServerError,
} from './executeCommandErrors';
export type { ExecuteCommandStructuredError } from './executeCommandErrors';

export type {
    AnalysisDatabaseFilter,
    AnalyzeCommandRequest,
    AnalysisOptions,
    AlignmentSidecarDocument,
    AvailableDatabase,
    FormattedBulletLine,
    IdLineFields,
    ParticipantEntry,
    ScopedFindMatch,
    ScopedFindRequest,
    SpeakerInfo,
    UtteranceInfo,
} from './executeCommandPayloads';

/**
 * Execute-command identifiers used throughout the extension.
 */
export type TalkbankExecuteCommandName = (typeof talkbankExecuteCommandList)[number];

/**
 * Execute-command identifiers used for normative database discovery.
 */
export const talkbankExecuteCommandNames = {
    showDependencyGraph: 'talkbank/showDependencyGraph',
    getAlignmentSidecar: 'talkbank/getAlignmentSidecar',
    analyze: 'talkbank/analyze',
    kidevalDatabases: 'talkbank/kidevalDatabases',
    evalDatabases: 'talkbank/evalDatabases',
    getParticipants: 'talkbank/getParticipants',
    formatIdLine: 'talkbank/formatIdLine',
    getSpeakers: 'talkbank/getSpeakers',
    filterDocument: 'talkbank/filterDocument',
    getUtterances: 'talkbank/getUtterances',
    formatBulletLine: 'talkbank/formatBulletLine',
    scopedFind: 'talkbank/scopedFind',
} as const;

/**
 * Ordered execute-command identifier list shared with parity tests.
 */
export const talkbankExecuteCommandList = [
    talkbankExecuteCommandNames.showDependencyGraph,
    talkbankExecuteCommandNames.getAlignmentSidecar,
    talkbankExecuteCommandNames.analyze,
    talkbankExecuteCommandNames.kidevalDatabases,
    talkbankExecuteCommandNames.evalDatabases,
    talkbankExecuteCommandNames.getParticipants,
    talkbankExecuteCommandNames.formatIdLine,
    talkbankExecuteCommandNames.getSpeakers,
    talkbankExecuteCommandNames.filterDocument,
    talkbankExecuteCommandNames.getUtterances,
    talkbankExecuteCommandNames.formatBulletLine,
    talkbankExecuteCommandNames.scopedFind,
] as const;

/**
 * Execute-command identifiers used for normative database discovery.
 */
export type DatabaseDiscoveryCommand =
    | typeof talkbankExecuteCommandNames.kidevalDatabases
    | typeof talkbankExecuteCommandNames.evalDatabases;

/**
 * Thin typed client for TalkBank extension commands exposed through LSP.
 */
export class TalkbankExecuteCommandClient {
    /**
     * Create a new facade over a running language client.
     *
     * @param client - Underlying VS Code language client.
     */
    constructor(private readonly client: LanguageClient) {}

    /**
     * Return whether the underlying language client is running.
     *
     * @returns True when requests can be sent.
     */
    isRunning(): boolean {
        return this.client.isRunning();
    }

    /**
     * Request the dependency graph for a document position.
     *
     * @param uri - Document URI string.
     * @param position - Cursor position.
     * @returns A discriminated response — either a `dot` variant carrying
     *   Graphviz source, or an `unavailable` variant carrying a user-facing
     *   reason (e.g., "No %mor tier found"). Callers MUST branch on `kind`
     *   before passing anything to a DOT renderer.
     */
    showDependencyGraph(
        uri: string,
        position: vscode.Position,
    ): Effect.Effect<DependencyGraphResponse, ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.showDependencyGraph,
            [uri, { line: position.line, character: position.character }],
            decodeDependencyGraphResponse,
        );
    }

    /**
     * Request alignment-sidecar timing data for a document.
     *
     * @param uri - Document URI string.
     * @returns Decoded sidecar payload, or `null` when unavailable.
     */
    getAlignmentSidecar(
        uri: string,
    ): Effect.Effect<AlignmentSidecarDocument | null, ExecuteCommandRequestError> {
        return Effect.map(
            this.executeRaw(talkbankExecuteCommandNames.getAlignmentSidecar, [uri]),
            parseAlignmentSidecarDocument,
        );
    }

    /**
     * Request the parsed participant list for a document.
     *
     * @param uri - Document URI string.
     * @returns Speaker metadata array.
     */
    getSpeakers(uri: string): Effect.Effect<SpeakerInfo[], ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.getSpeakers,
            [uri],
            decodeSpeakerInfoList,
        );
    }

    /**
     * Request `@ID` participant rows for a document.
     *
     * @param uri - Document URI string.
     * @returns Participant editor rows.
     */
    getParticipants(uri: string): Effect.Effect<ParticipantEntry[], ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.getParticipants,
            [uri],
            decodeParticipantEntryList,
        );
    }

    /**
     * Format one `@ID` header line from plain field values.
     *
     * @param fields - Participant editor field values.
     * @returns Canonical `@ID` line text.
     */
    formatIdLine(
        fields: IdLineFields,
    ): Effect.Effect<string, ExecuteCommandRequestError | ExecuteCommandResponseError> {
        return this.executeString(talkbankExecuteCommandNames.formatIdLine, [fields]);
    }

    /**
     * Request a filtered copy of the document containing only selected speakers.
     *
     * @param uri - Document URI string.
     * @param speakers - Speaker codes to retain.
     * @returns Filtered CHAT text.
     */
    filterDocument(
        uri: string,
        speakers: string[],
    ): Effect.Effect<string, ExecuteCommandRequestError | ExecuteCommandResponseError> {
        return this.executeString(talkbankExecuteCommandNames.filterDocument, [{ uri, speakers }]);
    }

    /**
     * Execute semantic scoped-find against one document.
     *
     * @param request - Scoped search request.
     * @returns Matching spans.
     */
    scopedFind(
        request: ScopedFindRequest,
    ): Effect.Effect<ScopedFindMatch[], ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.scopedFind,
            [request],
            decodeScopedFindMatchList,
        );
    }

    /**
     * Request server-side formatting for a timing bullet insertion.
     *
     * @param prevMs - Start timestamp for the segment.
     * @param currentMs - End timestamp for the segment.
     * @param speaker - Speaker code to scaffold.
     * @returns Formatted bullet and new utterance text.
     */
    formatBulletLine(
        prevMs: number,
        currentMs: number,
        speaker: string,
    ): Effect.Effect<FormattedBulletLine, ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.formatBulletLine,
            [{ prev_ms: prevMs, current_ms: currentMs, speaker }],
            decodeFormattedBulletLine,
        );
    }

    /**
     * Discover normative databases available under a CLAN library directory.
     *
     * @param command - Discovery command to invoke.
     * @param libraryDir - Directory containing database files.
     * @returns Discovered database metadata entries.
     */
    discoverDatabases(
        command: DatabaseDiscoveryCommand,
        libraryDir: string,
    ): Effect.Effect<AvailableDatabase[], ExecuteCommandStructuredError> {
        return this.executeStructured(command, [libraryDir], decodeAvailableDatabaseList);
    }

    /**
     * Execute one analysis command on a file or directory target.
     *
     * @param commandName - Analysis command identifier.
     * @param targetUri - File or directory URI string.
     * @param options - Command-specific options.
     * @returns Arbitrary command result payload.
     */
    analyze(
        commandName: string,
        targetUri: string,
        options: AnalysisOptions,
    ): Effect.Effect<unknown, ExecuteCommandRequestError> {
        const request: AnalyzeCommandRequest = buildAnalyzeCommandRequest(commandName, targetUri, options);
        return this.executeRaw(talkbankExecuteCommandNames.analyze, [request]);
    }

    /**
     * Request utterance metadata used by coder mode.
     *
     * @param uri - Document URI string.
     * @returns Utterance descriptors.
     */
    getUtterances(uri: string): Effect.Effect<UtteranceInfo[], ExecuteCommandStructuredError> {
        return this.executeStructured(
            talkbankExecuteCommandNames.getUtterances,
            [uri],
            decodeUtteranceInfoList,
        );
    }

    /**
     * Send one typed `workspace/executeCommand` request.
     *
     * @param command - Server command identifier.
     * @param args - Command arguments.
     * @returns Typed result payload.
     */
    private executeStructured<decodedPayload>(
        command: TalkbankExecuteCommandName,
        args: unknown[],
        decode: (payload: unknown) => decodedPayload,
    ): Effect.Effect<decodedPayload, ExecuteCommandStructuredError> {
        return Effect.flatMap(this.executeRaw(command, args), payload => Effect.try({
            try: () => decode(payload),
            catch: cause => {
                if (cause instanceof StructuredPayloadDecodeError) {
                    return cause;
                }
                if (cause instanceof Error) {
                    return new ExecuteCommandServerError({
                        command,
                        details: cause.message,
                    });
                }
                return new ExecuteCommandResponseError({
                    command,
                    details: 'Structured payload decoder failed with a non-Error cause',
                    payload,
                });
            },
        }));
    }

    /**
     * Send one string-valued `workspace/executeCommand` request.
     *
     * @param command - Server command identifier.
     * @param args - Command arguments.
     * @returns String result payload.
     */
    private executeString(
        command: TalkbankExecuteCommandName,
        args: unknown[]
    ): Effect.Effect<string, ExecuteCommandRequestError | ExecuteCommandResponseError> {
        return Effect.flatMap(this.executeRaw(command, args), payload => typeof payload === 'string'
            ? Effect.succeed(payload)
            : Effect.fail(new ExecuteCommandResponseError({
                command,
                details: 'Expected string response payload',
                payload,
            })));
    }

    /**
     * Send one raw `workspace/executeCommand` request.
     *
     * @param command - Server command identifier.
     * @param args - Command arguments.
     * @returns Untyped result payload from the LSP transport.
     */
    private executeRaw(
        command: TalkbankExecuteCommandName,
        args: unknown[]
    ): Effect.Effect<unknown, ExecuteCommandRequestError> {
        return Effect.mapError(
            tryAsync(`execute ${command}`, () => this.client.sendRequest<unknown>(
                'workspace/executeCommand',
                {
                    command,
                    arguments: args,
                },
            )),
            error => new ExecuteCommandRequestError({
                command,
                details: error.cause instanceof Error
                    ? error.cause.message
                    : `Failed to execute ${command}`,
                cause: error.cause,
            }),
        );
    }
}
