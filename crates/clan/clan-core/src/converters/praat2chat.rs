// `writeln!` to `String` / `&mut Vec<u8>` is infallible — the
// `fmt::Write` / `io::Write` impls for those types never return
// `Err`. `unwrap` is the conventional pattern across this crate's
// text emitters; flagging each call individually would require
// ~150 annotations across the converter files. The crate's
// `deny`-level panic lints still apply; this file-level `unwrap`
// allow is the documented per-emitter exception.
#![allow(clippy::unwrap_used)]

//! Praat TextGrid bidirectional conversion (TextGrid <--> CHAT).
//!
//! Converts between Praat TextGrid files and CHAT format. TextGrid files
//! contain time-aligned interval tiers widely used in phonetic research.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409302)
//! for the original PRAAT2CHAT command documentation.
//!
//! # Conversion functions
//!
//! - [`praat_to_chat()`] / [`praat_to_chat_with_options()`] -- TextGrid to CHAT
//! - [`chat_to_praat()`] -- CHAT to TextGrid
//!
//! # TextGrid format support
//!
//! Both long (normal) and short TextGrid formats are supported. Tier names are
//! mapped to CHAT speaker codes (first 3 characters, uppercased). Empty
//! intervals and point tiers are skipped. Untimed utterances are excluded
//! from CHAT-to-TextGrid conversion.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - Supports bidirectional conversion (TextGrid to CHAT and CHAT to TextGrid)
//!   in a single module, whereas CLAN has separate commands.
//! - Handles both long and short TextGrid formats with a unified parser.

use std::fmt::Write;

use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::{TransformError, spoken_main_text};

/// A parsed TextGrid interval.
#[derive(Debug)]
struct TextGridInterval {
    /// Start time in seconds.
    xmin: f64,
    /// End time in seconds.
    xmax: f64,
    /// Text content.
    text: String,
}

/// A parsed TextGrid tier.
#[derive(Debug)]
struct TextGridTier {
    /// Tier name (used as speaker code).
    name: String,
    /// Intervals in this tier.
    intervals: Vec<TextGridInterval>,
}

/// Parse a Praat TextGrid file (short or long format).
fn parse_textgrid(content: &str) -> Result<Vec<TextGridTier>, TransformError> {
    let mut tiers = Vec::new();

    if content.trim_start().starts_with("\"") {
        // Short format
        return parse_textgrid_short(content);
    }

    // Long (normal) format
    let mut lines_iter = content.lines().peekable();
    let mut current_tier_name = String::new();
    let mut current_intervals = Vec::new();
    let mut in_intervals = false;

    while let Some(line) = lines_iter.next() {
        let trimmed = line.trim();

        if trimmed.starts_with("name = ") {
            current_tier_name = extract_quoted_value(trimmed);
            current_intervals.clear();
            in_intervals = false;
        } else if trimmed == "intervals: size =" || trimmed.starts_with("intervals: size =") {
            in_intervals = true;
        } else if in_intervals && trimmed.starts_with("xmin = ") {
            let xmin = extract_float_value(trimmed)?;
            let xmax_line = lines_iter.next().unwrap_or("");
            let xmax = extract_float_value(xmax_line.trim())?;
            let text_line = lines_iter.next().unwrap_or("");
            let text = extract_quoted_value(text_line.trim());
            current_intervals.push(TextGridInterval { xmin, xmax, text });
        } else if trimmed.starts_with("item [") && !current_tier_name.is_empty() {
            // New tier — flush previous
            if !current_intervals.is_empty() {
                tiers.push(TextGridTier {
                    name: current_tier_name.clone(),
                    intervals: std::mem::take(&mut current_intervals),
                });
            }
            in_intervals = false;
        }
    }

    // Flush last tier
    if !current_tier_name.is_empty() && !current_intervals.is_empty() {
        tiers.push(TextGridTier {
            name: current_tier_name,
            intervals: current_intervals,
        });
    }

    Ok(tiers)
}

/// Parse TextGrid short format.
fn parse_textgrid_short(content: &str) -> Result<Vec<TextGridTier>, TransformError> {
    let mut tiers = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    // Skip header lines until we find tier data
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("\"IntervalTier\"") || trimmed.starts_with("\"TextTier\"") {
            let is_interval = trimmed.starts_with("\"IntervalTier\"");
            i += 1;
            if i >= lines.len() {
                break;
            }

            let name = lines[i].trim().trim_matches('"').to_owned();
            i += 1;

            // Skip xmin, xmax
            i += 2;
            if i >= lines.len() {
                break;
            }

            let count: usize = lines[i].trim().parse().unwrap_or(0);
            i += 1;

            if is_interval {
                let mut intervals = Vec::new();
                for _ in 0..count {
                    if i + 2 >= lines.len() {
                        break;
                    }
                    let xmin: f64 = lines[i].trim().parse().unwrap_or(0.0);
                    i += 1;
                    let xmax: f64 = lines[i].trim().parse().unwrap_or(0.0);
                    i += 1;
                    let text = lines[i].trim().trim_matches('"').to_owned();
                    i += 1;
                    intervals.push(TextGridInterval { xmin, xmax, text });
                }
                tiers.push(TextGridTier { name, intervals });
            } else {
                // Point tier — skip
                i += count * 2;
            }
        } else {
            i += 1;
        }
    }

    Ok(tiers)
}

/// Extract a quoted string value from a TextGrid line like `name = "CHI"`.
fn extract_quoted_value(line: &str) -> String {
    if let Some(start) = line.find('"')
        && let Some(end) = line[start + 1..].find('"')
    {
        return line[start + 1..start + 1 + end].to_owned();
    }
    String::new()
}

/// Extract a float value from a line like `xmin = 0.123`.
fn extract_float_value(line: &str) -> Result<f64, TransformError> {
    let value = line.split('=').nth(1).unwrap_or("").trim();
    value
        .parse()
        .map_err(|_| TransformError::Parse(format!("Invalid float: {line}")))
}

/// Convert a Praat TextGrid file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"praat_corpus"` as the corpus name.
pub fn praat_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    praat_to_chat_with_options(content, "eng", "praat_corpus")
}

/// Convert a Praat TextGrid file to CHAT format with custom options.
pub fn praat_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let tiers = parse_textgrid(content)?;
    let lang = LanguageCode::new(language);

    // Use tier names as speaker codes (truncate to 3 chars uppercase)
    let speakers: Vec<(SpeakerCode, String)> = tiers
        .iter()
        .map(|t| {
            let code = t.name.chars().take(3).collect::<String>().to_uppercase();
            (SpeakerCode::new(&code), t.name.clone())
        })
        .collect();

    let participant_entries: Vec<ParticipantEntry> = speakers
        .iter()
        .map(|(code, _)| ParticipantEntry {
            speaker_code: code.clone(),
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

    for (code, _) in &speakers {
        lines.push(Line::header(Header::ID(
            IDHeader::new(
                lang.clone(),
                code.clone(),
                ParticipantRole::new("Unidentified"),
            )
            .with_corpus(corpus),
        )));
    }

    // Convert intervals to utterances
    for (tier_idx, tier) in tiers.iter().enumerate() {
        let spk = &speakers[tier_idx].0;

        for interval in &tier.intervals {
            if interval.text.is_empty() {
                continue;
            }

            let words: Vec<UtteranceContent> = interval
                .text
                .split_whitespace()
                .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
                .collect();

            if words.is_empty() {
                continue;
            }

            let start_ms = (interval.xmin * 1000.0) as u64;
            let end_ms = (interval.xmax * 1000.0) as u64;

            let main_tier =
                MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY })
                    .with_bullet(Bullet::new(start_ms, end_ms));

            lines.push(Line::utterance(Utterance::new(main_tier)));
        }
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

/// Convert a CHAT file to Praat TextGrid long format.
///
/// Groups utterances by speaker into separate interval tiers. Only timed
/// utterances (those with timing bullets) are included. Returns an empty
/// string if no timed utterances are found.
pub fn chat_to_praat(chat: &ChatFile) -> Result<String, TransformError> {
    let mut output = String::new();

    // Collect utterances per speaker
    let mut speaker_intervals: Vec<(String, Vec<TextGridInterval>)> = Vec::new();
    let mut speaker_map: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    let mut max_time: f64 = 0.0;

    for line in chat.lines.iter() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        let speaker = utt.main.speaker.to_string();
        let text = spoken_main_text(&utt.main);

        let (xmin, xmax) = if let Some(ref bullet) = utt.main.content.bullet {
            let start = bullet.timing.start_ms as f64 / 1000.0;
            let end = bullet.timing.end_ms as f64 / 1000.0;
            (start, end)
        } else {
            continue; // Skip untimed utterances
        };

        if xmax > max_time {
            max_time = xmax;
        }

        let idx = if let Some(&idx) = speaker_map.get(&speaker) {
            idx
        } else {
            let idx = speaker_intervals.len();
            speaker_map.insert(speaker.clone(), idx);
            speaker_intervals.push((speaker, Vec::new()));
            idx
        };

        speaker_intervals[idx]
            .1
            .push(TextGridInterval { xmin, xmax, text });
    }

    if speaker_intervals.is_empty() {
        return Ok(String::new());
    }

    // Write TextGrid header
    writeln!(output, "File type = \"ooTextFile\"").unwrap();
    writeln!(output, "Object class = \"TextGrid\"").unwrap();
    writeln!(output).unwrap();
    writeln!(output, "xmin = 0").unwrap();
    writeln!(output, "xmax = {max_time}").unwrap();
    writeln!(output, "tiers? <exists>").unwrap();
    writeln!(output, "size = {}", speaker_intervals.len()).unwrap();
    writeln!(output, "item []:").unwrap();

    for (tier_idx, (name, intervals)) in speaker_intervals.iter().enumerate() {
        writeln!(output, "    item [{}]:", tier_idx + 1).unwrap();
        writeln!(output, "        class = \"IntervalTier\"").unwrap();
        writeln!(output, "        name = \"{name}\"").unwrap();
        writeln!(output, "        xmin = 0").unwrap();
        writeln!(output, "        xmax = {max_time}").unwrap();
        writeln!(output, "        intervals: size = {}", intervals.len()).unwrap();

        for (int_idx, interval) in intervals.iter().enumerate() {
            writeln!(output, "        intervals [{}]:", int_idx + 1).unwrap();
            writeln!(output, "            xmin = {}", interval.xmin).unwrap();
            writeln!(output, "            xmax = {}", interval.xmax).unwrap();
            let escaped = interval.text.replace('"', "\"\"");
            writeln!(output, "            text = \"{escaped}\"").unwrap();
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_quoted_value_basic() {
        assert_eq!(extract_quoted_value("name = \"CHI\""), "CHI");
        assert_eq!(
            extract_quoted_value("text = \"hello world\""),
            "hello world"
        );
    }

    #[test]
    fn parse_textgrid_long_format() {
        let tg = r#"File type = "ooTextFile"
Object class = "TextGrid"

xmin = 0
xmax = 5.0
tiers? <exists>
size = 1
item []:
    item [1]:
        class = "IntervalTier"
        name = "CHI"
        xmin = 0
        xmax = 5.0
        intervals: size = 2
        intervals [1]:
            xmin = 0.0
            xmax = 2.5
            text = "hello"
        intervals [2]:
            xmin = 2.5
            xmax = 5.0
            text = "world"
"#;
        let tiers = parse_textgrid(tg).unwrap();
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].name, "CHI");
        assert_eq!(tiers[0].intervals.len(), 2);
    }

    #[test]
    fn praat_to_chat_basic() {
        let tg = r#"File type = "ooTextFile"
Object class = "TextGrid"

xmin = 0
xmax = 5.0
tiers? <exists>
size = 1
item []:
    item [1]:
        class = "IntervalTier"
        name = "CHI"
        xmin = 0
        xmax = 5.0
        intervals: size = 1
        intervals [1]:
            xmin = 0.0
            xmax = 2.5
            text = "hello world"
"#;
        let chat = praat_to_chat(tg).unwrap();
        let output = chat.to_string();
        assert!(output.contains("*CHI:"));
        assert!(output.contains("hello"));
    }
}
