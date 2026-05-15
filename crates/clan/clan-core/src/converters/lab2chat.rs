//! LAB timing label files to CHAT conversion.
//!
//! Converts LAB (label) timing files into CHAT format. LAB files contain
//! time-aligned word or segment labels commonly used in speech research
//! tools (e.g., HTK, Kaldi).
//!
//! # Supported formats
//!
//! - **Three-column**: `start_time end_time label` (times in seconds)
//! - **Two-column**: `time label` (end time inferred from the next entry)
//!
//! Silence markers (`sil`, `sp`, `#`) are skipped during conversion.
//! Each non-silence label becomes a separate utterance with timing bullets.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Supports both two-column and three-column LAB formats in a single
//!   parser, automatically detecting the column layout.
//! - Silence markers are identified by a fixed set (`sil`, `sp`, `#`)
//!   rather than configurable patterns.

use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// A parsed LAB entry.
#[derive(Debug)]
struct LabEntry {
    /// Start time in seconds.
    start: f64,
    /// End time in seconds.
    end: f64,
    /// Label text.
    label: String,
}

/// Parse a LAB file.
///
/// Supported formats:
/// - Three-column: `start_time end_time label`
/// - Two-column: `time label` (uses next entry's time as end)
fn parse_lab(content: &str) -> Result<Vec<LabEntry>, TransformError> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.len() {
            3 => {
                let start: f64 = parts[0]
                    .parse()
                    .map_err(|_| TransformError::Parse(format!("Invalid start time: {line}")))?;
                let end: f64 = parts[1]
                    .parse()
                    .map_err(|_| TransformError::Parse(format!("Invalid end time: {line}")))?;
                entries.push(LabEntry {
                    start,
                    end,
                    label: parts[2].to_owned(),
                });
            }
            2 => {
                let time: f64 = parts[0]
                    .parse()
                    .map_err(|_| TransformError::Parse(format!("Invalid time: {line}")))?;
                entries.push(LabEntry {
                    start: time,
                    end: 0.0, // Will be filled from next entry
                    label: parts[1].to_owned(),
                });
            }
            _ => {
                return Err(TransformError::Parse(format!(
                    "Invalid LAB line (expected 2 or 3 columns): {line}"
                )));
            }
        }
    }

    // Fix two-column format: use next entry's start as current entry's end
    for i in 0..entries.len() {
        if entries[i].end == 0.0 {
            entries[i].end = if i + 1 < entries.len() {
                entries[i + 1].start
            } else {
                entries[i].start + 1.0 // Default 1 second for last entry
            };
        }
    }

    Ok(entries)
}

/// Convert a LAB timing label file to CHAT format with default options.
///
/// Uses `"SPK"` as the speaker, `"eng"` as the language, and `"lab_corpus"`
/// as the corpus name.
pub fn lab_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    lab_to_chat_with_options(content, "SPK", "eng", "lab_corpus")
}

/// Convert a LAB timing label file to CHAT format with custom options.
pub fn lab_to_chat_with_options(
    content: &str,
    speaker: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let entries = parse_lab(content)?;

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

    for entry in &entries {
        // Skip silence markers
        if entry.label == "sil" || entry.label == "sp" || entry.label == "#" {
            continue;
        }

        let words = vec![UtteranceContent::Word(Box::new(Word::simple(&entry.label)))];

        let start_ms = (entry.start * 1000.0) as u64;
        let end_ms = (entry.end * 1000.0) as u64;

        let main_tier = MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY })
            .with_bullet(Bullet::new(start_ms, end_ms));

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lab_three_column() {
        let entries = parse_lab("0.0 1.5 hello\n1.5 3.0 world\n").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label, "hello");
        assert!((entries[0].start - 0.0).abs() < f64::EPSILON);
        assert!((entries[0].end - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_lab_two_column() {
        let entries = parse_lab("0.0 hello\n1.5 world\n").unwrap();
        assert_eq!(entries.len(), 2);
        assert!((entries[0].end - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lab_to_chat_basic() {
        let lab = "0.0 1.5 hello\n1.5 3.0 world\n";
        let chat = lab_to_chat(lab).unwrap();
        let output = chat.to_string();
        assert!(output.contains("@UTF8"));
        assert!(output.contains("hello"));
    }
}
