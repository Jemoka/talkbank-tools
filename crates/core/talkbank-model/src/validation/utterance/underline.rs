//! Underline-marker balance validation for utterance content trees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::model::{
    BracketedContent, BracketedItem, Utterance, UtteranceContent, Word, WordContent,
};
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

/// Validate underline markers are balanced in an utterance.
///
/// CHAT format uses control characters `\u0002\u0001` for underline begin
/// and `\u0002\u0002` for underline end.
///
/// This validates that within a single utterance:
/// - Every underline begin has a corresponding underline end
/// - Underline markers are properly paired (no crossing/interleaving)
///
/// Uses stack-based validation to ensure proper pairing, not just counting.
///
/// Note: Underline markers are within-utterance only (they do not cross utterances).
pub(crate) fn check_underline_balance(utterance: &Utterance, errors: &impl ErrorSink) {
    let tier_span = utterance.main.span;
    // Stack of spans for each open underline-begin marker
    let mut begin_spans: Vec<Span> = Vec::new();

    for content in &utterance.main.content.content {
        walk_underline_balance_in_content(content, &mut begin_spans, tier_span, errors);
    }

    // Check for unclosed begin markers
    for begin_span in &begin_spans {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedUnderlineBegin,
                Severity::Error,
                *begin_span,
                "Unmatched underline begin: unclosed begin marker (␂␁)",
            )
            .with_suggestion("Ensure each underline begin (␂␁) has a matching underline end (␂␂)"),
        );
    }
}

/// Walk underline-balance state through one utterance-content node.
///
/// The traversal propagates a shared begin stack so nested groups and words
/// participate in the same pairing context.
fn walk_underline_balance_in_content(
    item: &UtteranceContent,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    match item {
        UtteranceContent::UnderlineBegin(marker) => {
            begin_spans.push(if marker.span.is_dummy() {
                fallback_span
            } else {
                marker.span
            });
        }
        UtteranceContent::UnderlineEnd(marker) => {
            let end_span = if marker.span.is_dummy() {
                fallback_span
            } else {
                marker.span
            };
            apply_underline_end(begin_spans, end_span, errors);
        }
        UtteranceContent::Word(word) => {
            walk_underline_balance_in_word(word, begin_spans, fallback_span, errors);
        }
        UtteranceContent::AnnotatedWord(word) => {
            walk_underline_balance_in_word(&word.inner, begin_spans, fallback_span, errors);
        }
        UtteranceContent::ReplacedWord(replaced) => {
            walk_underline_balance_in_word(&replaced.word, begin_spans, fallback_span, errors);
            for replacement in &replaced.replacement.words {
                walk_underline_balance_in_word(replacement, begin_spans, fallback_span, errors);
            }
        }
        UtteranceContent::Group(group) => {
            walk_underline_balance_in_bracketed(&group.content, begin_spans, fallback_span, errors);
        }
        UtteranceContent::AnnotatedGroup(group) => {
            walk_underline_balance_in_bracketed(
                &group.inner.content,
                begin_spans,
                fallback_span,
                errors,
            );
        }
        UtteranceContent::PhoGroup(group) => {
            walk_underline_balance_in_bracketed(&group.content, begin_spans, fallback_span, errors);
        }
        UtteranceContent::SinGroup(group) => {
            walk_underline_balance_in_bracketed(&group.content, begin_spans, fallback_span, errors);
        }
        UtteranceContent::Quotation(quote) => {
            walk_underline_balance_in_bracketed(&quote.content, begin_spans, fallback_span, errors);
        }
        UtteranceContent::Retrace(retrace) => {
            walk_underline_balance_in_bracketed(
                &retrace.content,
                begin_spans,
                fallback_span,
                errors,
            );
        }
        UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

/// Walk underline-balance state through bracketed content recursively.
fn walk_underline_balance_in_bracketed(
    content: &BracketedContent,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    for item in &content.content {
        match item {
            BracketedItem::UnderlineBegin(marker) => {
                begin_spans.push(if marker.span.is_dummy() {
                    fallback_span
                } else {
                    marker.span
                });
            }
            BracketedItem::UnderlineEnd(marker) => {
                let end_span = if marker.span.is_dummy() {
                    fallback_span
                } else {
                    marker.span
                };
                apply_underline_end(begin_spans, end_span, errors);
            }
            BracketedItem::Word(word) => {
                walk_underline_balance_in_word(word, begin_spans, fallback_span, errors);
            }
            BracketedItem::AnnotatedWord(word) => {
                walk_underline_balance_in_word(&word.inner, begin_spans, fallback_span, errors);
            }
            BracketedItem::ReplacedWord(replaced) => {
                walk_underline_balance_in_word(&replaced.word, begin_spans, fallback_span, errors);
                for replacement in &replaced.replacement.words {
                    walk_underline_balance_in_word(replacement, begin_spans, fallback_span, errors);
                }
            }
            BracketedItem::AnnotatedGroup(group) => {
                walk_underline_balance_in_bracketed(
                    &group.inner.content,
                    begin_spans,
                    fallback_span,
                    errors,
                );
            }
            BracketedItem::PhoGroup(group) => {
                walk_underline_balance_in_bracketed(
                    &group.content,
                    begin_spans,
                    fallback_span,
                    errors,
                );
            }
            BracketedItem::SinGroup(group) => {
                walk_underline_balance_in_bracketed(
                    &group.content,
                    begin_spans,
                    fallback_span,
                    errors,
                );
            }
            BracketedItem::Quotation(quote) => {
                walk_underline_balance_in_bracketed(
                    &quote.content,
                    begin_spans,
                    fallback_span,
                    errors,
                );
            }
            BracketedItem::Retrace(retrace) => {
                walk_underline_balance_in_bracketed(
                    &retrace.content,
                    begin_spans,
                    fallback_span,
                    errors,
                );
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::Separator(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk underline-balance state through inline word content markers.
fn walk_underline_balance_in_word(
    word: &Word,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    let word_span = if word.span.is_dummy() {
        fallback_span
    } else {
        word.span
    };
    for wc in &word.content {
        match wc {
            WordContent::UnderlineBegin(wb) => {
                let span = if wb.span.is_dummy() {
                    word_span
                } else {
                    wb.span
                };
                begin_spans.push(span);
            }
            WordContent::UnderlineEnd(we) => {
                let span = if we.span.is_dummy() {
                    word_span
                } else {
                    we.span
                };
                apply_underline_end(begin_spans, span, errors);
            }
            _ => {}
        }
    }
}

/// Apply one underline-end marker against the current begin stack.
///
/// If no open begin exists, emit `UnmatchedUnderlineEnd` at the end marker span.
fn apply_underline_end(begin_spans: &mut Vec<Span>, end_span: Span, errors: &impl ErrorSink) {
    if begin_spans.pop().is_none() {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedUnderlineEnd,
                Severity::Error,
                end_span,
                "Unmatched underline end (␂␂) without corresponding begin (␂␁)",
            )
            .with_suggestion(
                "Ensure each underline end (␂␂) has a matching underline begin (␂␁) before it",
            ),
        );
    }
}
