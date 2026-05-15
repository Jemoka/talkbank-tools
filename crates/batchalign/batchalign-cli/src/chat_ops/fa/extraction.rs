//! Word extraction for forced alignment (Wor domain).

use talkbank_model::alignment::helpers::{TierDomain, WordItem, counts_for_tier, walk_words};
use talkbank_model::model::{UtteranceContent, Word};

/// Collect alignable word texts from utterance content for forced alignment.
///
/// Uses the `Wor` alignment domain to decide which words are alignable.
/// Extracted texts are the cleaned (CHAT-marker-free) forms.
///
/// Compound fillers (`&-you_know`, `&-sort_of`) are split at underscores
/// into separate words for the FA engine, because ASR models (Whisper,
/// wav2vec) return them as multiple tokens that the DP aligner must
/// match individually.
///
/// * `content` - The top-level content items of an utterance.
/// * `out` - Accumulator that word texts are pushed into.
pub fn collect_fa_words(content: &[UtteranceContent], out: &mut Vec<String>) {
    // domain=None: recurse into all groups unconditionally (FA needs all words)
    walk_words(content, None, &mut |leaf| match leaf {
        WordItem::Word(word) => {
            if counts_for_tier(word, TierDomain::Wor) {
                push_fa_word(word, out);
            }
        }
        WordItem::ReplacedWord(replaced) => {
            if counts_for_tier(&replaced.word, TierDomain::Wor) {
                push_fa_word(&replaced.word, out);
            }
        }
        WordItem::Separator(_) => {}
    });
}

/// Push a word's cleaned text to the FA word list, splitting compound fillers
/// at underscores so each part aligns separately against ASR output.
fn push_fa_word(word: &Word, out: &mut Vec<String>) {
    for part in super::split_compound_filler(word) {
        out.push(part);
    }
}
