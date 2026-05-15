//! Alignment-domain counting/extraction over main-tier content trees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use crate::model::{
    BracketedContent, BracketedItem, ContentAnnotation, ReplacedWord, UtteranceContent, Word,
};

use super::domain::TierDomain;
use super::rules::{
    annotations_have_alignment_ignore, counts_for_tier_in_context, is_tag_marker_separator,
    should_align_replaced_word_in_pho_sin, should_skip_group,
};
use super::to_chat_display_string as to_string;

/// One extracted alignable item shown in mismatch diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct TierPosition {
    /// Display text for this item (e.g., "hello", "&-um", ".")
    pub text: String,
    /// Optional description for complex items (e.g., "[/]" for retracing)
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Typed counts for cross-tier alignment
//
// These newtypes exist so a pipeline can't accidentally confuse "number of
// Mor-alignable words on the main tier" with "number of %mor items on the
// dependent tier". They agree on an Aligned utterance and disagree on a
// MisalignmentBug — both are interesting facts the type system must let us
// state without primitive obsession.
//
// The types are intentionally minimal: they expose only `get()` and basic
// derives. Arithmetic, serialization, and display formatting happen at the
// call site with explicit unwrapping, because any time you're doing math
// on these you are likely crossing a semantic boundary that deserves a
// comment.
// ---------------------------------------------------------------------------

/// Count of CHAT main-tier words alignable to the `%mor` dependent tier.
///
/// Produced by [`Utterance::mor_alignable_word_count`](crate::model::Utterance::mor_alignable_word_count)
/// applying the rules in `alignment/helpers/rules.rs::counts_for_tier`.
/// This is the **expected** size of the `%mor` tier on this utterance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorAlignableWordCount(usize);

impl MorAlignableWordCount {
    /// Wrap a raw count.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Inner count.
    pub fn get(self) -> usize {
        self.0
    }

    /// `true` iff this utterance has no Mor-alignable content — i.e.,
    /// `%mor` should not be produced at all (NotApplicable).
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for MorAlignableWordCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Count of items on a `%mor` dependent tier.
///
/// Produced by `mor_tier.items.len()` when the tier is present. When
/// paired with a [`MorAlignableWordCount`], the two either agree
/// (Aligned) or disagree (MisalignmentBug); the types prevent you from
/// swapping them in the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MorItemCount(usize);

impl MorItemCount {
    /// Wrap a raw count.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Inner count.
    pub fn get(self) -> usize {
        self.0
    }

    /// `true` iff the tier is empty (or was absent, in which case we
    /// represent the absence as `None` at the call site — do not
    /// substitute `MorItemCount::new(0)` for a missing tier).
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for MorItemCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Extract alignable items with their display text for a given alignment domain.
///
/// The returned sequence matches alignment traversal order and is used to build
/// human-readable mismatch diagnostics (`main` vs dependent-tier views).
pub fn collect_tier_items(content: &[UtteranceContent], domain: TierDomain) -> Vec<TierPosition> {
    let mut items = Vec::new();
    for item in content {
        extract_alignable_from_item(item, domain, false, &mut items);
    }
    items
}

/// Count alignable units for a given alignment domain.
///
/// This is the fast path for preflight length checks before building richer
/// positional mismatch details.
pub fn count_tier_positions(content: &[UtteranceContent], domain: TierDomain) -> usize {
    content
        .iter()
        .map(|item| count_alignable_item(item, domain, false))
        .sum()
}

/// Count alignable content up to (but not including) a specific index.
///
/// This is useful for LSP hover features where you need to know how many
/// alignable items precede a given position in the content array.
/// The result uses the same domain-specific inclusion rules as full alignment.
///
/// # Parameters
/// - `content`: The utterance content to count
/// - `max_index`: Only count items before this index (exclusive)
/// - `domain`: The alignment domain (Mor, Pho, or Sin)
///
/// # Returns
/// The count of alignable items in `content[0..max_index]`
///
/// # Examples
/// ```
/// use talkbank_model::alignment::{count_tier_positions_until, TierDomain};
/// use talkbank_model::model::{UtteranceContent, Word};
///
/// let content = vec![
///     UtteranceContent::Word(Box::new(Word::new_unchecked("hello", "hello"))),
///     UtteranceContent::Word(Box::new(Word::new_unchecked("world", "world"))),
/// ];
///
/// // Count items before index 1 (only first word)
/// let count = count_tier_positions_until(&content, 1, TierDomain::Mor);
/// assert_eq!(count, 1);
///
/// // Count items before index 2 (both words)
/// let count = count_tier_positions_until(&content, 2, TierDomain::Mor);
/// assert_eq!(count, 2);
/// ```
pub fn count_tier_positions_until(
    content: &[UtteranceContent],
    max_index: usize,
    domain: TierDomain,
) -> usize {
    content
        .iter()
        .take(max_index)
        .map(|item| count_alignable_item(item, domain, false))
        .sum()
}

/// Counts one main-tier item's contribution in the target alignment domain.
fn count_alignable_item(item: &UtteranceContent, domain: TierDomain, in_retrace: bool) -> usize {
    match item {
        UtteranceContent::Word(word) => count_alignable_word(word, &[], domain, in_retrace),
        UtteranceContent::AnnotatedWord(annotated) => count_alignable_word(
            &annotated.inner,
            &annotated.scoped_annotations,
            domain,
            in_retrace,
        ),
        UtteranceContent::ReplacedWord(replaced) => {
            count_alignable_replaced_word(replaced, domain, in_retrace)
        }
        UtteranceContent::Group(group) => {
            count_bracketed_alignable_content(&group.content, domain, in_retrace)
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            if should_skip_group(&annotated.scoped_annotations, domain) {
                0
            } else {
                count_bracketed_alignable_content(&annotated.inner.content, domain, in_retrace)
            }
        }
        UtteranceContent::PhoGroup(pho) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                count_bracketed_alignable_content(&pho.content, domain, in_retrace)
            }
            TierDomain::Pho => 1,
            TierDomain::Sin => 0,
        },
        UtteranceContent::SinGroup(sin) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                count_bracketed_alignable_content(&sin.content, domain, in_retrace)
            }
            TierDomain::Sin => 1,
            TierDomain::Pho => 0,
        },
        UtteranceContent::Quotation(quot) => {
            count_bracketed_alignable_content(&quot.content, domain, in_retrace)
        }
        UtteranceContent::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                1
            } else {
                0
            }
        }
        UtteranceContent::Pause(_) => {
            // Pauses are phonological events that get transcribed in %pho tiers
            // but NOT in %wor tiers (which only contain actual words)
            // %mor and %sin also don't align to pauses
            if domain == TierDomain::Pho { 1 } else { 0 }
        }
        UtteranceContent::AnnotatedAction(_) => {
            if domain == TierDomain::Sin {
                1
            } else {
                0
            }
        }
        UtteranceContent::Retrace(retrace) => {
            // Retrace content is excluded from %mor (not morphologically analyzed),
            // but included in %pho/%sin/%wor (the words were phonologically produced).
            if domain == TierDomain::Mor {
                0
            } else {
                count_bracketed_alignable_content(&retrace.content, domain, true)
            }
        }
        // All remaining variants are non-alignable for every dependent tier:
        // events, markers, formatting, freecodes, overlap points, internal bullets.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => 0,
    }
}

/// Counts bracketed content recursively for alignment in `domain`.
fn count_bracketed_alignable_content(
    content: &BracketedContent,
    domain: TierDomain,
    in_retrace: bool,
) -> usize {
    content
        .content
        .iter()
        .map(|item| count_bracketed_item(item, domain, in_retrace))
        .sum()
}

/// Counts one bracketed item's alignment contribution in `domain`.
fn count_bracketed_item(item: &BracketedItem, domain: TierDomain, in_retrace: bool) -> usize {
    match item {
        BracketedItem::Word(word) => count_alignable_word(word, &[], domain, in_retrace),
        BracketedItem::AnnotatedWord(annotated) => count_alignable_word(
            &annotated.inner,
            &annotated.scoped_annotations,
            domain,
            in_retrace,
        ),
        BracketedItem::ReplacedWord(replaced) => {
            count_alignable_replaced_word(replaced, domain, in_retrace)
        }
        BracketedItem::AnnotatedGroup(annotated) => {
            if should_skip_group(&annotated.scoped_annotations, domain) {
                0
            } else {
                count_bracketed_alignable_content(&annotated.inner.content, domain, in_retrace)
            }
        }
        BracketedItem::PhoGroup(pho) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                count_bracketed_alignable_content(&pho.content, domain, in_retrace)
            }
            TierDomain::Pho => 1,
            TierDomain::Sin => 0,
        },
        BracketedItem::SinGroup(sin) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                count_bracketed_alignable_content(&sin.content, domain, in_retrace)
            }
            TierDomain::Sin => 1,
            TierDomain::Pho => 0,
        },
        BracketedItem::Quotation(quot) => {
            count_bracketed_alignable_content(&quot.content, domain, in_retrace)
        }
        BracketedItem::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                1
            } else {
                0
            }
        }
        BracketedItem::Retrace(retrace) => {
            // Retrace content is excluded from %mor but counted for %pho/%sin/%wor.
            if domain == TierDomain::Mor {
                0
            } else {
                count_bracketed_alignable_content(&retrace.content, domain, true)
            }
        }
        // All remaining variants are non-alignable inside bracketed content:
        // pauses, actions, events, markers, formatting, freecodes, overlap points.
        BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Pause(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::OverlapPoint(_)
        | BracketedItem::InternalBullet(_)
        | BracketedItem::Freecode(_)
        | BracketedItem::LongFeatureBegin(_)
        | BracketedItem::LongFeatureEnd(_)
        | BracketedItem::UnderlineBegin(_)
        | BracketedItem::UnderlineEnd(_)
        | BracketedItem::NonvocalBegin(_)
        | BracketedItem::NonvocalEnd(_)
        | BracketedItem::NonvocalSimple(_)
        | BracketedItem::OtherSpokenEvent(_) => 0,
    }
}

/// Counts one word token after per-domain exclusion rules.
fn count_alignable_word(
    word: &Word,
    annotations: &[ContentAnnotation],
    domain: TierDomain,
    in_retrace: bool,
) -> usize {
    // For individual words, retrace annotations only skip for Mor domain.
    // Retraced words may still appear in %pho/%sin (they were spoken, just corrected).
    // Note: Groups with retrace skip ALL domains - see should_skip_group.
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(annotations) {
        return 0;
    }

    if !counts_for_tier_in_context(word, domain, in_retrace) {
        return 0;
    }

    1
}

/// Counts a `ReplacedWord` node after replacement/retrace rules.
fn count_alignable_replaced_word(
    entry: &ReplacedWord,
    domain: TierDomain,
    in_retrace: bool,
) -> usize {
    // For replaced words (like groups), retrace annotations only skip for Mor domain
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(&entry.scoped_annotations) {
        return 0;
    }

    match domain {
        TierDomain::Mor => {
            // %mor aligns to replacement words when present because
            // morphology follows the corrected transcript slot.
            if !entry.replacement.words.is_empty() {
                entry
                    .replacement
                    .words
                    .iter()
                    .filter(|word| counts_for_tier_in_context(word, domain, in_retrace))
                    .count()
            } else if counts_for_tier_in_context(&entry.word, domain, in_retrace) {
                1
            } else {
                0
            }
        }
        TierDomain::Wor => {
            // %wor aligns to the originally spoken surface form, not the
            // editorial replacement.
            usize::from(counts_for_tier_in_context(&entry.word, domain, in_retrace))
        }
        TierDomain::Pho | TierDomain::Sin => {
            // %pho and %sin align to the original word (what was actually
            // spoken/produced), not the replacement. This means a replaced word
            // always contributes at most 1 item, regardless of how many replacement
            // words there are.
            if should_align_replaced_word_in_pho_sin(
                &entry.word,
                !entry.replacement.words.is_empty(),
            ) {
                1
            } else {
                0
            }
        }
    }
}

/// Extracts alignable units from one top-level utterance content item.
///
/// Output order matches traversal order so mismatch diagnostics map cleanly to
/// the original transcript sequence.
fn extract_alignable_from_item(
    item: &UtteranceContent,
    domain: TierDomain,
    in_retrace: bool,
    output: &mut Vec<TierPosition>,
) {
    match item {
        UtteranceContent::Word(word) => {
            extract_alignable_from_word(word, &[], domain, in_retrace, output)
        }
        UtteranceContent::AnnotatedWord(annotated) => extract_alignable_from_word(
            &annotated.inner,
            &annotated.scoped_annotations,
            domain,
            in_retrace,
            output,
        ),
        UtteranceContent::ReplacedWord(replaced) => {
            extract_alignable_from_replaced_word(replaced, domain, in_retrace, output)
        }
        UtteranceContent::Group(group) => {
            extract_alignable_from_bracketed_content(&group.content, domain, in_retrace, output)
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            if !should_skip_group(&annotated.scoped_annotations, domain) {
                extract_alignable_from_bracketed_content(
                    &annotated.inner.content,
                    domain,
                    in_retrace,
                    output,
                )
            }
        }
        UtteranceContent::PhoGroup(pho) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                extract_alignable_from_bracketed_content(&pho.content, domain, in_retrace, output)
            }
            TierDomain::Pho => {
                output.push(TierPosition {
                    text: to_string(pho),
                    description: Some("phonological group".to_string()),
                });
            }
            TierDomain::Sin => {}
        },
        UtteranceContent::SinGroup(sin) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                extract_alignable_from_bracketed_content(&sin.content, domain, in_retrace, output)
            }
            TierDomain::Sin => {
                output.push(TierPosition {
                    text: to_string(sin),
                    description: Some("sign group".to_string()),
                });
            }
            TierDomain::Pho => {}
        },
        UtteranceContent::Quotation(quot) => {
            extract_alignable_from_bracketed_content(&quot.content, domain, in_retrace, output)
        }
        UtteranceContent::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                output.push(TierPosition {
                    text: to_string(sep),
                    description: None,
                });
            }
        }
        UtteranceContent::Pause(pause) => {
            if domain == TierDomain::Pho {
                output.push(TierPosition {
                    text: to_string(pause),
                    description: Some("pause".to_string()),
                });
            }
        }
        UtteranceContent::AnnotatedAction(action) => {
            if domain == TierDomain::Sin {
                output.push(TierPosition {
                    text: to_string(action),
                    description: Some("action".to_string()),
                });
            }
        }
        UtteranceContent::Retrace(retrace) => {
            // Retrace content is excluded from %mor but extracted for %pho/%sin/%wor.
            if domain != TierDomain::Mor {
                extract_alignable_from_bracketed_content(&retrace.content, domain, true, output);
            }
        }
        // All remaining variants produce no alignable items:
        // events, markers, formatting, freecodes, overlap points, internal bullets.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Freecode(_)
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

/// Extracts alignable units from bracketed content recursively.
///
/// Nested groups/quotations are traversed depth-first while preserving document
/// order in the emitted list.
fn extract_alignable_from_bracketed_content(
    content: &BracketedContent,
    domain: TierDomain,
    in_retrace: bool,
    output: &mut Vec<TierPosition>,
) {
    for item in &content.content {
        extract_alignable_from_bracketed_item(item, domain, in_retrace, output);
    }
}

/// Extracts alignable units from one bracketed item variant.
///
/// This mirrors top-level extraction rules but for bracket-scoped structures.
fn extract_alignable_from_bracketed_item(
    item: &BracketedItem,
    domain: TierDomain,
    in_retrace: bool,
    output: &mut Vec<TierPosition>,
) {
    match item {
        BracketedItem::Word(word) => {
            extract_alignable_from_word(word, &[], domain, in_retrace, output)
        }
        BracketedItem::AnnotatedWord(annotated) => extract_alignable_from_word(
            &annotated.inner,
            &annotated.scoped_annotations,
            domain,
            in_retrace,
            output,
        ),
        BracketedItem::ReplacedWord(replaced) => {
            extract_alignable_from_replaced_word(replaced, domain, in_retrace, output)
        }
        BracketedItem::AnnotatedGroup(annotated) => {
            if !should_skip_group(&annotated.scoped_annotations, domain) {
                extract_alignable_from_bracketed_content(
                    &annotated.inner.content,
                    domain,
                    in_retrace,
                    output,
                )
            }
        }
        BracketedItem::PhoGroup(pho) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                extract_alignable_from_bracketed_content(&pho.content, domain, in_retrace, output)
            }
            TierDomain::Pho => {
                output.push(TierPosition {
                    text: to_string(pho),
                    description: Some("phonological group".to_string()),
                });
            }
            TierDomain::Sin => {}
        },
        BracketedItem::SinGroup(sin) => match domain {
            TierDomain::Mor | TierDomain::Wor => {
                extract_alignable_from_bracketed_content(&sin.content, domain, in_retrace, output)
            }
            TierDomain::Sin => {
                output.push(TierPosition {
                    text: to_string(sin),
                    description: Some("sign group".to_string()),
                });
            }
            TierDomain::Pho => {}
        },
        BracketedItem::Quotation(quot) => {
            extract_alignable_from_bracketed_content(&quot.content, domain, in_retrace, output)
        }
        BracketedItem::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                output.push(TierPosition {
                    text: to_string(sep),
                    description: None,
                });
            }
        }
        BracketedItem::Retrace(retrace) => {
            // Retrace content is excluded from %mor but extracted for %pho/%sin/%wor.
            if domain != TierDomain::Mor {
                extract_alignable_from_bracketed_content(&retrace.content, domain, true, output);
            }
        }
        // All remaining variants produce no alignable items inside bracketed content:
        // pauses, actions, events, markers, formatting, freecodes, overlap points.
        BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Pause(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::OverlapPoint(_)
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

/// Extracts one alignable word token after domain-specific filtering.
///
/// For MOR alignment, retrace-ignored annotations suppress the token.
fn extract_alignable_from_word(
    word: &Word,
    annotations: &[ContentAnnotation],
    domain: TierDomain,
    in_retrace: bool,
    output: &mut Vec<TierPosition>,
) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(annotations) {
        return;
    }

    if !counts_for_tier_in_context(word, domain, in_retrace) {
        return;
    }

    output.push(TierPosition {
        text: to_string(word),
        description: None,
    });
}

/// Extracts alignable units from a replaced-word node.
///
/// %mor prefers replacement words when available, whereas %pho/%sin/%wor keep
/// the originally produced form for alignment.
fn extract_alignable_from_replaced_word(
    entry: &ReplacedWord,
    domain: TierDomain,
    in_retrace: bool,
    output: &mut Vec<TierPosition>,
) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(&entry.scoped_annotations) {
        return;
    }

    match domain {
        TierDomain::Mor => {
            // %mor aligns to replacement words when present because
            // morphology follows the corrected transcript slot.
            if !entry.replacement.words.is_empty() {
                for word in &entry.replacement.words {
                    if counts_for_tier_in_context(word, domain, in_retrace) {
                        output.push(TierPosition {
                            text: to_string(word),
                            description: None,
                        });
                    }
                }
            } else if counts_for_tier_in_context(&entry.word, domain, in_retrace) {
                output.push(TierPosition {
                    text: to_string(&entry.word),
                    description: None,
                });
            }
        }
        TierDomain::Wor => {
            // %wor aligns to the originally spoken surface form, not the
            // editorial replacement.
            if counts_for_tier_in_context(&entry.word, domain, in_retrace) {
                output.push(TierPosition {
                    text: to_string(&entry.word),
                    description: None,
                });
            }
        }
        TierDomain::Pho | TierDomain::Sin => {
            // %pho and %sin align to the original word (what was actually
            // spoken/produced), not the replacement.
            if should_align_replaced_word_in_pho_sin(
                &entry.word,
                !entry.replacement.words.is_empty(),
            ) {
                output.push(TierPosition {
                    text: to_string(&entry.word),
                    description: None,
                });
            }
        }
    }
}
