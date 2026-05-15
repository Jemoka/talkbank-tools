//! LIPP phonetic profile to CHAT conversion.
//!
//! Converts LIPP (Logical International Phonetics Programs) phonetic profile
//! data into CHAT format. Each entry becomes an utterance, and phonetic
//! transcriptions are placed on `%pho` dependent tiers.
//!
//! # Input format
//!
//! Tab-separated word and phonetic transcription, one pair per line:
//! ```text
//! cat    kaet
//! dog    dog
//! ```
//!
//! Lines starting with `#` are treated as comments. Single words without
//! a tab-separated phonetic field are imported without a `%pho` tier.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Phonetic transcriptions are placed on `%pho` dependent tiers via the
//!   typed AST rather than string concatenation.
//! - Each word/phonetic pair becomes its own utterance; CLAN's LIPP2CHAT
//!   may group entries differently.

use talkbank_model::Span;
use talkbank_model::{
    ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier, NonEmptyString,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// A parsed LIPP entry.
#[derive(Debug)]
struct LippEntry {
    /// Orthographic form.
    word: String,
    /// Phonetic transcription.
    phonetic: String,
}

/// Parse LIPP profile content.
///
/// Format: tab-separated word and phonetic transcription, one pair per line.
fn parse_lipp(content: &str) -> Vec<LippEntry> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() == 2 {
            entries.push(LippEntry {
                word: parts[0].trim().to_owned(),
                phonetic: parts[1].trim().to_owned(),
            });
        } else {
            // Single word without phonetic
            entries.push(LippEntry {
                word: line.to_owned(),
                phonetic: String::new(),
            });
        }
    }

    entries
}

/// Convert LIPP phonetic profile to CHAT format with default options.
///
/// Uses `"CHI"` as the speaker, `"eng"` as the language, and `"lipp_corpus"`
/// as the corpus name.
pub fn lipp_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    lipp_to_chat_with_options(content, "CHI", "eng", "lipp_corpus")
}

/// Convert LIPP phonetic profile to CHAT format with custom options.
pub fn lipp_to_chat_with_options(
    content: &str,
    speaker: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let entries = parse_lipp(content);

    let lang = LanguageCode::new(language);
    let spk = SpeakerCode::new(speaker);
    let role = ParticipantRole::new("Target_Child");

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

    // Each entry becomes one utterance with optional %pho tier
    for entry in &entries {
        let words = vec![UtteranceContent::Word(Box::new(Word::simple(&entry.word)))];

        let main_tier = MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY });

        let mut utt = Utterance::new(main_tier);

        if !entry.phonetic.is_empty()
            && let (Some(label), Some(phon)) = (
                NonEmptyString::new("pho"),
                NonEmptyString::new(&entry.phonetic),
            )
        {
            utt = utt.with_user_defined(label, phon);
        }

        lines.push(Line::utterance(utt));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lipp_basic() {
        let entries = parse_lipp("cat\tkæt\ndog\tdɔɡ\n");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].word, "cat");
        assert_eq!(entries[0].phonetic, "kæt");
    }

    #[test]
    fn lipp_to_chat_basic() {
        let lipp = "cat\tkæt\ndog\tdɔɡ\n";
        let chat = lipp_to_chat(lipp).unwrap();
        let output = chat.to_string();
        assert!(output.contains("@UTF8"));
        assert!(output.contains("*CHI:"));
        assert!(output.contains("cat"));
    }
}
