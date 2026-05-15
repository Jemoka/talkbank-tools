//! @Participants header parsing
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>
//!
//! **Grammar Rule**:
//! ```javascript
//! participants_header: $ => seq(
//!     token('@Participants:\t'),
//!     $.participants_contents,
//!     $.newline
//! )
//!
//! participants_contents: $ => seq(
//!     $.participant,
//!     repeat(seq(',', $.whitespaces, $.participant))
//! )
//!
//! participant: $ => seq(
//!     $.speaker,
//!     repeat(seq($.whitespaces, $.participant_word))
//! )
//! ```

use crate::node_types::*;
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::parser::tree_parsing::parser_helpers::check_not_missing;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{
    Header, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode, WarningText,
};

/// Build `Header::Unknown` for malformed `@Participants` input.
fn unknown_participants_header(
    node: Node,
    source: &str,
    parse_reason: impl Into<String>,
) -> Header {
    let text = match node.utf8_text(source.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => "@Participants".to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some("Expected @Participants:\tCODE [NAME] ROLE[, ...]".to_string()),
    }
}

/// Parse Participants header from tree-sitter node
pub fn parse_participants_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is a participants_header node
    if node.kind() != PARTICIPANTS_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected participants_header node, got: {}", node.kind()),
        ));
        return unknown_participants_header(
            node,
            source,
            "Participants header CST node had unexpected kind",
        );
    }

    // Find participants_contents child (prefix + header_sep + contents + newline)
    let contents = match find_child_by_kind(node, PARTICIPANTS_CONTENTS) {
        Some(child) => child,
        _ => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantsHeader,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    "participants_header",
                ),
                "Missing participants_contents in @Participants header",
            ));
            return unknown_participants_header(
                node,
                source,
                "Missing participants_contents in @Participants header",
            );
        }
    };

    // Iterate through children to find participant nodes
    // Grammar: participant, repeat(seq(',', whitespaces, participant))
    // Position 0: participant
    // Position 1: comma, 2: whitespaces, 3: participant, etc.
    let child_count = contents.child_count();
    // Pre-allocate: typically (child_count + 2) / 3 participants (comma, whitespace, participant pattern)
    let mut entries = Vec::with_capacity(child_count.div_ceil(3));
    let mut idx = 0;

    // First participant (required)
    if idx < child_count
        && let Some(child) = contents.child(idx as u32)
    {
        // CRITICAL: Check for MISSING nodes before processing
        if !check_not_missing(child, source, errors, "participants_contents") {
            idx += 1;
        } else if child.kind() == PARTICIPANT {
            if let ParseOutcome::Parsed(entry) = parse_participant_entry(child, source, errors) {
                entries.push(entry);
            }
            idx += 1;
        } else {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantsHeader,
                Severity::Error,
                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                ErrorContext::new(
                    source,
                    child.start_byte()..child.end_byte(),
                    "participants_contents",
                ),
                format!(
                    "Expected 'participant' at position {}, got: {}",
                    idx,
                    child.kind()
                ),
            ));
            // Try to recover by skipping
            idx += 1;
        }
    }

    // Subsequent participants (optional)
    while idx < child_count {
        // Check for comma
        if let Some(child) = contents.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes
            if !check_not_missing(child, source, errors, "participants_contents") {
                idx += 1;
                continue;
            }
            if child.kind() == COMMA {
                idx += 1;
            } else {
                // If not comma, maybe end of list or unexpected
                errors.report(ParseError::new(
                    ErrorCode::EmptyParticipantsHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "participants_contents",
                    ),
                    format!("Expected ',' at position {}, got: {}", idx, child.kind()),
                ));
                idx += 1;
                continue;
            }
        } else {
            break;
        }

        // Check for whitespaces
        if let Some(child) = contents.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes
            if !check_not_missing(child, source, errors, "participants_contents") {
                idx += 1;
                continue;
            }
            if child.kind() == WHITESPACES {
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::EmptyParticipantsHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "participants_contents",
                    ),
                    format!(
                        "Expected 'whitespaces' at position {}, got: {}",
                        idx,
                        child.kind()
                    ),
                ));
                // Recover
                idx += 1;
            }
        }

        // Check for participant
        if let Some(child) = contents.child(idx as u32) {
            // CRITICAL: Check for MISSING nodes
            if !check_not_missing(child, source, errors, "participants_contents") {
                idx += 1;
                continue;
            }
            if child.kind() == PARTICIPANT {
                if let ParseOutcome::Parsed(entry) = parse_participant_entry(child, source, errors)
                {
                    entries.push(entry);
                }
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::EmptyParticipantsHeader,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(
                        source,
                        child.start_byte()..child.end_byte(),
                        "participants_contents",
                    ),
                    format!(
                        "Expected 'participant' at position {}, got: {}",
                        idx,
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        }
    }

    Header::Participants {
        entries: entries.into(),
    }
}

/// Finds child by kind.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

/// Parse a single participant entry
///
/// **Structure**: speaker [name_words...] role
fn parse_participant_entry(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParticipantEntry> {
    // Extract speaker code (first child)
    let speaker_node = match node.child(0u32) {
        Some(child) => child,
        None => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantCode,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), PARTICIPANT),
                "Participant entry missing speaker code",
            ));
            return ParseOutcome::rejected();
        }
    };

    if !check_not_missing(speaker_node, source, errors, PARTICIPANT) {
        return ParseOutcome::rejected();
    }

    let speaker_code = match speaker_node.utf8_text(source.as_bytes()) {
        Ok(text) if !text.trim().is_empty() => text.to_string(),
        Ok(_) => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantCode,
                Severity::Error,
                SourceLocation::from_offsets(speaker_node.start_byte(), speaker_node.end_byte()),
                ErrorContext::new(
                    source,
                    speaker_node.start_byte()..speaker_node.end_byte(),
                    PARTICIPANT,
                ),
                "Participant code cannot be empty",
            ));
            return ParseOutcome::rejected();
        }
        Err(_) => {
            errors.report(ParseError::new(
                ErrorCode::UnparsableContent,
                Severity::Error,
                SourceLocation::from_offsets(speaker_node.start_byte(), speaker_node.end_byte()),
                ErrorContext::new(
                    source,
                    speaker_node.start_byte()..speaker_node.end_byte(),
                    PARTICIPANT,
                ),
                "Unparsable content: participant speaker code is not valid UTF-8",
            ));
            return ParseOutcome::rejected();
        }
    };

    // Extract participant_word children (name parts and role)
    let words: Vec<String> = {
        let mut cursor = node.walk();
        let mut words = Vec::new();
        for child in node.children(&mut cursor) {
            if child.kind() != PARTICIPANT_WORD {
                continue;
            }

            if !check_not_missing(child, source, errors, PARTICIPANT_WORD) {
                continue;
            }

            match child.utf8_text(source.as_bytes()) {
                Ok(text) => words.push(text.to_string()),
                Err(_) => errors.report(ParseError::new(
                    ErrorCode::UnparsableContent,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), PARTICIPANT),
                    "Unparsable content: participant name or role token is not valid UTF-8",
                )),
            }
        }
        words
    };

    // Last word is role, previous words are name
    let role = match words.last() {
        Some(value) if !value.trim().is_empty() => value.clone(),
        _ => {
            errors.report(ParseError::new(
                ErrorCode::EmptyParticipantRole,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), PARTICIPANT),
                "Participant role cannot be empty",
            ));
            return ParseOutcome::rejected();
        }
    };
    let name = if words.len() > 1 {
        Some(ParticipantName::new(words[..words.len() - 1].join(" ")))
    } else {
        None
    };

    ParseOutcome::parsed(ParticipantEntry {
        speaker_code: SpeakerCode::new(speaker_code),
        name,
        role: ParticipantRole::new(role),
    })
}
