//! Parsing for option-driven and mixed-shape headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use crate::error::ErrorSink;
use crate::model::{self, Header};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::helpers::{
    find_child_by_kind, get_required_content_by_kind, parse_options_flags,
};

/// Construct a best-effort `Header::Unknown` when a special header fails to parse.
fn unknown_header_from_node(
    header_actual: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    let text = match header_actual.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => header_actual.kind().to_string(),
    };

    Header::Unknown {
        text: model::WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

/// Parse headers whose structure deviates from the simple `@xxx value` shape.
///
/// Special headers such as `@Comment`, `@Number`, `@Recording Quality`, and `@Transcription`
/// expose structured child nodes. All enum values are accepted via `from_text()` — the
/// validator flags unsupported values.
pub(super) fn parse_special_header(
    header_kind: &str,
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match header_kind {
        COMMENT_HEADER => {
            let Some(content_node) = find_child_by_kind(header_actual, TEXT_WITH_BULLETS_AND_PICS)
            else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing comment content",
                    None,
                ));
            };
            Some(Header::Comment {
                content: parse_bullet_content(content_node, input, errors),
            })
        }
        NUMBER_HEADER => {
            let Some(option_text) = get_required_content_by_kind(
                header_actual,
                input,
                NUMBER_OPTION,
                errors,
                NUMBER_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Number option",
                    Some("Use @Number:\t1|2|3|4|5|more|audience"),
                ));
            };
            // All values accepted; unsupported ones flagged by the validator.
            let number = talkbank_model::model::Number::from_text(&option_text);
            Some(Header::Number { number })
        }
        RECORDING_QUALITY_HEADER => {
            let Some(option_text) = get_required_content_by_kind(
                header_actual,
                input,
                RECORDING_QUALITY_OPTION,
                errors,
                RECORDING_QUALITY_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Recording Quality option",
                    Some("Use @Recording Quality:\t1|2|3|4|5"),
                ));
            };
            // All values accepted; unsupported ones flagged by the validator.
            let quality = talkbank_model::model::RecordingQuality::from_text(&option_text);
            Some(Header::RecordingQuality { quality })
        }
        TRANSCRIPTION_HEADER => {
            let Some(option_text) = get_required_content_by_kind(
                header_actual,
                input,
                TRANSCRIPTION_OPTION,
                errors,
                TRANSCRIPTION_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Transcription option",
                    Some("Use a valid @Transcription option value"),
                ));
            };
            // All values accepted; unsupported ones flagged by the validator.
            let transcription = talkbank_model::model::Transcription::from_text(&option_text);
            Some(Header::Transcription { transcription })
        }
        BIRTH_OF_HEADER => {
            let Some(participant) = get_required_content_by_kind(
                header_actual,
                input,
                SPEAKER,
                errors,
                BIRTH_OF_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing participant code in @Birth of header",
                    None,
                ));
            };
            let Some(date) = get_required_content_by_kind(
                header_actual,
                input,
                DATE_CONTENTS,
                errors,
                BIRTH_OF_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing date value in @Birth of header",
                    None,
                ));
            };
            Some(Header::Birth {
                participant: model::SpeakerCode::new(participant),
                date: model::ChatDate::new(date),
            })
        }
        BIRTHPLACE_OF_HEADER => {
            let Some(participant) = get_required_content_by_kind(
                header_actual,
                input,
                SPEAKER,
                errors,
                BIRTHPLACE_OF_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing participant code in @Birthplace of header",
                    None,
                ));
            };
            let Some(place) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                BIRTHPLACE_OF_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing place value in @Birthplace of header",
                    None,
                ));
            };
            Some(Header::Birthplace {
                participant: model::SpeakerCode::new(participant),
                place: model::BirthplaceDescription::new(place),
            })
        }
        L1_OF_HEADER => {
            let Some(participant) =
                get_required_content_by_kind(header_actual, input, SPEAKER, errors, L1_OF_HEADER)
                    .into_option()
            else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing participant code in @L1 of header",
                    None,
                ));
            };
            let Some(language) = get_required_content_by_kind(
                header_actual,
                input,
                LANGUAGE_CODE,
                errors,
                L1_OF_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing language value in @L1 of header",
                    None,
                ));
            };
            Some(Header::L1Of {
                participant: model::SpeakerCode::new(participant),
                language: model::LanguageName::new(language),
            })
        }
        OPTIONS_HEADER => Some(Header::Options {
            options: parse_options_flags(header_actual, input, errors).into(),
        }),
        _ => None,
    }
    .into()
}
