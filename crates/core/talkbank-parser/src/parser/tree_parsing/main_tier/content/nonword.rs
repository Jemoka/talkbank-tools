//! Nonword content parsing
//!
//! Handles the unified nonword category: events (&=action) and zero/action (0)
//! NOTE: Other spoken events (&*SPEAKER) are handled separately in base/other_spoken.rs
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{ErrorSink, Span};
use crate::model::{Action, Annotated, Event, UtteranceContent};
use crate::node_types::{BASE_ANNOTATIONS, EVENT, EVENT_SEGMENT, NONWORD, WHITESPACES, ZERO};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::annotations::parse_scoped_annotations;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::expect_child;

/// Intermediate representation of parsed nonword before converting to UtteranceContent
enum ParsedNonword {
    Event(Event, Span),
    Action(Action, Span),
}

/// Converts `nonword_with_optional_annotations` into `UtteranceContent`.
///
/// Nonwords in CHAT format:
/// - Events: &=text (e.g., &=laugh, &=cries)
/// - Action/omission: 0 (zero marker)
///   NOTE: Other spoken events (&*SPEAKER) are NOT nonwords - they're parsed separately
pub(crate) fn parse_nonword_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let child_count = node.child_count();
    let mut parsed_nonword: Option<ParsedNonword> = None;
    let mut annotations = Vec::with_capacity(2);
    let mut idx: u32 = 0;

    // Position 0: nonword (required)
    // Grammar: nonword: $ => choice($.event, $.zero)
    if let ParseOutcome::Parsed(child) = expect_child(
        node,
        idx,
        NONWORD,
        source,
        errors,
        "nonword_with_optional_annotations",
    ) {
        // Determine which type of nonword (event or zero)
        if let Some(nonword_type) = child.child(0) {
            let span = Span::new(child.start_byte() as u32, child.end_byte() as u32);

            match nonword_type.kind() {
                EVENT => {
                    // Parse event (&=action)
                    if let Some(segment_child) = nonword_type.child(1)
                        && segment_child.kind() == EVENT_SEGMENT
                        && let Ok(event_type) = segment_child.utf8_text(source.as_bytes())
                    {
                        parsed_nonword = Some(ParsedNonword::Event(
                            Event::new(event_type).with_span(span),
                            span,
                        ));
                    }
                }
                ZERO => {
                    // Parse zero/action (0)
                    parsed_nonword = Some(ParsedNonword::Action(Action::with_span(span), span));
                }
                _ => {
                    errors.report(unexpected_node_error(child, source, "nonword"));
                }
            }
        }
        idx += 1;
    }

    // Position 1+: optional whitespaces and base_annotations
    while (idx as usize) < child_count {
        if let Some(child) = node.child(idx) {
            match child.kind() {
                WHITESPACES => {
                    // Whitespace between nonword and annotations - expected
                    idx += 1;
                }
                BASE_ANNOTATIONS => {
                    // Parse the base_annotations container node.
                    // Nonwords (events/actions) never carry retrace markers,
                    // so we only take the content annotations.
                    let parsed = parse_scoped_annotations(child, source, errors);
                    annotations.extend(parsed.content);
                    idx += 1;
                }
                _ => {
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "nonword_with_optional_annotations",
                    ));
                    idx += 1;
                }
            }
        } else {
            break;
        }
    }

    // Convert to appropriate UtteranceContent variant
    ParseOutcome::from(parsed_nonword.map(|nonword| {
        let full_span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

        match nonword {
            ParsedNonword::Event(event, _span) => {
                if annotations.is_empty() {
                    // Bare event
                    UtteranceContent::Event(event)
                } else {
                    // Annotated event
                    UtteranceContent::AnnotatedEvent(
                        Annotated::new(event)
                            .with_scoped_annotations(annotations)
                            .with_span(full_span),
                    )
                }
            }
            ParsedNonword::Action(action, _span) => {
                // Actions are ALWAYS wrapped in AnnotatedAction (even if no annotations)
                UtteranceContent::AnnotatedAction(
                    Annotated::new(action)
                        .with_scoped_annotations(annotations)
                        .with_span(full_span),
                )
            }
        }
    }))
}
