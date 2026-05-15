//! Post-parse normalization passes for CHAT line models.
//!
//! These passes rewrite parser output into canonical model forms expected by
//! validators and downstream alignment code.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Shortenings>

use crate::model::{
    BracketedContent, BracketedItem, Header, Line, MainTier, ReplacedWord, UtteranceContent, Word,
    WordCategory, WordContent, WordShortening, WordText,
};

/// Return whether the `@Options` header enables CA mode.
///
/// CA mode is used by multiple downstream passes (including validation rules such as terminator handling).
/// This module uses the flag to decide whether to run CA-omission normalization.
pub(super) fn headers_enable_ca_mode(headers: &[Header]) -> bool {
    headers.iter().any(|header| {
        matches!(header, Header::Options { options } if options.iter().any(|opt| opt.enables_ca_mode()))
    })
}

/// Normalize CA omission shorthand across all utterances when CA mode is enabled.
///
/// Specifically, this pass targets words categorized as `WordCategory::CAOmission` whose content is a
/// standalone `WordContent::Shortening` token (the internal representation for parenthesized omission text).
/// It rewrites that shortening token into plain text so later passes operate on a canonical word shape.
pub(super) fn normalize_ca_omissions(lines: &mut [Line]) {
    for line in lines {
        if let Line::Utterance(utterance) = line {
            normalize_ca_omissions_main_tier(&mut utterance.main);
        }
    }
}

/// Normalize CA omission shorthand within a single `MainTier`.
///
/// This function only handles omission-token normalization; broader CA-mode validation behavior
/// (including terminator policy) is handled elsewhere.
pub(crate) fn normalize_ca_omissions_main_tier(main: &mut MainTier) {
    for content in &mut main.content.content {
        normalize_ca_omission_content(content);
    }
}

/// Recursively normalize one `UtteranceContent` node for CA omission markers.
fn normalize_ca_omission_content(content: &mut UtteranceContent) {
    match content {
        UtteranceContent::Word(word) => normalize_ca_omission_word(word),
        UtteranceContent::AnnotatedWord(annotated) => {
            normalize_ca_omission_word(&mut annotated.inner);
        }
        UtteranceContent::ReplacedWord(replaced) => {
            normalize_ca_omission_replaced_word(replaced.as_mut());
        }
        UtteranceContent::Group(group) => normalize_ca_omission_bracketed(&mut group.content),
        UtteranceContent::AnnotatedGroup(annotated) => {
            normalize_ca_omission_bracketed(&mut annotated.inner.content);
        }
        UtteranceContent::Retrace(retrace) => {
            normalize_ca_omission_bracketed(&mut retrace.content);
        }
        UtteranceContent::PhoGroup(group) => normalize_ca_omission_bracketed(&mut group.content),
        UtteranceContent::SinGroup(group) => normalize_ca_omission_bracketed(&mut group.content),
        UtteranceContent::Quotation(quote) => normalize_ca_omission_bracketed(&mut quote.content),
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
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

/// Recursively normalize words inside bracketed/group content trees for CA omissions.
fn normalize_ca_omission_bracketed(content: &mut BracketedContent) {
    for item in &mut content.content {
        match item {
            BracketedItem::Word(word) => normalize_ca_omission_word(word),
            BracketedItem::AnnotatedWord(annotated) => {
                normalize_ca_omission_word(&mut annotated.inner);
            }
            BracketedItem::ReplacedWord(replaced) => {
                normalize_ca_omission_replaced_word(replaced.as_mut());
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                normalize_ca_omission_bracketed(&mut annotated.inner.content);
            }
            BracketedItem::Retrace(retrace) => {
                normalize_ca_omission_bracketed(&mut retrace.content);
            }
            BracketedItem::PhoGroup(group) => normalize_ca_omission_bracketed(&mut group.content),
            BracketedItem::SinGroup(group) => normalize_ca_omission_bracketed(&mut group.content),
            BracketedItem::Quotation(quote) => normalize_ca_omission_bracketed(&mut quote.content),
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
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
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

/// Normalize omission markers in both sides of a replacement-word construct.
///
/// Replacement forms can contain CA-omission shorthand on either side; this keeps their normalized
/// representation consistent with the non-replacement path.
fn normalize_ca_omission_replaced_word(replaced: &mut ReplacedWord) {
    normalize_ca_omission_word(&mut replaced.word);
    for word in &mut replaced.replacement.words {
        normalize_ca_omission_word(word);
    }
}

/// Rewrite standalone CA-omission shortening tokens into text in-place.
///
/// This applies only when the word is categorized as `CAOmission` and contains exactly one
/// `WordContent::Shortening` plus optional non-lexical markers. Mixed lexical content is left unchanged.
/// The rewrite canonicalizes representation; it does not implement CA-mode validation policy.
fn normalize_ca_omission_word(word: &mut Word) {
    if matches!(word.category.as_ref(), Some(category) if *category != WordCategory::CAOmission) {
        return;
    }

    let mut shortening_index = None;
    for (idx, item) in word.content.iter().enumerate() {
        match item {
            WordContent::Shortening(_) => {
                if shortening_index.is_some() {
                    return;
                }
                shortening_index = Some(idx);
            }
            WordContent::Text(_) | WordContent::CompoundMarker(_) => {
                // Not a standalone CA omission form.
                return;
            }
            WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::CADelimiter(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::UnderlineBegin(_)
            | WordContent::UnderlineEnd(_)
            | WordContent::CliticBoundary(_) => {}
        }
    }

    if let Some(shortening_index) = shortening_index {
        let WordContent::Shortening(shortening) = word.content[shortening_index].clone() else {
            return;
        };
        let WordShortening(inner) = shortening;
        word.content
            .replace_at(shortening_index, WordContent::Text(WordText(inner)));
        word.category = Some(WordCategory::CAOmission);
    }
}
