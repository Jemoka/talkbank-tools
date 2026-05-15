//! SRT (SubRip) subtitle to CHAT conversion.
//!
//! Parses SRT subtitle files and converts them to CHAT format, mapping
//! each subtitle block to an utterance with timing bullets derived from the
//! SRT timestamps.
//!
//! # SRT format
//!
//! SRT files consist of numbered blocks separated by blank lines:
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:03,000
//! Hello world
//! ```
//!
//! Timestamps use `HH:MM:SS,mmm` format (both comma and period separators
//! are accepted). Multi-line subtitle text within a block is joined with
//! spaces.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Accepts both comma and period as millisecond separators in timestamps,
//!   handling common SRT variants without preprocessing.
//! - All subtitle entries are assigned to a single default speaker; CLAN's
//!   SRT2CHAT behaves similarly but may differ in speaker handling.

use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// An SRT subtitle entry.
#[derive(Debug)]
struct SrtEntry {
    /// Start time in milliseconds.
    start_ms: u64,
    /// End time in milliseconds.
    end_ms: u64,
    /// Subtitle text lines (joined).
    text: String,
}

/// Parse an SRT timestamp like "00:01:23,456" into milliseconds.
fn parse_srt_timestamp(s: &str) -> Result<u64, TransformError> {
    let s = s.trim();
    // Format: HH:MM:SS,mmm or HH:MM:SS.mmm
    let parts: Vec<&str> = s.splitn(2, [',', '.']).collect();
    if parts.len() != 2 {
        return Err(TransformError::Parse(format!("Invalid SRT timestamp: {s}")));
    }

    let millis: u64 = parts[1]
        .parse()
        .map_err(|_| TransformError::Parse(format!("Invalid milliseconds in: {s}")))?;

    let time_parts: Vec<&str> = parts[0].split(':').collect();
    if time_parts.len() != 3 {
        return Err(TransformError::Parse(format!(
            "Invalid time format in: {s}"
        )));
    }

    let hours: u64 = time_parts[0]
        .parse()
        .map_err(|_| TransformError::Parse(format!("Invalid hours in: {s}")))?;
    let minutes: u64 = time_parts[1]
        .parse()
        .map_err(|_| TransformError::Parse(format!("Invalid minutes in: {s}")))?;
    let seconds: u64 = time_parts[2]
        .parse()
        .map_err(|_| TransformError::Parse(format!("Invalid seconds in: {s}")))?;

    Ok(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis)
}

/// Parse SRT content into a list of subtitle entries.
fn parse_srt(content: &str) -> Result<Vec<SrtEntry>, TransformError> {
    let mut entries = Vec::new();
    let mut lines_iter = content.lines().peekable();

    while lines_iter.peek().is_some() {
        // Skip empty lines between blocks
        while lines_iter.peek().is_some_and(|line| line.trim().is_empty()) {
            lines_iter.next();
        }

        // Read subtitle index (skip it)
        let Some(index_line) = lines_iter.next() else {
            break;
        };
        if index_line.trim().is_empty() {
            continue;
        }
        // Verify it's a number
        if index_line.trim().parse::<u64>().is_err() {
            return Err(TransformError::Parse(format!(
                "Expected subtitle index, got: {index_line}"
            )));
        }

        // Read timestamp line: "HH:MM:SS,mmm --> HH:MM:SS,mmm"
        let Some(ts_line) = lines_iter.next() else {
            return Err(TransformError::Parse(
                "Unexpected end of file after subtitle index".to_owned(),
            ));
        };
        let arrow_parts: Vec<&str> = ts_line.split("-->").collect();
        if arrow_parts.len() != 2 {
            return Err(TransformError::Parse(format!(
                "Invalid timestamp line: {ts_line}"
            )));
        }
        let start_ms = parse_srt_timestamp(arrow_parts[0])?;
        let end_ms = parse_srt_timestamp(arrow_parts[1])?;

        // Read text lines until empty line or EOF
        let mut text_lines = Vec::new();
        while let Some(line) = lines_iter.peek() {
            if line.trim().is_empty() {
                break;
            }
            // Peek-then-next: `peek()` returned `Some` immediately
            // above, so `next()` cannot return `None`.
            #[allow(clippy::unwrap_used)]
            text_lines.push(lines_iter.next().unwrap().trim().to_owned());
        }

        let text = text_lines.join(" ");
        if !text.is_empty() {
            entries.push(SrtEntry {
                start_ms,
                end_ms,
                text,
            });
        }
    }

    Ok(entries)
}

/// Convert an SRT subtitle file to a CHAT file with default options.
///
/// Uses `"SPK"` as the speaker, `"eng"` as the language, and `"srt_corpus"`
/// as the corpus name.
pub fn srt_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    srt_to_chat_with_options(content, "SPK", "eng", "srt_corpus")
}

/// Convert an SRT subtitle file to a CHAT file with custom speaker/language.
pub fn srt_to_chat_with_options(
    content: &str,
    speaker: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let entries = parse_srt(content)?;

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
        let words: Vec<UtteranceContent> = entry
            .text
            .split_whitespace()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let main_tier = MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY })
            .with_bullet(Bullet::new(entry.start_ms, entry.end_ms));

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_srt_timestamp_basic() {
        assert_eq!(parse_srt_timestamp("00:01:23,456").unwrap(), 83456);
        assert_eq!(parse_srt_timestamp("00:00:00,000").unwrap(), 0);
        assert_eq!(parse_srt_timestamp("01:00:00,000").unwrap(), 3_600_000);
    }

    #[test]
    fn srt_to_chat_basic() {
        let srt = "\
1
00:00:01,000 --> 00:00:03,000
Hello world

2
00:00:04,000 --> 00:00:06,000
How are you
";
        let chat = srt_to_chat(srt).unwrap();
        let text = chat.to_string();
        assert!(text.contains("@UTF8"));
        assert!(text.contains("*SPK:"));
        assert!(text.contains("Hello"));
    }
}
