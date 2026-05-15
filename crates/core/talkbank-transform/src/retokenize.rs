//! Shared retokenization helpers for reconciling CHAT words with NLP tokenization.
//!
//! This module owns the pure CHAT-side retokenization path: deterministic
//! word-to-token mapping, token parsing helpers, AST rebuild, and final
//! morphosyntax injection. Batchalign keeps only the Stanza-facing orchestration
//! that decides when and with what payloads to invoke this transform.

use smallvec::SmallVec;
use talkbank_model::model::{GrammaticalRelation, Mor, ParseHealthTier, Utterance};

use crate::extract::ExtractedWord;
use crate::inject::MisalignmentDiagnostic;

mod parse_helpers;
mod rebuild;
pub use parse_helpers::{
    handle_ending_punct_skip, is_ending_punct, is_tag_marker_text, resolve_token_text,
    try_parse_token_as_bracketed_item, try_parse_token_as_utterance_content,
    try_parse_token_as_word,
};
use rebuild::{RetokenizeContext, rebuild_content};

/// Maps each CHAT word (by index) to one or more NLP token indices.
///
/// Invariant: `word_count() == original_word_count` passed at construction.
/// Each entry is non-empty for successfully mapped words, though fallback
/// strategies may leave gaps when the two tokenizations diverge too much.
///
/// # Arena-style optimizations
///
/// Uses a dense `Vec<SmallVec<[usize; 4]>>` indexed by word index instead of
/// `HashMap<usize, Vec<usize>>`, eliminating hashing overhead and providing
/// O(1) lookup. Most words map to 1-2 tokens, so `SmallVec<[usize; 4]>`
/// keeps them inline without heap allocation.
#[derive(Debug, Clone)]
pub struct WordTokenMapping {
    inner: Vec<SmallVec<[usize; 4]>>,
}

impl WordTokenMapping {
    /// Number of original words this mapping covers.
    pub fn word_count(&self) -> usize {
        self.inner.len()
    }

    /// Token indices for the given original word index.
    ///
    /// Returns an empty slice if the word has no mapping.
    pub fn tokens_for_word(&self, word_idx: usize) -> &[usize] {
        self.inner.get(word_idx).map_or(&[], |s| s.as_slice())
    }

    /// Get a non-empty mapping for the given word, or `None`.
    pub fn get_nonempty(&self, word_idx: usize) -> Option<&[usize]> {
        self.inner
            .get(word_idx)
            .filter(|v| !v.is_empty())
            .map(|s| s.as_slice())
    }
}

/// Build a mapping from original word index to NLP token indices.
///
/// First tries deterministic span-join mapping when normalized concatenated text
/// is identical on both sides. When text diverges, uses a conservative
/// length-aware monotonic fallback (no DP).
pub fn build_word_token_mapping(
    original_words: &[ExtractedWord],
    stanza_tokens: &[String],
) -> WordTokenMapping {
    if let Some(mapping) = try_deterministic_word_token_mapping(original_words, stanza_tokens) {
        return WordTokenMapping { inner: mapping };
    }

    tracing::warn!(
        original_word_count = original_words.len(),
        stanza_token_count = stanza_tokens.len(),
        "retokenize text diverged; using length-aware monotonic fallback without DP"
    );

    WordTokenMapping {
        inner: build_length_fallback_mapping(original_words.len(), stanza_tokens.len()),
    }
}

/// Normalize a text unit for alignment comparison.
pub fn normalize_alignment_unit(text: &str) -> String {
    text.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

/// Try to build a deterministic span-join mapping.
///
/// Returns `None` if normalized text does not match and fallback should be used.
pub fn try_deterministic_word_token_mapping(
    original_words: &[ExtractedWord],
    stanza_tokens: &[String],
) -> Option<Vec<SmallVec<[usize; 4]>>> {
    if original_words.is_empty() || stanza_tokens.is_empty() {
        return Some(vec![SmallVec::new(); original_words.len()]);
    }

    let mut original_ranges: Vec<(usize, usize)> = Vec::with_capacity(original_words.len());
    let mut token_ranges: Vec<(usize, usize)> = Vec::with_capacity(stanza_tokens.len());
    let mut original_concat = String::new();
    let mut token_concat = String::new();

    let mut cursor = 0usize;
    for word in original_words {
        let normalized = normalize_alignment_unit(word.text.as_str());
        if normalized.is_empty() {
            return None;
        }
        let len = normalized.chars().count();
        original_ranges.push((cursor, cursor + len));
        cursor += len;
        original_concat.push_str(&normalized);
    }

    cursor = 0;
    for token in stanza_tokens {
        let normalized = normalize_alignment_unit(token);
        if normalized.is_empty() {
            return None;
        }
        let len = normalized.chars().count();
        token_ranges.push((cursor, cursor + len));
        cursor += len;
        token_concat.push_str(&normalized);
    }

    if original_concat != token_concat {
        return None;
    }

    let mut mapping: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); original_words.len()];
    let mut token_idx = 0usize;

    for (word_idx, &(word_start, word_end)) in original_ranges.iter().enumerate() {
        while token_idx < token_ranges.len() && token_ranges[token_idx].1 <= word_start {
            token_idx += 1;
        }

        let mut cursor_idx = token_idx;
        while cursor_idx < token_ranges.len() {
            let (token_start, token_end) = token_ranges[cursor_idx];
            if token_start >= word_end {
                break;
            }
            if token_end > word_start {
                mapping[word_idx].push(cursor_idx);
            }
            cursor_idx += 1;
        }

        if mapping[word_idx].is_empty() {
            return None;
        }
    }

    Some(mapping)
}

fn build_length_fallback_mapping(
    original_word_count: usize,
    stanza_token_count: usize,
) -> Vec<SmallVec<[usize; 4]>> {
    let mut mapping: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); original_word_count];

    if original_word_count == 0 || stanza_token_count == 0 {
        return mapping;
    }

    if original_word_count == stanza_token_count {
        for (idx, slot) in mapping.iter_mut().enumerate() {
            slot.push(idx);
        }
        return mapping;
    }

    for (word_idx, slot) in mapping.iter_mut().enumerate() {
        let start = word_idx * stanza_token_count / original_word_count;
        let mut end = (word_idx + 1) * stanza_token_count / original_word_count;
        if end <= start {
            end = (start + 1).min(stanza_token_count);
        }
        for token_idx in start..end {
            slot.push(token_idx);
        }
    }

    mapping
}

/// Retokenize an utterance to match NLP tokenization, then inject morphosyntax.
pub fn retokenize_utterance(
    parser: &talkbank_parser::TreeSitterParser,
    utterance: &mut Utterance,
    original_words: &[ExtractedWord],
    stanza_tokens: &[String],
    mors: Vec<Mor>,
    terminator: talkbank_model::Terminator,
    gra_relations: Vec<GrammaticalRelation>,
) -> Result<(), MisalignmentDiagnostic> {
    if original_words.is_empty() || stanza_tokens.is_empty() {
        return Ok(());
    }
    // Render typed terminator to its CHAT surface form for retokenization
    // diagnostics that compare against the source bytes.
    let terminator_surface = terminator.to_string();
    let expected_terminator = Some(terminator_surface.as_str());

    let mapping = build_word_token_mapping(original_words, stanza_tokens);

    let mut ctx = RetokenizeContext {
        parser,
        mapping: &mapping,
        stanza_tokens,
        original_words,
        mors: &mors,
        expected_terminator,
        word_counter: 0,
        mor_cursor: 0,
        diagnostics: Vec::new(),
        emitted_tokens: std::collections::HashSet::new(),
    };

    let old_content = std::mem::take(&mut utterance.main.content.content.0);
    let mut new_content = Vec::with_capacity(old_content.len());

    rebuild_content(old_content, &mut ctx, &mut new_content);
    utterance.main.content.content.0 = new_content;

    if !ctx.diagnostics.is_empty() {
        utterance.mark_parse_taint(ParseHealthTier::Main);
        for warning in &ctx.diagnostics {
            tracing::warn!("retokenize: {warning}");
        }
    }

    tracing::debug!(
        mor_count = mors.len(),
        gra_count = gra_relations.len(),
        word_counter = ctx.word_counter,
        mor_cursor = ctx.mor_cursor,
        "retokenize_utterance: about to inject"
    );
    crate::inject::inject_morphosyntax(utterance, mors, terminator, gra_relations)
}

#[cfg(test)]
mod tests {
    use talkbank_model::{ChatCleanedText, ChatRawText, WordIdx};

    use super::*;

    fn extracted_words(words: &[&str]) -> Vec<ExtractedWord> {
        // Test fixture path via the explicit `test_unchecked` escape
        // hatch on each text type. The `test_` prefix and `#[doc(hidden)]`
        // flag them as fixture-only.
        words
            .iter()
            .enumerate()
            .map(|(idx, word)| ExtractedWord {
                text: ChatCleanedText::test_unchecked(*word),
                raw_text: ChatRawText::test_unchecked(*word),
                utterance_word_index: WordIdx(idx),
                form_type: None,
                lang: None,
            })
            .collect()
    }

    #[test]
    fn deterministic_mapping_succeeds_for_exact_match() {
        let original_words = extracted_words(&["i", "eat"]);
        let stanza_tokens = vec!["i".to_string(), "eat".to_string()];

        let mapping = try_deterministic_word_token_mapping(&original_words, &stanza_tokens)
            .expect("deterministic mapping should succeed");

        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping[0].as_slice(), &[0]);
        assert_eq!(mapping[1].as_slice(), &[1]);
    }

    #[test]
    fn deterministic_mapping_succeeds_for_split_and_merge() {
        let original_words = extracted_words(&["gon", "na", "eat"]);
        let stanza_tokens = vec!["gonna".to_string(), "eat".to_string()];

        let mapping = try_deterministic_word_token_mapping(&original_words, &stanza_tokens)
            .expect("deterministic mapping should succeed");

        assert_eq!(mapping.len(), 3);
        assert_eq!(mapping[0].as_slice(), &[0]);
        assert_eq!(mapping[1].as_slice(), &[0]);
        assert_eq!(mapping[2].as_slice(), &[1]);
    }

    #[test]
    fn deterministic_mapping_rejects_diverged_text() {
        let original_words = extracted_words(&["i", "eat"]);
        let stanza_tokens = vec!["you".to_string(), "eat".to_string()];

        assert!(try_deterministic_word_token_mapping(&original_words, &stanza_tokens).is_none());
    }

    #[test]
    fn fallback_mapping_uses_positions_for_equal_lengths() {
        let original_words = extracted_words(&["gonna", "eat"]);
        let stanza_tokens = vec!["going".to_string(), "eat".to_string()];

        let mapping = build_word_token_mapping(&original_words, &stanza_tokens);

        assert_eq!(mapping.word_count(), 2);
        assert_eq!(mapping.tokens_for_word(0), &[0]);
        assert_eq!(mapping.tokens_for_word(1), &[1]);
    }

    #[test]
    fn fallback_mapping_uses_monotonic_bins_for_length_mismatch() {
        let original_words = extracted_words(&["i", "don't", "know"]);
        let stanza_tokens = vec!["alpha".to_string(), "beta".to_string()];

        let mapping = build_word_token_mapping(&original_words, &stanza_tokens);

        assert_eq!(mapping.word_count(), 3);
        assert_eq!(mapping.tokens_for_word(0), &[0]);
        assert_eq!(mapping.tokens_for_word(1), &[0]);
        assert_eq!(mapping.tokens_for_word(2), &[1]);
    }
}
