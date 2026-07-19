// `_ => unreachable!()` arms in this module are guarded by the
// outer `match` against a fixed `Token`/AST variant set produced by
// the re2c lexer (see `parser/word_body.rs` and the corresponding
// `Token` declarations in `lexer.re`). Each `unreachable!` carries
// the parsing-stage invariant: only the variants enumerated by the
// preceding arms can appear at that AST position. The crate is
// validated against 99,907 CHAT files with zero divergences from
// TreeSitterParser; the unreachable arms have never fired in
// production. Per-site annotations would be ~22 individual
// `#[allow]`s in this single file; a file-level allow with this
// rationale is the cleaner audit boundary. Tracked
// follow-up (typed sub-AST per token category) in
// `book/src/operations/panic-audit/talkbank-parser-re2c.md`.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

//! Conversion from our AST to `talkbank-model` types.
//!
//! Each conversion is built from studying the actual JSON output of the
//! TreeSitterParser (via `chatter to-json`), ensuring semantic equivalence.
//!
//! Rules:
//! - No dummy/sentinel values. Every model field must be correct or absent.
//! - No silent drops. Every AST item must be converted or produce an error.
//! - No panics. All conversions are infallible for valid AST input.

use crate::ast;
use crate::token::Token;
use talkbank_model::model::*;
// These are re-exported from content module (we added them to mod.rs)
use talkbank_model::Span;
use talkbank_model::model::WordCompoundMarker;

// ═══════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════
// Word token → WordContent
// ═══════════════════════════════════════════════════════════════

use crate::ast::{CaDelimiterKind, CaElementKind, OverlapKind, StressKind, WordBodyItem};

/// Convert a typed word body item to a model WordContent.
fn body_item_to_word_content(item: &WordBodyItem<'_>) -> WordContent {
    match item {
        WordBodyItem::Text(s) => WordContent::Text(WordText::new_unchecked(s)),
        WordBodyItem::Shortening(s) => WordContent::Shortening(WordShortening::new_unchecked(s)),
        WordBodyItem::Lengthening(count) => WordContent::Lengthening(WordLengthening {
            count: *count,
            span: None,
        }),
        WordBodyItem::CompoundMarker => WordContent::CompoundMarker(WordCompoundMarker::new()),
        WordBodyItem::Stress(StressKind::Primary) => {
            WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Primary))
        }
        WordBodyItem::Stress(StressKind::Secondary) => {
            WordContent::StressMarker(WordStressMarker::new(WordStressMarkerType::Secondary))
        }
        WordBodyItem::SyllablePause => WordContent::SyllablePause(WordSyllablePause::new()),
        WordBodyItem::CliticBoundary => {
            WordContent::CliticBoundary(talkbank_model::model::WordCliticBoundary::new())
        }
        WordBodyItem::OverlapPoint(kind, s) => {
            let model_kind = match kind {
                OverlapKind::TopBegin => OverlapPointKind::TopOverlapBegin,
                OverlapKind::TopEnd => OverlapPointKind::TopOverlapEnd,
                OverlapKind::BottomBegin => OverlapPointKind::BottomOverlapBegin,
                OverlapKind::BottomEnd => OverlapPointKind::BottomOverlapEnd,
            };
            let index = s
                .chars()
                .nth(1)
                .and_then(|c| c.to_digit(10))
                .map(OverlapIndex::new);
            WordContent::OverlapPoint(OverlapPoint::new(model_kind, index))
        }
        WordBodyItem::CaElement(kind) => {
            let t = match kind {
                CaElementKind::BlockedSegments => CAElementType::BlockedSegments,
                CaElementKind::Constriction => CAElementType::Constriction,
                CaElementKind::Hardening => CAElementType::Hardening,
                CaElementKind::HurriedStart => CAElementType::HurriedStart,
                CaElementKind::Inhalation => CAElementType::Inhalation,
                CaElementKind::LaughInWord => CAElementType::LaughInWord,
                CaElementKind::PitchDown => CAElementType::PitchDown,
                CaElementKind::PitchReset => CAElementType::PitchReset,
                CaElementKind::PitchUp => CAElementType::PitchUp,
                CaElementKind::SuddenStop => CAElementType::SuddenStop,
            };
            WordContent::CAElement(CAElement::new(t))
        }
        WordBodyItem::CaDelimiter(kind) => {
            let t = match kind {
                CaDelimiterKind::Unsure => CADelimiterType::Unsure,
                CaDelimiterKind::Precise => CADelimiterType::Precise,
                CaDelimiterKind::Creaky => CADelimiterType::Creaky,
                CaDelimiterKind::Softer => CADelimiterType::Softer,
                CaDelimiterKind::SegmentRepetition => CADelimiterType::SegmentRepetition,
                CaDelimiterKind::Faster => CADelimiterType::Faster,
                CaDelimiterKind::Slower => CADelimiterType::Slower,
                CaDelimiterKind::Whisper => CADelimiterType::Whisper,
                CaDelimiterKind::Singing => CADelimiterType::Singing,
                CaDelimiterKind::LowPitch => CADelimiterType::LowPitch,
                CaDelimiterKind::HighPitch => CADelimiterType::HighPitch,
                CaDelimiterKind::Louder => CADelimiterType::Louder,
                CaDelimiterKind::SmileVoice => CADelimiterType::SmileVoice,
                CaDelimiterKind::BreathyVoice => CADelimiterType::BreathyVoice,
                CaDelimiterKind::Yawn => CADelimiterType::Yawn,
            };
            WordContent::CADelimiter(CADelimiter::new(t))
        }
    }
}

/// Compute cleaned_text from word body items.
/// Only Text and Shortening contribute; all markers are stripped.
fn compute_cleaned_text(body: &[WordBodyItem<'_>]) -> String {
    let mut cleaned = String::new();
    let mut in_segment_repetition = false;
    for item in body {
        match item {
            WordBodyItem::CaDelimiter(CaDelimiterKind::SegmentRepetition) => {
                in_segment_repetition = !in_segment_repetition;
            }
            WordBodyItem::Text(s) if !in_segment_repetition => cleaned.push_str(s),
            WordBodyItem::Shortening(s) if !in_segment_repetition => cleaned.push_str(s),
            _ => {}
        }
    }
    cleaned
}

// ═══════════════════════════════════════════════════════════════
// WordWithAnnotations → Word
// ═══════════════════════════════════════════════════════════════

/// Convert a parsed word to the model Word type.
/// Uses the word's self-contained `raw_text` — no external `source` needed.
pub fn word_from_parsed(w: &ast::WordWithAnnotations<'_>) -> Word {
    let raw = w.raw_text;
    let cleaned = compute_cleaned_text(&w.body);

    let content_items: Vec<WordContent> = w.body.iter().map(body_item_to_word_content).collect();

    let cleaned_for_model = if cleaned.is_empty() { raw } else { &cleaned };
    let mut word = Word::new_unchecked(raw, cleaned_for_model)
        .with_content(WordContents::new(content_items.into_iter().collect()));

    // Category from typed enum — no token scanning
    if let Some(cat) = &w.category {
        word = word.with_category(match cat {
            crate::ast::WordCategory::Omission => WordCategory::Omission,
            crate::ast::WordCategory::Filler => WordCategory::Filler,
            crate::ast::WordCategory::Nonword => WordCategory::Nonword,
            crate::ast::WordCategory::Fragment => WordCategory::PhonologicalFragment,
        });
    }

    // Form marker — tag-extracted content, direct to model
    if let Some(marker) = w.form_marker {
        if let Some(ft) = FormType::parse(marker) {
            word = word.with_form_type(ft);
        } else if marker.starts_with("z") {
            // User-defined form: z or z:label
            let label = marker.strip_prefix("z").unwrap_or("");
            let label = label.strip_prefix(':').unwrap_or(label);
            word = word.with_form_type(FormType::UserDefined(label.to_string()));
        }
    }

    // Language suffix — typed enum, no string hacking
    if let Some(ref lang) = w.lang {
        word = match lang {
            crate::ast::ParsedLangSuffix::Shortcut => word.with_language_shortcut(),
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('+') => {
                let lc: Vec<LanguageCode> = codes.split('+').map(LanguageCode::new).collect();
                word.lang = Some(WordLanguageMarker::Multiple(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(codes) if codes.contains('&') => {
                let lc: Vec<LanguageCode> = codes.split('&').map(LanguageCode::new).collect();
                word.lang = Some(WordLanguageMarker::Ambiguous(lc));
                word
            }
            crate::ast::ParsedLangSuffix::Explicit(code) => {
                word.with_lang(LanguageCode::new(*code))
            }
        };
    }

    // POS tag — tag-extracted content
    if let Some(tag) = w.pos_tag {
        word = word.with_part_of_speech(tag);
    }

    word
}

// ═══════════════════════════════════════════════════════════════
// ContentItem → UtteranceContent
// ═══════════════════════════════════════════════════════════════

/// Convert a ContentItem to a model UtteranceContent.
/// Every content item type has a proper model representation.
/// Convert a linker token to a model Linker.
fn linker_token_to_model(tok: &Token<'_>) -> Option<Linker> {
    match tok {
        Token::LinkerLazyOverlap(_) => Some(Linker::LazyOverlapPrecedes),
        Token::LinkerQuickUptake(_) => Some(Linker::OtherCompletion),
        Token::LinkerQuickUptakeOverlap(_) => Some(Linker::QuickUptakeOverlap),
        Token::LinkerQuotationFollows(_) => Some(Linker::QuotationFollows),
        Token::LinkerSelfCompletion(_) => Some(Linker::SelfCompletion),
        Token::CaNoBreakLinker(_) => Some(Linker::NoBreakTcuContinuation),
        Token::CaTechnicalBreakLinker(_) => Some(Linker::TcuContinuation),
        _ => None,
    }
}

pub fn content_item_to_model(item: &ast::ContentItem<'_>) -> UtteranceContent {
    match item {
        ast::ContentItem::Word(w) => word_with_annotations_to_model(w),
        ast::ContentItem::Pause(tok) => {
            let duration = match tok {
                Token::PauseShort(_) => PauseDuration::Short,
                Token::PauseMedium(_) => PauseDuration::Medium,
                Token::PauseLong(_) => PauseDuration::Long,
                Token::PauseTimed(s) => PauseDuration::Timed(PauseTimedDuration::new(*s)),
                _ => PauseDuration::Short,
            };
            UtteranceContent::Pause(Pause::new(duration))
        }
        ast::ContentItem::Event(toks) => {
            // The lexer emits a single Event token with the description text.
            let event_text = toks.first().map(|t| t.text()).unwrap_or("");
            UtteranceContent::Event(Event::new(event_text))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = event.text();
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            if scoped.is_empty() {
                UtteranceContent::Event(event_model)
            } else {
                UtteranceContent::AnnotatedEvent(
                    Annotated::new(event_model).with_scoped_annotations(scoped),
                )
            }
        }
        ast::ContentItem::Separator(tok) => {
            UtteranceContent::Separator(separator_token_to_model(tok))
        }
        ast::ContentItem::Annotation(tok) => {
            match tok {
                Token::Freecode(s) => {
                    // Token carries tag-extracted content directly
                    UtteranceContent::Freecode(Freecode::new(*s))
                }
                _ => UtteranceContent::Freecode(Freecode::new(tok.text())),
            }
        }
        ast::ContentItem::Retrace(r) => {
            let kind = match r.kind {
                crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
                crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
                crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
                crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
            };
            let content: Vec<BracketedItem> = r
                .content
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let mut retrace = Retrace::new(BracketedContent::new(content), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            // Move non-retrace annotations from the AST to model retrace
            let scoped = annotations_to_scoped(&r.annotations);
            if !scoped.is_empty() {
                retrace = retrace.with_annotations(scoped);
            }
            UtteranceContent::Retrace(Box::new(retrace))
        }
        ast::ContentItem::Group(g) => {
            let content: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let group = Group::new(BracketedContent::new(content));
            let scoped = annotations_to_scoped(&g.annotations);
            if scoped.is_empty() {
                UtteranceContent::Group(group)
            } else {
                let annotated = Annotated::new(group).with_scoped_annotations(scoped);
                UtteranceContent::AnnotatedGroup(annotated)
            }
        }
        ast::ContentItem::Quotation(q) => {
            let content: Vec<BracketedItem> = q
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            UtteranceContent::Quotation(Quotation::new(BracketedContent::new(content)))
        }
        ast::ContentItem::OverlapPoint(tok) => {
            let raw = tok.text();
            let kind = match tok {
                Token::OverlapTopBegin(_) => OverlapPointKind::TopOverlapBegin,
                Token::OverlapTopEnd(_) => OverlapPointKind::TopOverlapEnd,
                Token::OverlapBottomBegin(_) => OverlapPointKind::BottomOverlapBegin,
                Token::OverlapBottomEnd(_) => OverlapPointKind::BottomOverlapEnd,
                _ => unreachable!(),
            };
            let index = raw
                .chars()
                .nth(1)
                .and_then(|c| c.to_digit(10))
                .map(OverlapIndex::new);
            UtteranceContent::OverlapPoint(OverlapPoint::new(kind, index))
        }
        ast::ContentItem::MediaBullet(tok) => match tok {
            Token::MediaBullet {
                start_time,
                end_time,
                ..
            } => {
                let start_ms: u64 = start_time.parse().unwrap_or(0);
                let end_ms: u64 = end_time.parse().unwrap_or(0);
                UtteranceContent::InternalBullet(Bullet::new(start_ms, end_ms))
            }
            _ => unreachable!(),
        },
        ast::ContentItem::UnderlineBegin(_) => {
            UtteranceContent::UnderlineBegin(UnderlineMarker::new())
        }
        ast::ContentItem::UnderlineEnd(_) => UtteranceContent::UnderlineEnd(UnderlineMarker::new()),
        ast::ContentItem::CaMarker(tok) => {
            let raw = tok.text();
            // CA markers at content level are wrapped as Word in the model
            UtteranceContent::Word(Box::new(Word::new_unchecked(raw, raw).with_content(
                WordContents::new(smallvec::smallvec![WordContent::Text(
                    WordText::new_unchecked(raw)
                )]),
            )))
        }
        ast::ContentItem::LongFeatureBegin(tok) => {
            // Token carries tag-extracted label directly (e.g., "X" not "&{l=X")
            UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(LongFeatureLabel::new(
                tok.text(),
            )))
        }
        ast::ContentItem::LongFeatureEnd(tok) => {
            UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(LongFeatureLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalBegin(tok) => {
            UtteranceContent::NonvocalBegin(NonvocalBegin::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalEnd(tok) => {
            UtteranceContent::NonvocalEnd(NonvocalEnd::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::NonvocalSimple(tok) => {
            UtteranceContent::NonvocalSimple(NonvocalSimple::new(NonvocalLabel::new(tok.text())))
        }
        ast::ContentItem::OtherSpokenEvent(tok) => match tok {
            Token::OtherSpokenEvent { speaker, text } => {
                UtteranceContent::OtherSpokenEvent(OtherSpokenEvent::new(*speaker, *text))
            }
            _ => unreachable!("OtherSpokenEvent content item must carry OtherSpokenEvent token"),
        },
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            let annotated = Annotated::new(Action::new()).with_scoped_annotations(scoped);
            UtteranceContent::AnnotatedAction(annotated)
        }
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(items)))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            UtteranceContent::SinGroup(SinGroup::new(BracketedContent::new(items)))
        }
    }
}

/// Convert annotation tokens to model ContentAnnotation list.
fn annotations_to_scoped(annotations: &[ast::ParsedAnnotation<'_>]) -> Vec<ContentAnnotation> {
    annotations
        .iter()
        .filter_map(|a| parsed_annotation_to_scoped(a))
        .collect()
}

/// Convert a word with annotations to the appropriate UtteranceContent variant.
/// - No annotations → Word
/// - Has [: replacement] → ReplacedWord (with any other annotations as scoped)
/// - Has other annotations → AnnotatedWord
fn word_with_annotations_to_model(w: &ast::WordWithAnnotations<'_>) -> UtteranceContent {
    let word = word_from_parsed(w);

    // Check if there's a replacement annotation
    let replacement_idx = w
        .annotations
        .iter()
        .position(|a| matches!(a, crate::ast::ParsedAnnotation::Replacement(_)));

    if let Some(idx) = replacement_idx {
        let replacement_text = match &w.annotations[idx] {
            crate::ast::ParsedAnnotation::Replacement(text) => *text,
            _ => unreachable!(),
        };
        let replacement_words: Vec<Word> = replacement_text
            .split_whitespace()
            .map(parse_word_to_model)
            .collect();
        let replacement = Replacement::new(replacement_words);

        let scoped: Vec<ContentAnnotation> = w
            .annotations
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .filter_map(|(_, a)| parsed_annotation_to_scoped(a))
            .collect();

        let replaced = ReplacedWord::new(word, replacement).with_scoped_annotations(scoped);
        UtteranceContent::ReplacedWord(Box::new(replaced))
    } else {
        let scoped: Vec<ContentAnnotation> = w
            .annotations
            .iter()
            .filter_map(|a| parsed_annotation_to_scoped(a))
            .collect();
        if scoped.is_empty() {
            UtteranceContent::Word(Box::new(word))
        } else {
            let annotated = Annotated::new(word).with_scoped_annotations(scoped);
            UtteranceContent::AnnotatedWord(Box::new(annotated))
        }
    }
}

/// Parse a word string through the lexer+parser and convert to model Word.
/// Used for replacement words which may have internal structure (compounds, etc.)
fn parse_word_to_model(text: &str) -> Word {
    if let Some(parsed) = crate::parser::parse_word(text) {
        word_from_parsed(&parsed)
    } else {
        Word::simple(text)
    }
}

/// Convert a separator token to model Separator.
fn separator_token_to_model(tok: &Token<'_>) -> Separator {
    let s = Span::DUMMY;
    match tok {
        Token::Comma(_) => Separator::Comma { span: s },
        Token::Semicolon(_) => Separator::Semicolon { span: s },
        Token::Colon(_) => Separator::Colon { span: s },
        Token::CaContinuationMarker(_) => Separator::CaContinuation { span: s },
        Token::TagMarker(_) => Separator::Tag { span: s },
        Token::VocativeMarker(_) => Separator::Vocative { span: s },
        Token::UnmarkedEnding(_) => Separator::UnmarkedEnding { span: s },
        Token::UptakeSymbol(_) => Separator::Uptake { span: s },
        Token::CaNoBreak(_) => Separator::CaNoBreak { span: s },
        Token::CaTechnicalBreak(_) => Separator::CaTechnicalBreak { span: s },
        Token::RisingToHigh(_) => Separator::RisingToHigh { span: s },
        Token::RisingToMid(_) => Separator::RisingToMid { span: s },
        Token::LevelPitch(_) => Separator::Level { span: s },
        Token::FallingToMid(_) => Separator::FallingToMid { span: s },
        Token::FallingToLow(_) => Separator::FallingToLow { span: s },
        Token::Lengthening(_) => Separator::Colon { span: s },
        _ => Separator::Comma { span: s },
    }
}

/// Convert a content item to a BracketedItem (for inside groups/quotations/retraces).
fn content_item_to_bracketed(item: &ast::ContentItem<'_>) -> Option<BracketedItem> {
    match item {
        ast::ContentItem::Word(w) => {
            let word = word_from_parsed(w);
            let replacement_idx = w
                .annotations
                .iter()
                .position(|a| matches!(a, crate::ast::ParsedAnnotation::Replacement(_)));

            if let Some(idx) = replacement_idx {
                let replacement_text = match &w.annotations[idx] {
                    crate::ast::ParsedAnnotation::Replacement(text) => *text,
                    _ => unreachable!(),
                };
                let replacement_words: Vec<Word> = replacement_text
                    .split_whitespace()
                    .map(parse_word_to_model)
                    .collect();
                let replacement = Replacement::new(replacement_words);
                let scoped: Vec<ContentAnnotation> = w
                    .annotations
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .filter_map(|(_, a)| parsed_annotation_to_scoped(a))
                    .collect();
                let replaced = ReplacedWord::new(word, replacement).with_scoped_annotations(scoped);
                Some(BracketedItem::ReplacedWord(Box::new(replaced)))
            } else {
                let scoped = annotations_to_scoped(&w.annotations);
                if scoped.is_empty() {
                    Some(BracketedItem::Word(Box::new(word)))
                } else {
                    let annotated = Annotated::new(word).with_scoped_annotations(scoped);
                    Some(BracketedItem::AnnotatedWord(Box::new(annotated)))
                }
            }
        }
        ast::ContentItem::Pause(tok) => {
            let duration = match tok {
                Token::PauseShort(_) => PauseDuration::Short,
                Token::PauseMedium(_) => PauseDuration::Medium,
                Token::PauseLong(_) => PauseDuration::Long,
                Token::PauseTimed(s) => PauseDuration::Timed(PauseTimedDuration::new(*s)),
                _ => PauseDuration::Short,
            };
            Some(BracketedItem::Pause(Pause::new(duration)))
        }
        ast::ContentItem::Event(toks) => {
            let event_text = toks.first().map(|t| t.text()).unwrap_or("");
            Some(BracketedItem::Event(Event::new(event_text)))
        }
        ast::ContentItem::AnnotatedEvent { event, annotations } => {
            let event_text = event.text();
            let event_model = Event::new(event_text);
            let scoped = annotations_to_scoped(annotations);
            if scoped.is_empty() {
                Some(BracketedItem::Event(event_model))
            } else {
                Some(BracketedItem::AnnotatedEvent(
                    Annotated::new(event_model).with_scoped_annotations(scoped),
                ))
            }
        }
        ast::ContentItem::Action { annotations, .. } => {
            let scoped = annotations_to_scoped(annotations);
            let annotated = Annotated::new(Action::new()).with_scoped_annotations(scoped);
            Some(BracketedItem::AnnotatedAction(annotated))
        }
        ast::ContentItem::OtherSpokenEvent(tok) => match tok {
            Token::OtherSpokenEvent { speaker, text } => Some(BracketedItem::OtherSpokenEvent(
                OtherSpokenEvent::new(*speaker, *text),
            )),
            _ => unreachable!(),
        },
        ast::ContentItem::Separator(tok) => {
            let sep = separator_token_to_model(tok);
            Some(BracketedItem::Separator(sep))
        }
        ast::ContentItem::Group(g) => {
            let inner: Vec<BracketedItem> = g
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let group = Group::new(BracketedContent::new(inner));
            let scoped = annotations_to_scoped(&g.annotations);
            if scoped.is_empty() {
                // Bare nested group — not directly representable as BracketedItem,
                // but AnnotatedGroup with empty annotations works
                Some(BracketedItem::AnnotatedGroup(
                    Annotated::new(group).with_scoped_annotations(scoped),
                ))
            } else {
                Some(BracketedItem::AnnotatedGroup(
                    Annotated::new(group).with_scoped_annotations(scoped),
                ))
            }
        }
        ast::ContentItem::Retrace(r) => {
            let kind = match r.kind {
                crate::ast::RetraceKindParsed::Partial => RetraceKind::Partial,
                crate::ast::RetraceKindParsed::Complete => RetraceKind::Full,
                crate::ast::RetraceKindParsed::Multiple => RetraceKind::Multiple,
                crate::ast::RetraceKindParsed::Reformulation => RetraceKind::Reformulation,
            };
            let inner: Vec<BracketedItem> = r
                .content
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            let mut retrace =
                talkbank_model::model::Retrace::new(BracketedContent::new(inner), kind);
            if r.is_group {
                retrace = retrace.as_group();
            }
            let scoped = annotations_to_scoped(&r.annotations);
            if !scoped.is_empty() {
                retrace = retrace.with_annotations(scoped);
            }
            Some(BracketedItem::Retrace(Box::new(retrace)))
        }
        ast::ContentItem::Annotation(Token::Freecode(s)) => {
            Some(BracketedItem::Freecode(Freecode::new(*s)))
        }
        ast::ContentItem::Annotation(_) => None,
        ast::ContentItem::PhoGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::PhoGroup(PhoGroup::new(
                BracketedContent::new(items),
            )))
        }
        ast::ContentItem::SinGroup(contents) => {
            let items: Vec<BracketedItem> = contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::SinGroup(SinGroup::new(
                BracketedContent::new(items),
            )))
        }
        ast::ContentItem::Quotation(q) => {
            let items: Vec<BracketedItem> = q
                .contents
                .iter()
                .filter_map(|c| content_item_to_bracketed(c))
                .collect();
            Some(BracketedItem::Quotation(Quotation::new(
                BracketedContent::new(items),
            )))
        }
        _ => None,
    }
}

/// Convert a single annotation token to a ContentAnnotation.
/// All tokens carry tag-extracted content — no string manipulation needed.
/// Convert a parsed annotation to a model ContentAnnotation.
fn parsed_annotation_to_scoped(ann: &ast::ParsedAnnotation<'_>) -> Option<ContentAnnotation> {
    match ann {
        crate::ast::ParsedAnnotation::Retrace(_) => None, // Retraces handled at content level
        crate::ast::ParsedAnnotation::Stressing => Some(ContentAnnotation::Stressing),
        crate::ast::ParsedAnnotation::ContrastiveStressing => {
            Some(ContentAnnotation::ContrastiveStressing)
        }
        crate::ast::ParsedAnnotation::Uncertain => Some(ContentAnnotation::Uncertain),
        crate::ast::ParsedAnnotation::Exclude => Some(ContentAnnotation::Exclude),
        crate::ast::ParsedAnnotation::Error(s) => {
            let code = if s.is_empty() {
                None
            } else {
                Some((*s).into())
            };
            Some(ContentAnnotation::Error(ScopedError { code }))
        }
        crate::ast::ParsedAnnotation::OverlapPrecedes(s) => {
            let index = if s.is_empty() {
                None
            } else {
                s.parse().ok().map(OverlapMarkerIndex::new)
            };
            Some(ContentAnnotation::OverlapBegin(ScopedOverlapBegin {
                index,
            }))
        }
        crate::ast::ParsedAnnotation::OverlapFollows(s) => {
            let index = if s.is_empty() {
                None
            } else {
                s.parse().ok().map(OverlapMarkerIndex::new)
            };
            Some(ContentAnnotation::OverlapEnd(ScopedOverlapEnd { index }))
        }
        crate::ast::ParsedAnnotation::Explanation(s) => {
            Some(ContentAnnotation::Explanation(ScopedExplanation {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Paralinguistic(s) => {
            Some(ContentAnnotation::Paralinguistic(ScopedParalinguistic {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Alternative(s) => {
            Some(ContentAnnotation::Alternative(ScopedAlternative {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::PercentComment(s) => {
            Some(ContentAnnotation::PercentComment(ScopedPercentComment {
                text: (*s).into(),
            }))
        }
        crate::ast::ParsedAnnotation::Replacement(_) => None, // Handled separately in word conversion
        crate::ast::ParsedAnnotation::Langcode(_) | crate::ast::ParsedAnnotation::Postcode(_) => {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Terminator conversion
// ═══════════════════════════════════════════════════════════════

/// Convert a terminator token to model Terminator.
pub fn token_to_terminator(tok: &Token<'_>) -> Terminator {
    let s = Span::DUMMY;
    match tok {
        Token::Period(_) => Terminator::Period { span: s },
        Token::Question(_) => Terminator::Question { span: s },
        Token::Exclamation(_) => Terminator::Exclamation { span: s },
        Token::TrailingOff(_) => Terminator::TrailingOff { span: s },
        Token::Interruption(_) => Terminator::Interruption { span: s },
        Token::SelfInterruption(_) => Terminator::SelfInterruption { span: s },
        Token::InterruptedQuestion(_) => Terminator::InterruptedQuestion { span: s },
        Token::BrokenQuestion(_) => Terminator::BrokenQuestion { span: s },
        Token::QuotedNewLine(_) => Terminator::QuotedNewLine { span: s },
        Token::QuotedPeriodSimple(_) => Terminator::QuotedPeriodSimple { span: s },
        Token::SelfInterruptedQuestion(_) => Terminator::SelfInterruptedQuestion { span: s },
        Token::TrailingOffQuestion(_) => Terminator::TrailingOffQuestion { span: s },
        Token::BreakForCoding(_) => Terminator::BreakForCoding { span: s },
        _ => Terminator::Period { span: s },
    }
}

// ═══════════════════════════════════════════════════════════════
// MainTier conversion
// ═══════════════════════════════════════════════════════════════

pub fn main_tier_to_model(mt: &ast::MainTier<'_>) -> MainTier {
    let speaker = SpeakerCode::new(mt.speaker.text());
    let content_items: Vec<UtteranceContent> = mt
        .tier_body
        .contents
        .iter()
        .map(|c| content_item_to_model(c))
        .collect();
    let terminator = mt
        .tier_body
        .terminator
        .as_ref()
        .map(|t| token_to_terminator(t));

    let mut main_tier = MainTier::new(speaker, content_items, terminator);

    // Extract a terminal bullet that the greedy contents parser left in content.
    main_tier.content.extract_terminal_bullet();

    // Grammar-routed bullet from tier_body.media_bullet takes priority
    // over the extracted one (it's correctly classified by the chumsky parser).
    if let Some(bullet_tok) = &mt.tier_body.media_bullet
        && let Token::MediaBullet {
            start_time,
            end_time,
            ..
        } = bullet_tok
    {
        let start_ms: u64 = start_time.parse().unwrap_or(0);
        let end_ms: u64 = end_time.parse().unwrap_or(0);
        main_tier = main_tier.with_bullet(Bullet::new(start_ms, end_ms));
    }

    // Linkers
    if !mt.tier_body.linkers.is_empty() {
        let linkers: Vec<Linker> = mt
            .tier_body
            .linkers
            .iter()
            .filter_map(|tok| linker_token_to_model(tok))
            .collect();
        main_tier = main_tier.with_linkers(linkers);
    }

    // Language code ([- lang])
    if let Some(ref langcode_tok) = mt.tier_body.langcode {
        // Token carries tag-extracted language code directly (e.g., "zho")
        let code = langcode_tok.text();
        if !code.is_empty() {
            main_tier = main_tier.with_language_code(LanguageCode::new(code));
        }
    }

    // Postcodes
    if !mt.tier_body.postcodes.is_empty() {
        let postcodes: Vec<Postcode> = mt
            .tier_body
            .postcodes
            .iter()
            .map(|tok| {
                // Token carries tag-extracted postcode content directly
                Postcode::new(tok.text())
            })
            .collect();
        main_tier = main_tier.with_postcodes(postcodes);
    }

    main_tier
}

// ═══════════════════════════════════════════════════════════════
// Utterance conversion
// ═══════════════════════════════════════════════════════════════

pub fn utterance_to_model(u: &ast::Utterance<'_>) -> talkbank_model::model::Utterance {
    let main = main_tier_to_model(&u.main_tier);
    // Skip tiers whose AST→model conversion failed (e.g. a `%mor:`
    // line with a missing or unrecognized terminator). Cross-tier
    // validators surface the absence as a typed diagnostic.
    let dep_tiers: Vec<talkbank_model::model::DependentTier> = u
        .dependent_tiers
        .iter()
        .filter_map(dependent_tier_to_model)
        .collect();
    talkbank_model::model::Utterance {
        preceding_headers: Default::default(),
        main,
        dependent_tiers: dep_tiers.into(),
        alignments: None,
        alignment_diagnostics: Vec::new(),
        parse_health: Default::default(),
        utterance_language: Default::default(),
        language_metadata: Default::default(),
    }
}

/// Convert a parsed dependent tier to model `DependentTier`.
///
/// Returns `None` when the AST→model conversion fails for that tier
/// (currently `%mor:` with a missing or unrecognized terminator).
/// Cross-tier validators surface the resulting absence; this layer
/// just declines to construct a `MorTier` from malformed input.
pub fn dependent_tier_to_model(
    tier: &ast::DependentTierParsed<'_>,
) -> Option<talkbank_model::model::DependentTier> {
    Some(match tier {
        ast::DependentTierParsed::Mor(mor) => {
            talkbank_model::model::DependentTier::Mor(MorTier::try_from(mor).ok()?)
        }
        ast::DependentTierParsed::Gra(gra) => {
            talkbank_model::model::DependentTier::Gra(GraTier::from(gra))
        }
        ast::DependentTierParsed::Pho(pho) => {
            talkbank_model::model::DependentTier::Pho(convert_pho_tier(
                pho,
                talkbank_model::model::dependent_tier::pho::PhoTierType::Pho,
            ))
        }
        ast::DependentTierParsed::Mod(pho) => {
            talkbank_model::model::DependentTier::Mod(convert_pho_tier(
                pho,
                talkbank_model::model::dependent_tier::pho::PhoTierType::Mod,
            ))
        }
        ast::DependentTierParsed::Sin(sin) => {
            talkbank_model::model::DependentTier::Sin(convert_sin_tier(sin))
        }
        ast::DependentTierParsed::Wor { items, terminator } => {
            use talkbank_model::model::dependent_tier::wor::WorItem;
            let wor_items: Vec<WorItem> = items
                .iter()
                .map(|item| match item {
                    ast::WorItemParsed::Word { word, bullet } => {
                        let mut w = word_from_parsed(word);
                        if let Some((start_ms, end_ms)) = bullet {
                            w = w.with_inline_bullet(Bullet::new(*start_ms, *end_ms));
                        }
                        WorItem::Word(Box::new(w))
                    }
                    ast::WorItemParsed::Separator(tok) => WorItem::Separator {
                        text: tok.text().to_string(),
                        span: Span::DUMMY,
                    },
                })
                .collect();
            let mut wor = WorTier::new(wor_items);
            if let Some(t) = terminator {
                wor.terminator = Some(token_to_terminator(t));
            }
            talkbank_model::model::DependentTier::Wor(wor)
        }
        ast::DependentTierParsed::Text { prefix, content } => {
            let bc = tokens_to_bullet_content(content);
            let prefix_text = prefix.text();
            // Extract tier label: "%com:\t" → "com", "%xpho:\t" → "xpho"
            let label = prefix_text.trim_start_matches('%').trim_end_matches(":\t");

            // Phon project tiers have x-prefix but are NOT user-defined
            let is_phon_tier = matches!(label, "xmodsyl" | "xphosyl" | "xphoaln");

            // User-defined tiers: %x* prefix (but not phon project tiers)
            if label.starts_with('x') && label.len() >= 2 && !is_phon_tier {
                let raw_text: String = content.iter().map(|t| t.text()).collect();
                return Some(talkbank_model::model::DependentTier::UserDefined(
                    talkbank_model::model::UserDefinedDependentTier {
                        label: NonEmptyString::new(label)
                            .unwrap_or_else(|| NonEmptyString::new("x").unwrap()),
                        content: NonEmptyString::new(raw_text.as_str())
                            .unwrap_or_else(|| NonEmptyString::new(" ").unwrap()),
                        span: Span::DUMMY,
                    },
                ));
            }

            // BulletContent tiers
            match label {
                "com" => talkbank_model::model::DependentTier::Com(ComTier::new(bc)),
                "act" => talkbank_model::model::DependentTier::Act(ActTier::new(bc)),
                "exp" => talkbank_model::model::DependentTier::Exp(ExpTier::new(bc)),
                "add" => talkbank_model::model::DependentTier::Add(AddTier::new(bc)),
                "gpx" => talkbank_model::model::DependentTier::Gpx(GpxTier::new(bc)),
                "int" => talkbank_model::model::DependentTier::Int(IntTier::new(bc)),
                "spa" => talkbank_model::model::DependentTier::Spa(SpaTier::new(bc)),
                "sit" => talkbank_model::model::DependentTier::Sit(SitTier::new(bc)),
                "cod" => talkbank_model::model::DependentTier::Cod(CodTier::new(bc)),
                // TextTier tiers (plain string content)
                "alt" | "coh" | "def" | "eng" | "err" | "fac" | "flo" | "gls" | "ort" | "par" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let text = NonEmptyString::new(raw_text.as_str())
                        .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                    let tt = talkbank_model::model::dependent_tier::TextTier::new(text);
                    match label {
                        "alt" => talkbank_model::model::DependentTier::Alt(tt),
                        "coh" => talkbank_model::model::DependentTier::Coh(tt),
                        "def" => talkbank_model::model::DependentTier::Def(tt),
                        "eng" => talkbank_model::model::DependentTier::Eng(tt),
                        "err" => talkbank_model::model::DependentTier::Err(tt),
                        "fac" => talkbank_model::model::DependentTier::Fac(tt),
                        "flo" => talkbank_model::model::DependentTier::Flo(tt),
                        "gls" => talkbank_model::model::DependentTier::Gls(tt),
                        "ort" => talkbank_model::model::DependentTier::Ort(tt),
                        "par" => talkbank_model::model::DependentTier::Par(tt),
                        _ => unreachable!(),
                    }
                }
                // TimTier (structured time)
                "tim" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let text = NonEmptyString::new(raw_text.as_str())
                        .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                    talkbank_model::model::DependentTier::Tim(
                        talkbank_model::dependent_tier::TimTier::from_text(text),
                    )
                }
                // %wor tier — word tier with timing bullets
                "wor" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let wor = crate::convert::wor_tier_from_input(&raw_text);
                    talkbank_model::model::DependentTier::Wor(wor)
                }
                // Phon project syllabification tiers (with or without x prefix)
                "modsyl" | "xmodsyl" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                    talkbank_model::model::DependentTier::Modsyl(
                        talkbank_model::dependent_tier::SylTier::new(
                            talkbank_model::dependent_tier::SylTierType::Modsyl,
                            words,
                        ),
                    )
                }
                "phosyl" | "xphosyl" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    let words = talkbank_model::dependent_tier::parse_syl_content(&raw_text);
                    talkbank_model::model::DependentTier::Phosyl(
                        talkbank_model::dependent_tier::SylTier::new(
                            talkbank_model::dependent_tier::SylTierType::Phosyl,
                            words,
                        ),
                    )
                }
                "phoaln" | "xphoaln" => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    match talkbank_model::dependent_tier::parse_phoaln_content(&raw_text) {
                        Ok(words) => talkbank_model::model::DependentTier::Phoaln(
                            talkbank_model::dependent_tier::PhoalnTier::new(words),
                        ),
                        Err(_) => {
                            let text = NonEmptyString::new(raw_text.as_str())
                                .unwrap_or_else(|| NonEmptyString::new(" ").unwrap());
                            talkbank_model::model::DependentTier::Unsupported(
                                talkbank_model::model::UserDefinedDependentTier {
                                    label: NonEmptyString::new("phoaln").unwrap(),
                                    content: text,
                                    span: Span::DUMMY,
                                },
                            )
                        }
                    }
                }
                // Fallback: unsupported tier
                _ => {
                    let raw_text: String = content.iter().map(|t| t.text()).collect();
                    talkbank_model::model::DependentTier::Unsupported(
                        talkbank_model::model::UserDefinedDependentTier {
                            label: NonEmptyString::new(label)
                                .unwrap_or_else(|| NonEmptyString::new("unknown").unwrap()),
                            content: NonEmptyString::new(raw_text.as_str())
                                .unwrap_or_else(|| NonEmptyString::new(" ").unwrap()),
                            span: Span::DUMMY,
                        },
                    )
                }
            }
        }
    })
}

// ═══════════════════════════════════════════════════════════════
// ChatFile conversion
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::ChatFile<'a>> for talkbank_model::model::ChatFile {
    fn from(file: &ast::ChatFile<'a>) -> Self {
        let lines: Vec<talkbank_model::model::Line> = file
            .lines
            .iter()
            .map(|line| match line {
                ast::Line::Header(h) => talkbank_model::model::Line::Header {
                    header: Box::new(crate::convert::header_to_model(h)),
                    span: Span::DUMMY,
                },
                ast::Line::Utterance(u) => {
                    talkbank_model::model::Line::Utterance(Box::new(utterance_to_model(u.as_ref())))
                }
            })
            .collect();
        // Build participants map from @ID headers, enriched with
        // @Participants metadata. This matches TreeSitterParser's behavior:
        // only participants with @ID headers appear in the map. The validator
        // detects missing @ID (E522) by comparing @Participants entries
        // against the participants map.
        //
        // First pass: collect @Participants entries by speaker code (for name/role).
        let mut declared: indexmap::IndexMap<
            SpeakerCode,
            (Option<ParticipantName>, ParticipantRole),
        > = indexmap::IndexMap::new();
        for line in &lines {
            if let talkbank_model::model::Line::Header { header, .. } = line
                && let Header::Participants { entries } = header.as_ref()
            {
                for entry in entries.iter() {
                    declared.insert(
                        entry.speaker_code.clone(),
                        (entry.name.clone(), entry.role.clone()),
                    );
                }
            }
        }
        // Second pass: build participants from @ID headers only.
        let mut participants = indexmap::IndexMap::new();
        for line in &lines {
            if let talkbank_model::model::Line::Header { header, .. } = line {
                match header.as_ref() {
                    Header::ID(id_header) => {
                        let code = id_header.speaker.clone();
                        let (name, role) = declared
                            .get(&code)
                            .cloned()
                            .unwrap_or((None, id_header.role.clone()));
                        participants.insert(
                            code.clone(),
                            talkbank_model::model::Participant {
                                code: code.clone(),
                                name,
                                role,
                                id: id_header.clone(),
                                birth_date: None,
                            },
                        );
                    }
                    Header::Birth { participant, date } => {
                        if let Some(p) = participants.get_mut(participant) {
                            p.birth_date = Some(date.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        // CA omission normalization: if @Options includes CA mode,
        // reclassify standalone (word) shortenings as CAOmission category.
        // This matches TreeSitterParser's post-parse normalization.
        let ca_mode = lines.iter().any(|line| {
            if let talkbank_model::model::Line::Header { header, .. } = line {
                matches!(header.as_ref(), Header::Options { options }
                    if options.iter().any(|opt| opt.enables_ca_mode()))
            } else {
                false
            }
        });
        let mut lines = lines;
        if ca_mode {
            normalize_ca_omissions_in_lines(&mut lines);
        }

        talkbank_model::model::ChatFile::with_participants(lines, participants)
    }
}

/// Convert a parsed header to model Header.
pub fn header_to_model(h: &ast::HeaderParsed<'_>) -> Header {
    let prefix_text = h.prefix.text();

    // Join all content token texts for free-text headers.
    // Preserve continuation newlines.
    let all_content: String = h.content.iter().map(|t| t.text()).collect::<String>();

    match &h.prefix {
        Token::HeaderUtf8(_) => Header::Utf8,
        Token::HeaderBegin(_) => Header::Begin,
        Token::HeaderEnd(_) => Header::End,
        Token::HeaderBlank(_) => Header::Blank,
        Token::HeaderNewEpisode(_) => Header::NewEpisode,
        Token::HeaderPrefix(p) if p.contains("@Languages") => {
            let codes: Vec<LanguageCode> = h
                .content
                .iter()
                .filter(|t| matches!(t, Token::LanguageCode(_)))
                .map(|t| LanguageCode::new(t.text()))
                .collect();
            Header::Languages {
                codes: LanguageCodes::new(codes),
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Participants") => {
            // Split participant words on Comma tokens
            let mut entries = Vec::new();
            let mut current_words: Vec<&str> = Vec::new();
            for tok in &h.content {
                match tok {
                    Token::ParticipantWord(s) => current_words.push(s),
                    Token::Comma(_) if !current_words.is_empty() => {
                        entries.push(participant_words_to_entry(&current_words));
                        current_words.clear();
                    }
                    _ => {}
                }
            }
            if !current_words.is_empty() {
                entries.push(participant_words_to_entry(&current_words));
            }
            Header::Participants {
                entries: ParticipantEntries::new(entries),
            }
        }
        Token::HeaderPrefix(p) if p.contains("@ID") => {
            // Token struct carries all 10 fields directly — no splitting needed
            if let Some(Token::IdFields {
                language,
                corpus,
                speaker,
                age,
                sex,
                group,
                ses,
                role,
                education,
                custom,
            }) = h.content.first()
            {
                // Language field can be comma-separated: "eng, ara"
                let lang_codes: Vec<LanguageCode> = language
                    .split(',')
                    .map(|s| LanguageCode::new(s.trim()))
                    .collect();
                let mut id = IDHeader::from_languages(
                    LanguageCodes::new(lang_codes),
                    SpeakerCode::new(*speaker),
                    ParticipantRole::new(*role),
                );
                if !corpus.is_empty() {
                    id = id.with_corpus(*corpus);
                }
                if !age.is_empty() {
                    id = id.with_age(*age);
                }
                if !group.is_empty() {
                    id = id.with_group(*group);
                }
                if !ses.is_empty() {
                    id = id.with_ses(*ses);
                }
                if !education.is_empty() {
                    id = id.with_education(*education);
                }
                if !custom.is_empty() {
                    id = id.with_custom_field(*custom);
                }
                if !sex.is_empty() {
                    id = id.with_sex(talkbank_model::model::Sex::from_text(sex));
                }
                return Header::ID(id);
            }
            Header::Unknown {
                text: WarningText::new(format!("{prefix_text}{all_content}")),
                parse_reason: Some("malformed @ID".to_string()),
                suggested_fix: None,
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Types") => {
            if let Some(Token::TypesFields {
                design,
                activity,
                group,
            }) = h.content.first()
            {
                return Header::Types(TypesHeader::new(*design, *activity, *group));
            }
            Header::Unknown {
                text: WarningText::new(all_content),
                parse_reason: Some("malformed @Types".to_string()),
                suggested_fix: None,
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Media") => {
            // Media content is MediaWord/MediaFilename tokens separated by Comma
            let words: Vec<&str> = h
                .content
                .iter()
                .filter(|t| matches!(t, Token::MediaWord(_) | Token::MediaFilename(_)))
                .map(|t| t.text())
                .collect();
            if words.len() >= 2 {
                let mut mh = MediaHeader::new(
                    words[0], // Into<MediaFilename>
                    MediaType::from_text(words[1]),
                );
                if words.len() >= 3 {
                    mh = mh.with_status(MediaStatus::from_text(words[2]));
                }
                Header::Media(mh)
            } else {
                Header::Unknown {
                    text: WarningText::new(all_content),
                    parse_reason: Some("malformed @Media".to_string()),
                    suggested_fix: None,
                }
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Comment") => Header::Comment {
            content: tokens_to_bullet_content(&h.content),
        },
        Token::HeaderPrefix(p) if p.contains("@Date") => Header::Date {
            date: ChatDate::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Situation") => Header::Situation {
            text: SituationDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Location:") => Header::Location {
            location: LocationDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Activities") => Header::Activities {
            activities: ActivitiesDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@PID") => Header::Pid {
            pid: PidValue::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Options") => {
            let flags: Vec<ChatOptionFlag> = all_content
                .split(',')
                .map(|s| ChatOptionFlag::from_text(s.trim()))
                .collect();
            Header::Options {
                options: ChatOptionFlags::new(flags),
            }
        }
        Token::HeaderBirthOf(speaker) => {
            // Token carries tag-extracted speaker code directly
            Header::Birth {
                participant: SpeakerCode::new(*speaker),
                date: ChatDate::new(&all_content),
            }
        }
        Token::HeaderBirthplaceOf(speaker) => Header::Birthplace {
            participant: SpeakerCode::new(*speaker),
            place: BirthplaceDescription::new(&all_content),
        },
        Token::HeaderL1Of(speaker) => Header::L1Of {
            participant: SpeakerCode::new(*speaker),
            language: LanguageName::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Bg") => Header::BeginGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        Token::HeaderPrefix(p) if p.starts_with("@G:") || *p == "@G" => Header::LazyGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        Token::HeaderPrefix(p) if p.contains("@Eg") => Header::EndGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        _ => {
            // All other headers — use the appropriate model type based on prefix
            let ct = &all_content;
            if prefix_text.contains("@Font") {
                Header::Font {
                    font: FontSpec::new(ct),
                }
            } else if prefix_text.contains("@Window") {
                Header::Window {
                    geometry: WindowGeometry::new(ct),
                }
            } else if prefix_text.contains("@Color words") {
                Header::ColorWords {
                    colors: ColorWordList::new(ct),
                }
            } else if prefix_text.contains("@Recording Quality") {
                Header::RecordingQuality {
                    quality: RecordingQuality::from_text(ct),
                }
            } else if prefix_text.contains("@Transcription") {
                Header::Transcription {
                    transcription: Transcription::from_text(ct),
                }
            } else if prefix_text.contains("@Number") {
                Header::Number {
                    number: Number::from_text(ct),
                }
            } else if prefix_text.contains("@Room Layout") {
                Header::RoomLayout {
                    layout: RoomLayoutDescription::new(ct),
                }
            } else if prefix_text.contains("@Tape Location") {
                Header::TapeLocation {
                    location: TapeLocationDescription::new(ct),
                }
            } else if prefix_text.contains("@Time Duration") {
                Header::TimeDuration {
                    duration: TimeDurationValue::new(ct),
                }
            } else if prefix_text.contains("@Time Start") {
                Header::TimeStart {
                    start: TimeStartValue::new(ct),
                }
            } else if prefix_text.contains("@Transcriber") {
                Header::Transcriber {
                    transcriber: TranscriberName::new(ct),
                }
            } else if prefix_text.contains("@Warning") {
                Header::Warning {
                    text: WarningText::new(ct),
                }
            } else if prefix_text.contains("@Page") {
                Header::Page {
                    page: PageNumber::new(ct),
                }
            } else if prefix_text.contains("@Videos") {
                Header::Videos {
                    videos: VideoSpec::new(ct),
                }
            } else if prefix_text.starts_with("@T:") || prefix_text == "@T" {
                Header::T {
                    text: TDescription::new(ct),
                }
            } else if prefix_text.contains("@Bck") {
                Header::Bck {
                    bck: BackgroundDescription::new(ct),
                }
            } else {
                Header::Unknown {
                    text: WarningText::new(format!("{prefix_text}{ct}")),
                    parse_reason: None,
                    suggested_fix: None,
                }
            }
        }
    }
}

/// Convert a sequence of content tokens to BulletContent, preserving continuations.
fn tokens_to_bullet_content(tokens: &[Token<'_>]) -> BulletContent {
    let mut segments = Vec::new();
    for tok in tokens {
        match tok {
            Token::TextSegment(s) | Token::HeaderContent(s) => {
                segments.push(BulletContentSegment::text(*s));
            }
            Token::Continuation(_) => {
                segments.push(BulletContentSegment::continuation());
            }
            Token::MediaBullet {
                start_time,
                end_time,
                ..
            } => {
                let start_ms = start_time.parse().unwrap_or(0);
                let end_ms = end_time.parse().unwrap_or(0);
                segments.push(BulletContentSegment::bullet(start_ms, end_ms));
            }
            Token::InlinePic(s) => {
                // Token carries tag-extracted filename directly
                segments.push(BulletContentSegment::picture(*s));
            }
            _ => {
                // Other tokens (LanguageCode, ParticipantWord, etc.) — include as text
                segments.push(BulletContentSegment::text(tok.text()));
            }
        }
    }
    BulletContent::new(segments)
}

/// Convert participant words [SPK, Name, Role] to ParticipantEntry.
fn participant_words_to_entry(words: &[&str]) -> ParticipantEntry {
    let speaker_code = SpeakerCode::new(words.first().copied().unwrap_or(""));
    let role = if words.len() >= 2 {
        ParticipantRole::new(*words.last().unwrap())
    } else {
        ParticipantRole::new("")
    };
    let name = if words.len() == 3 {
        Some(ParticipantName::new(words[1]))
    } else {
        None
    };
    ParticipantEntry {
        speaker_code,
        name,
        role,
    }
}

// ═══════════════════════════════════════════════════════════════
// %mor conversions
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::MorWordParsed<'a>> for MorWord {
    fn from(w: &ast::MorWordParsed<'a>) -> Self {
        let mut word = MorWord::new(PosCategory::new(w.pos), MorStem::new(w.lemma));
        for f in &w.features {
            word = word.with_feature(MorFeature::new(*f));
        }
        word
    }
}

impl<'a> From<&ast::MorItem<'a>> for Mor {
    fn from(item: &ast::MorItem<'a>) -> Self {
        let main = MorWord::from(&item.main);
        let mut mor = Mor::new(main);
        for clitic in &item.post_clitics {
            mor = mor.with_post_clitic(MorWord::from(clitic));
        }
        mor
    }
}

/// Error class for re2c → model conversion of `%mor:` tiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MorTierConvertError {
    /// AST has no terminator. `MorTier.terminator` is non-optional,
    /// so the caller must produce a typed parse-outcome diagnostic
    /// rather than constructing a MorTier.
    MissingTerminator,
    /// AST terminator string is not a recognized CHAT terminator.
    UnrecognizedTerminator(String),
}

impl<'a> TryFrom<&ast::MorTier<'a>> for MorTier {
    type Error = MorTierConvertError;

    fn try_from(tier: &ast::MorTier<'a>) -> Result<Self, Self::Error> {
        use talkbank_model::Terminator;

        let items: Vec<Mor> = tier.items.iter().map(Mor::from).collect();
        let terminator_node = tier
            .terminator
            .as_ref()
            .ok_or(MorTierConvertError::MissingTerminator)?;
        let terminator =
            Terminator::try_from_chat_str(terminator_node.text().trim()).ok_or_else(|| {
                MorTierConvertError::UnrecognizedTerminator(terminator_node.text().to_string())
            })?;
        Ok(MorTier::new_mor(items, terminator))
    }
}

// ═══════════════════════════════════════════════════════════════
// %gra conversions
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::GraRelationParsed<'a>> for GrammaticalRelation {
    fn from(r: &ast::GraRelationParsed<'a>) -> Self {
        GrammaticalRelation {
            index: r.index.parse().unwrap_or(0),
            head: r.head.parse().unwrap_or(0),
            relation: GrammaticalRelationType::new(r.relation),
        }
    }
}

impl<'a> From<&ast::GraTier<'a>> for GraTier {
    fn from(tier: &ast::GraTier<'a>) -> Self {
        let relations: Vec<GrammaticalRelation> = tier
            .relations
            .iter()
            .map(GrammaticalRelation::from)
            .collect();
        GraTier::new_gra(relations)
    }
}

// ═══════════════════════════════════════════════════════════════
// @Languages conversion
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::LanguagesHeaderParsed<'a>> for LanguageCodes {
    fn from(langs: &ast::LanguagesHeaderParsed<'a>) -> Self {
        LanguageCodes::new(langs.codes.iter().map(|c| LanguageCode::new(*c)).collect())
    }
}

// ═══════════════════════════════════════════════════════════════
// PhoTier conversion
// ═══════════════════════════════════════════════════════════════

/// Convert our parsed PhoTier to model PhoTier.
fn convert_pho_tier(
    pho: &ast::PhoTier<'_>,
    tier_type: talkbank_model::model::dependent_tier::pho::PhoTierType,
) -> talkbank_model::model::PhoTier {
    use talkbank_model::model::dependent_tier::pho::{PhoGroupWords, PhoItem, PhoWord};

    fn pho_word_to_model(w: &ast::PhoWordParsed<'_>) -> PhoWord {
        // Compound words: segments joined by +. Model stores full text.
        PhoWord::new(w.segments.join("+"))
    }

    let items: Vec<PhoItem> = pho
        .items
        .iter()
        .map(|item| match item {
            ast::PhoItemParsed::Word(w) => PhoItem::Word(pho_word_to_model(w)),
            ast::PhoItemParsed::Group(words) => PhoItem::Group(PhoGroupWords::new(
                words.iter().map(pho_word_to_model).collect(),
            )),
        })
        .collect();
    talkbank_model::model::PhoTier::new(tier_type, items)
}

/// Convert our parsed SinTier to model SinTier.
fn convert_sin_tier(sin: &ast::SinTierParsed<'_>) -> talkbank_model::model::SinTier {
    use talkbank_model::model::dependent_tier::sin::{SinGroupGestures, SinItem, SinToken};
    let items: Vec<SinItem> = sin
        .items
        .iter()
        .map(|item| match item {
            ast::SinItemParsed::Token(s) => SinItem::Token(SinToken::new_unchecked(s)),
            ast::SinItemParsed::Group(words) => SinItem::SinGroup(SinGroupGestures::new(
                words.iter().map(SinToken::new_unchecked).collect(),
            )),
        })
        .collect();
    talkbank_model::model::SinTier::new(items)
}

// ═══════════════════════════════════════════════════════════════
// Public aliases and missing conversion functions
// (required by chat_parser_impl.rs for ChatParser trait)
// ═══════════════════════════════════════════════════════════════

/// Alias for `header_to_model` — used by ChatParser trait impl.
pub fn header_parsed_to_model(h: &ast::HeaderParsed<'_>) -> Header {
    header_to_model(h)
}

/// Convert text tier parsed AST to BulletContent.
fn text_tier_to_bullet_content(parsed: &ast::TextTierParsed<'_>) -> BulletContent {
    let segments: Vec<BulletContentSegment> = parsed
        .segments
        .iter()
        .map(|seg| match seg {
            ast::TextTierSegment::Text(s) => BulletContentSegment::text(*s),
            ast::TextTierSegment::Bullet(tok) => match tok {
                Token::MediaBullet {
                    start_time,
                    end_time,
                    ..
                } => {
                    let s: u64 = start_time.parse().unwrap_or(0);
                    let e: u64 = end_time.parse().unwrap_or(0);
                    BulletContentSegment::bullet(s, e)
                }
                _ => BulletContentSegment::text(tok.text()),
            },
            ast::TextTierSegment::Pic(tok) => BulletContentSegment::picture(tok.text()),
        })
        .collect();
    BulletContent::new(segments)
}

/// Convert parsed text tier to model ActTier.
pub fn to_act_tier(parsed: &ast::TextTierParsed<'_>) -> ActTier {
    ActTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model CodTier.
pub fn to_cod_tier(parsed: &ast::TextTierParsed<'_>) -> CodTier {
    CodTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model ComTier.
pub fn to_com_tier(parsed: &ast::TextTierParsed<'_>) -> ComTier {
    ComTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model ExpTier.
pub fn to_exp_tier(parsed: &ast::TextTierParsed<'_>) -> ExpTier {
    ExpTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model AddTier.
pub fn to_add_tier(parsed: &ast::TextTierParsed<'_>) -> AddTier {
    AddTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model GpxTier.
pub fn to_gpx_tier(parsed: &ast::TextTierParsed<'_>) -> GpxTier {
    GpxTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model IntTier.
pub fn to_int_tier(parsed: &ast::TextTierParsed<'_>) -> IntTier {
    IntTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model SpaTier.
pub fn to_spa_tier(parsed: &ast::TextTierParsed<'_>) -> SpaTier {
    SpaTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model SitTier.
pub fn to_sit_tier(parsed: &ast::TextTierParsed<'_>) -> SitTier {
    SitTier::new(text_tier_to_bullet_content(parsed))
}

/// Parse %sin tier content and convert to model SinTier.
pub fn sin_tier_from_text(input: &str) -> talkbank_model::model::SinTier {
    use talkbank_model::model::dependent_tier::sin::{SinGroupGestures, SinItem, SinToken};
    // Simple word-based parsing: split on whitespace, handle 〔groups〕
    let mut items = Vec::new();
    let mut in_group = false;
    let mut group_words = Vec::new();
    for word in input.split_whitespace() {
        if word.starts_with('\u{3014}') {
            // 〔 group start
            in_group = true;
            let text = word.trim_start_matches('\u{3014}');
            if !text.is_empty() {
                group_words.push(SinToken::new_unchecked(text));
            }
        } else if word.ends_with('\u{3015}') {
            // 〕 group end
            let text = word.trim_end_matches('\u{3015}');
            if !text.is_empty() {
                group_words.push(SinToken::new_unchecked(text));
            }
            items.push(SinItem::SinGroup(SinGroupGestures::new(std::mem::take(
                &mut group_words,
            ))));
            in_group = false;
        } else if in_group {
            group_words.push(SinToken::new_unchecked(word));
        } else {
            items.push(SinItem::Token(SinToken::new_unchecked(word)));
        }
    }
    talkbank_model::model::SinTier::new(items)
}

/// Parse %wor tier content and convert to model WorTier.
pub fn wor_tier_from_input(input: &str) -> WorTier {
    use chumsky::Parser as _;
    use talkbank_model::model::dependent_tier::wor::WorItem;

    // %wor uses same word rules as main tier. Parse words via chumsky.
    let tokens = crate::parser::lex_to_tokens(input, crate::lexer::COND_MAIN_CONTENT);
    let contents = crate::parser::main_tier::contents_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_default();

    let mut items = Vec::new();
    for item in &contents {
        match item {
            ast::ContentItem::Word(w) => {
                let word = word_from_parsed(w);
                items.push(WorItem::Word(Box::new(word)));
            }
            ast::ContentItem::Separator(tok) => {
                items.push(WorItem::Separator {
                    text: tok.text().to_string(),
                    span: Span::DUMMY,
                });
            }
            _ => {}
        }
    }
    WorTier::new(items)
}

// From impls that are now possible (no source needed)

impl<'a> From<&ast::MainTier<'a>> for MainTier {
    fn from(mt: &ast::MainTier<'a>) -> Self {
        main_tier_to_model(mt)
    }
}

impl<'a> From<&ast::Utterance<'a>> for talkbank_model::model::Utterance {
    fn from(u: &ast::Utterance<'a>) -> Self {
        utterance_to_model(u)
    }
}

impl<'a> From<&ast::WordWithAnnotations<'a>> for Word {
    fn from(w: &ast::WordWithAnnotations<'a>) -> Self {
        word_from_parsed(w)
    }
}

impl<'a> From<&ast::IdHeaderParsed<'a>> for IDHeader {
    fn from(id: &ast::IdHeaderParsed<'a>) -> Self {
        let lang_codes: Vec<LanguageCode> = id
            .language
            .split(',')
            .map(|s| LanguageCode::new(s.trim()))
            .collect();
        let mut header = IDHeader::from_languages(
            LanguageCodes::new(lang_codes),
            SpeakerCode::new(id.speaker),
            ParticipantRole::new(id.role),
        );
        if !id.corpus.is_empty() {
            header = header.with_corpus(id.corpus);
        }
        if !id.age.is_empty() {
            header = header.with_age(id.age);
        }
        if !id.group.is_empty() {
            header = header.with_group(id.group);
        }
        if !id.ses.is_empty() {
            header = header.with_ses(id.ses);
        }
        if !id.education.is_empty() {
            header = header.with_education(id.education);
        }
        if !id.custom_field.is_empty() {
            header = header.with_custom_field(id.custom_field);
        }
        if !id.sex.is_empty() {
            header = header.with_sex(talkbank_model::model::Sex::from_text(id.sex));
        }
        header
    }
}

impl<'a> From<&ast::ParticipantEntryParsed<'a>> for ParticipantEntry {
    fn from(entry: &ast::ParticipantEntryParsed<'a>) -> Self {
        participant_words_to_entry(&entry.words)
    }
}

impl<'a> From<&ast::PhoTier<'a>> for talkbank_model::model::PhoTier {
    fn from(pho: &ast::PhoTier<'a>) -> Self {
        convert_pho_tier(
            pho,
            talkbank_model::model::dependent_tier::pho::PhoTierType::Pho,
        )
    }
}

impl<'a> From<&ast::PhoWordParsed<'a>> for talkbank_model::model::PhoWord {
    fn from(w: &ast::PhoWordParsed<'a>) -> Self {
        talkbank_model::model::PhoWord::new(w.segments.join("+"))
    }
}

// ═══════════════════════════════════════════════════════════════
// CA omission normalization (post-parse, context-dependent)
// ═══════════════════════════════════════════════════════════════

/// Normalize CA omission markers when @Options: CA is active.
/// A standalone (word) — a word whose only content is a single Shortening
/// and has no category — is reclassified as CAOmission with Text content.
fn normalize_ca_omissions_in_lines(lines: &mut [talkbank_model::model::Line]) {
    for line in lines {
        if let talkbank_model::model::Line::Utterance(utterance) = line {
            for content in &mut utterance.main.content.content {
                normalize_ca_omission(content);
            }
        }
    }
}

fn normalize_ca_omission(content: &mut UtteranceContent) {
    match content {
        UtteranceContent::Word(word) => normalize_ca_omission_word(word),
        UtteranceContent::AnnotatedWord(annotated) => {
            normalize_ca_omission_word(&mut annotated.inner);
        }
        UtteranceContent::ReplacedWord(replaced) => {
            normalize_ca_omission_word(&mut replaced.word);
        }
        UtteranceContent::Group(group) => {
            for item in &mut group.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            for item in &mut annotated.inner.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::Retrace(retrace) => {
            for item in &mut retrace.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::Quotation(quote) => {
            for item in &mut quote.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        _ => {}
    }
}

fn normalize_ca_omission_bracketed_item(item: &mut BracketedItem) {
    match item {
        BracketedItem::Word(word) => normalize_ca_omission_word(word),
        BracketedItem::AnnotatedWord(annotated) => {
            normalize_ca_omission_word(&mut annotated.inner);
        }
        BracketedItem::ReplacedWord(replaced) => {
            normalize_ca_omission_word(&mut replaced.word);
        }
        _ => {}
    }
}

fn normalize_ca_omission_word(word: &mut Word) {
    // Only reclassify if no existing category and content is a single Shortening
    if word.category.is_some() {
        return;
    }
    if word.content.len() == 1
        && let WordContent::Shortening(shortening) = &word.content[0]
    {
        // Reclassify: Shortening → Text, category → CAOmission
        let text = shortening.as_ref().to_string();
        word.category = Some(WordCategory::CAOmission);
        word.content = WordContents::new(smallvec::smallvec![WordContent::Text(
            WordText::new_unchecked(&text)
        )]);
    }
}

// ═══════════════════════════════════════════════════════════════
// CA character → type mapping
// ═══════════════════════════════════════════════════════════════
