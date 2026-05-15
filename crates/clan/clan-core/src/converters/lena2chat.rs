//! LENA device XML (`.its`) to CHAT conversion.
//!
//! Converts LENA (Language Environment Analysis) device output files
//! (`.its` format) into CHAT format. LENA XML contains segment-level
//! annotations with speaker types and timing information but no actual
//! transcribed words.
//!
//! # Speaker mapping
//!
//! LENA segment types are mapped to CHAT speaker codes:
//!
//! | LENA type | CHAT speaker | Description |
//! |-----------|-------------|-------------|
//! | `CHN`/`CXN` | `CHI` | Child near/far |
//! | `FAN`/`FAF` | `MOT` | Female adult near/far |
//! | `MAN`/`MAF` | `FAT` | Male adult near/far |
//! | `OLN`/`OLF` | `OTH` | Other child overlap |
//! | `TVN`/`TVF` | `ENV` | TV/electronic media |
//! | `NON`/`NOF` | `ENV` | Noise |
//! | `SIL` | *(skipped)* | Silence |
//!
//! Since LENA does not provide transcribed words, all utterances use `xxx`
//! (untranscribed) as placeholder text, with optional word count annotation.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Speaker mapping from LENA segment types to CHAT codes is a fixed table;
//!   CLAN's LENA2CHAT may support additional or configurable mappings.

use quick_xml::Reader;
use quick_xml::events::Event;
use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// A parsed LENA segment.
#[derive(Debug)]
struct LenaSegment {
    /// Segment type (CHN, CXN, FAN, MAN, OLN, etc.).
    segment_type: String,
    /// Start time in milliseconds.
    start_ms: u64,
    /// End time in milliseconds.
    end_ms: u64,
    /// Word count (if available).
    word_count: Option<u64>,
}

/// Map LENA segment type to CHAT speaker code.
fn lena_speaker(segment_type: &str) -> &str {
    match segment_type {
        "CHN" | "CXN" => "CHI", // Child near/far
        "FAN" | "FAF" => "MOT", // Female adult near/far (mapped to Mother)
        "MAN" | "MAF" => "FAT", // Male adult near/far (mapped to Father)
        "OLN" | "OLF" => "OTH", // Other child overlap near/far
        "TVN" | "TVF" => "ENV", // TV/electronic near/far
        "NON" | "NOF" => "ENV", // Noise near/far
        "SIL" => "SIL",         // Silence (skipped)
        _ => "UNK",
    }
}

/// Parse LENA ITS XML content using `quick-xml`.
fn parse_lena(content: &str) -> Result<Vec<LenaSegment>, TransformError> {
    let mut reader = Reader::from_str(content);
    let mut segments = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                // Look for any element that has a `spkr` attribute.
                let mut spkr = None;
                let mut start_time = None;
                let mut end_time = None;
                let mut word_cnt = None;

                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"spkr" => {
                            spkr = Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                        b"startTime" => {
                            start_time =
                                Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                        b"endTime" => {
                            end_time = Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                        b"wordCnt" => {
                            word_cnt = attr.unescape_value().ok().and_then(|v| v.parse().ok());
                        }
                        _ => {}
                    }
                }

                if let Some(segment_type) = spkr {
                    let start_ms = parse_iso8601_duration(start_time.as_deref().unwrap_or("PT0S"));
                    let end_ms = parse_iso8601_duration(end_time.as_deref().unwrap_or("PT0S"));
                    segments.push(LenaSegment {
                        segment_type,
                        start_ms,
                        end_ms,
                        word_count: word_cnt,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TransformError::Parse(format!(
                    "LENA XML parse error at position {}: {e}",
                    reader.error_position()
                )));
            }
            _ => {}
        }
    }

    Ok(segments)
}

/// Parse an ISO 8601 duration string (e.g. `"PT1H2M3.5S"`) into milliseconds.
///
/// Supports the subset used by LENA ITS files: `PT[nH][nM][n[.n]S]`.
/// Returns 0 for unrecognizable input.
fn parse_iso8601_duration(s: &str) -> u64 {
    let s = s.trim();
    let Some(body) = s.strip_prefix("PT") else {
        return 0;
    };
    let body = body.strip_suffix('S').unwrap_or(body);

    let mut total_seconds: f64 = 0.0;
    let mut number_buf = String::new();

    for ch in body.chars() {
        match ch {
            'H' => {
                total_seconds += number_buf.parse::<f64>().unwrap_or(0.0) * 3600.0;
                number_buf.clear();
            }
            'M' => {
                total_seconds += number_buf.parse::<f64>().unwrap_or(0.0) * 60.0;
                number_buf.clear();
            }
            _ => number_buf.push(ch),
        }
    }

    // Remaining number is seconds (after stripping trailing 'S')
    if !number_buf.is_empty() {
        total_seconds += number_buf.parse::<f64>().unwrap_or(0.0);
    }

    (total_seconds * 1000.0) as u64
}

/// Convert a LENA ITS file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"lena_corpus"` as the corpus name.
pub fn lena_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    lena_to_chat_with_options(content, "eng", "lena_corpus")
}

/// Convert a LENA ITS file to CHAT format with custom options.
pub fn lena_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let segments = parse_lena(content)?;
    let lang = LanguageCode::new(language);

    // Collect unique CHAT speakers from segments
    let mut speaker_set: Vec<String> = Vec::new();
    for seg in &segments {
        let spk = lena_speaker(&seg.segment_type);
        if spk != "SIL" && !speaker_set.contains(&spk.to_owned()) {
            speaker_set.push(spk.to_owned());
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
                "MOT" => "Mother",
                "FAT" => "Father",
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
            "MOT" => "Mother",
            "FAT" => "Father",
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

    for seg in &segments {
        let chat_speaker = lena_speaker(&seg.segment_type);
        if chat_speaker == "SIL" {
            continue;
        }

        // LENA doesn't provide actual words, so use placeholder "xxx"
        // (untranscribed) with word count annotation
        let word_text = if let Some(wc) = seg.word_count {
            if wc > 0 {
                format!("xxx({})", wc)
            } else {
                "xxx".to_owned()
            }
        } else {
            "xxx".to_owned()
        };

        let words = vec![UtteranceContent::Word(Box::new(Word::simple(&word_text)))];

        let main_tier = MainTier::new(
            SpeakerCode::new(chat_speaker),
            words,
            Terminator::Period { span: Span::DUMMY },
        )
        .with_bullet(Bullet::new(seg.start_ms, seg.end_ms));

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso8601_duration_basic() {
        assert_eq!(parse_iso8601_duration("PT1.5S"), 1500);
        assert_eq!(parse_iso8601_duration("PT0S"), 0);
        assert_eq!(parse_iso8601_duration("PT1M30S"), 90000);
        assert_eq!(parse_iso8601_duration("PT1H2M3S"), 3723000);
    }

    #[test]
    fn lena_speaker_mapping() {
        assert_eq!(lena_speaker("CHN"), "CHI");
        assert_eq!(lena_speaker("FAN"), "MOT");
        assert_eq!(lena_speaker("MAN"), "FAT");
        assert_eq!(lena_speaker("SIL"), "SIL");
    }

    #[test]
    fn parse_lena_basic() {
        let its = r#"<?xml version="1.0" encoding="UTF-8"?>
<ITS>
  <ProcessingUnit>
    <item spkr="CHN" startTime="PT1S" endTime="PT3.5S" wordCnt="2"/>
    <item spkr="FAN" startTime="PT4S" endTime="PT6.5S" wordCnt="3"/>
  </ProcessingUnit>
</ITS>"#;

        let segments = parse_lena(its).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].segment_type, "CHN");
        assert_eq!(segments[0].start_ms, 1000);
        assert_eq!(segments[0].end_ms, 3500);
        assert_eq!(segments[0].word_count, Some(2));
        assert_eq!(segments[1].segment_type, "FAN");
        assert_eq!(segments[1].start_ms, 4000);
    }
}
