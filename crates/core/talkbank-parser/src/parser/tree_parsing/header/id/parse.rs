//! Parsing for `@ID` headers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use crate::node_types::{ID_CONTENTS, ID_HEADER};
use tree_sitter::Node;

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use talkbank_model::ParseOutcome;
use talkbank_model::model::Header;

use super::fields::{
    parse_optional_sex_field, parse_optional_terminal_field, parse_optional_text_field,
    parse_required_text_field,
};

/// Parse ID header from tree-sitter node
pub fn parse_id_header(node: Node, source: &str, errors: &impl ErrorSink) -> Header {
    // Verify this is an id_header node
    if node.kind() != ID_HEADER {
        errors.report(ParseError::new(
            ErrorCode::TreeParsingError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
            format!("Expected id_header node, got: {}", node.kind()),
        ));
        return unknown_id_header("ID header CST node had unexpected kind");
    }

    // Find id_contents child (prefix + header_sep + contents + newline)
    let id_contents = match find_child_by_kind(node, ID_CONTENTS) {
        Some(child) => child,
        None => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "id_header"),
                "Missing id_contents child in id_header",
            ));
            return unknown_id_header("ID header CST node is missing id_contents");
        }
    };

    // Extract fields by position from id_contents
    let child_count = id_contents.child_count();
    let mut idx = 0usize;

    let language = parse_required_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        ErrorCode::EmptyIDLanguage,
        "Missing id_languages field in @ID header",
    );

    let corpus = parse_optional_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        "id_corpus",
    );

    let speaker = parse_required_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        ErrorCode::EmptyIDSpeaker,
        "Missing id_speaker field in @ID header",
    );

    let age =
        parse_optional_text_field(id_contents, &mut idx, child_count, source, errors, "id_age");

    let sex = parse_optional_sex_field(id_contents, &mut idx, child_count, source, errors);

    let group = parse_optional_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        "id_group",
    );

    let ses =
        parse_optional_text_field(id_contents, &mut idx, child_count, source, errors, "id_ses");

    let role = parse_required_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        ErrorCode::EmptyIDRole,
        "Empty role field in @ID header: the role (8th field) must not be blank",
    );

    let education = parse_optional_text_field(
        id_contents,
        &mut idx,
        child_count,
        source,
        errors,
        "id_education",
    );

    let custom_field = parse_optional_terminal_field(
        id_contents,
        idx,
        child_count,
        source,
        errors,
        "id_custom_field",
    );

    let (language, corpus, speaker, age, sex, group, ses, role, education, custom_field) = match (
        language,
        corpus,
        speaker,
        age,
        sex,
        group,
        ses,
        role,
        education,
        custom_field,
    ) {
        (
            ParseOutcome::Parsed(language),
            ParseOutcome::Parsed(corpus),
            ParseOutcome::Parsed(speaker),
            ParseOutcome::Parsed(age),
            ParseOutcome::Parsed(sex),
            ParseOutcome::Parsed(group),
            ParseOutcome::Parsed(ses),
            ParseOutcome::Parsed(role),
            ParseOutcome::Parsed(education),
            ParseOutcome::Parsed(custom_field),
        ) => (
            language,
            corpus,
            speaker,
            age,
            sex,
            group,
            ses,
            role,
            education,
            custom_field,
        ),
        _ => return unknown_id_header("ID header contains malformed fields"),
    };

    // No Rust-side trimming needed — the grammar's optional($.whitespaces)
    // wrappers and trimming field regexes ensure field content arrives without
    // leading/trailing whitespace.

    // Parse comma-separated language codes (e.g., "eng, spa" → [eng, spa]).
    let language_codes: Vec<talkbank_model::model::LanguageCode> = language
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(talkbank_model::model::LanguageCode::new)
        .collect();
    let languages = talkbank_model::model::LanguageCodes::new(language_codes);

    let mut id_header = talkbank_model::model::IDHeader::from_languages(languages, speaker, role);
    if let Some(c) = corpus {
        id_header = id_header.with_corpus(c);
    }
    if let Some(a) = age {
        id_header = id_header.with_age(a);
    }
    if let Some(s) = sex {
        id_header = id_header.with_sex(s);
    }
    if let Some(g) = group {
        id_header = id_header.with_group(g);
    }
    if let Some(ses_val) = ses {
        id_header = id_header.with_ses(talkbank_model::model::SesValue::from_text(&ses_val));
    }
    if let Some(e) = education {
        id_header = id_header.with_education(e);
    }
    if let Some(cf) = custom_field {
        id_header = id_header.with_custom_field(cf);
    }

    Header::ID(id_header)
}

/// Build `Header::Unknown` for malformed `@ID` input.
fn unknown_id_header(parse_reason: impl Into<String>) -> Header {
    Header::Unknown {
        text: "@ID".into(),
        parse_reason: Some(parse_reason.into()),
        suggested_fix: Some(
            "Expected @ID format: @ID:\\tlang|corpus|speaker|age|sex|group|ses|role|education|custom|"
                .to_string(),
        ),
    }
}

/// Find first direct child matching `kind`.
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}
