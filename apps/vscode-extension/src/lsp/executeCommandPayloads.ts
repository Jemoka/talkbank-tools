/* eslint-disable @typescript-eslint/naming-convention */
/**
 * Structured execute-command payload types and runtime decoders.
 *
 * This module keeps the extension's consumed command payload shapes in one
 * place so transport code does not also need to own payload validation.
 */

import { Schema } from 'effect';

import {
    decodeStructuredWithSchema,
    parseWithSchema,
} from '../effectBoundary';

/**
 * Participant/speaker information returned by the language server.
 */
export interface SpeakerInfo {
    /**
     * CHAT speaker code.
     */
    code: string;
    /**
     * Participant display name.
     */
    name: string;
    /**
     * Participant role label.
     */
    role: string;
}

/**
 * Plain field values used by the participant editor.
 */
export interface IdLineFields {
    /**
     * Language code field.
     */
    language: string;
    /**
     * Corpus name field.
     */
    corpus: string;
    /**
     * Speaker code field.
     */
    speaker: string;
    /**
     * Age field.
     */
    age: string;
    /**
     * Sex field.
     */
    sex: string;
    /**
     * Group field.
     */
    group: string;
    /**
     * SES field.
     */
    ses: string;
    /**
     * Role field.
     */
    role: string;
    /**
     * Education field.
     */
    education: string;
    /**
     * Custom field.
     */
    custom: string;
}

/**
 * One participant editor row returned by the language server.
 */
export interface ParticipantEntry {
    /**
     * Zero-based line number for the `@ID` header.
     */
    line: number;
    /**
     * Pipe-delimited `@ID` fields expanded into an object.
     */
    fields: IdLineFields;
}

/**
 * One semantic scoped-find match returned by the language server.
 */
export interface ScopedFindMatch {
    /**
     * Zero-based line number.
     */
    line: number;
    /**
     * Zero-based character offset on the line.
     */
    character: number;
    /**
     * Match length in characters.
     */
    length: number;
    /**
     * Tier identifier such as `main` or `mor`.
     */
    tier: string;
    /**
     * Speaker code for the containing utterance.
     */
    speaker: string;
    /**
     * Source line text shown in the picker.
     */
    line_text: string;
}

/**
 * Request payload for semantic scoped-find.
 */
export interface ScopedFindRequest {
    /**
     * Document URI string.
     */
    uri: string;
    /**
     * Search query text or regex source.
     */
    query: string;
    /**
     * Tier scope to search within.
     */
    scope: string;
    /**
     * Optional speaker filters.
     */
    speakers: string[];
    /**
     * Whether `query` is regex syntax.
     */
    regex: boolean;
}

/**
 * Formatted bullet line result returned by the language server.
 */
export interface FormattedBulletLine {
    /**
     * Bullet text appended to the current utterance line.
     */
    bullet: string;
    /**
     * New utterance line scaffold inserted after the bullet.
     */
    new_line: string;
}

/**
 * Minimal utterance metadata used by coder mode.
 */
export interface UtteranceInfo {
    /**
     * Zero-based utterance line number.
     */
    line: number;
    /**
     * Speaker code for the utterance.
     */
    speaker: string;
    /**
     * Whether the utterance already has a `%cod` tier.
     */
    has_cod: boolean;
}

/**
 * Demographic filter payload used for database-backed analyses.
 */
export interface AnalysisDatabaseFilter {
    language?: string;
    group?: string;
    gender?: 'Male' | 'Female' | 'Both';
    age_from_months?: number;
    age_to_months?: number;
    speaker_codes?: string[];
}

/**
 * Typed `talkbank/analyze` option bag passed through to the server.
 */
export interface AnalysisOptions {
    mor?: boolean;
    words?: boolean;
    mainTier?: boolean;
    limit?: number;
    keywords?: string[];
    search?: string[];
    maxDepth?: number;
    tier?: string;
    threshold?: number;
    maxUtterances?: number;
    databasePath?: string;
    databaseFilter?: AnalysisDatabaseFilter;
    syllableMode?: boolean;
    dssMaxUtterances?: number;
    ipsynMaxUtterances?: number;
    scriptPath?: string;
    secondFile?: string;
    templatePath?: string;
    minUtterances?: number;
    tier1?: string;
    tier2?: string;
    sortByFrequency?: boolean;
}

/**
 * Canonical object payload sent to the `talkbank/analyze` execute-command.
 */
export interface AnalyzeCommandRequest {
    commandName: string;
    targetUri: string;
    options: AnalysisOptions;
}

/**
 * Build the canonical `talkbank/analyze` request object.
 */
export function buildAnalyzeCommandRequest(
    commandName: string,
    targetUri: string,
    options: AnalysisOptions = {},
): AnalyzeCommandRequest {
    return {
        commandName,
        targetUri,
        options,
    };
}

/**
 * One normative database discovered from a CLAN library directory.
 */
export interface AvailableDatabase {
    path: string;
    language: string;
    corpus_type: string | null;
    display_name: string;
}

/**
 * LSP position payload used inside alignment-sidecar responses.
 */
export interface LspPosition {
    line: number;
    character: number;
}

/**
 * LSP range payload used inside alignment-sidecar responses.
 */
export interface LspRange {
    start: LspPosition;
    end: LspPosition;
}

/**
 * Playback timing information included in the alignment sidecar.
 */
export interface SidecarTiming {
    start_ms: number;
    end_ms: number;
    skip: boolean;
}

/**
 * Playback-facing subset of one alignment-sidecar utterance.
 */
export interface AlignmentSidecarUtterance {
    utterance_index: number;
    speaker: string;
    range: LspRange | null;
    timing: SidecarTiming | null;
}

/**
 * Playback-facing alignment sidecar payload returned by the language server.
 */
export interface AlignmentSidecarDocument {
    schema_version: number;
    uri: string;
    utterances: AlignmentSidecarUtterance[];
}

const DependencyGraphDotSchema = Schema.Struct({
    kind: Schema.Literal('dot'),
    source: Schema.String,
});

const DependencyGraphUnavailableSchema = Schema.Struct({
    kind: Schema.Literal('unavailable'),
    reason: Schema.String,
});

export const DependencyGraphResponseSchema = Schema.Union(
    DependencyGraphDotSchema,
    DependencyGraphUnavailableSchema,
);

const SpeakerInfoSchema = Schema.Struct({
    code: Schema.String,
    name: Schema.String,
    role: Schema.String,
});

const SpeakerInfoListSchema = Schema.Array(SpeakerInfoSchema);

const IdLineFieldsSchema = Schema.Struct({
    language: Schema.String,
    corpus: Schema.String,
    speaker: Schema.String,
    age: Schema.String,
    sex: Schema.String,
    group: Schema.String,
    ses: Schema.String,
    role: Schema.String,
    education: Schema.String,
    custom: Schema.String,
});

const ParticipantEntrySchema = Schema.Struct({
    line: Schema.NonNegativeInt,
    fields: IdLineFieldsSchema,
});

export const ParticipantEntryListSchema = Schema.Array(ParticipantEntrySchema);

const ScopedFindMatchSchema = Schema.Struct({
    line: Schema.NonNegativeInt,
    character: Schema.NonNegativeInt,
    length: Schema.NonNegativeInt,
    tier: Schema.String,
    speaker: Schema.String,
    line_text: Schema.String,
});

const ScopedFindMatchListSchema = Schema.Array(ScopedFindMatchSchema);

const FormattedBulletLineSchema = Schema.Struct({
    bullet: Schema.String,
    new_line: Schema.String,
});

const UtteranceInfoSchema = Schema.Struct({
    line: Schema.NonNegativeInt,
    speaker: Schema.String,
    has_cod: Schema.Boolean,
});

const UtteranceInfoListSchema = Schema.Array(UtteranceInfoSchema);

const AnalysisDatabaseGenderSchema = Schema.Union(
    Schema.Literal('Male'),
    Schema.Literal('Female'),
    Schema.Literal('Both'),
);

export const AnalysisDatabaseFilterSchema = Schema.Struct({
    language: Schema.optional(Schema.String),
    group: Schema.optional(Schema.String),
    gender: Schema.optional(AnalysisDatabaseGenderSchema),
    age_from_months: Schema.optional(Schema.NonNegativeInt),
    age_to_months: Schema.optional(Schema.NonNegativeInt),
    speaker_codes: Schema.optional(Schema.Array(Schema.String)),
});

const AvailableDatabaseSchema = Schema.Struct({
    path: Schema.String,
    language: Schema.String,
    corpus_type: Schema.NullOr(Schema.String),
    display_name: Schema.String,
});

const AvailableDatabaseListSchema = Schema.Array(AvailableDatabaseSchema);

const LspPositionSchema = Schema.Struct({
    line: Schema.NonNegativeInt,
    character: Schema.NonNegativeInt,
});

const LspRangeSchema = Schema.Struct({
    start: LspPositionSchema,
    end: LspPositionSchema,
});

const SidecarTimingSchema = Schema.Struct({
    start_ms: Schema.NonNegativeInt,
    end_ms: Schema.NonNegativeInt,
    skip: Schema.Boolean,
});

const AlignmentSidecarUtteranceSchema = Schema.Struct({
    utterance_index: Schema.NonNegativeInt,
    speaker: Schema.String,
    range: Schema.NullOr(LspRangeSchema),
    timing: Schema.NullOr(SidecarTimingSchema),
});

const AlignmentSidecarEnvelopeSchema = Schema.Struct({
    schema_version: Schema.NonNegativeInt,
    uri: Schema.String,
    utterances: Schema.Array(Schema.Unknown),
});

type AnalysisDatabaseFilterLike = Omit<AnalysisDatabaseFilter, 'speaker_codes'> & {
    readonly speaker_codes?: readonly string[];
};

function toMutableArray<item>(items: readonly item[]): item[] {
    return [...items];
}

export function toMutableAnalysisDatabaseFilter(
    filter: AnalysisDatabaseFilterLike,
): AnalysisDatabaseFilter {
    return {
        ...filter,
        speaker_codes: filter.speaker_codes === undefined ? undefined : [...filter.speaker_codes],
    };
}

/**
 * Discriminated response from `talkbank/showDependencyGraph`.
 *
 * `kind === 'dot'` carries Graphviz source ready to render.
 * `kind === 'unavailable'` carries a user-facing reason ("No %mor tier found")
 * and must NOT be passed to a Graphviz renderer.
 */
export type DependencyGraphResponse = Schema.Schema.Type<typeof DependencyGraphResponseSchema>;

/**
 * Decode the `talkbank/showDependencyGraph` response payload.
 */
export function decodeDependencyGraphResponse(payload: unknown): DependencyGraphResponse {
    return decodeStructuredWithSchema(
        'talkbank/showDependencyGraph',
        payload,
        DependencyGraphResponseSchema,
    );
}

/**
 * Decode the `talkbank/getSpeakers` response payload.
 */
export function decodeSpeakerInfoList(payload: unknown): SpeakerInfo[] {
    return toMutableArray(decodeStructuredWithSchema(
        'talkbank/getSpeakers',
        payload,
        SpeakerInfoListSchema,
    ));
}

/**
 * Decode the `talkbank/getParticipants` response payload.
 */
export function decodeParticipantEntryList(payload: unknown): ParticipantEntry[] {
    return toMutableArray(decodeStructuredWithSchema(
        'talkbank/getParticipants',
        payload,
        ParticipantEntryListSchema,
    ));
}

/**
 * Parse a generic participant-entry array without command-specific error handling.
 */
export function parseParticipantEntryList(payload: unknown): ParticipantEntry[] | undefined {
    const parsed = parseWithSchema(ParticipantEntryListSchema, payload);
    return parsed === undefined ? undefined : toMutableArray(parsed);
}

/**
 * Decode the `talkbank/scopedFind` response payload.
 */
export function decodeScopedFindMatchList(payload: unknown): ScopedFindMatch[] {
    return toMutableArray(decodeStructuredWithSchema(
        'talkbank/scopedFind',
        payload,
        ScopedFindMatchListSchema,
    ));
}

/**
 * Decode the `talkbank/formatBulletLine` response payload.
 */
export function decodeFormattedBulletLine(payload: unknown): FormattedBulletLine {
    return decodeStructuredWithSchema(
        'talkbank/formatBulletLine',
        payload,
        FormattedBulletLineSchema,
    );
}

/**
 * Decode the `talkbank/getUtterances` response payload.
 */
export function decodeUtteranceInfoList(payload: unknown): UtteranceInfo[] {
    return toMutableArray(decodeStructuredWithSchema(
        'talkbank/getUtterances',
        payload,
        UtteranceInfoListSchema,
    ));
}

/**
 * Decode the database discovery payload used by KidEval and Eval.
 */
export function decodeAvailableDatabaseList(payload: unknown): AvailableDatabase[] {
    return toMutableArray(decodeStructuredWithSchema(
        'talkbank/databaseDiscovery',
        payload,
        AvailableDatabaseListSchema,
    ));
}

/**
 * Parse one generic analysis database-filter payload.
 */
export function parseAnalysisDatabaseFilter(
    payload: unknown,
): AnalysisDatabaseFilter | undefined {
    const parsed = parseWithSchema(AnalysisDatabaseFilterSchema, payload);
    return parsed === undefined ? undefined : toMutableAnalysisDatabaseFilter(parsed);
}

/**
 * Parse the playback-facing subset of the alignment-sidecar payload.
 *
 * Returns `null` for malformed envelopes or string error payloads so callers can
 * safely fall back to bullet parsing.
 */
export function parseAlignmentSidecarDocument(payload: unknown): AlignmentSidecarDocument | null {
    if (typeof payload === 'string') {
        return null;
    }

    const envelope = parseWithSchema(AlignmentSidecarEnvelopeSchema, payload);
    if (envelope === undefined) {
        return null;
    }

    const utterances = envelope.utterances.flatMap(rawUtterance => {
        const utterance = parseWithSchema(AlignmentSidecarUtteranceSchema, rawUtterance);
        return utterance === undefined ? [] : [utterance];
    });

    return {
        schema_version: envelope.schema_version,
        uri: envelope.uri,
        utterances: toMutableArray(utterances),
    };
}
