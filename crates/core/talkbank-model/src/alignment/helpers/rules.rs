//! Rule predicates used by cross-tier alignment logic.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MorExclude_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use crate::model::{ContentAnnotation, Separator, Word, WordCategory};

use super::domain::TierDomain;

/// Returns `true` if a group with these annotations should be skipped during alignment.
///
/// Retrace annotations skip only for the Mor domain; retraced content
/// was phonologically produced, so %pho, %sin, and %wor include it.
/// This helper exists so callers can consistently apply the same domain gate
/// when handling annotated groups at any nesting level.
pub fn should_skip_group(annotations: &[ContentAnnotation], domain: TierDomain) -> bool {
    // Retrace annotations skip ONLY for Mor domain (linguistic content analysis).
    // Retraced content WAS produced phonologically (speaker said it before correcting),
    // so %pho, %sin, and %wor all include transcriptions for retraced groups.
    domain == TierDomain::Mor && annotations_have_alignment_ignore(annotations)
}

/// Returns `true` if any annotation in the slice excludes content from alignment.
///
/// This helper is domain-agnostic; callers decide whether exclusion applies in
/// the current alignment domain.
pub fn annotations_have_alignment_ignore(annotations: &[ContentAnnotation]) -> bool {
    annotations.iter().any(is_alignment_ignore_annotation)
}

/// Returns whether one annotation indicates alignment exclusion semantics.
///
/// The `[e]` exclude marker represents suppressed material, so alignment
/// policies may choose to drop the annotated content.
///
/// Retrace markers are no longer `ContentAnnotation` variants — they are
/// handled as first-class `Retrace` content variants at the `UtteranceContent`
/// and `BracketedItem` level.
fn is_alignment_ignore_annotation(annotation: &ContentAnnotation) -> bool {
    matches!(annotation, ContentAnnotation::Exclude)
}

/// Return whether a word participates in alignment for the target domain.
///
/// This is the canonical domain gate used by callers that do not carry
/// container context.
pub fn counts_for_tier(word: &Word, domain: TierDomain) -> bool {
    counts_for_tier_in_context(word, domain, false)
}

/// Return whether a word participates in alignment for the target domain,
/// given whether it appears inside retraced content.
///
/// The `in_retrace` parameter remains for call-site compatibility, but `%wor`
/// membership no longer depends on retrace ancestry. `%wor` now models spoken
/// main-tier word slots directly, regardless of whether those words appear
/// inside or outside a retrace container.
pub fn counts_for_tier_in_context(word: &Word, domain: TierDomain, _in_retrace: bool) -> bool {
    // Empty words (from parser artifacts) should never align
    if word.cleaned_text().is_empty() {
        return false;
    }

    if word
        .category
        .as_ref()
        .is_some_and(WordCategory::is_omission)
    {
        return false;
    }

    match domain {
        // %mor = linguistic/morphological content (excludes ALL fragments, untranscribed)
        TierDomain::Mor => is_linguistic_content(word),

        // %wor = word-level timing over spoken main-tier word slots.
        //
        // %wor is a timing-annotation tier: it records word-level start/end
        // bullets for tokens the FA engine can meaningfully anchor to audio.
        //
        // Excluded:
        // - Untranscribed (`xxx`, `yyy`, `www`): no known phoneme sequence;
        //   CTC alignment cannot produce timings for unknown material.
        // - Phonological fragments (`&+`, WordCategory::PhonologicalFragment):
        //   incomplete phoneme sequences.
        // - Nonwords (`&~`, WordCategory::Nonword): interactional/gestural
        //   sounds without stable phonemic content.
        //
        // Included:
        // - Regular words and fillers (`&-`, WordCategory::Filler): stable,
        //   alignable phoneme sequences.
        //
        // This matches batchalign2: BA2 classified `&+` and `&~` as
        // `TokenType.ANNOT` (excluded), while `&-` was `TokenType.FP`
        // (included). See batchalign2/formats/chat/lexer.py `__handle`.
        TierDomain::Wor => {
            !is_wor_timing_token(word)
                && word.untranscribed().is_none()
                && !is_wor_excluded_category(word)
        }

        // %pho and %sin include everything that was phonologically/gesturally produced
        // This includes fragments, untranscribed material, etc.
        TierDomain::Pho | TierDomain::Sin => true,
    }
}

/// Return whether a word category is excluded from `%wor`.
///
/// Excluded: phonological fragments (`&+`) and nonwords (`&~`).
/// NOT excluded: fillers (`&-`), which have stable, alignable phoneme sequences.
///
/// This matches batchalign2: both `&+` and `&~` were `TokenType.ANNOT` (excluded
/// from `phonated_words`), while `&-` was `TokenType.FP` (included).
/// See `batchalign2/formats/chat/lexer.py` `__handle`.
fn is_wor_excluded_category(word: &Word) -> bool {
    matches!(
        word.category,
        Some(WordCategory::Nonword | WordCategory::PhonologicalFragment)
    )
}

/// Return whether the word category is fragment-like for strict domains.
///
/// Fragment-like categories are filtered only in stricter domains like `%mor`.
fn is_fragment_like(word: &Word) -> bool {
    matches!(
        word.category,
        Some(WordCategory::Nonword | WordCategory::Filler | WordCategory::PhonologicalFragment)
    )
}

/// Return whether a word contributes linguistic content for `%mor` alignment.
///
/// Excludes:
/// - All fragments: &-markers, nonwords, fillers, phonological fragments
/// - Untranscribed material: xxx, yyy, www
fn is_linguistic_content(word: &Word) -> bool {
    !is_fragment_like(word) && word.untranscribed().is_none()
}

/// Return whether a word token is `%wor` timing metadata (`start_end` digits).
///
/// These tokens are alignment metadata rather than lexical items and therefore
/// must be excluded from lexical alignment counts.
fn is_wor_timing_token(word: &Word) -> bool {
    // %wor tiers interleave lexical tokens with timing markers like `100_200`.
    // Those markers are alignment metadata, not alignable lexical content.
    let raw = word.raw_text.as_bytes();
    let Some(split_at) = raw.iter().position(|&byte| byte == b'_') else {
        return false;
    };
    if split_at == 0 || split_at + 1 >= raw.len() || raw[split_at + 1..].contains(&b'_') {
        return false;
    }

    raw[..split_at].iter().all(|byte| byte.is_ascii_digit())
        && raw[split_at + 1..].iter().all(|byte| byte.is_ascii_digit())
}

/// Return whether a replaced word should align in `%pho`/`%sin` domains.
///
/// Omissions never align. Fragment-like words are excluded when a replacement exists.
pub fn should_align_replaced_word_in_pho_sin(word: &Word, has_replacement: bool) -> bool {
    if word
        .category
        .as_ref()
        .is_some_and(WordCategory::is_omission)
    {
        return false;
    }

    if has_replacement && is_fragment_like(word) {
        return false;
    }

    true
}

/// Return whether the separator contributes a `%mor` tag-marker item.
///
/// These separators map to explicit `%mor` symbols and therefore count as
/// alignable units in morphological alignment.
pub fn is_tag_marker_separator(sep: &Separator) -> bool {
    // Tag markers that have corresponding %mor items:
    // - Tag („) -> end|end
    // - Vocative (‡) -> beg|beg
    // - Comma (,) -> cm|cm (used as tag marker in some corpora)
    matches!(
        sep,
        Separator::Tag { .. } | Separator::Vocative { .. } | Separator::Comma { .. }
    )
}
