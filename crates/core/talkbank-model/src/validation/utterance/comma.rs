//! Comma validation: E258 (consecutive commas) and E259 (comma after non-spoken).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{BracketedItem, Separator, Utterance, UtteranceContent, Word};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Returns `true` if the word is a real spoken word (not a filler, nonword,
/// omission, phonological fragment, or untranscribed marker).
fn is_real_word(word: &Word) -> bool {
    word.untranscribed().is_none() && word.category.is_none()
}

/// Returns `true` if this content item licenses a subsequent comma.
///
/// This includes real spoken words plus pauses. Pauses are not spoken content,
/// but CLAN CHECK treats them as comma-licensing because `(` is not in its
/// `PUNCTUATION_SET` and therefore pauses set `CommaWordFound`. We follow
/// this behavior by design decision.
///
/// Notable divergence from CLAN CHECK: omission words (`0word`) do NOT
/// license commas here, even though CLAN treats them as regular words
/// (CLAN simply doesn't exclude `0` from its word-prefix check). We consider
/// omissions non-spoken since the word was not actually uttered.
fn is_comma_licensing(item: &UtteranceContent) -> bool {
    match item {
        UtteranceContent::Word(word) => is_real_word(word),
        UtteranceContent::AnnotatedWord(annotated) => is_real_word(&annotated.inner),
        UtteranceContent::ReplacedWord(replaced) => is_real_word(&replaced.word),
        // Recurse into groups: <words> [//] still contains spoken words.
        UtteranceContent::AnnotatedGroup(group) => group
            .inner
            .content
            .content
            .iter()
            .any(is_spoken_bracketed_item),
        UtteranceContent::Group(group) => {
            group.content.content.iter().any(is_spoken_bracketed_item)
        }
        UtteranceContent::PhoGroup(pho) => pho.content.content.iter().any(is_spoken_bracketed_item),
        UtteranceContent::SinGroup(sin) => sin.content.content.iter().any(is_spoken_bracketed_item),
        UtteranceContent::Quotation(quot) => {
            quot.content.content.iter().any(is_spoken_bracketed_item)
        }
        // Pauses license commas (matching CLAN CHECK behavior).
        UtteranceContent::Pause(_) => true,
        // Retrace content may contain spoken words — recurse to check.
        UtteranceContent::Retrace(retrace) => {
            retrace.content.content.iter().any(is_spoken_bracketed_item)
        }
        // Events, separators, actions, markers, etc. do not license commas.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => false,
    }
}

/// Returns `true` if this bracketed item licenses a subsequent comma.
///
/// Parallel to [`is_comma_licensing`] for `BracketedItem`.
fn is_spoken_bracketed_item(item: &BracketedItem) -> bool {
    match item {
        BracketedItem::Word(word) => is_real_word(word),
        BracketedItem::AnnotatedWord(annotated) => is_real_word(&annotated.inner),
        BracketedItem::ReplacedWord(replaced) => is_real_word(&replaced.word),
        BracketedItem::AnnotatedGroup(group) => group
            .inner
            .content
            .content
            .iter()
            .any(is_spoken_bracketed_item),
        BracketedItem::PhoGroup(pho) => pho.content.content.iter().any(is_spoken_bracketed_item),
        BracketedItem::SinGroup(sin) => sin.content.content.iter().any(is_spoken_bracketed_item),
        BracketedItem::Quotation(quot) => quot.content.content.iter().any(is_spoken_bracketed_item),
        // Pauses license commas (matching CLAN CHECK behavior).
        BracketedItem::Pause(_) => true,
        // Retrace content may contain spoken words — recurse to check.
        BracketedItem::Retrace(retrace) => {
            retrace.content.content.iter().any(is_spoken_bracketed_item)
        }
        // Events, actions, separators, markers, etc. do not license commas.
        BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::OverlapPoint(_)
        | BracketedItem::Separator(_)
        | BracketedItem::InternalBullet(_)
        | BracketedItem::Freecode(_)
        | BracketedItem::LongFeatureBegin(_)
        | BracketedItem::LongFeatureEnd(_)
        | BracketedItem::UnderlineBegin(_)
        | BracketedItem::UnderlineEnd(_)
        | BracketedItem::NonvocalBegin(_)
        | BracketedItem::NonvocalEnd(_)
        | BracketedItem::NonvocalSimple(_)
        | BracketedItem::OtherSpokenEvent(_) => false,
    }
}

/// E259: Validate that comma-licensing content appears before each comma.
///
/// A comma requires at least one real spoken word or pause to have appeared
/// before it in the utterance. Once any comma-licensing content is seen, all
/// subsequent commas are valid regardless of what immediately precedes them.
///
/// Pauses `(...)` license commas even though they are not spoken words,
/// matching CLAN CHECK behavior (where `(` is not a punctuation character
/// and thus pauses set `CommaWordFound`).
pub(crate) fn check_comma_after_non_spoken(utterance: &Utterance, errors: &impl ErrorSink) {
    let items = &utterance.main.content.content;
    let mut seen_real_word = false;

    for item in items {
        if is_comma_licensing(item) {
            seen_real_word = true;
            continue;
        }

        if let UtteranceContent::Separator(Separator::Comma { span }) = item
            && !seen_real_word
        {
            errors.report(
                ParseError::new(
                    ErrorCode::CommaAfterNonSpokenContent,
                    Severity::Error,
                    SourceLocation::new(*span),
                    ErrorContext::new(",", *span, ","),
                    "Comma without any prior spoken word in the utterance",
                )
                .with_suggestion("Remove the comma or add a spoken word earlier in the utterance"),
            );
        }
    }
}

/// E258: Validate that no two comma separators appear consecutively in
/// document order.
///
/// Uses [`walk_content`] to traverse all content including inside groups,
/// so `hello , <, world> [= yes] .` is caught — the comma before the
/// group and the comma inside the group are consecutive in document order.
pub(crate) fn check_consecutive_commas(utterance: &Utterance, errors: &impl ErrorSink) {
    let mut prev_comma_span: Option<crate::Span> = None;

    walk_content(&utterance.main.content.content.0, None, &mut |item| {
        match item {
            ContentItem::Separator(Separator::Comma { span }) => {
                if let Some(_prev_span) = prev_comma_span {
                    errors.report(
                        ParseError::new(
                            ErrorCode::ConsecutiveCommas,
                            Severity::Error,
                            SourceLocation::new(*span),
                            ErrorContext::new(",,", *span, ","),
                            "Consecutive commas in utterance",
                        )
                        .with_suggestion(
                            "Use a single comma, or replace ,, with the tag marker \u{201E} (U+201E)",
                        ),
                    );
                }
                prev_comma_span = Some(*span);
            }
            // Any non-comma content item resets the consecutive check.
            _ => {
                prev_comma_span = None;
            }
        }
    });
}
