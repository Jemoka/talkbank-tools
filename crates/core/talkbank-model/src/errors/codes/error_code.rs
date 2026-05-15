//! Canonical diagnostic code enum for TalkBank parsing/validation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

/// Standard error codes for CHAT parsing and validation.
///
/// This enum uses the `#[error_code_enum]` procedural macro which generates:
/// - Serde rename attributes for each variant
/// - `as_str()` method for ErrorCode -> &str conversion
/// - `new()` method for &str -> ErrorCode conversion
/// - `Display` implementation
/// - `documentation_url()` method
///
/// All mappings are generated from a single source of truth, eliminating
/// the fragile double-maintenance pattern.
#[talkbank_derive::error_code_enum]
pub enum ErrorCode {
    // =========================================================================
    // Generic/Internal Errors (E0xx, E1xx)
    // =========================================================================
    /// Internal error (unexpected condition in parser or validator).
    #[code("E001")]
    InternalError,
    /// Test-only error code used in unit tests.
    #[code("E002")]
    TestError,
    /// Input string is empty.
    #[code("E003")]
    EmptyString,

    // =========================================================================
    // Structural/File Errors (E1xx)
    // =========================================================================
    /// Invalid line format in the CHAT file.
    #[code("E101")]
    InvalidLineFormat,

    // =========================================================================
    // Parser Errors (E3xx)
    // =========================================================================
    /// Missing main tier (speaker line) in utterance.
    #[code("E301")]
    MissingMainTier,
    /// Expected tree-sitter node is missing.
    #[code("E302")]
    MissingNode,
    /// General syntax error in CHAT input.
    #[code("E303")]
    SyntaxError,
    /// Missing speaker code on main tier line.
    #[code("E304")]
    MissingSpeaker,
    /// Missing utterance terminator (`.`, `?`, `!`, etc.).
    #[code("E305")]
    MissingTerminator,
    /// Utterance contains no words or content.
    #[code("E306")]
    EmptyUtterance,
    /// Speaker code is syntactically invalid.
    #[code("E307")]
    InvalidSpeaker,
    /// Speaker code is not declared in `@Participants`.
    #[code("E308")]
    UndeclaredSpeaker,
    /// Unexpected syntax encountered during parsing.
    #[code("E309")]
    UnexpectedSyntax,
    /// Parser failed to produce a valid parse tree.
    #[code("E310")]
    ParseFailed,
    /// Unexpected node type in the parse tree.
    #[code("E311")]
    UnexpectedNode,
    /// Unclosed bracket in annotation or word content.
    #[code("E312")]
    UnclosedBracket,
    /// Unclosed parenthesis in annotation or word content.
    #[code("E313")]
    UnclosedParenthesis,
    /// Annotation is syntactically incomplete.
    #[code("E314")]
    IncompleteAnnotation,
    /// Invalid control character in input.
    #[code("E315")]
    InvalidControlCharacter,
    /// Content could not be parsed.
    #[code("E316")]
    UnparsableContent,
    /// Line could not be parsed.
    #[code("E319")]
    UnparsableLine,
    /// Header line could not be parsed.
    #[code("E320")]
    UnparsableHeader,
    /// Utterance could not be parsed.
    #[code("E321")]
    UnparsableUtterance,
    /// Empty colon with no content following it.
    #[code("E322")]
    EmptyColon,
    /// Missing colon after speaker code.
    #[code("E323")]
    MissingColonAfterSpeaker,
    /// Unrecognized error in utterance content.
    #[code("E324")]
    UnrecognizedUtteranceError,
    /// Unexpected child node in utterance.
    #[code("E325")]
    UnexpectedUtteranceChild,
    /// Unexpected line type in CHAT file.
    #[code("E326")]
    UnexpectedLineType,
    /// Error during tree-sitter CST traversal.
    #[code("E330")]
    TreeParsingError,
    /// Unexpected node encountered in a specific parsing context.
    #[code("E331")]
    UnexpectedNodeInContext,
    /// Unknown base content type in word.
    #[code("E340")]
    UnknownBaseContent,
    /// Unbalanced quotation marks spanning across utterances.
    #[code("E341")]
    UnbalancedQuotationCrossUtterance,
    /// Tree-sitter inserted a MISSING placeholder for a required element.
    #[code("E342")]
    MissingRequiredElement,
    /// Invalid nesting of scoped annotations.
    #[code("E344")]
    InvalidContentAnnotationNesting,
    /// Unmatched scoped annotation end marker.
    #[code("E346")]
    UnmatchedContentAnnotationEnd,
    /// Unbalanced overlap markers.
    #[code("E347")]
    UnbalancedOverlap,
    /// Missing overlap end marker.
    #[code("E348")]
    MissingOverlapEnd,
    /// Missing opening quotation mark.
    #[code("E351")]
    MissingQuoteBegin,
    /// Missing closing quotation mark.
    #[code("E352")]
    MissingQuoteEnd,
    /// Missing context for other-completion annotation.
    #[code("E353")]
    MissingOtherCompletionContext,
    /// Missing trailing-off terminator.
    #[code("E354")]
    MissingTrailingOffTerminator,
    /// Interleaved scoped annotations (overlapping scopes).
    #[code("E355")]
    InterleavedContentAnnotations,
    /// Unmatched underline begin marker.
    #[code("E356")]
    UnmatchedUnderlineBegin,
    /// Unmatched underline end marker.
    #[code("E357")]
    UnmatchedUnderlineEnd,
    /// Unmatched long feature begin marker.
    #[code("E358")]
    UnmatchedLongFeatureBegin,
    /// Unmatched long feature end marker.
    #[code("E359")]
    UnmatchedLongFeatureEnd,
    /// Invalid media bullet format.
    #[code("E360")]
    InvalidMediaBullet,
    /// Invalid timestamp value in media bullet.
    #[code("E361")]
    InvalidTimestamp,
    /// Timestamp end is before start (backwards range).
    #[code("E362")]
    TimestampBackwards,
    /// Invalid postcode format.
    #[code("E363")]
    InvalidPostcode,
    /// Malformed word content.
    #[code("E364")]
    MalformedWordContent,
    /// Malformed tier content.
    #[code("E365")]
    MalformedTierContent,
    /// Long feature begin/end labels do not match.
    #[code("E366")]
    LongFeatureLabelMismatch,
    /// Unmatched nonvocal begin marker.
    #[code("E367")]
    UnmatchedNonvocalBegin,
    /// Unmatched nonvocal end marker.
    #[code("E368")]
    UnmatchedNonvocalEnd,
    /// Nonvocal begin/end labels do not match.
    #[code("E369")]
    NonvocalLabelMismatch,
    /// Structural ordering error in utterance elements.
    #[code("E370")]
    StructuralOrderError,
    /// Pause marker inside a phonological group.
    #[code("E371")]
    PauseInPhoGroup,
    /// Nested quotation (quotation inside quotation).
    #[code("E372")]
    NestedQuotation,
    /// Invalid overlap index value.
    #[code("E373")]
    InvalidOverlapIndex,
    /// Failed to parse scoped annotation content.
    #[code("E375")]
    ContentAnnotationParseError,
    /// Failed to parse replacement annotation content.
    #[code("E376")]
    ReplacementParseError,
    /// Failed to parse `%mor` tier content.
    #[code("E382")]
    MorParseError,
    /// Replacement `[: text]` on fragment or phonological fragment (`&+`).
    #[code("E387")]
    ReplacementOnFragment,
    /// Replacement `[: text]` on nonword (`&~`).
    #[code("E388")]
    ReplacementOnNonword,
    /// Replacement `[: text]` on filler (`&-`).
    #[code("E389")]
    ReplacementOnFiller,
    /// Replacement text contains an omission (`0word`).
    #[code("E390")]
    ReplacementContainsOmission,
    /// Replacement text contains untranscribed marker (`xxx`/`yyy`/`www`).
    #[code("E391")]
    ReplacementContainsUntranscribed,

    // =========================================================================
    // Word Errors (E2xx)
    // =========================================================================
    /// Missing form type on special word.
    #[code("E202")]
    MissingFormType,
    /// Invalid form type value.
    #[code("E203")]
    InvalidFormType,
    /// Unknown annotation type in word.
    #[code("E207")]
    UnknownAnnotation,
    /// Replacement annotation is empty.
    #[code("E208")]
    EmptyReplacement,
    /// Spoken content portion of word is empty.
    #[code("E209")]
    EmptySpokenContent,
    /// Illegal replacement for fragment (deprecated: use E387).
    #[code("E210")]
    IllegalReplacementForFragment,
    /// Invalid word format.
    #[code("E212")]
    InvalidWordFormat,
    /// Untranscribed marker in replacement (deprecated: use E391).
    #[code("E213")]
    UntranscribedInReplacement,
    /// Empty annotated scoped annotations.
    #[code("E214")]
    EmptyAnnotatedContentAnnotations,
    /// Illegal digits in word content.
    #[code("E220")]
    IllegalDigits,
    /// Unbalanced CA (Conversation Analysis) delimiter.
    #[code("E230")]
    UnbalancedCADelimiter,
    /// Unbalanced shortening markers.
    #[code("E231")]
    UnbalancedShortening,
    /// Invalid compound marker position.
    #[code("E232")]
    InvalidCompoundMarkerPosition,
    /// Empty part in compound word.
    #[code("E233")]
    EmptyCompoundPart,
    /// Illegal use of untranscribed marker.
    #[code("E241")]
    IllegalUntranscribed,
    /// Unbalanced quotation marks within a word.
    #[code("E242")]
    UnbalancedQuotation,
    /// Illegal characters in word content.
    #[code("E243")]
    IllegalCharactersInWord,
    /// Consecutive stress markers in word.
    #[code("E244")]
    ConsecutiveStressMarkers,
    /// Stress marker not placed before spoken material.
    #[code("E245")]
    StressNotBeforeSpokenMaterial,
    /// Lengthening marker not placed after spoken material.
    #[code("E246")]
    LengtheningNotAfterSpokenMaterial,
    /// Multiple primary stress markers in one word.
    #[code("E247")]
    MultiplePrimaryStress,
    /// Tertiary language needs an explicit language code.
    #[code("E248")]
    TertiaryLanguageNeedsExplicitCode,
    /// Missing language context for language-tagged word.
    #[code("E249")]
    MissingLanguageContext,
    /// Secondary stress marker without primary stress.
    #[code("E250")]
    SecondaryStressWithoutPrimary,
    /// Word content text is empty.
    #[code("E251")]
    EmptyWordContentText,
    /// Syllable pause not between spoken material.
    #[code("E252")]
    SyllablePauseNotBetweenSpokenMaterial,
    /// Word content is empty.
    #[code("E253")]
    EmptyWordContent,
    /// Explicit word language marker uses a language absent from `@Languages`.
    #[code("E254")]
    UndeclaredExplicitWordLanguage,
    /// Whole-utterance language switch should use `[- LANG]` instead of tagging every word with `@s`.
    #[code("E255")]
    WholeUtteranceLanguageSwitchShouldUsePrecode,
    /// Consecutive commas (`,,`) — should use single comma or `‚` (CLAN CHECK 107)
    #[code("E258")]
    ConsecutiveCommas,
    /// Comma after non-spoken content (paralinguistic event, filler, nonword, placeholder, omitted word)
    #[code("E259")]
    CommaAfterNonSpokenContent,

    // =========================================================================
    // Dependent Tier Structural Errors (E4xx)
    // =========================================================================
    /// Duplicate dependent tier on same utterance.
    #[code("E401")]
    DuplicateDependentTier,
    /// Dependent tier without a preceding main tier.
    #[code("E404")]
    OrphanedDependentTier,

    // =========================================================================
    // Header Errors (E5xx)
    // =========================================================================
    /// Duplicate header line.
    #[code("E501")]
    DuplicateHeader,
    /// Missing `@End` header.
    #[code("E502")]
    MissingEndHeader,
    /// Missing `@UTF8` header.
    #[code("E503")]
    MissingUTF8Header,
    /// Missing required header (e.g., `@Participants`).
    #[code("E504")]
    MissingRequiredHeader,
    /// Invalid `@ID` header format.
    #[code("E505")]
    InvalidIDFormat,
    /// Empty `@Participants` header.
    #[code("E506")]
    EmptyParticipantsHeader,
    /// Empty `@Languages` header.
    #[code("E507")]
    EmptyLanguagesHeader,
    /// Empty `@Date` header.
    #[code("E508")]
    EmptyDateHeader,
    /// Empty `@Media` header.
    #[code("E509")]
    EmptyMediaHeader,
    /// Empty language field in `@ID` header.
    #[code("E510")]
    EmptyIDLanguage,
    /// Empty speaker field in `@ID` header.
    #[code("E511")]
    EmptyIDSpeaker,
    /// Empty participant code in `@Participants`.
    #[code("E512")]
    EmptyParticipantCode,
    /// Empty participant role in `@Participants`.
    #[code("E513")]
    EmptyParticipantRole,
    /// Empty role field in `@ID` header.
    #[code("E515")]
    EmptyIDRole,
    /// Empty date value.
    #[code("E516")]
    EmptyDate,
    /// Invalid age format in `@ID` header.
    #[code("E517")]
    InvalidAgeFormat,
    /// Invalid date format.
    #[code("E518")]
    InvalidDateFormat,
    /// Invalid ISO 639 language code.
    #[code("E519")]
    InvalidLanguageCode,
    /// Speaker code not defined in `@Participants`.
    #[code("E522")]
    SpeakerNotDefined,
    /// `@ID` header references an undeclared participant.
    #[code("E523")]
    OrphanIDHeader,
    /// `@Birth` header references an unknown participant.
    #[code("E524")]
    BirthUnknownParticipant,
    /// Unknown or unrecognized header type.
    #[code("E525")]
    UnknownHeader,
    /// Unmatched `@Bg` (begin gem) without corresponding `@Eg`.
    #[code("E526")]
    UnmatchedBeginGem,
    /// Unmatched `@Eg` (end gem) without corresponding `@Bg`.
    #[code("E527")]
    UnmatchedEndGem,
    /// Gem begin/end labels do not match.
    #[code("E528")]
    GemLabelMismatch,
    /// Nested `@Bg` (begin gem inside existing gem scope).
    #[code("E529")]
    NestedBeginGem,
    /// Lazy gem (`@G`) used inside an explicit gem scope.
    #[code("E530")]
    LazyGemInsideScope,
    /// `@Media` filename does not match the file being parsed.
    #[code("E531")]
    MediaFilenameMismatch,
    /// Invalid participant role value.
    #[code("E532")]
    InvalidParticipantRole,
    /// Empty `@Options` header.
    #[code("E533")]
    EmptyOptionsHeader,
    /// Unsupported `@Options` value.
    #[code("E534")]
    UnsupportedOption,
    /// Unsupported `@Media` type (not `audio`, `video`, or `missing`).
    #[code("E535")]
    UnsupportedMediaType,
    /// Unsupported `@Media` status value.
    #[code("E536")]
    UnsupportedMediaStatus,
    /// Unsupported `@Number` value.
    #[code("E537")]
    UnsupportedNumber,
    /// Unsupported `@Recording Quality` value.
    #[code("E538")]
    UnsupportedRecordingQuality,
    /// Unsupported `@Transcription` value.
    #[code("E539")]
    UnsupportedTranscription,
    /// Invalid `@Time Duration` format.
    #[code("E540")]
    InvalidTimeDuration,
    /// Invalid `@Time Start` format.
    #[code("E541")]
    InvalidTimeStart,
    /// Unsupported `@ID` sex value (not `male` or `female`).
    #[code("E542")]
    UnsupportedSex,
    /// Header out of canonical order (e.g., `@Options` before `@Participants`).
    #[code("E543")]
    HeaderOutOfOrder,
    /// `@Media` header declares linkage (no `unlinked` / `missing` /
    /// `notrans` status) but the transcript carries no timing evidence
    /// (no bullets on any utterance, no `%wor` bullets, no `@Bg`/`@Eg`
    /// time range). Reinstates the legacy Java-Chatter check.
    #[code("E544")]
    MediaLinkageWithoutTiming,
    /// Invalid `@Birth of` date format — must match `DD-MMM-YYYY`
    /// per CLAN `depfile.cut`'s `@d<dd-lll-yyyy>` template.
    #[code("E545")]
    InvalidBirthDateFormat,
    /// Unsupported `@ID` SES value.
    #[code("E546")]
    UnsupportedSesValue,

    // =========================================================================
    // Tier Errors (E6xx)
    // =========================================================================
    /// Generic tier validation error.
    #[code("E600")]
    TierValidationError,
    /// Invalid dependent tier name or format.
    #[code("E601")]
    InvalidDependentTier,
    /// Malformed tier header line.
    #[code("E602")]
    MalformedTierHeader,
    /// Invalid `%tim` tier format.
    #[code("E603")]
    InvalidTimTierFormat,
    /// `%gra` tier present without corresponding `%mor` tier.
    #[code("E604")]
    GraWithoutMor,
    /// Unsupported dependent tier (not a standard `%` tier or `%x` user-defined tier).
    #[code("E605")]
    UnsupportedDependentTier,

    // =========================================================================
    // Temporal/Media Bullet Errors (E7xx)
    // =========================================================================
    /// Unexpected node in tier content.
    #[code("E700")]
    UnexpectedTierNode,
    /// Tier begin time is not monotonically increasing (CLAN Error 83).
    #[code("E701")]
    TierBeginTimeNotMonotonic,
    /// Invalid morphology format on `%mor` tier.
    #[code("E702")]
    InvalidMorphologyFormat,
    /// Unexpected node in morphology tier.
    #[code("E703")]
    UnexpectedMorphologyNode,
    /// Speaker overlaps with themselves (CLAN Error 133).
    #[code("E704")]
    SpeakerSelfOverlap,
    /// `%mor` tier has fewer words than main tier.
    #[code("E705")]
    MorCountMismatchTooFew,
    /// `%mor` tier has more words than main tier.
    #[code("E706")]
    MorCountMismatchTooMany,
    /// `%mor` tier terminator presence does not match main tier.
    #[code("E707")]
    MorTerminatorPresenceMismatch,
    /// Malformed grammar relation on `%gra` tier.
    #[code("E708")]
    MalformedGrammarRelation,
    /// Invalid index in grammar relation.
    #[code("E709")]
    InvalidGrammarIndex,
    /// Unexpected node in `%gra` tier.
    #[code("E710")]
    UnexpectedGrammarNode,
    /// `%mor` word has empty stem, POS category, prefix, or suffix.
    #[code("E711")]
    MorEmptyContent,
    /// `%gra` word index is out of range.
    #[code("E712")]
    GraInvalidWordIndex,
    /// `%gra` head index is out of range.
    #[code("E713")]
    GraInvalidHeadIndex,
    /// `%pho`, `%mod`, or `%wor` tier has fewer alignable words than main tier.
    #[code("E714")]
    PhoCountMismatchTooFew,
    /// `%pho`, `%mod`, or `%wor` tier has more alignable words than main tier.
    #[code("E715")]
    PhoCountMismatchTooMany,
    /// `%mor` tier terminator value does not match main tier.
    #[code("E716")]
    MorTerminatorValueMismatch,
    /// `%sin` tier has fewer words than main tier.
    #[code("E718")]
    SinCountMismatchTooFew,
    /// `%sin` tier has more words than main tier.
    #[code("E719")]
    SinCountMismatchTooMany,
    /// `%mor` and `%gra` tier word counts do not match.
    #[code("E720")]
    MorGraCountMismatch,
    /// `%gra` indices are not sequential.
    #[code("E721")]
    GraNonSequentialIndex,
    /// `%gra` tier has no ROOT relation.
    #[code("E722")]
    GraNoRoot,
    /// `%gra` tier has multiple ROOT relations.
    #[code("E723")]
    GraMultipleRoots,
    /// `%gra` tier contains a circular dependency.
    #[code("E724")]
    GraCircularDependency,
    /// `%modsyl` tier word count does not match `%mod` tier.
    #[code("E725")]
    ModsylModCountMismatch,
    /// `%phosyl` tier word count does not match `%pho` tier.
    #[code("E726")]
    PhosylPhoCountMismatch,
    /// `%phoaln` tier word count does not match `%mod` tier.
    #[code("E727")]
    PhoalnModCountMismatch,
    /// `%phoaln` tier word count does not match `%pho` tier.
    #[code("E728")]
    PhoalnPhoCountMismatch,
    /// Bullet start time overlaps with previous tier's end time (CLAN Error 84).
    ///
    /// Current tier's BEG is less than the previous tier's END, indicating
    /// overlapping timing. Unlike speaker self-overlap (E704), this applies
    /// across different speakers.
    #[code("E729")]
    BulletOverlap,
    /// Bullet timing gap exceeds threshold (CLAN Error 85).
    ///
    /// Gap between current tier's BEG and previous tier's END exceeds the
    /// acceptable discontinuity threshold. Only reported in bullet consistency
    /// mode (`+c0`).
    #[code("E730")]
    BulletGap,
    /// Speaker's bullet start time is before their own previous bullet end time
    /// (CLAN Error 133).
    ///
    /// Supplements the overlap-marker-based E704 with actual bullet timing
    /// check for same-speaker self-overlap.
    #[code("E731")]
    SpeakerBulletSelfOverlap,
    /// Missing bullet on tier when bullet consistency mode is active (CLAN Error 110).
    ///
    /// When `+c0` or `+c1` is specified, every main tier must have timing.
    #[code("E732")]
    MissingBullet,
    /// `%mod` tier has fewer words than main tier.
    ///
    /// The model-phonology tier (`%mod`) has fewer alignable tokens than the
    /// main-tier words. Each main-tier word must have a corresponding `%mod`
    /// token. This code is separate from E714 (`%pho`) because the two tiers
    /// represent distinct phonological layers.
    #[code("E733")]
    ModCountMismatchTooFew,
    /// `%mod` tier has more words than main tier.
    ///
    /// The model-phonology tier (`%mod`) has more alignable tokens than the
    /// main-tier words. Remove the extra `%mod` tokens so counts match.
    /// This code is separate from E715 (`%pho`) for the same reason as E733.
    #[code("E734")]
    ModCountMismatchTooMany,

    // =========================================================================
    // Warnings (Wxxx)
    // =========================================================================
    /// Speaker code not found in `@Participants` (non-fatal).
    #[code("W108")]
    SpeakerNotFoundInParticipants,
    /// Missing whitespace before content on main tier.
    #[code("W210")]
    MissingWhitespaceBeforeContent,
    /// Missing whitespace after overlap marker.
    #[code("W211")]
    MissingWhitespaceAfterOverlap,
    /// User-defined dependent tier is empty.
    #[code("W601")]
    EmptyUserDefinedTier,
    /// Unknown user-defined dependent tier name.
    #[code("W602")]
    UnknownUserDefinedTier,
    /// Legacy warning from older CHAT validation.
    #[code("W999")]
    LegacyWarning,

    // =========================================================================
    // Generic/Unknown (MUST be last for fallback in new())
    // =========================================================================
    /// Unknown or unrecognized error code (fallback).
    #[code("E999")]
    UnknownError,
}
