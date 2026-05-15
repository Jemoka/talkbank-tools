//! Header parsing dispatch from tree-sitter nodes to strongly-typed `Header` values.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use super::super::header_parser::helpers::parse_optional_gem_label;
use super::finder::find_header_node_in_tree;
use crate::error::{
    ErrorCode, ErrorCollector, ErrorContext, ErrorSink, ParseError, ParseErrors, ParseResult,
    Severity, SourceLocation,
};
use crate::model::{ChatDate, Header, TapeLocationDescription, WarningText};
use crate::node_types::*;
use crate::parser::TreeSitterParser;
use tree_sitter::Node;

use crate::parser::tree_parsing::header::{
    parse_id_header, parse_languages_header, parse_media_header, parse_participants_header,
    parse_pid_header, parse_situation_header, parse_t_header, parse_types_header,
};
use talkbank_model::ParseOutcome;

impl TreeSitterParser {
    /// Parse one header line in isolation using a minimal wrapper CHAT document.
    ///
    /// Because tree-sitter requires a complete CHAT document for context, this method
    /// wraps the input in two synthetic documents (pre-`@Begin` and post-`@Begin`
    /// positions) and attempts to parse from each. Structural headers (`@UTF8`,
    /// `@Begin`, `@End`, `@New Episode`, `@Blank`) are recognized on a fast path
    /// without wrapping.
    ///
    /// # Parameters
    ///
    /// - `input`: A single CHAT header line, e.g., `@Languages:\teng`,
    ///   `@Participants:\tCHI Target_Child`, or `@Date:\t01-JAN-2020`.
    ///
    /// # Returns
    ///
    /// A strongly-typed `Header` enum variant corresponding to the parsed header.
    ///
    /// # Errors
    ///
    /// Returns `ParseErrors` when:
    /// - Tree-sitter fails to produce a parse tree for either wrapper.
    /// - The header node falls outside the input byte range (detected as a wrapper
    ///   artifact rather than the user's header).
    /// - The header CST node is malformed or has an unrecognized kind.
    pub fn parse_header(&self, input: &str) -> ParseResult<Header> {
        // Fast path for structural headers that can't be wrapped without
        // colliding with the wrapper's own structural headers
        let trimmed = input.trim();
        match trimmed {
            "@UTF8" => return Ok(Header::Utf8),
            "@Begin" => return Ok(Header::Begin),
            "@End" => return Ok(Header::End),
            "@New Episode" => return Ok(Header::NewEpisode),
            "@Blank" => return Ok(Header::Blank),
            _ => {}
        }

        const PRE_BEGIN_PREFIX: &str = "@UTF8\n";
        const PRE_BEGIN_SUFFIX: &str = "@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@End\n";
        const POST_BEGIN_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
        const POST_BEGIN_SUFFIX: &str = "@End\n";

        let pre_begin_wrapped = format!("{}{}\n{}", PRE_BEGIN_PREFIX, input, PRE_BEGIN_SUFFIX);
        let post_begin_wrapped = format!("{}{}\n{}", POST_BEGIN_PREFIX, input, POST_BEGIN_SUFFIX);

        let try_parse = |wrapped: &str,
                         header_index: usize,
                         input_offset: usize|
         -> ParseResult<Header> {
            let tree = self
                .parser
                .borrow_mut()
                .parse(wrapped, None)
                .ok_or_else(|| {
                    let mut errors = ParseErrors::new();
                    errors.push(
                        ParseError::new(
                            ErrorCode::TierValidationError,
                            Severity::Error,
                            SourceLocation::from_offsets(0, input.len()),
                            ErrorContext::new(input, 0..input.len(), "header"),
                            "Tier validation error: tree-sitter could not parse this header line",
                        )
                        .with_suggestion("Check that the header line follows CHAT format (e.g., @Header:<TAB>value)"),
                    );
                    errors
                })?;

            let ts_root = tree.root_node();
            // Navigate source_file → full_document for multi-root grammar
            let root = if ts_root.kind() == "source_file" {
                ts_root
                    .child(0)
                    .filter(|c| c.kind() == "full_document")
                    .unwrap_or(ts_root)
            } else {
                ts_root
            };
            let header_node = find_header_node_in_tree(root, header_index)?;

            // Verify the found header node is within the input's byte range,
            // not from the wrapper prefix/suffix. Without this check, when the
            // input header parses as ERROR (e.g. @Participants before @Begin),
            // the finder returns a wrapper header at the same index.
            let input_end = input_offset + input.len();
            if header_node.start_byte() < input_offset || header_node.start_byte() >= input_end {
                let mut errors = ParseErrors::new();
                errors.push(
                    ParseError::new(
                        ErrorCode::TierValidationError,
                        Severity::Error,
                        SourceLocation::from_offsets(0, input.len()),
                        ErrorContext::new(input, 0..input.len(), "header"),
                        "Tier validation error: header node resolved outside input range (wrapper artifact)",
                    )
                    .with_suggestion("Check header formatting — the line may be malformed or in the wrong position"),
                );
                return Err(errors);
            }

            // Dispatch to appropriate header parser
            // Use OffsetAdjustingErrorSink to ensure errors are relative to input, not wrapper
            use crate::error::OffsetAdjustingErrorSink;
            let inner_sink = ErrorCollector::new();
            let error_sink = OffsetAdjustingErrorSink::new(&inner_sink, input_offset, input);
            let header = if header_node.is_error() {
                error_sink.report(ParseError::new(
                    ErrorCode::MalformedWordContent,
                    Severity::Error,
                    SourceLocation::from_offsets(header_node.start_byte(), header_node.end_byte()),
                    ErrorContext::new(
                        wrapped,
                        header_node.start_byte()..header_node.end_byte(),
                        "",
                    ),
                    format!(
                        "Malformed header at byte {}..{}",
                        header_node.start_byte(),
                        header_node.end_byte()
                    ),
                ));
                let text = match header_node.utf8_text(wrapped.as_bytes()) {
                    Ok(text) => text.to_string(),
                    Err(_) => header_node.kind().to_string(),
                };
                Header::Unknown {
                    text: WarningText::new(text),
                    parse_reason: Some("Malformed header content".to_string()),
                    suggested_fix: None,
                }
            } else {
                match header_node.kind() {
                    UTF8_HEADER => Header::Utf8,
                    BEGIN_HEADER => Header::Begin,
                    END_HEADER => Header::End,
                    LANGUAGES_HEADER => parse_languages_header(header_node, wrapped, &error_sink),
                    PARTICIPANTS_HEADER => {
                        parse_participants_header(header_node, wrapped, &error_sink)
                    }
                    ID_HEADER => parse_id_header(header_node, wrapped, &error_sink),
                    COMMENT_HEADER => {
                        use crate::parser::tree_parsing::bullet_content::parse_bullet_content;

                        // Grammar: seq(prefix, header_sep, text_with_bullets_and_pics, newline)
                        let content =
                            match find_child_by_kind(header_node, TEXT_WITH_BULLETS_AND_PICS) {
                                Some(content_node) => {
                                    parse_bullet_content(content_node, wrapped, &error_sink)
                                }
                                None => {
                                    error_sink.report(ParseError::new(
                                        ErrorCode::TreeParsingError,
                                        Severity::Error,
                                        SourceLocation::from_offsets(
                                            header_node.start_byte(),
                                            header_node.end_byte(),
                                        ),
                                        ErrorContext::new(
                                            wrapped,
                                            header_node.start_byte()..header_node.end_byte(),
                                            COMMENT_HEADER,
                                        ),
                                        "Missing comment content".to_string(),
                                    ));
                                    return Ok(unknown_header_with_reason(
                                        header_node,
                                        wrapped,
                                        "Missing comment content",
                                        None,
                                    ));
                                }
                            };
                        Header::Comment { content }
                    }
                    DATE_HEADER => {
                        // Grammar: seq(prefix, header_sep, date_contents, newline)
                        let Some(content) = find_child_text(header_node, wrapped, DATE_CONTENTS)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    DATE_HEADER,
                                ),
                                "Missing @Date content",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing @Date content",
                                None,
                            ));
                        };
                        Header::Date {
                            date: ChatDate::new(content),
                        }
                    }
                    PID_HEADER => parse_pid_header(header_node, wrapped, &error_sink),
                    MEDIA_HEADER => parse_media_header(header_node, wrapped, &error_sink),
                    SITUATION_HEADER => parse_situation_header(header_node, wrapped, &error_sink),
                    BIRTH_OF_HEADER => {
                        let Some(participant) = find_child_text(header_node, wrapped, SPEAKER)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    BIRTH_OF_HEADER,
                                ),
                                "Missing participant code in @Birth of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing participant code in @Birth of header",
                                None,
                            ));
                        };
                        let Some(date) = find_child_text(header_node, wrapped, DATE_CONTENTS)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    BIRTH_OF_HEADER,
                                ),
                                "Missing date value in @Birth of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing date value in @Birth of header",
                                None,
                            ));
                        };
                        Header::Birth {
                            participant: crate::model::SpeakerCode::new(participant),
                            date: ChatDate::new(date),
                        }
                    }
                    BIRTHPLACE_OF_HEADER => {
                        let Some(participant) = find_child_text(header_node, wrapped, SPEAKER)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    BIRTHPLACE_OF_HEADER,
                                ),
                                "Missing participant code in @Birthplace of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing participant code in @Birthplace of header",
                                None,
                            ));
                        };
                        let Some(place) = find_child_text(header_node, wrapped, FREE_TEXT) else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    BIRTHPLACE_OF_HEADER,
                                ),
                                "Missing place value in @Birthplace of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing place value in @Birthplace of header",
                                None,
                            ));
                        };
                        Header::Birthplace {
                            participant: crate::model::SpeakerCode::new(participant),
                            place: crate::model::BirthplaceDescription::new(place),
                        }
                    }
                    L1_OF_HEADER => {
                        let Some(participant) = find_child_text(header_node, wrapped, SPEAKER)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    L1_OF_HEADER,
                                ),
                                "Missing participant code in @L1 of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing participant code in @L1 of header",
                                None,
                            ));
                        };
                        let Some(language) = find_child_text(header_node, wrapped, LANGUAGE_CODE)
                        else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    L1_OF_HEADER,
                                ),
                                "Missing language value in @L1 of header",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing language value in @L1 of header",
                                None,
                            ));
                        };
                        Header::L1Of {
                            participant: crate::model::SpeakerCode::new(participant),
                            language: crate::model::LanguageName::new(language),
                        }
                    }
                    WARNING_HEADER => {
                        let Some(content) = find_child_text(header_node, wrapped, FREE_TEXT) else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    WARNING_HEADER,
                                ),
                                "Missing @Warning content",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing @Warning content",
                                None,
                            ));
                        };
                        Header::Warning {
                            text: WarningText::new(content),
                        }
                    }
                    VIDEOS_HEADER => {
                        let Some(content) = find_child_text(header_node, wrapped, FREE_TEXT) else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    VIDEOS_HEADER,
                                ),
                                "Missing @Videos content",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing @Videos content",
                                None,
                            ));
                        };
                        Header::Videos {
                            videos: crate::model::VideoSpec::new(content),
                        }
                    }
                    NEW_EPISODE_HEADER => Header::NewEpisode,
                    BG_HEADER => {
                        let label = match parse_optional_gem_label(
                            find_child_by_kind(header_node, FREE_TEXT),
                            wrapped,
                            &error_sink,
                        ) {
                            ParseOutcome::Parsed(label) => label,
                            ParseOutcome::Rejected => None,
                        };
                        Header::BeginGem { label }
                    }
                    EG_HEADER => {
                        let label = match parse_optional_gem_label(
                            find_child_by_kind(header_node, FREE_TEXT),
                            wrapped,
                            &error_sink,
                        ) {
                            ParseOutcome::Parsed(label) => label,
                            ParseOutcome::Rejected => None,
                        };
                        Header::EndGem { label }
                    }
                    BLANK_HEADER => Header::Blank,
                    TAPE_LOCATION_HEADER => {
                        let Some(content) = find_child_text(header_node, wrapped, FREE_TEXT) else {
                            error_sink.report(ParseError::new(
                                ErrorCode::TreeParsingError,
                                Severity::Error,
                                SourceLocation::from_offsets(
                                    header_node.start_byte(),
                                    header_node.end_byte(),
                                ),
                                ErrorContext::new(
                                    wrapped,
                                    header_node.start_byte()..header_node.end_byte(),
                                    TAPE_LOCATION_HEADER,
                                ),
                                "Missing @Tape Location content",
                            ));
                            return Ok(unknown_header_with_reason(
                                header_node,
                                wrapped,
                                "Missing @Tape Location content",
                                None,
                            ));
                        };
                        Header::TapeLocation {
                            location: TapeLocationDescription::new(content),
                        }
                    }
                    TYPES_HEADER => parse_types_header(header_node, wrapped, &error_sink),
                    T_HEADER => parse_t_header(header_node, wrapped, &error_sink),
                    unknown => {
                        error_sink.report(ParseError::new(
                            ErrorCode::MalformedTierContent,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                header_node.start_byte(),
                                header_node.end_byte(),
                            ),
                            ErrorContext::new(
                                wrapped,
                                header_node.start_byte()..header_node.end_byte(),
                                "",
                            ),
                            format!(
                                "Unknown header type '{}' - will be flagged during validation",
                                unknown
                            ),
                        ));
                        unknown_header_with_reason(
                            header_node,
                            wrapped,
                            format!("Unrecognized header type: {}", unknown),
                            None,
                        )
                    }
                }
            };

            let parse_errors = inner_sink.into_vec();
            if !parse_errors.is_empty() {
                let mut errors = ParseErrors::new();
                errors.errors.extend(parse_errors);
                return Err(errors);
            }

            Ok(header)
        };

        let pre_begin_attempt = try_parse(&pre_begin_wrapped, 1, PRE_BEGIN_PREFIX.len());
        if pre_begin_attempt.is_ok() {
            return pre_begin_attempt;
        }

        let post_begin_attempt = try_parse(&post_begin_wrapped, 5, POST_BEGIN_PREFIX.len());
        match (pre_begin_attempt, post_begin_attempt) {
            (Ok(header), _) => Ok(header),
            (Err(_), Ok(header)) => Ok(header),
            (Err(pre_err), Err(post_err)) => {
                if post_err.len() <= pre_err.len() {
                    Err(post_err)
                } else {
                    Err(pre_err)
                }
            }
        }
    }
}

/// Return UTF-8 text for the first child node of `kind`.
fn find_child_text(node: Node, input: &str, kind: &str) -> Option<String> {
    match find_child_by_kind(node, kind) {
        Some(child) => match child.utf8_text(input.as_bytes()) {
            Ok(text) => Some(text.to_string()),
            Err(_) => None,
        },
        None => None,
    }
}

/// Build a `Header::Unknown` while preserving source text and parse reason.
fn unknown_header_with_reason(
    node: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    let text = match node.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => node.kind().to_string(),
    };

    Header::Unknown {
        text: WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

/// Return the first direct child with the requested `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
