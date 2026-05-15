//! PLAY annotation to CHAT conversion.
//!
//! Converts PLAY (Phonological and Lexical Acquisition in Young children)
//! annotation files into CHAT format.
//!
//! # Input format
//!
//! Tab-separated fields: `speaker`, `start_time`, `end_time`, `text`.
//! Times are in milliseconds and may be empty. Lines starting with `#` or
//! `%` are skipped. Lines with fewer than 2 tab-separated fields are ignored.
//!
//! Unique speakers are automatically collected and registered as CHAT
//! participants with the `Unidentified` role.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Automatically discovers and registers all unique speakers from the
//!   input data rather than requiring a predefined speaker list.
//! - Timing fields are optional; entries without timestamps produce
//!   utterances without timing bullets.

use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// A parsed PLAY entry.
#[derive(Debug)]
struct PlayEntry {
    /// Speaker label.
    speaker: String,
    /// Start time in milliseconds.
    start_ms: Option<u64>,
    /// End time in milliseconds.
    end_ms: Option<u64>,
    /// Utterance text.
    text: String,
}

/// Parse PLAY annotation content.
///
/// Format: tab-separated fields — speaker, start_time, end_time, text.
/// Times may be empty or in milliseconds.
fn parse_play(content: &str) -> Result<Vec<PlayEntry>, TransformError> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('%') {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 2 {
            continue;
        }

        let speaker = fields[0].trim().to_owned();
        let (start_ms, end_ms, text) = if fields.len() >= 4 {
            (
                fields[1].trim().parse::<u64>().ok(),
                fields[2].trim().parse::<u64>().ok(),
                fields[3].trim().to_owned(),
            )
        } else {
            (None, None, fields[1].trim().to_owned())
        };

        if !text.is_empty() {
            entries.push(PlayEntry {
                speaker,
                start_ms,
                end_ms,
                text,
            });
        }
    }

    Ok(entries)
}

/// Convert a PLAY annotation file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"play_corpus"` as the corpus name.
pub fn play_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    play_to_chat_with_options(content, "eng", "play_corpus")
}

/// Convert a PLAY annotation file to CHAT format with custom options.
pub fn play_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let entries = parse_play(content)?;

    let lang = LanguageCode::new(language);

    // Collect unique speakers
    let mut speakers: Vec<String> = Vec::new();
    for entry in &entries {
        if !speakers.contains(&entry.speaker) {
            speakers.push(entry.speaker.clone());
        }
    }
    if speakers.is_empty() {
        speakers.push("SPK".to_owned());
    }

    let participant_entries: Vec<ParticipantEntry> = speakers
        .iter()
        .map(|s| ParticipantEntry {
            speaker_code: SpeakerCode::new(s),
            name: None,
            role: ParticipantRole::new("Unidentified"),
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

    // Add @ID headers for each speaker
    for s in &speakers {
        lines.push(Line::header(Header::ID(
            IDHeader::new(
                lang.clone(),
                SpeakerCode::new(s),
                ParticipantRole::new("Unidentified"),
            )
            .with_corpus(corpus),
        )));
    }

    for entry in &entries {
        let words: Vec<UtteranceContent> = entry
            .text
            .split_whitespace()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let spk = SpeakerCode::new(&entry.speaker);
        let mut main_tier = MainTier::new(spk, words, Terminator::Period { span: Span::DUMMY });

        if let (Some(start), Some(end)) = (entry.start_ms, entry.end_ms) {
            main_tier = main_tier.with_bullet(Bullet::new(start, end));
        }

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_play_basic() {
        let entries = parse_play("CHI\t0\t1500\thello world\nMOT\t1500\t3000\thi there\n").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].speaker, "CHI");
        assert_eq!(entries[0].text, "hello world");
    }

    #[test]
    fn play_to_chat_basic() {
        let play = "CHI\t0\t1500\thello world\n";
        let chat = play_to_chat(play).unwrap();
        let output = chat.to_string();
        assert!(output.contains("*CHI:"));
        assert!(output.contains("hello"));
    }
}
