//! SALT (Systematic Analysis of Language Transcripts) to CHAT conversion.
//!
//! Converts SALT transcription files into CHAT format. SALT is a widely
//! used clinical transcription system with its own conventions for speaker
//! codes, morpheme annotations, and error marking.
//!
//! # Speaker mapping
//!
//! | SALT code | CHAT speaker | Role |
//! |-----------|-------------|------|
//! | `C` | `CHI` | Target_Child |
//! | `E` | `EXA` | Investigator |
//! | `P` | `PAR` | (Parent) |
//! | `I` | `INV` | (Investigator) |
//!
//! # SALT annotation stripping
//!
//! SALT-specific annotations are removed during conversion:
//! - Morpheme codes (`word/3s` --> `word`)
//! - Error markers (`word*` --> `word`)
//! - Maze markers (`(word)` --> skipped)
//! - Comment markers (`{...}`, `[...]` --> skipped)
//! - Bound morpheme markers (`_word` --> `word`)
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - SALT annotation stripping (morpheme codes, error markers, mazes) is
//!   performed during parsing rather than via string replacement on output.
//! - Speaker mapping from SALT codes to CHAT codes uses a fixed table
//!   rather than interactive prompts.

use talkbank_model::Span;
use talkbank_model::{
    ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier, ParticipantEntries,
    ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance, UtteranceContent, Word,
};

use crate::framework::TransformError;

/// A parsed SALT header.
#[derive(Debug, Default)]
struct SaltHeader {
    /// Participant name/ID.
    name: Option<String>,
    /// Age (e.g., "5;6" for years;months).
    age: Option<String>,
    /// Gender.
    gender: Option<String>,
    /// Context/setting.
    context: Option<String>,
}

/// A parsed SALT utterance.
#[derive(Debug)]
struct SaltUtterance {
    /// Speaker code (C = child, E = examiner, etc.).
    speaker: String,
    /// Utterance words (SALT codes stripped).
    words: Vec<String>,
}

/// Parse SALT header lines (lines starting with `$` or `+`).
fn parse_salt_header(content: &str) -> SaltHeader {
    let mut header = SaltHeader::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("$ ") || trimmed.starts_with("+ ") {
            let rest = &trimmed[2..];
            if let Some(val) = rest.strip_prefix("Name: ") {
                header.name = Some(val.trim().to_owned());
            } else if let Some(val) = rest.strip_prefix("Age: ") {
                header.age = Some(val.trim().to_owned());
            } else if let Some(val) = rest.strip_prefix("Gender: ") {
                header.gender = Some(val.trim().to_owned());
            } else if let Some(val) = rest.strip_prefix("Context: ") {
                header.context = Some(val.trim().to_owned());
            }
        }
    }

    header
}

/// Map SALT speaker code to CHAT speaker code.
fn salt_speaker(code: &str) -> &str {
    match code {
        "C" => "CHI",
        "E" => "EXA",
        "P" => "PAR", // Parent
        "I" => "INV", // Investigator
        _ => {
            // Use first 3 chars uppercased if longer
            code
        }
    }
}

/// Clean a SALT word by removing SALT-specific annotations.
///
/// SALT uses codes like `word/3s` (morpheme codes), `word*` (errors),
/// `(word)` (omitted words), `[word]` (comments).
fn clean_salt_word(word: &str) -> Option<String> {
    let word = word.trim();

    // Skip empty and comment markers
    if word.is_empty() {
        return None;
    }

    // Skip SALT maze markers (parenthesized false starts)
    if word.starts_with('(') && word.ends_with(')') {
        return None;
    }

    // Skip SALT comment markers
    if word.starts_with('{') || word.starts_with('[') {
        return None;
    }

    // Strip SALT morpheme codes: word/3s → word
    let cleaned = if let Some(slash) = word.find('/') {
        &word[..slash]
    } else {
        word
    };

    // Strip error marker: word* → word
    let cleaned = cleaned.trim_end_matches('*');

    // Strip bound morpheme marker
    let cleaned = cleaned.trim_start_matches('_');

    if cleaned.is_empty() {
        return None;
    }

    Some(cleaned.to_owned())
}

/// Parse SALT utterance lines.
fn parse_salt_utterances(content: &str) -> Vec<SaltUtterance> {
    let mut utterances = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip header/meta lines
        if trimmed.is_empty()
            || trimmed.starts_with('$')
            || trimmed.starts_with('+')
            || trimmed.starts_with('-')
            || trimmed.starts_with(';')
        {
            continue;
        }

        // SALT utterances start with speaker code followed by space
        // e.g., "C I want that." or "E What do you want?"
        let (speaker, rest) = if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b' ' {
            let code = &trimmed[..1];
            (code.to_owned(), trimmed[2..].trim())
        } else {
            continue;
        };

        // Strip utterance terminator (. ? !) from end
        let rest = rest
            .trim_end_matches('.')
            .trim_end_matches('?')
            .trim_end_matches('!')
            .trim();

        let words: Vec<String> = rest
            .split_whitespace()
            .filter_map(clean_salt_word)
            .collect();

        if !words.is_empty() {
            utterances.push(SaltUtterance { speaker, words });
        }
    }

    utterances
}

/// Convert a SALT file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"salt_corpus"` as the corpus name.
pub fn salt_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    salt_to_chat_with_options(content, "eng", "salt_corpus")
}

/// Convert a SALT file to CHAT format with custom options.
pub fn salt_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let _header = parse_salt_header(content);
    let utterances = parse_salt_utterances(content);

    let lang = LanguageCode::new(language);

    // Collect unique CHAT speakers
    let mut speaker_set: Vec<String> = Vec::new();
    for utt in &utterances {
        let chat_spk = salt_speaker(&utt.speaker).to_owned();
        if !speaker_set.contains(&chat_spk) {
            speaker_set.push(chat_spk);
        }
    }
    if speaker_set.is_empty() {
        speaker_set.push("CHI".to_owned());
    }

    let participant_entries: Vec<ParticipantEntry> = speaker_set
        .iter()
        .map(|s| {
            let role = match s.as_str() {
                "CHI" => "Target_Child",
                "EXA" => "Investigator",
                _ => "Unidentified",
            };
            ParticipantEntry {
                speaker_code: SpeakerCode::new(s),
                name: None,
                role: ParticipantRole::new(role),
            }
        })
        .collect();

    let mut lines = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::header(Header::Languages {
            codes: LanguageCodes::new(vec![lang.clone()]),
        }),
        Line::header(Header::Participants {
            entries: ParticipantEntries::new(participant_entries),
        }),
    ];

    for s in &speaker_set {
        let role = match s.as_str() {
            "CHI" => "Target_Child",
            "EXA" => "Investigator",
            _ => "Unidentified",
        };
        lines.push(Line::header(Header::ID(
            IDHeader::new(
                lang.clone(),
                SpeakerCode::new(s),
                ParticipantRole::new(role),
            )
            .with_corpus(corpus),
        )));
    }

    for utt in &utterances {
        let chat_spk = salt_speaker(&utt.speaker);

        let words: Vec<UtteranceContent> = utt
            .words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let main_tier = MainTier::new(
            SpeakerCode::new(chat_spk),
            words,
            Terminator::Period { span: Span::DUMMY },
        );

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_salt_word_basic() {
        assert_eq!(clean_salt_word("want/3s"), Some("want".to_owned()));
        assert_eq!(clean_salt_word("go*"), Some("go".to_owned()));
        assert_eq!(clean_salt_word("(um)"), None);
        assert_eq!(clean_salt_word("{comment}"), None);
    }

    #[test]
    fn parse_salt_utterances_basic() {
        let salt = "C I want that.\nE What do you want?\n";
        let utts = parse_salt_utterances(salt);
        assert_eq!(utts.len(), 2);
        assert_eq!(utts[0].speaker, "C");
        assert_eq!(utts[0].words, vec!["I", "want", "that"]);
    }

    #[test]
    fn salt_to_chat_basic() {
        let salt = "$ Name: Test\nC I want that.\nE What do you want?\n";
        let chat = salt_to_chat(salt).unwrap();
        let output = chat.to_string();
        assert!(output.contains("@UTF8"));
        assert!(output.contains("*CHI:"));
        assert!(output.contains("*EXA:"));
    }
}
