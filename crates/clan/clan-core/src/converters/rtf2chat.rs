//! RTF (Rich Text Format) to CHAT conversion.
//!
//! Converts Rich Text Format (RTF) files into CHAT format by stripping
//! RTF formatting commands and extracting plain text content.
//!
//! # Processing steps
//!
//! 1. **RTF stripping**: Removes control words, groups,
//!    font/color/stylesheet tables, and converts Unicode escapes (`\uN?`)
//!    to characters. Handles `\par` (newline) and `\tab` (tab).
//! 2. **Turn extraction**: Looks for CHAT-style speaker
//!    prefixes (`*CHI:`, `*MOT:`) in the plain text. If none are found, all
//!    text is assigned to a default `SPK` speaker.
//! 3. **CHAT construction**: Builds a proper `ChatFile` with headers,
//!    participants, and utterances.
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - RTF stripping handles basic control words, Unicode escapes, and
//!   group skipping (font tables, stylesheets, etc.) without a full RTF
//!   parser library.
//! - Detects CHAT-style speaker prefixes (`*CHI:`, `*MOT:`) in the
//!   extracted text; falls back to a default speaker if none are found.

use talkbank_model::Span;
use talkbank_model::{
    ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier, ParticipantEntries,
    ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance, UtteranceContent, Word,
};

use crate::framework::TransformError;

/// Strip RTF control sequences and extract plain text.
///
/// Handles basic RTF structure: `{\rtf1 ... }`, control words (`\par`, `\b`, etc.),
/// Unicode escapes (`\u1234?`), and curly-brace groups.
fn strip_rtf(content: &str) -> String {
    let mut output = String::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut skip_group = false;

    while i < chars.len() {
        match chars[i] {
            '{' => {
                depth += 1;
                // Skip known non-text groups
                let rest: String = chars[i..].iter().take(20).collect();
                if rest.starts_with("{\\fonttbl")
                    || rest.starts_with("{\\colortbl")
                    || rest.starts_with("{\\stylesheet")
                    || rest.starts_with("{\\info")
                    || rest.starts_with("{\\header")
                    || rest.starts_with("{\\footer")
                    || rest.starts_with("{\\pict")
                {
                    skip_group = true;
                }
                i += 1;
            }
            '}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                skip_group = false;
                i += 1;
            }
            '\\' if !skip_group => {
                i += 1;
                if i >= chars.len() {
                    break;
                }

                match chars[i] {
                    // Escaped special chars
                    '{' | '}' | '\\' => {
                        output.push(chars[i]);
                        i += 1;
                    }
                    // Unicode escape: \uN? (skip replacement char)
                    'u' if i + 1 < chars.len()
                        && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '-') =>
                    {
                        i += 1;
                        let mut num = String::new();
                        if chars[i] == '-' {
                            num.push('-');
                            i += 1;
                        }
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            num.push(chars[i]);
                            i += 1;
                        }
                        if let Ok(code) = num.parse::<i32>() {
                            let code = if code < 0 {
                                (code + 65536) as u32
                            } else {
                                code as u32
                            };
                            if let Some(ch) = char::from_u32(code) {
                                output.push(ch);
                            }
                        }
                        // Skip replacement character
                        if i < chars.len() && chars[i] == '?' {
                            i += 1;
                        }
                    }
                    // Control words
                    _ => {
                        let mut word = String::new();
                        while i < chars.len() && chars[i].is_ascii_alphabetic() {
                            word.push(chars[i]);
                            i += 1;
                        }
                        // Skip optional numeric parameter
                        if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-')
                            {
                                i += 1;
                            }
                        }
                        // Skip trailing space
                        if i < chars.len() && chars[i] == ' ' {
                            i += 1;
                        }

                        match word.as_str() {
                            "par" | "line" => output.push('\n'),
                            "tab" => output.push('\t'),
                            _ => {}
                        }
                    }
                }
            }
            _ if !skip_group => {
                output.push(chars[i]);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    output
}

/// Split extracted text into speaker-attributed lines.
///
/// Looks for CHAT-style speaker prefixes (`*CHI:`, `*MOT:`) in the text.
/// If none found, treats each non-empty line as an utterance from a default speaker.
fn extract_turns(text: &str) -> Vec<(String, String)> {
    let mut turns = Vec::new();
    let mut current_speaker = String::new();
    let mut current_text = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for CHAT-style speaker prefix
        if trimmed.starts_with('*')
            && let Some(colon_pos) = trimmed.find(':')
        {
            let spk = &trimmed[1..colon_pos];
            if spk.len() <= 4
                && spk
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                // Flush previous
                if !current_text.is_empty() {
                    turns.push((current_speaker.clone(), current_text.trim().to_owned()));
                    current_text.clear();
                }
                current_speaker = spk.to_owned();
                let rest = trimmed[colon_pos + 1..].trim();
                if !rest.is_empty() {
                    current_text.push_str(rest);
                }
                continue;
            }
        }

        if current_speaker.is_empty() {
            current_speaker = "SPK".to_owned();
        }
        if !current_text.is_empty() {
            current_text.push(' ');
        }
        current_text.push_str(trimmed);
    }

    if !current_text.is_empty() {
        turns.push((current_speaker, current_text.trim().to_owned()));
    }

    turns
}

/// Convert an RTF file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"rtf_corpus"` as the corpus name.
pub fn rtf_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    rtf_to_chat_with_options(content, "eng", "rtf_corpus")
}

/// Convert an RTF file to CHAT format with custom options.
pub fn rtf_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let plain = strip_rtf(content);
    let turns = extract_turns(&plain);

    let lang = LanguageCode::new(language);

    // Collect unique speakers
    let mut speaker_set: Vec<String> = Vec::new();
    for (spk, _) in &turns {
        if !speaker_set.contains(spk) {
            speaker_set.push(spk.clone());
        }
    }
    if speaker_set.is_empty() {
        speaker_set.push("SPK".to_owned());
    }

    let participant_entries: Vec<ParticipantEntry> = speaker_set
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

    for s in &speaker_set {
        lines.push(Line::header(Header::ID(
            IDHeader::new(
                lang.clone(),
                SpeakerCode::new(s),
                ParticipantRole::new("Unidentified"),
            )
            .with_corpus(corpus),
        )));
    }

    for (spk, text) in &turns {
        let words: Vec<UtteranceContent> = text
            .split_whitespace()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let main_tier = MainTier::new(
            SpeakerCode::new(spk),
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
    fn strip_rtf_basic() {
        let rtf = r"{\rtf1\ansi Hello world\par Goodbye}";
        let text = strip_rtf(rtf);
        assert!(text.contains("Hello world"));
        assert!(text.contains("Goodbye"));
    }

    #[test]
    fn strip_rtf_escaped_chars() {
        let rtf = r"{\rtf1 curly \{ brace \}}";
        let text = strip_rtf(rtf);
        assert!(text.contains("curly { brace }"));
    }

    #[test]
    fn extract_turns_with_speakers() {
        let text = "*CHI:\thello\n*MOT:\thi there\n";
        let turns = extract_turns(text);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].0, "CHI");
        assert_eq!(turns[0].1, "hello");
    }

    #[test]
    fn rtf_to_chat_basic() {
        let rtf = r"{\rtf1\ansi *CHI: hello world\par *MOT: goodbye}";
        let chat = rtf_to_chat(rtf).unwrap();
        let output = chat.to_string();
        assert!(output.contains("@UTF8"));
    }
}
