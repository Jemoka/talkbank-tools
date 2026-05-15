// `writeln!` to `String` is infallible. See the same comment in
// `praat2chat.rs` for the convention.
#![allow(clippy::unwrap_used)]

//! CHAT to SRT/WebVTT subtitle conversion.
//!
//! Converts CHAT files to SRT (SubRip) or WebVTT subtitle format, using
//! timing bullets from utterances to generate subtitle timestamps. Utterances
//! without timing bullets are skipped. Speaker codes, CHAT annotations
//! (brackets, postcodes), terminators, fillers, and fragments are stripped
//! from the subtitle text.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409296)
//! for the original CHAT2SRT command documentation.
//!
//! # Output formats
//!
//! - [`chat_to_srt()`] -- SRT format with `HH:MM:SS,mmm` timestamps
//! - [`chat_to_vtt()`] -- WebVTT format with `HH:MM:SS.mmm` timestamps
//!
//! # Differences from CLAN
//!
//! - Reads CHAT via typed AST, so subtitle text extraction is based on
//!   structured content nodes rather than regex/string stripping.
//! - Strips CHAT annotations (brackets, postcodes, fillers, fragments)
//!   by walking AST content variants, not by pattern-matching raw text.
//! - Supports WebVTT output in addition to SRT.

use std::fmt::Write;

use talkbank_model::{ChatFile, Line};

use crate::framework::{TransformError, spoken_main_text};

/// Format milliseconds as SRT timestamp: "HH:MM:SS,mmm".
fn format_srt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// Convert a CHAT file to SRT subtitle format.
///
/// Each utterance with timing bullets becomes a subtitle entry.
/// Utterances without timing are skipped.
pub fn chat_to_srt(chat: &ChatFile) -> Result<String, TransformError> {
    let mut output = String::new();
    let mut index = 1u64;

    for line in chat.lines.iter() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        let Some(ref bullet) = utt.main.content.bullet else {
            continue;
        };

        let start = format_srt_timestamp(bullet.timing.start_ms);
        let end = format_srt_timestamp(bullet.timing.end_ms);

        let text = spoken_main_text(&utt.main);

        if text.is_empty() {
            continue;
        }

        writeln!(output, "{index}").unwrap();
        writeln!(output, "{start} --> {end}").unwrap();
        writeln!(output, "{text}").unwrap();
        writeln!(output).unwrap();
        index += 1;
    }

    Ok(output)
}

/// Convert a CHAT file to WebVTT subtitle format.
///
/// Similar to [`chat_to_srt()`] but uses `HH:MM:SS.mmm` timestamps (period
/// separator) and includes the `WEBVTT` header. Utterances without timing
/// bullets are skipped.
pub fn chat_to_vtt(chat: &ChatFile) -> Result<String, TransformError> {
    let mut output = String::from("WEBVTT\n\n");
    let mut index = 1u64;

    for line in chat.lines.iter() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        let Some(ref bullet) = utt.main.content.bullet else {
            continue;
        };

        let start = format_vtt_timestamp(bullet.timing.start_ms);
        let end = format_vtt_timestamp(bullet.timing.end_ms);

        let text = spoken_main_text(&utt.main);

        if text.is_empty() {
            continue;
        }

        writeln!(output, "{index}").unwrap();
        writeln!(output, "{start} --> {end}").unwrap();
        writeln!(output, "{text}").unwrap();
        writeln!(output).unwrap();
        index += 1;
    }

    Ok(output)
}

/// Format milliseconds as WebVTT timestamp: "HH:MM:SS.mmm".
fn format_vtt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Utterance;

    fn first_utterance(chat: &ChatFile) -> &Utterance {
        chat.lines
            .iter()
            .find_map(|line| match line {
                Line::Utterance(utt) => Some(utt),
                _ => None,
            })
            .expect("expected utterance")
    }

    #[test]
    fn format_srt_timestamp_basic() {
        assert_eq!(format_srt_timestamp(0), "00:00:00,000");
        assert_eq!(format_srt_timestamp(83456), "00:01:23,456");
        assert_eq!(format_srt_timestamp(3_600_000), "01:00:00,000");
    }

    #[test]
    fn format_vtt_timestamp_basic() {
        assert_eq!(format_vtt_timestamp(0), "00:00:00.000");
        assert_eq!(format_vtt_timestamp(83456), "00:01:23.456");
    }

    #[test]
    fn spoken_text_basic() {
        let chat = crate::converters::text2chat::text_to_chat("hello world.").unwrap();
        let utt = first_utterance(&chat);
        let text = spoken_main_text(&utt.main);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn spoken_text_with_annotations() {
        let chat = talkbank_transform::parse_and_validate(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Participant\n@ID:\teng|corpus|CHI|||||Participant|||\n*CHI:\tI want [/] want that .\n@End\n",
            talkbank_model::ParseValidateOptions::default(),
        )
        .unwrap();
        let utt = first_utterance(&chat);
        let text = spoken_main_text(&utt.main);
        assert_eq!(text, "I want want that");
    }
}
