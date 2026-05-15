use super::{
    AsrPipelineSnapshot, AsrWord, LONG_PAUSE_SENTENCE_STARTERS, LONG_PAUSE_SPLIT_MS, MAX_TURN_LEN,
    PreparedMonologueChunk, SpeakerIndex, cleanup,
};

/// Stages 4b-5b: Cantonese normalization, long turn splitting, and
/// pause-based splitting.
///
/// Takes words that have already been through number expansion and
/// produces speaker-attributed chunks ready for retokenization.
pub fn finalize_words_to_chunks(
    words: Vec<AsrWord>,
    speaker: SpeakerIndex,
    lang: &str,
) -> Vec<PreparedMonologueChunk> {
    finalize_words_to_chunks_with_snapshot(words, speaker, lang, None)
}

/// Snapshot-aware variant of [`finalize_words_to_chunks`].
///
/// Populates `after_cantonese_norm` (only for `yue`) and
/// `after_long_turn_split` when `snapshot` is `Some`. `None` is the
/// zero-overhead default.
pub fn finalize_words_to_chunks_with_snapshot(
    words: Vec<AsrWord>,
    speaker: SpeakerIndex,
    lang: &str,
    mut snapshot: Option<&mut AsrPipelineSnapshot>,
) -> Vec<PreparedMonologueChunk> {
    // Stage 4b: Cantonese normalization (simplified→HK traditional + domain replacements)
    // CANTONESE-SPECIFIC BOUNDARY: Only applied when lang == "yue".
    let mut words = if lang == "yue" {
        let normalized = normalize_cantonese_words(words);
        if let Some(ref mut s) = snapshot {
            s.after_cantonese_norm = Some(normalized.clone());
        }
        normalized
    } else {
        words
    };

    // 2026-04-23 English transcribe-pipeline corrections — the two
    // **per-word** rules (I-cap, title-period strip) must run
    // BEFORE stage 6 retokenize, because retokenize splits on
    // trailing `.` and would slice `Dr.` in half if the period
    // weren't stripped first. English-gated; no-op for other
    // languages.
    cleanup::apply_english_transcribe_rules_pre_retokenize(&mut words, lang);

    // Stage 5: long turn splitting
    let chunks = split_long_turns(words);
    if let Some(ref mut s) = snapshot {
        s.after_long_turn_split = chunks.clone();
    }

    // Stage 5b: add timing-gap boundaries for long unpunctuated runs.
    let chunks = split_on_long_pauses(chunks);

    chunks
        .into_iter()
        .filter(|chunk| !chunk.is_empty())
        .map(|words| PreparedMonologueChunk { speaker, words })
        .collect()
}

/// Split one prepared chunk into smaller prepared chunks according to word-level
/// utterance assignments.
pub fn split_prepared_chunk_by_assignments(
    chunk: &PreparedMonologueChunk,
    assignments: &[usize],
) -> Vec<PreparedMonologueChunk> {
    if chunk.words.len() <= 1 || assignments.is_empty() || assignments.len() != chunk.words.len() {
        return vec![chunk.clone()];
    }

    let mut split_chunks = Vec::new();
    let mut current_group = assignments[0];
    let mut current_words = Vec::new();

    for (word, group) in chunk.words.iter().cloned().zip(assignments.iter().copied()) {
        if !current_words.is_empty() && group != current_group {
            split_chunks.push(PreparedMonologueChunk {
                speaker: chunk.speaker,
                words: std::mem::take(&mut current_words),
            });
            current_group = group;
        }
        current_words.push(word);
    }

    if !current_words.is_empty() {
        split_chunks.push(PreparedMonologueChunk {
            speaker: chunk.speaker,
            words: current_words,
        });
    }

    if split_chunks.is_empty() {
        vec![chunk.clone()]
    } else {
        split_chunks
    }
}

/// Normalize Cantonese text in all words (simplified→HK traditional + domain replacements).
///
/// CANTONESE-SPECIFIC FEATURE: This function is only called when `lang == "yue"` (line 31).
/// It applies domain-specific Cantonese normalization via `super::cantonese::normalize_cantonese`.
fn normalize_cantonese_words(words: Vec<AsrWord>) -> Vec<AsrWord> {
    words
        .into_iter()
        .map(|w| AsrWord {
            text: w.text.map(super::cantonese::normalize_cantonese),
            ..w
        })
        .collect()
}

/// Split a word list into chunks of at most [`MAX_TURN_LEN`].
pub(super) fn split_long_turns(words: Vec<AsrWord>) -> Vec<Vec<AsrWord>> {
    if words.len() <= MAX_TURN_LEN {
        return vec![words];
    }
    words.chunks(MAX_TURN_LEN).map(|c| c.to_vec()).collect()
}

pub(super) fn split_on_long_pauses(chunks: Vec<Vec<AsrWord>>) -> Vec<Vec<AsrWord>> {
    let mut result = Vec::new();

    for chunk in chunks {
        if chunk.len() <= 1 {
            result.push(chunk);
            continue;
        }

        let mut current = Vec::new();
        let mut previous_timed: Option<AsrWord> = None;

        for word in chunk {
            if previous_timed
                .as_ref()
                .is_some_and(|prev| long_pause_starts_new_utterance(prev, &word, current.len()))
                && !current.is_empty()
            {
                result.push(std::mem::take(&mut current));
            }

            if word.start_ms.is_some() && word.end_ms.is_some() {
                previous_timed = Some(word.clone());
            }
            current.push(word);
        }

        if !current.is_empty() {
            result.push(current);
        }
    }

    result
}

fn long_pause_starts_new_utterance(prev: &AsrWord, next: &AsrWord, current_len: usize) -> bool {
    if current_len < 2 {
        return false;
    }

    let (Some(prev_end), Some(next_start)) = (prev.end_ms, next.start_ms) else {
        return false;
    };
    if next_start - prev_end < LONG_PAUSE_SPLIT_MS {
        return false;
    }

    let starter = next
        .text
        .as_str()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();
    LONG_PAUSE_SENTENCE_STARTERS.contains(&starter.as_str())
}
