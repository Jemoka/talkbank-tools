//! Word extraction for UTR / FA — collects alignable word texts from CHAT
//! utterance content, with compound-filler splitting.
//!
//! Extracted from tbtbt's `chat_ops/fa/extraction.rs` + `fa/mod.rs::split_compound_filler`.
//! Kept local to the UTR strategy module for now; lift into `talkbank-model`
//! if FA grows the same need.

use talkbank_model::alignment::helpers::{TierDomain, WordItem, counts_for_tier, walk_words};
use talkbank_model::model::{UtteranceContent, Word, WordCategory};

/// Collect alignable word texts from utterance content for forced alignment /
/// UTR. Uses the `Wor` alignment domain.
pub fn collect_fa_words(content: &[UtteranceContent], out: &mut Vec<String>) {
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

fn push_fa_word(word: &Word, out: &mut Vec<String>) {
    for part in split_compound_filler(word) {
        out.push(part);
    }
}

/// Split a compound filler's cleaned text at underscores (e.g. `&-you_know`
/// → `["you", "know"]`), or return the cleaned text as a single element.
/// Both extraction and injection must agree on the split count.
pub fn split_compound_filler(word: &Word) -> Vec<String> {
    let text = word.cleaned_text();
    if word.category == Some(WordCategory::Filler) && text.contains('_') {
        text.split('_')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![text.to_string()]
    }
}
