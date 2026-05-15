//! Related-information spans for context-rich diagnostics.
//!
//! Maps validation errors to secondary locations that help the user understand
//! the root cause — for example, pointing to the `@Participants` header when
//! an undefined speaker code is used, or to the `%mor` tier when alignment
//! counts mismatch the main tier.

use talkbank_model::Span;
use talkbank_model::model::{ChatFile, Header, Line};
use talkbank_model::{ErrorCode, ParseError};
use tower_lsp::lsp_types::*;

use crate::backend::utils;

/// Compute related information for diagnostics based on error code.
pub fn compute_related_information(
    error: &ParseError,
    text: &str,
    uri: Option<&Url>,
    chat_file: Option<&ChatFile>,
    _error_range: Range,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let uri = uri?;
    let chat_file = chat_file?;

    let header_span = match error.code {
        ErrorCode::SpeakerNotDefined => find_header_span(chat_file, |header| {
            matches!(header, Header::Participants { .. })
        }),
        ErrorCode::MediaFilenameMismatch => {
            find_header_span(chat_file, |header| matches!(header, Header::Media(_)))
        }
        ErrorCode::InvalidLanguageCode => find_header_span(chat_file, |header| {
            matches!(header, Header::Languages { .. })
        }),
        _ => None,
    };

    if let Some(span) = header_span {
        let range = span_to_range(text, span);
        let message = match error.code {
            ErrorCode::SpeakerNotDefined => {
                "Speakers must be declared in @Participants header".to_string()
            }
            ErrorCode::MediaFilenameMismatch => {
                "Media filename should match @Media header".to_string()
            }
            ErrorCode::InvalidLanguageCode => {
                "Language codes should use ISO 639-3 standard".to_string()
            }
            _ => String::new(),
        };

        if !message.is_empty() {
            return Some(vec![DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                message,
            }]);
        }
    }

    // Fallback: use error suggestion if available
    error.suggestion.as_ref().map(|suggestion| {
        vec![DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
            },
            message: suggestion.clone(),
        }]
    })
}

/// Finds header span.
fn find_header_span(chat_file: &ChatFile, predicate: impl Fn(&Header) -> bool) -> Option<Span> {
    for line in &chat_file.lines {
        if let Line::Header { header, span } = line
            && predicate(header)
        {
            return Some(*span);
        }
    }
    None
}

/// Convert a byte-span into an LSP range using document offsets.
fn span_to_range(text: &str, span: Span) -> Range {
    Range {
        start: utils::offset_to_position(text, span.start),
        end: utils::offset_to_position(text, span.end),
    }
}
