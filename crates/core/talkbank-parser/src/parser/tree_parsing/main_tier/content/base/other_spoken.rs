//! Parsing for background other-speaker events (`&*SPK:text`).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::error::{ErrorSink, Span};
use crate::model::{OtherSpokenEvent, UtteranceContent};
use crate::node_types::{AMPERSAND, COLON, SPEAKER, STANDALONE_WORD, STAR};
use crate::parser::tree_parsing::parser_helpers::cst_assertions::{
    assert_child_count_exact, expect_child,
};
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse the background-other-speaker marker (`&*SPK:text`) described in the manual’s
/// Main Tier section.
///
/// The tree-sitter grammar (see `other_spoken_event` in `grammar.js`) enforces the `seq`
/// shape `[], ['&'], ['*'], [speaker], [':'], [standalone_word]`. We verify that structure,
/// capture the speaker text and trailing message, and return an `OtherSpokenEvent` that keeps
/// the byte span so downstream tooling can annotate the CHAT text with the speaker’s label and
/// preserve the alignment to the official CHAT description of other-speaker events.
pub(crate) fn parse_other_spoken_event(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    // &*SPEAKER:text - someone else speaking in background
    // Grammar: seq('&', '*', speaker, colon, standalone_word)
    // Position 0: '&'
    // Position 1: '*'
    // Position 2: speaker
    // Position 3: colon
    // Position 4: standalone_word (text)
    if !assert_child_count_exact(node, 5, source, errors, "other_spoken_event") {
        return ParseOutcome::rejected();
    }

    if expect_child(node, 0, AMPERSAND, source, errors, "other_spoken_event").is_none() {
        return ParseOutcome::rejected();
    }
    if expect_child(node, 1, STAR, source, errors, "other_spoken_event").is_none() {
        return ParseOutcome::rejected();
    }
    if expect_child(node, 3, COLON, source, errors, "other_spoken_event").is_none() {
        return ParseOutcome::rejected();
    }

    let ParseOutcome::Parsed(speaker_node) =
        expect_child(node, 2, SPEAKER, source, errors, "other_spoken_event")
    else {
        return ParseOutcome::rejected();
    };
    let ParseOutcome::Parsed(text_node) = expect_child(
        node,
        4,
        STANDALONE_WORD,
        source,
        errors,
        "other_spoken_event",
    ) else {
        return ParseOutcome::rejected();
    };

    let speaker_text = extract_utf8_text(speaker_node, source, errors, "speaker", "");
    let text = extract_utf8_text(text_node, source, errors, "other_spoken_text", "");

    let event = OtherSpokenEvent::with_span(
        speaker_text,
        text,
        Span::new(node.start_byte() as u32, node.end_byte() as u32),
    );
    ParseOutcome::parsed(UtteranceContent::OtherSpokenEvent(event))
}
