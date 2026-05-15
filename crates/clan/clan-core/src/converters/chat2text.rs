// `writeln!` to `String` is infallible. See `praat2chat.rs` for
// the convention.
#![allow(clippy::unwrap_used)]

//! CHAT to plain text conversion.
//!
//! Converts CHAT files to plain text by extracting the spoken content from
//! each utterance, stripping CHAT annotations, speaker codes, timing bullets,
//! postcodes, terminators, fillers, fragments, and dependent tiers.
//!
//! # CLAN Equivalence
//!
//! | CLAN command | Rust equivalent |
//! |---|---|
//! | N/A (no direct CLAN equivalent) | `chatter clan chat2text file.cha` |
//!
//! # Differences from CLAN
//!
//! - CLAN does not have a dedicated `chat2text` command. Researchers typically
//!   use shell tools (`grep`, `sed`) to strip CHAT formatting.
//! - Uses AST-based content extraction for reliable annotation stripping.
//! - Optionally includes speaker labels on each line.

use std::fmt::Write;

use talkbank_model::{ChatFile, Line};

use crate::framework::{TransformError, spoken_main_text};

/// Options for CHAT to text conversion.
#[derive(Debug, Clone, Default)]
pub struct Chat2TextOptions {
    /// Include speaker code prefix on each line (e.g., "CHI: hello world").
    pub include_speaker: bool,
}

/// Convert a CHAT file to plain text.
///
/// Extracts spoken content from each utterance, stripping all CHAT-specific
/// formatting (annotations, timing, postcodes, terminators).
pub fn chat_to_text(chat: &ChatFile) -> Result<String, TransformError> {
    chat_to_text_with_options(chat, &Chat2TextOptions::default())
}

/// Convert a CHAT file to plain text with options.
pub fn chat_to_text_with_options(
    chat: &ChatFile,
    options: &Chat2TextOptions,
) -> Result<String, TransformError> {
    let mut output = String::new();

    for line in chat.lines.iter() {
        let Line::Utterance(utt) = line else {
            continue;
        };

        let text = spoken_main_text(&utt.main);

        if text.is_empty() {
            continue;
        }

        if options.include_speaker {
            let speaker = &utt.main.speaker;
            writeln!(output, "{speaker}: {text}").unwrap();
        } else {
            writeln!(output, "{text}").unwrap();
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{Line, Utterance};

    fn first_utterance(chat: &talkbank_model::ChatFile) -> &Utterance {
        chat.lines
            .iter()
            .find_map(|line| match line {
                Line::Utterance(utt) => Some(utt),
                _ => None,
            })
            .expect("expected utterance")
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

    #[test]
    fn spoken_text_strips_fillers() {
        let chat = talkbank_transform::parse_and_validate(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Participant\n@ID:\teng|corpus|CHI|||||Participant|||\n*CHI:\t&-um I want that .\n@End\n",
            talkbank_model::ParseValidateOptions::default(),
        )
        .unwrap();
        let utt = first_utterance(&chat);
        let text = spoken_main_text(&utt.main);
        assert_eq!(text, "I want that");
    }

    #[test]
    fn spoken_text_strips_events() {
        let chat = talkbank_transform::parse_and_validate(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Participant\n@ID:\teng|corpus|CHI|||||Participant|||\n*CHI:\thello &=laughs world .\n@End\n",
            talkbank_model::ParseValidateOptions::default(),
        )
        .unwrap();
        let utt = first_utterance(&chat);
        let text = spoken_main_text(&utt.main);
        assert_eq!(text, "hello world");
    }

    #[test]
    fn spoken_text_strips_untranscribed() {
        let chat = talkbank_transform::parse_and_validate(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Participant\n@ID:\teng|corpus|CHI|||||Participant|||\n*CHI:\txxx .\n@End\n",
            talkbank_model::ParseValidateOptions::default(),
        )
        .unwrap();
        let utt = first_utterance(&chat);
        let text = spoken_main_text(&utt.main);
        assert_eq!(text, "");
    }
}
