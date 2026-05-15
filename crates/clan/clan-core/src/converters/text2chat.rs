//! Plain text to CHAT conversion.
//!
//! Converts plain text files into CHAT format by splitting on sentence-ending
//! punctuation (`.`, `?`, `!`) and assigning all utterances to a default
//! speaker. This is the simplest converter, useful for bootstrapping CHAT
//! files from raw text.
//!
//! Sentence terminators are preserved as CHAT terminators (period, question
//! mark, exclamation point). Trailing text without punctuation receives a
//! default period terminator. Newlines within the input are treated as
//! spaces (not sentence boundaries).
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Sentence boundary detection splits only on `.`, `?`, `!` (no
//!   interactive or configurable sentence-breaking rules).
//! - Newlines are normalized to spaces rather than treated as utterance
//!   boundaries, which may differ from CLAN's TEXT2CHAT behavior.

use talkbank_model::Span;
use talkbank_model::{
    ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier, ParticipantEntries,
    ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance, UtteranceContent, Word,
};

use crate::framework::TransformError;

/// Convert plain text to a CHAT file with default options.
///
/// Uses `"SPK"` as the speaker, `"eng"` as the language, and `"text_corpus"`
/// as the corpus name. Splits text into utterances at sentence-ending
/// punctuation (`.`, `?`, `!`). Trailing text without punctuation gets a
/// default period terminator.
pub fn text_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    text_to_chat_with_options(content, "SPK", "eng", "text_corpus")
}

/// Convert plain text to a CHAT file with custom speaker/language.
pub fn text_to_chat_with_options(
    content: &str,
    speaker: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let lang = LanguageCode::new(language);
    let spk = SpeakerCode::new(speaker);
    let role = ParticipantRole::new("Unidentified");

    let mut lines = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::header(Header::Languages {
            codes: LanguageCodes::new(vec![lang.clone()]),
        }),
        Line::header(Header::Participants {
            entries: ParticipantEntries::new(vec![ParticipantEntry {
                speaker_code: spk.clone(),
                name: None,
                role: role.clone(),
            }]),
        }),
        Line::header(Header::ID(
            IDHeader::new(lang.clone(), spk.clone(), role).with_corpus(corpus),
        )),
    ];

    // Split content into sentences at sentence-ending punctuation
    let sentences = split_sentences(content);

    for (sentence, terminator) in &sentences {
        let words: Vec<UtteranceContent> = sentence
            .split_whitespace()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let term = match terminator {
            '?' => Terminator::Question { span: Span::DUMMY },
            '!' => Terminator::Exclamation { span: Span::DUMMY },
            _ => Terminator::Period { span: Span::DUMMY },
        };

        let main_tier = MainTier::new(spk.clone(), words, term);
        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

/// Split text into (sentence, terminator_char) pairs.
fn split_sentences(text: &str) -> Vec<(String, char)> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch == '.' || ch == '?' || ch == '!' {
            let trimmed = current.trim().to_owned();
            if !trimmed.is_empty() {
                sentences.push((trimmed, ch));
            }
            current.clear();
        } else if ch == '\n' || ch == '\r' {
            current.push(' ');
        } else {
            current.push(ch);
        }
    }

    // Handle trailing text without punctuation
    let trimmed = current.trim().to_owned();
    if !trimmed.is_empty() {
        sentences.push((trimmed, '.'));
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_to_chat_basic() {
        let text = "Hello world. How are you? I am fine!";
        let chat = text_to_chat(text).unwrap();
        let output = chat.to_string();
        assert!(output.contains("@UTF8"));
        assert!(output.contains("*SPK:"));
        assert!(output.contains("Hello"));
    }

    #[test]
    fn split_sentences_basic() {
        let sentences = split_sentences("Hello world. How are you?");
        assert_eq!(sentences.len(), 2);
        assert_eq!(sentences[0], ("Hello world".to_owned(), '.'));
        assert_eq!(sentences[1], ("How are you".to_owned(), '?'));
    }

    #[test]
    fn split_sentences_trailing_text() {
        let sentences = split_sentences("Hello world");
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], ("Hello world".to_owned(), '.'));
    }
}
