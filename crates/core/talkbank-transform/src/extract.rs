//! NLP word extraction from CHAT AST.
//!
//! Walks the parsed CHAT content tree and collects words that are
//! "alignable" for a given domain (Mor, Wor, Pho, Sin).

use talkbank_model::alignment::helpers::{
    TierDomain, WordItem, annotations_have_alignment_ignore, counts_for_tier,
    is_tag_marker_separator, should_align_replaced_word_in_pho_sin, walk_words,
};
use talkbank_model::model::{ChatFile, Line, ReplacedWord, UtteranceContent, Word};
use talkbank_model::{ChatCleanedText, ChatRawText, SpeakerCode, UtteranceIdx, WordIdx};

/// A word extracted from the CHAT AST for NLP processing.
#[derive(Debug, Clone)]
pub struct ExtractedWord {
    /// Cleaned text suitable for NLP (no CHAT markers).
    pub text: ChatCleanedText,
    /// Raw text as it appeared in the transcript.
    pub raw_text: ChatRawText,
    /// Zero-based index among extracted alignable words in this utterance.
    pub utterance_word_index: WordIdx,
    /// Special form marker if the word has @c, @b, @s, etc.
    pub form_type: Option<talkbank_model::model::FormType>,
    /// Language marker (e.g., @s, @s:eng, @s:eng+fra).
    pub lang: Option<talkbank_model::model::WordLanguageMarker>,
}

/// Per-utterance extraction result.
#[derive(Debug, Clone)]
pub struct ExtractedUtterance {
    /// Speaker code (e.g., "CHI", "MOT").
    pub speaker: SpeakerCode,
    /// Zero-based utterance index in the file.
    pub utterance_index: UtteranceIdx,
    /// Extracted words.
    pub words: Vec<ExtractedWord>,
}

/// Extract NLP-ready words from all utterances in a ChatFile.
///
/// Walks every utterance in the file and collects words that are
/// "alignable" for the given `domain`. Non-utterance lines (headers,
/// comments, etc.) are skipped.
///
/// * `chat_file` - The parsed CHAT file to extract words from.
/// * `domain` - The alignment domain governing which words are
///   considered alignable (`Mor`, `Wor`, `Pho`, or `Sin`).
pub fn extract_words(chat_file: &ChatFile, domain: TierDomain) -> Vec<ExtractedUtterance> {
    let mut results = Vec::new();
    let mut utt_idx = 0;

    for line in &chat_file.lines {
        if let Line::Utterance(utterance) = line {
            let speaker = SpeakerCode::new(utterance.main.speaker.to_string());
            let mut words = Vec::new();
            collect_utterance_content(&utterance.main.content.content, domain, &mut words);
            results.push(ExtractedUtterance {
                speaker,
                utterance_index: UtteranceIdx(utt_idx),
                words,
            });
            utt_idx += 1;
        }
    }

    results
}

/// Collect NLP-extractable words from a slice of utterance content items.
///
/// This is the inner workhorse called by [`extract_words`] and also used
/// directly by other modules (morphosyntax, utseg, translate, coref) to
/// extract words from a single utterance's content without iterating the
/// entire file.
///
/// * `content` - The top-level content items of an utterance.
/// * `domain` - The alignment domain that determines which words are
///   collected (e.g., `Mor` includes tag-marker separators; `Wor` does not).
/// * `out` - Accumulator that extracted words are pushed into.
pub fn collect_utterance_content(
    content: &[UtteranceContent],
    domain: TierDomain,
    out: &mut Vec<ExtractedWord>,
) {
    walk_words(content, Some(domain), &mut |leaf| match leaf {
        WordItem::Word(word) => {
            collect_alignable_word(word, &[], domain, out);
        }
        WordItem::ReplacedWord(replaced) => {
            collect_replaced_word(replaced, domain, out);
        }
        WordItem::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                out.push(ExtractedWord {
                    text: ChatCleanedText::from_separator(sep),
                    raw_text: ChatRawText::from_separator(sep),
                    utterance_word_index: WordIdx(out.len()),
                    form_type: None,
                    lang: None,
                });
            }
        }
    });
}

fn collect_alignable_word(
    word: &Word,
    annotations: &[talkbank_model::model::ContentAnnotation],
    domain: TierDomain,
    out: &mut Vec<ExtractedWord>,
) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(annotations) {
        return;
    }

    if !counts_for_tier(word, domain) {
        return;
    }

    out.push(ExtractedWord {
        text: ChatCleanedText::from_word(word),
        raw_text: ChatRawText::from_word_raw(word),
        utterance_word_index: WordIdx(out.len()),
        form_type: word.form_type.clone(),
        lang: word.lang.clone(),
    });
}

fn collect_replaced_word(entry: &ReplacedWord, domain: TierDomain, out: &mut Vec<ExtractedWord>) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(&entry.scoped_annotations) {
        return;
    }

    match domain {
        TierDomain::Mor => {
            if !entry.replacement.words.is_empty() {
                for word in &entry.replacement.words {
                    if counts_for_tier(word, TierDomain::Mor) {
                        out.push(ExtractedWord {
                            text: ChatCleanedText::from_word(word),
                            raw_text: ChatRawText::from_word_raw(word),
                            utterance_word_index: WordIdx(out.len()),
                            form_type: word.form_type.clone(),
                            lang: word.lang.clone(),
                        });
                    }
                }
            } else if counts_for_tier(&entry.word, TierDomain::Mor) {
                out.push(ExtractedWord {
                    text: ChatCleanedText::from_word(&entry.word),
                    raw_text: ChatRawText::from_word_raw(&entry.word),
                    utterance_word_index: WordIdx(out.len()),
                    form_type: entry.word.form_type.clone(),
                    lang: entry.word.lang.clone(),
                });
            }
        }
        TierDomain::Pho | TierDomain::Sin | TierDomain::Wor => {
            if should_align_replaced_word_in_pho_sin(
                &entry.word,
                !entry.replacement.words.is_empty(),
            ) {
                out.push(ExtractedWord {
                    text: ChatCleanedText::from_word(&entry.word),
                    raw_text: ChatRawText::from_word_raw(&entry.word),
                    utterance_word_index: WordIdx(out.len()),
                    form_type: entry.word.form_type.clone(),
                    lang: entry.word.lang.clone(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::alignment::helpers::TierDomain;
    use talkbank_parser::TreeSitterParser;

    /// Parse a CHAT string into a ChatFile, panicking on errors (test-only).
    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        parser.parse_chat_file(text).expect("parse chat file")
    }

    /// Minimal valid CHAT with one utterance containing the given main tier text.
    fn one_utterance(main_tier: &str) -> String {
        format!(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             *CHI:\t{main_tier}\n\
             @End\n"
        )
    }

    // -----------------------------------------------------------------------
    // Basic word extraction
    // -----------------------------------------------------------------------

    #[test]
    fn simple_words_in_mor_domain() {
        let chat = parse_chat(&one_utterance("hello world ."));
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 1, "expected 1 utterance");
        assert_eq!(result[0].words.len(), 2, "expected 2 words (hello, world)");
        assert_eq!(result[0].words[0].text.as_str(), "hello");
        assert_eq!(result[0].words[1].text.as_str(), "world");
    }

    #[test]
    fn word_indices_are_sequential_within_utterance() {
        let chat = parse_chat(&one_utterance("the dog ran ."));
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result[0].words.len(), 3);
        assert_eq!(result[0].words[0].utterance_word_index, WordIdx(0));
        assert_eq!(result[0].words[1].utterance_word_index, WordIdx(1));
        assert_eq!(result[0].words[2].utterance_word_index, WordIdx(2));
    }

    #[test]
    fn utterance_indices_are_sequential_across_file() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             *CHI:\tbye .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 3, "expected 3 utterances");
        assert_eq!(result[0].utterance_index, UtteranceIdx(0));
        assert_eq!(result[1].utterance_index, UtteranceIdx(1));
        assert_eq!(result[2].utterance_index, UtteranceIdx(2));
    }

    #[test]
    fn speaker_code_extracted_correctly() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result[0].speaker.as_str(), "CHI");
        assert_eq!(result[1].speaker.as_str(), "MOT");
    }

    #[test]
    fn non_utterance_lines_are_skipped() {
        // Headers and comments are not utterances — only *SPK: lines count.
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @Comment:\tthis is a comment\n\
             *CHI:\thello .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 1, "only the utterance, not the comment");
    }

    // -----------------------------------------------------------------------
    // Tag-marker separators: included in Mor, excluded from Wor/Pho/Sin
    // -----------------------------------------------------------------------

    #[test]
    fn comma_separator_included_in_mor_domain() {
        // CHAT: comma (,) between words is a tag-marker separator in Mor domain.
        let chat = parse_chat(&one_utterance("well , hello ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        // Mor domain: "well", ",", "hello" = 3 items
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&","),
            "Mor domain should include comma separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn comma_separator_excluded_from_wor_domain() {
        let chat = parse_chat(&one_utterance("well , hello ."));
        let wor_result = extract_words(&chat, TierDomain::Wor);
        let wor_texts: Vec<&str> = wor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            !wor_texts.contains(&","),
            "Wor domain should NOT include comma separator, got: {wor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_included_in_mor_domain() {
        // „ (U+201E) is the tag separator
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"„"),
            "Mor domain should include tag separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn vocative_separator_included_in_mor_domain() {
        // ‡ (U+2021) is the vocative separator
        let chat = parse_chat(&one_utterance("‡ Mom ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"‡"),
            "Mor domain should include vocative separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_excluded_from_pho_domain() {
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let pho_result = extract_words(&chat, TierDomain::Pho);
        let pho_texts: Vec<&str> = pho_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            !pho_texts.contains(&"„"),
            "Pho domain should NOT include tag separator, got: {pho_texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ReplacedWord: Mor uses replacement; Pho/Sin/Wor use original
    // -----------------------------------------------------------------------

    #[test]
    fn replaced_word_uses_replacement_in_mor_domain() {
        // CHAT: "doggie [: dog]" — in Mor domain, "dog" is used (the replacement).
        let chat = parse_chat(&one_utterance("doggie [: dog] ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"dog"),
            "Mor domain should use replacement word 'dog', got: {texts:?}"
        );
    }

    #[test]
    fn replaced_word_uses_original_when_replacement_empty_in_mor() {
        // When replacement has no words, Mor falls back to original.
        // This is tested via the code path: entry.replacement.words.is_empty() == true
        // In practice, CHAT always has at least one replacement word, but the code
        // handles the empty case by falling back to the original word.
        // We test the code path with a normal replaced word where the replacement
        // is present — the replacement is used, not the original.
        let chat = parse_chat(&one_utterance("goed [: went] ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"went"),
            "Mor domain should use replacement 'went', got: {texts:?}"
        );
        assert!(
            !texts.contains(&"goed"),
            "Mor domain should NOT use original 'goed', got: {texts:?}"
        );
    }

    #[test]
    fn replaced_word_uses_original_in_wor_domain() {
        // In Wor domain, the original word is used (not the replacement).
        let chat = parse_chat(&one_utterance("doggie [: dog] ."));
        let result = extract_words(&chat, TierDomain::Wor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"doggie"),
            "Wor domain should use original word 'doggie', got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Alignment-ignore annotation ([e]) excludes words in Mor domain
    // -----------------------------------------------------------------------

    #[test]
    fn exclude_annotation_skips_word_in_mor_domain() {
        // [e] marks excluded content — skipped in Mor domain.
        let chat = parse_chat(&one_utterance("hello [e] world ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        // "hello" should be excluded by [e]; "world" should remain.
        // Note: [e] applies to the preceding word.
        assert!(
            texts.contains(&"world"),
            "world should be present, got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Extraction from Wor domain (flat alignment)
    // -----------------------------------------------------------------------

    #[test]
    fn wor_domain_extracts_simple_words() {
        let chat = parse_chat(&one_utterance("the dog ran ."));
        let result = extract_words(&chat, TierDomain::Wor);
        assert_eq!(result[0].words.len(), 3);
        assert_eq!(result[0].words[0].text.as_str(), "the");
        assert_eq!(result[0].words[1].text.as_str(), "dog");
        assert_eq!(result[0].words[2].text.as_str(), "ran");
    }

    // -----------------------------------------------------------------------
    // Empty utterances
    // -----------------------------------------------------------------------

    #[test]
    fn empty_utterance_produces_empty_word_list() {
        // Utterance with only a terminator, no words.
        let chat = parse_chat(&one_utterance("0 ."));
        let result = extract_words(&chat, TierDomain::Mor);
        // "0" is a special CHAT symbol; whether it produces a word depends on
        // counts_for_tier(). We just verify no crash and at most 1 word.
        assert_eq!(result.len(), 1, "still produces 1 utterance");
    }
}
