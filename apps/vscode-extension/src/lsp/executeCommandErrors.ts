/**
 * Tagged errors for the typed `workspace/executeCommand` facade.
 *
 * Split out from `executeCommandClient.ts` so the facade class and its
 * associated error family can live in separate modules. Consumers
 * that want just the error union can import from here; the facade
 * module re-exports these names for backward compatibility.
 */

/* eslint-disable @typescript-eslint/naming-convention */

import { Data } from 'effect';
import { StructuredPayloadDecodeError } from '../effectBoundary';
import type { TalkbankExecuteCommandName } from './executeCommandClient';

/**
 * Tagged error for one failed `workspace/executeCommand` transport request.
 */
export class ExecuteCommandRequestError extends Data.TaggedError('ExecuteCommandRequestError')<{
    readonly command: TalkbankExecuteCommandName;
    readonly details: string;
    readonly cause: unknown;
}> {}

/**
 * Tagged error for one structured command response that was not the expected shape.
 */
export class ExecuteCommandResponseError extends Data.TaggedError('ExecuteCommandResponseError')<{
    readonly command: TalkbankExecuteCommandName;
    readonly details: string;
    readonly payload: unknown;
}> {}

/**
 * Tagged error for one server-returned string error payload.
 */
export class ExecuteCommandServerError extends Data.TaggedError('ExecuteCommandServerError')<{
    readonly command: TalkbankExecuteCommandName;
    readonly details: string;
}> {}

/**
 * Typed error family for structured execute-command responses.
 */
export type ExecuteCommandStructuredError =
    | ExecuteCommandRequestError
    | ExecuteCommandResponseError
    | ExecuteCommandServerError
    | StructuredPayloadDecodeError;
