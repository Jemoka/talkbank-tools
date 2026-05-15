use super::{
    AsrNormalizedText, AsrOutput, AsrWord, ENDING_PUNCT, MOR_PUNCT, RTL_PUNCT, SpeakerIndex,
    Utterance, cleanup, expand_numbers_in_words, finalize_words_to_chunks,
    prepare_words_pre_expansion,
};

/// Run the full ASR post-processing pipeline on raw ASR output.
///
/// Applies compound merging, timing conversion, multi-word splitting,
/// number expansion, long turn splitting, punctuation-based retokenization,
/// disfluency replacement, and n-gram retrace detection. Returns
/// speaker-attributed utterances ready for CHAT assembly via `build_chat()`.
pub fn process_raw_asr(output: &AsrOutput, lang: &str) -> Vec<Utterance> {
    let mut all_utterances = utterances_from_prepared_chunks(prepare_asr_chunks(output, lang));
    finalize_utterances(&mut all_utterances, lang);
    all_utterances
}

/// Normalize raw ASR monologues into pre-CHAT chunks while preserving speaker
/// boundaries.
///
/// This is the monolithic pipeline that applies all stages in sequence.
/// For pipelines that need to intercept the number expansion step (e.g. to
/// route through Python IPC), use [`super::prepare_words_pre_expansion`] and
/// [`super::finalize_words_to_chunks`] separately.
pub fn prepare_asr_chunks(output: &AsrOutput, lang: &str) -> Vec<super::PreparedMonologueChunk> {
    let mut prepared = Vec::new();

    for monologue in &output.monologues {
        let words = prepare_words_pre_expansion(&monologue.elements, lang);
        // Stage 4: number expansion (Rust fallback tables + CJK + currency)
        let words = expand_numbers_in_words(words, lang);
        prepared.extend(finalize_words_to_chunks(words, monologue.speaker, lang));
    }

    prepared
}

/// Retokenize prepared chunks into utterances using punctuation boundaries.
pub fn utterances_from_prepared_chunks(
    chunks: Vec<super::PreparedMonologueChunk>,
) -> Vec<Utterance> {
    let mut utterances = Vec::new();
    for chunk in chunks {
        utterances.extend(retokenize(chunk.speaker, chunk.words));
    }
    utterances
}

/// Apply the post-retokenization cleanup passes shared by all ASR paths.
pub fn finalize_utterances(utterances: &mut [Utterance], lang: &str) {
    // Matches BA2's DisfluencyReplacementEngine which ran after ASR on all utterances.
    cleanup::apply_disfluency_replacements(utterances, lang);

    // Matches BA2's NgramRetraceEngine which ran after disfluency on all utterances.
    cleanup::apply_retrace_detection(utterances, lang);

    // 2026-04-23 transcribe-pipeline correction: utterance-initial
    // cap. The two per-word rules (I-cap, title-period strip) have
    // already run pre-retokenize in `finalize_words_to_chunks`;
    // this post-retokenize hook handles the per-utterance rule that
    // needs to see utterance boundaries. English-gated.
    cleanup::apply_english_transcribe_rules_post_retokenize(utterances, lang);
}

/// Check if a word is or ends with a sentence-ending punctuation mark.
fn is_ending_punct(word: &str) -> bool {
    if ENDING_PUNCT.contains(&word) {
        return true;
    }
    // Check RTL punctuation
    for (rtl, _) in RTL_PUNCT {
        if word == *rtl {
            return true;
        }
    }
    false
}

/// Check if a word ends with ending punctuation (last char).
fn ends_with_ending_punct(word: &str) -> bool {
    match word.chars().last() {
        Some(c) => {
            let mut buf = [0u8; 4];
            is_ending_punct(c.encode_utf8(&mut buf))
        }
        None => false,
    }
}

/// Normalize RTL punctuation to ASCII equivalent.
fn normalize_punct(word: &str) -> String {
    for (rtl, ascii) in RTL_PUNCT {
        if word == *rtl {
            return ascii.to_string();
        }
    }
    word.to_string()
}

/// Split a word list into utterances based on punctuation boundaries.
///
/// This is the simple punctuation-based retokenizer (no BERT model).
///
/// # Panic safety
///
/// The `unwrap()` calls on `buf.last()`, `buf.last_mut()`, and `buf.pop()` are
/// all immediately preceded by `buf.push(word)` within the same loop iteration,
/// so `buf` is guaranteed non-empty at each call site.
#[allow(clippy::unwrap_used)]
pub(super) fn retokenize(speaker: SpeakerIndex, words: Vec<AsrWord>) -> Vec<Utterance> {
    let mut utterances = Vec::new();
    let mut buf: Vec<AsrWord> = Vec::new();

    for word in words {
        // Normalize Japanese period and remove inverted punctuation
        let word = AsrWord {
            text: word
                .text
                .map(|t| t.replace('。', ".").replace(['¿', '¡'], " ")),
            ..word
        };

        buf.push(word);

        // `buf.push(word)` immediately above guarantees `buf` is
        // non-empty at this point.
        #[allow(clippy::unwrap_used)]
        let last_text = buf.last().unwrap().text.as_str();

        if is_ending_punct(last_text) {
            // Whole word is ending punct — flush utterance
            let punct = normalize_punct(last_text);
            // Same `buf.push` guarantee above.
            #[allow(clippy::unwrap_used)]
            {
                buf.last_mut().unwrap().text = AsrNormalizedText::new(punct);
            }
            utterances.push(Utterance {
                speaker,
                words: std::mem::take(&mut buf),
                lang: None,
            });
        } else if ends_with_ending_punct(last_text) {
            // Last character is ending punct — split the word.
            // `buf.push` above guarantees pop() returns Some.
            #[allow(clippy::unwrap_used)]
            let text = buf.pop().unwrap();
            let s = text.text.as_str();
            let last_char_boundary = s.char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            let word_part = &s[..last_char_boundary];
            let punct_part = &s[last_char_boundary..];

            if !word_part.is_empty() {
                buf.push(AsrWord::new(word_part, text.start_ms, text.end_ms));
            }
            buf.push(AsrWord::new(normalize_punct(punct_part), None, None));
            utterances.push(Utterance {
                speaker,
                words: std::mem::take(&mut buf),
                lang: None,
            });
        }
    }

    // Flush remaining words
    if !buf.is_empty() {
        // Remove trailing MOR_PUNCT
        while buf
            .last()
            .is_some_and(|w| MOR_PUNCT.contains(&w.text.as_str()))
        {
            buf.pop();
        }
        if !buf.is_empty() {
            // Append terminator
            buf.push(AsrWord::new(".", None, None));
            utterances.push(Utterance {
                speaker,
                words: buf,
                lang: None,
            });
        }
    }

    utterances
}
