//! Parsing for structured headers with dedicated sub-parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use crate::error::{ErrorCollector, ErrorSink};
use crate::model::Header;
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use crate::parser::tree_parsing::header::{
    parse_id_header, parse_languages_header, parse_media_header, parse_participants_header,
    parse_pid_header, parse_situation_header, parse_types_header,
};

/// Parse structured headers and forward sub-parser diagnostics to `errors`.
pub(super) fn parse_structured_header(
    header_kind: &str,
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    match header_kind {
        LANGUAGES_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_languages_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        PARTICIPANTS_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_participants_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        ID_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_id_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        MEDIA_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_media_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        SITUATION_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_situation_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        TYPES_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_types_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        PID_HEADER => {
            let header_errors = ErrorCollector::new();
            let header = parse_pid_header(header_actual, input, &header_errors);
            errors.report_all(header_errors.into_vec());
            Some(header)
        }
        _ => None,
    }
    .into()
}
