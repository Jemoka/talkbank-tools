//! Canonical domain model for CHAT format
//!
//! This module defines the data structures representing parsed CHAT transcripts.
//!
//! # Module Organization
//!
//! - `file/` - Document-level structures (ChatFile, Line, Utterance)
//! - `header/` - @ header lines (Header, IDHeader, MediaHeader)
//! - `content/` - Main tier content (Word, Pause, Event, Action, etc.)
//! - `annotation/` - Square bracket annotations `[*]`, `[: word]`, etc.
//! - `dependent_tier/` - % dependent tiers (MOR, GRA, PHO, etc.)
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Design Principles
//!
//! - **Type-safe**: Use enums and structs to encode CHAT semantics
//! - **Pure data**: Model contains only data, validation is separate
//! - **Serializable**: All types are JSON-serializable via serde
//! - **Composable**: Granular types that can be parsed independently
//!
//! # Data Contract Rules
//!
//! - Avoid dummy/sentinel values in data structures; unknown states should be explicit in types
//! - Prefer explicit domain enums over `Option` when `None` would carry ambiguous meaning
//! - Use explicit state enums for runtime metadata (e.g., `Uncomputed` vs `Computed`)
//! - Keep parse-origin metadata (`parse_health`, language metadata) in the model for downstream tooling
//! - Any new field added for parser recovery must have clear semantics and tests

// Core trait for CHAT serialization
mod write_chat;
pub use write_chat::WriteChat;

// Semantic equality trait for roundtrip testing
mod semantic_eq;
pub use semantic_eq::SemanticEq;
mod semantic_diff;
pub use semantic_diff::{
    DEFAULT_MAX_DIFFS, PathSegment, RenderMode, SemanticDiff, SemanticDiffContext,
    SemanticDiffKind, SemanticDiffReport, SemanticDifference, SemanticPath, normalize_span,
    normalize_span_option,
};
mod validation_tag;
pub use validation_tag::{ValidationTag, ValidationTagged};

// Shared macros used across model modules
mod macros;

// Organized submodules
pub mod annotation;
pub mod content;
pub mod dependent_tier;
pub mod file;
pub mod header;
// Standalone modules
mod alignment_set;
mod intern;
mod language_metadata;
mod non_empty_string;
mod participant;
mod provenance;
mod time;
mod user_defined_tier;

// Re-export NonEmptyString
pub use non_empty_string::NonEmptyString;
pub use time::MediaTiming;

// Re-export provenance types
pub use provenance::{
    AsrWordsJson, LanguageId, Morphosyntax, NlpResponse, NlpResponseJson, NlpTokens, Provenance,
    RawChatText, TierDomainMarker, TokenizedWords, TranscriptJson,
};

// Re-export interning functions
pub use intern::{
    language_interner, participant_interner, pos_interner, speaker_interner, stem_interner,
};

// Re-export file types
pub use file::{
    ChatFile, ChatFileLines, Line, ParseHealth, ParseHealthState, ParseHealthTier, Utterance,
    UtteranceLanguage, UtteranceLanguageMetadata,
};

// Re-export participant type
pub use participant::Participant;

// Re-export header types
pub use header::{
    ActivitiesDescription,
    ActivityType,
    AgeValue,
    BackgroundDescription,
    BirthplaceDescription,
    ChatDate,
    ChatOptionFlag,
    ChatOptionFlags,
    ColorWordList,
    CorpusName,
    CustomIdField,
    DesignType,
    EducationDescription,
    Ethnicity,
    FontSpec,
    GemLabel,
    GroupName,
    GroupType,
    Header,
    IDHeader,
    LanguageCode,
    LanguageCodes,
    LanguageName,
    LocationDescription,
    MediaHeader,
    MediaStatus,
    MediaType,
    Month,
    Number,
    PageNumber,
    ParticipantEntries,
    ParticipantEntry,
    ParticipantName,
    ParticipantRole,
    PidValue,
    RecordingQuality,
    RoomLayoutDescription,
    SesCode,
    SesValue,
    Sex,
    SituationDescription,
    SpeakerCode,
    TDescription,
    TapeLocationDescription,
    TimeDurationValue,
    TimeSegment,
    TimeStartValue,
    TimeValue,
    TranscriberName,
    Transcription,
    // @Types header components
    TypesHeader,
    VideoSpec,
    WarningText,
    WindowGeometry,
};

// Re-export content types
pub use content::{
    Action,
    BracketedContent,
    BracketedItem,
    Bullet,
    BulletSource,
    CADelimiter,
    CADelimiterType,
    CAElement,
    CAElementType,
    Event,
    EventType,
    FormType,
    Freecode,
    Group,
    Linker,
    LongFeatureBegin,
    LongFeatureEnd,
    // Labels and markers
    LongFeatureLabel,
    // Main tier structure
    MainTier,
    NonvocalBegin,
    NonvocalEnd,
    NonvocalLabel,
    NonvocalSimple,
    OtherSpokenEvent,
    OverlapIndex,
    OverlapPoint,
    OverlapPointKind,
    // Content items
    Pause,
    PauseDuration,
    PauseTimedDuration,
    PhoGroup,
    Postcode,
    Quotation,
    Retrace,
    RetraceKind,
    Separator,
    SinGroup,
    Terminator,
    TierContent,
    UnderlineMarker,
    UtteranceContent,
    // Word types
    Word,
    WordCategory,
    WordCliticBoundary,
    WordCompoundMarker,
    WordContent,
    WordContents,
    WordLanguageMarker,
    WordLengthening,
    WordShortening,
    WordStressMarker,
    WordStressMarkerType,
    WordSyllablePause,
    WordText,
    WordUnderlineBegin,
    WordUnderlineEnd,
};

// Re-export annotation types
pub use annotation::{
    Annotated, ContentAnnotation, OverlapMarkerIndex, ReplacedWord, Replacement, ScopedAlternative,
    ScopedError, ScopedExplanation, ScopedOverlapBegin, ScopedOverlapEnd, ScopedParalinguistic,
    ScopedPercentComment, ScopedUnknown,
};

// Re-export dependent tier types
pub use dependent_tier::{
    // ACT/COD
    ActTier,
    // Text tiers
    AddTier,
    // Bullet content
    BulletContent,
    BulletContentBullet,
    BulletContentPicture,
    BulletContentSegment,
    BulletContentText,
    CodTier,
    ComTier,
    // Dependent tier enum
    DependentTier,
    ExpTier,
    GpxTier,
    GraTier,
    GraTierType,
    // GRA
    GrammaticalRelation,
    GrammaticalRelationType,
    IntTier,
    // MOR
    Mor,
    MorChunk,
    MorChunkKind,
    MorFeature,
    MorStem,
    MorTier,
    MorTierType,
    MorWord,
    PhoItem,
    PhoTier,
    PhoTierType,
    // PHO
    PhoWord,
    PosCategory,
    // SIN
    SinGroupGestures,
    SinItem,
    SinTier,
    SinToken,
    SitTier,
    SpaTier,
    TextTier,
    UserDefinedDependentTier,
    WorTier,
};

// Re-export alignment metadata
pub use alignment_set::{AlignmentSet, AlignmentUnit, AlignmentUnits};

// Re-export language metadata
pub use language_metadata::{
    LanguageMetadata, LanguageSource, WordLanguageInfo, WordLanguageInfos,
};

// Re-export user-defined tier types
pub use user_defined_tier::{UserDefinedTier, UserDefinedTierLabel};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use talkbank_derive::ValidationTagged;

    /// Minimal utterance fixture wires required main-tier fields correctly.
    ///
    /// This test protects the basic `Utterance::new` contract used across parser outputs.
    #[test]
    fn test_simple_utterance_model() {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "hello", "hello",
            )))],
            Terminator::Period { span: Span::DUMMY },
        );

        let utterance = Utterance::new(main);

        assert_eq!(utterance.main.speaker, "CHI".into());
        assert_eq!(utterance.main.content.content.len(), 1);
        assert!(utterance.mor_tier().is_none());
    }

    /// Word builders preserve raw text, cleaned text, form type, and content structure.
    ///
    /// The assertions cover a representative multi-component lexical token.
    #[test]
    fn test_word_structure() {
        let word = Word::new_unchecked("hel(lo)@b", "hello")
            .with_form_type(FormType::B)
            .with_content(vec![
                WordContent::Text(WordText::new_unchecked("hel")),
                WordContent::Shortening(WordShortening::new_unchecked("lo")),
            ]);

        assert_eq!(word.raw_text, "hel(lo)@b");
        assert_eq!(word.cleaned_text(), "hello");
        assert_eq!(word.form_type, Some(FormType::B));
        assert_eq!(word.content.len(), 2);
    }

    /// Main-tier JSON serialization includes core speaker and content fields.
    ///
    /// This is a smoke test for serde wiring on a common top-level model type.
    #[test]
    fn test_json_serialization() -> Result<(), String> {
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "hello", "hello",
            )))],
            Terminator::Period { span: Span::DUMMY },
        );

        let json = serde_json::to_string_pretty(&main)
            .map_err(|err| format!("Failed to serialize main tier: {err}"))?;
        assert!(json.contains("CHI"));
        assert!(json.contains("hello"));
        Ok(())
    }

    /// Demo enum used to validate `ValidationTagged` defaults/overrides.
    #[derive(ValidationTagged)]
    enum DemoValidationState {
        Clean,
        ParseError,
        DeferredWarning,
        #[validation_tag(error)]
        ExplicitProblem,
    }

    /// `ValidationTagged` derives convention-based tags with explicit overrides.
    ///
    /// The enum fixture covers clean, warning, inferred error, and annotated error cases.
    #[test]
    fn test_validation_tagged_convention_and_annotation() {
        assert_eq!(
            DemoValidationState::Clean.validation_tag(),
            ValidationTag::Clean
        );
        assert_eq!(
            DemoValidationState::ParseError.validation_tag(),
            ValidationTag::Error
        );
        assert_eq!(
            DemoValidationState::DeferredWarning.validation_tag(),
            ValidationTag::Warning
        );
        assert_eq!(
            DemoValidationState::ExplicitProblem.validation_tag(),
            ValidationTag::Error
        );
        assert!(DemoValidationState::ParseError.is_validation_error());
        assert!(DemoValidationState::DeferredWarning.has_validation_issue());
    }

    /// Demo enum for `Unsupported` naming convention.
    #[derive(ValidationTagged)]
    #[allow(dead_code)]
    enum DemoUnsupportedState {
        Known,
        Unsupported(String),
    }

    /// `Unsupported` variant maps to `Warning` by naming convention.
    #[test]
    fn test_validation_tagged_unsupported_convention() {
        assert_eq!(
            DemoUnsupportedState::Known.validation_tag(),
            ValidationTag::Clean
        );
        assert_eq!(
            DemoUnsupportedState::Unsupported("x".into()).validation_tag(),
            ValidationTag::Warning
        );
        assert!(DemoUnsupportedState::Unsupported("x".into()).has_validation_issue());
        assert!(DemoUnsupportedState::Unsupported("x".into()).is_validation_warning());
        assert!(!DemoUnsupportedState::Known.has_validation_issue());
    }

    /// Existing model enums with `Unsupported` get `Warning` tag via derive.
    #[test]
    fn test_existing_enums_validation_tagged() {
        assert!(RecordingQuality::from_text("1").validation_tag() == ValidationTag::Clean);
        assert!(RecordingQuality::from_text("unknown").has_validation_issue());
        assert!(MediaType::from_text("audio").validation_tag() == ValidationTag::Clean);
        assert!(MediaType::from_text("unknown").has_validation_issue());
        assert!(Number::from_text("1").validation_tag() == ValidationTag::Clean);
        assert!(Number::from_text("unknown").has_validation_issue());
    }
}
