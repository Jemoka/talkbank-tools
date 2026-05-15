//! Token classification functions — determine token categories for parser dispatch.
//!
//! These functions translate grammar.js rule membership (e.g., which tokens are
//! terminators, linkers, annotations) into Rust discriminant checks. They are
//! used by the parser to decide which production to enter.

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};

/// Convert a `WordWithAnnotations` to the appropriate `ContentItem`.
///
/// If annotations include a retrace marker (`[/]`, `[//]`, etc.), ALL
/// annotations are moved to the retrace level — the word inside the
/// retrace gets no annotations. This matches grammar.js semantics
/// where annotations attach to `word_with_optional_annotations`, not
/// `standalone_word`.
pub fn word_to_content_item<'a>(word: WordWithAnnotations<'a>) -> ContentItem<'a> {
    let retrace_idx = word.annotations.iter().position(|a| a.is_retrace());
    if let Some(idx) = retrace_idx {
        let mut word = word;
        let ann = word.annotations.remove(idx);
        // `idx` came from `position(|a| a.is_retrace())` two lines
        // above; `retrace_kind()` returns `Some` for any annotation
        // that satisfies `is_retrace()`.
        #[allow(clippy::expect_used)]
        let kind = ann.retrace_kind().expect("is_retrace was true");
        // grammar.js: word_with_optional_annotations has replacement AND base_annotations
        // as separate fields. Replacement stays on the word; base_annotations (scoped
        // markers like [?], [!], [= text]) go to retrace level.
        // BUT: when replacement exists, scoped annotations go on the ReplacedWord,
        // not on the retrace (TreeSitterParser behavior: ReplacedWord.with_scoped_annotations).
        let has_replacement = word
            .annotations
            .iter()
            .any(|a| matches!(a, ParsedAnnotation::Replacement(_)));
        let mut retrace_annotations = Vec::new();
        let mut word_annotations = Vec::new();
        for ann in std::mem::take(&mut word.annotations) {
            match &ann {
                ParsedAnnotation::Replacement(_) => word_annotations.push(ann),
                _ if has_replacement => {
                    // When replacement exists, scoped annotations stay with the word
                    // (they become ReplacedWord's scoped_annotations in the model)
                    word_annotations.push(ann);
                }
                _ => retrace_annotations.push(ann),
            }
        }
        word.annotations = word_annotations;
        let retrace = Retrace {
            content: vec![ContentItem::Word(word)],
            kind,
            is_group: false,
            annotations: retrace_annotations,
        };
        ContentItem::Retrace(retrace)
    } else {
        ContentItem::Word(word)
    }
}

/// Convert a raw annotation token to a typed `ParsedAnnotation`.
pub fn token_to_parsed_annotation<'a>(tok: Token<'a>) -> ParsedAnnotation<'a> {
    match tok {
        Token::RetracePartial(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Partial),
        Token::RetraceComplete(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Complete),
        Token::RetraceMultiple(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Multiple),
        Token::RetraceReformulation(_) => {
            ParsedAnnotation::Retrace(RetraceKindParsed::Reformulation)
        }
        Token::ScopedStressing(_) => ParsedAnnotation::Stressing,
        Token::ScopedContrastiveStressing(_) => ParsedAnnotation::ContrastiveStressing,
        Token::ScopedUncertain(_) => ParsedAnnotation::Uncertain,
        Token::ExcludeMarker(_) => ParsedAnnotation::Exclude,
        Token::ErrorMarkerAnnotation(s) => ParsedAnnotation::Error(s),
        Token::OverlapPrecedes(s) => ParsedAnnotation::OverlapPrecedes(s),
        Token::OverlapFollows(s) => ParsedAnnotation::OverlapFollows(s),
        Token::ExplanationAnnotation(s) => ParsedAnnotation::Explanation(s),
        Token::ParaAnnotation(s) => ParsedAnnotation::Paralinguistic(s),
        Token::AltAnnotation(s) => ParsedAnnotation::Alternative(s),
        Token::PercentAnnotation(s) => ParsedAnnotation::PercentComment(s),
        Token::Replacement(s) => ParsedAnnotation::Replacement(s),
        Token::Langcode(s) => ParsedAnnotation::Langcode(s),
        Token::Postcode(s) => ParsedAnnotation::Postcode(s),
        // Caller-contract invariant: `token_to_parsed_annotation` is
        // only called from `parser/main_tier.rs` after
        // `is_annotation(d)` returns true. The token kinds enumerated
        // above exhaust `is_annotation`'s positive set (verified by
        // `parser/classify.rs::is_annotation` below). Reaching this
        // arm means `is_annotation` and the variant set drifted
        // apart — caught by the parser's golden-corpus equivalence
        // tests before merge.
        #[allow(clippy::panic)]
        other => panic!("token_to_parsed_annotation: unexpected {:?}", other.text()),
    }
}

// ── Token classification (from grammar.js rule definitions) ─────

pub fn is_terminator(d: Option<TokenDiscriminants>) -> bool {
    matches!(
        d,
        Some(
            TokenDiscriminants::Period
                | TokenDiscriminants::Question
                | TokenDiscriminants::Exclamation
                | TokenDiscriminants::TrailingOff
                | TokenDiscriminants::Interruption
                | TokenDiscriminants::SelfInterruption
                | TokenDiscriminants::InterruptedQuestion
                | TokenDiscriminants::BrokenQuestion
                | TokenDiscriminants::QuotedNewLine
                | TokenDiscriminants::QuotedPeriodSimple
                | TokenDiscriminants::SelfInterruptedQuestion
                | TokenDiscriminants::TrailingOffQuestion
                | TokenDiscriminants::BreakForCoding
        )
    )
}

pub fn is_linker(d: Option<TokenDiscriminants>) -> bool {
    matches!(
        d,
        Some(
            TokenDiscriminants::LinkerLazyOverlap
                | TokenDiscriminants::LinkerQuickUptake
                | TokenDiscriminants::LinkerQuickUptakeOverlap
                | TokenDiscriminants::LinkerQuotationFollows
                | TokenDiscriminants::LinkerSelfCompletion
                | TokenDiscriminants::CaNoBreakLinker
                | TokenDiscriminants::CaTechnicalBreakLinker
        )
    )
}

pub fn is_annotation(d: Option<TokenDiscriminants>) -> bool {
    matches!(
        d,
        Some(
            TokenDiscriminants::RetraceComplete
                | TokenDiscriminants::RetracePartial
                | TokenDiscriminants::RetraceMultiple
                | TokenDiscriminants::RetraceReformulation
                | TokenDiscriminants::ScopedStressing
                | TokenDiscriminants::ScopedContrastiveStressing
                | TokenDiscriminants::ScopedUncertain
                | TokenDiscriminants::ExcludeMarker
                | TokenDiscriminants::ErrorMarkerAnnotation
                | TokenDiscriminants::OverlapPrecedes
                | TokenDiscriminants::OverlapFollows
                | TokenDiscriminants::ExplanationAnnotation
                | TokenDiscriminants::ParaAnnotation
                | TokenDiscriminants::AltAnnotation
                | TokenDiscriminants::PercentAnnotation
                | TokenDiscriminants::Replacement
                | TokenDiscriminants::Langcode
        )
    )
}

pub fn is_separator(d: Option<TokenDiscriminants>) -> bool {
    matches!(
        d,
        Some(
            TokenDiscriminants::Comma
                | TokenDiscriminants::Semicolon
                | TokenDiscriminants::Colon
                | TokenDiscriminants::CaContinuationMarker
                | TokenDiscriminants::TagMarker
                | TokenDiscriminants::VocativeMarker
                | TokenDiscriminants::UnmarkedEnding
                | TokenDiscriminants::UptakeSymbol
                | TokenDiscriminants::CaNoBreak
                | TokenDiscriminants::CaTechnicalBreak
                | TokenDiscriminants::RisingToHigh
                | TokenDiscriminants::RisingToMid
                | TokenDiscriminants::LevelPitch
                | TokenDiscriminants::FallingToMid
                | TokenDiscriminants::FallingToLow
        )
    )
}

pub fn is_pause(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::PauseLong
            | TokenDiscriminants::PauseMedium
            | TokenDiscriminants::PauseShort
            | TokenDiscriminants::PauseTimed
    )
}

pub fn is_word_start(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::WordSegment
            | TokenDiscriminants::Zero
            | TokenDiscriminants::PrefixFiller
            | TokenDiscriminants::PrefixNonword
            | TokenDiscriminants::PrefixFragment
            | TokenDiscriminants::Shortening
            | TokenDiscriminants::StressPrimary
            | TokenDiscriminants::StressSecondary
            | TokenDiscriminants::Ampersand
            // CA markers can start a word (standalone or preceding text)
            | TokenDiscriminants::CaBlockedSegments | TokenDiscriminants::CaConstriction
            | TokenDiscriminants::CaHardening | TokenDiscriminants::CaHurriedStart
            | TokenDiscriminants::CaInhalation | TokenDiscriminants::CaLaughInWord
            | TokenDiscriminants::CaPitchDown | TokenDiscriminants::CaPitchReset
            | TokenDiscriminants::CaPitchUp | TokenDiscriminants::CaSuddenStop
            | TokenDiscriminants::CaUnsure | TokenDiscriminants::CaPrecise
            | TokenDiscriminants::CaCreaky | TokenDiscriminants::CaSofter
            | TokenDiscriminants::CaSegmentRepetition | TokenDiscriminants::CaFaster
            | TokenDiscriminants::CaSlower | TokenDiscriminants::CaWhisper
            | TokenDiscriminants::CaSinging | TokenDiscriminants::CaLowPitch
            | TokenDiscriminants::CaHighPitch | TokenDiscriminants::CaLouder
            | TokenDiscriminants::CaSmileVoice | TokenDiscriminants::CaBreathyVoice
            | TokenDiscriminants::CaYawn
    )
}

pub fn is_word_token(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::WordSegment
            | TokenDiscriminants::Zero
            | TokenDiscriminants::PrefixFiller
            | TokenDiscriminants::PrefixNonword
            | TokenDiscriminants::PrefixFragment
            | TokenDiscriminants::Shortening
            | TokenDiscriminants::Lengthening
            | TokenDiscriminants::StressPrimary | TokenDiscriminants::StressSecondary
            | TokenDiscriminants::CompoundMarker
            | TokenDiscriminants::OverlapTopBegin | TokenDiscriminants::OverlapTopEnd | TokenDiscriminants::OverlapBottomBegin | TokenDiscriminants::OverlapBottomEnd
            | TokenDiscriminants::SyllablePause
            | TokenDiscriminants::Tilde
            // Note: UnderlineBegin/End are NOT word tokens — they're content-level markers
            | TokenDiscriminants::CaBlockedSegments | TokenDiscriminants::CaConstriction
            | TokenDiscriminants::CaHardening | TokenDiscriminants::CaHurriedStart
            | TokenDiscriminants::CaInhalation | TokenDiscriminants::CaLaughInWord
            | TokenDiscriminants::CaPitchDown | TokenDiscriminants::CaPitchReset
            | TokenDiscriminants::CaPitchUp | TokenDiscriminants::CaSuddenStop
            | TokenDiscriminants::CaUnsure | TokenDiscriminants::CaPrecise
            | TokenDiscriminants::CaCreaky | TokenDiscriminants::CaSofter
            | TokenDiscriminants::CaSegmentRepetition | TokenDiscriminants::CaFaster
            | TokenDiscriminants::CaSlower | TokenDiscriminants::CaWhisper
            | TokenDiscriminants::CaSinging | TokenDiscriminants::CaLowPitch
            | TokenDiscriminants::CaHighPitch | TokenDiscriminants::CaLouder
            | TokenDiscriminants::CaSmileVoice | TokenDiscriminants::CaBreathyVoice
            | TokenDiscriminants::CaYawn
            | TokenDiscriminants::FormMarker
            | TokenDiscriminants::WordLangSuffix
            | TokenDiscriminants::PosTag
            | TokenDiscriminants::Ampersand
    )
}
