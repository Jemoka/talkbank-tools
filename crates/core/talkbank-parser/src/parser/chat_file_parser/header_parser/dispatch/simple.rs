//! Parsing for scalar/textual header forms.
//!
//! These headers can be decoded from a single content field without invoking
//! multi-node structural parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>

use crate::error::ErrorSink;
use crate::model::{self, Header};
use crate::node_types::{
    ACTIVITIES_HEADER, BCK_HEADER, COLOR_WORDS_HEADER, DATE_CONTENTS, DATE_HEADER, FONT_HEADER,
    FREE_TEXT, LOCATION_HEADER, PAGE_HEADER, PAGE_NUMBER, ROOM_LAYOUT_HEADER, T_HEADER,
    TAPE_LOCATION_HEADER, TIME_DURATION_CONTENTS, TIME_DURATION_HEADER, TIME_START_HEADER,
    TRANSCRIBER_HEADER, UNSUPPORTED_HEADER, VIDEOS_HEADER, WARNING_HEADER, WINDOW_HEADER,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::helpers::get_required_content_by_kind;

/// Build `Header::Unknown` from malformed simple-header input.
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

/// Parse a simple header into a concrete model variant.
pub(super) fn parse_simple_header(
    header_kind: &str,
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match header_kind {
        DATE_HEADER => {
            // Grammar: date_contents = choice(strict_date, generic_date).
            // Both alternatives are wrapped in the date_contents node; we extract
            // the text and let ChatDate::from_text() classify or mark Unsupported.
            // Validator reports E518 for malformed dates.
            let Some(content) = get_required_content_by_kind(
                header_actual,
                input,
                DATE_CONTENTS,
                errors,
                DATE_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Date content",
                    None,
                ));
            };
            Some(Header::Date {
                date: model::ChatDate::new(content),
            })
        }
        WINDOW_HEADER => {
            // Grammar: seq(prefix, header_sep, window_geometry, newline)
            // Child layout: [0]=prefix, [1]=sep, [2]=geometry, [3]=newline
            // Content at position 2
            let geometry = match header_actual.child(2u32) {
                Some(child) => match child.utf8_text(input.as_bytes()) {
                    Ok(text) => text,
                    Err(_) => {
                        return ParseOutcome::parsed(unknown_header_from_node(
                            header_actual,
                            input,
                            "Failed to decode @Window geometry as UTF-8",
                            None,
                        ));
                    }
                },
                None => {
                    return ParseOutcome::parsed(unknown_header_from_node(
                        header_actual,
                        input,
                        "Missing @Window geometry",
                        None,
                    ));
                }
            };
            Some(Header::Window {
                geometry: model::WindowGeometry::new(geometry),
            })
        }
        COLOR_WORDS_HEADER => {
            let Some(colors) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                COLOR_WORDS_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Color words content",
                    None,
                ));
            };
            Some(Header::ColorWords {
                colors: model::ColorWordList::new(colors),
            })
        }
        FONT_HEADER => {
            let Some(font) =
                get_required_content_by_kind(header_actual, input, FREE_TEXT, errors, FONT_HEADER)
                    .into_option()
            else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Font content",
                    None,
                ));
            };
            Some(Header::Font {
                font: model::FontSpec::new(font),
            })
        }
        TAPE_LOCATION_HEADER => {
            let Some(location) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                TAPE_LOCATION_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Tape Location content",
                    None,
                ));
            };
            Some(Header::TapeLocation {
                location: model::TapeLocationDescription::new(location),
            })
        }
        TIME_DURATION_HEADER => {
            // Grammar: time_duration_contents = choice(strict_time, generic_time).
            // TimeDurationValue::from_text() classifies or marks Unsupported.
            // Validator reports E541 for malformed durations.
            let Some(duration) = get_required_content_by_kind(
                header_actual,
                input,
                TIME_DURATION_CONTENTS,
                errors,
                TIME_DURATION_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Time Duration content",
                    None,
                ));
            };
            Some(Header::TimeDuration {
                duration: model::TimeDurationValue::new(duration),
            })
        }
        TIME_START_HEADER => {
            // Same grammar pattern as TIME_DURATION_HEADER.
            // Validator reports E542 for malformed start times.
            let Some(start) = get_required_content_by_kind(
                header_actual,
                input,
                TIME_DURATION_CONTENTS,
                errors,
                TIME_START_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Time Start content",
                    None,
                ));
            };
            Some(Header::TimeStart {
                start: model::TimeStartValue::new(start),
            })
        }
        LOCATION_HEADER => {
            let Some(location) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                LOCATION_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Location content",
                    None,
                ));
            };
            Some(Header::Location {
                location: model::LocationDescription::new(location),
            })
        }
        ROOM_LAYOUT_HEADER => {
            let Some(layout) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                ROOM_LAYOUT_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Room Layout content",
                    None,
                ));
            };
            Some(Header::RoomLayout {
                layout: model::RoomLayoutDescription::new(layout),
            })
        }
        TRANSCRIBER_HEADER => {
            let Some(transcriber) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                TRANSCRIBER_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Transcriber content",
                    None,
                ));
            };
            Some(Header::Transcriber {
                transcriber: model::TranscriberName::new(transcriber),
            })
        }
        WARNING_HEADER => {
            let Some(text) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                WARNING_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Warning content",
                    None,
                ));
            };
            Some(Header::Warning {
                text: model::WarningText::new(text),
            })
        }
        ACTIVITIES_HEADER => {
            let Some(activities) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                ACTIVITIES_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Activities content",
                    None,
                ));
            };
            Some(Header::Activities {
                activities: model::ActivitiesDescription::new(activities),
            })
        }
        BCK_HEADER => {
            let Some(bck) =
                get_required_content_by_kind(header_actual, input, FREE_TEXT, errors, BCK_HEADER)
                    .into_option()
            else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Bck content",
                    None,
                ));
            };
            Some(Header::Bck {
                bck: model::BackgroundDescription::new(bck),
            })
        }
        PAGE_HEADER => {
            let Some(page) = get_required_content_by_kind(
                header_actual,
                input,
                PAGE_NUMBER,
                errors,
                PAGE_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Page number",
                    None,
                ));
            };
            Some(Header::Page {
                page: model::PageNumber::new(page),
            })
        }
        VIDEOS_HEADER => {
            let Some(videos) = get_required_content_by_kind(
                header_actual,
                input,
                FREE_TEXT,
                errors,
                VIDEOS_HEADER,
            )
            .into_option() else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @Videos content",
                    None,
                ));
            };
            Some(Header::Videos {
                videos: model::VideoSpec::new(videos),
            })
        }
        T_HEADER => {
            let Some(text) =
                get_required_content_by_kind(header_actual, input, FREE_TEXT, errors, T_HEADER)
                    .into_option()
            else {
                return ParseOutcome::parsed(unknown_header_from_node(
                    header_actual,
                    input,
                    "Missing @T content",
                    None,
                ));
            };
            Some(Header::T {
                text: model::TDescription::new(text),
            })
        }
        UNSUPPORTED_HEADER => {
            // Catch-all for unknown @-headers that the grammar matched structurally.
            // Convert to Header::Unknown so the file can still be parsed.
            Some(unknown_header_from_node(
                header_actual,
                input,
                "Unsupported header type",
                None,
            ))
        }
        _ => None,
    }
    .into()
}
