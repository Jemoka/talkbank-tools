//! Parse isolated main-tier lines via multi-root grammar.
//!
//! With multi-root, `*CHI:\thello .` is parsed directly as a main_tier
//! fragment — no synthetic full-document wrapper needed.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::TreeSitterParser;
use crate::error::{
    ErrorCode, ErrorContext, ParseError, ParseErrors, ParseResult, Severity, SourceLocation,
};
use crate::model::MainTier;
use crate::parser::tree_parsing::main_tier::structure::{
    collect_main_tier_errors, convert_main_tier_node,
};

/// Parse one main tier line into `MainTier`.
///
/// With multi-root grammar, the input (e.g., `*CHI:\thello .`) is parsed
/// directly as a main_tier fragment. The root node is `source_file` and
/// the first child should be `main_tier`.
pub(super) fn parse_main_tier(parser: &TreeSitterParser, input: &str) -> ParseResult<MainTier> {
    // Multi-root: parse directly, no wrapper
    let to_parse = format!("{input}\n");

    let tree = {
        let mut ts_parser = parser.parser.borrow_mut();
        ts_parser.parse(to_parse.as_bytes(), None).ok_or_else(|| {
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Tree-sitter parse returned None",
            ));
            errors
        })?
    };

    // Navigate: source_file → main_tier
    let root = tree.root_node();
    let main_tier_node = if root.kind() == "source_file" {
        root.child(0)
            .filter(|c| c.kind() == "main_tier")
            .ok_or_else(|| {
                let mut errors = ParseErrors::new();
                let actual = root
                    .child(0)
                    .map(|c| c.kind().to_string())
                    .unwrap_or_default();
                errors.push(ParseError::new(
                    ErrorCode::MissingMainTier,
                    Severity::Error,
                    SourceLocation::from_offsets(0, input.len()),
                    ErrorContext::new(input, 0..input.len(), input),
                    format!("Expected main_tier fragment, got '{actual}'"),
                ));
                errors
            })?
    } else {
        return Err({
            let mut errors = ParseErrors::new();
            errors.push(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                format!("Expected source_file root, got '{}'", root.kind()),
            ));
            errors
        });
    };

    // Check for parse errors
    if main_tier_node.has_error() {
        let mut errors = ParseErrors::new();
        collect_main_tier_errors(main_tier_node, &to_parse, input, 0, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }
    }

    // Convert the main_tier node to MainTier model
    let errors_sink = crate::error::ErrorCollector::new();
    let main_tier =
        convert_main_tier_node(main_tier_node, &to_parse, input, &errors_sink).into_option();

    let tier_errors = errors_sink.into_vec();
    let has_actual_errors = tier_errors
        .iter()
        .any(|e| matches!(e.severity, Severity::Error));
    if has_actual_errors || main_tier.is_none() {
        let mut errors = ParseErrors::new();
        errors.errors.extend(tier_errors);
        if main_tier.is_none() && errors.is_empty() {
            errors.push(ParseError::new(
                ErrorCode::MissingMainTier,
                Severity::Error,
                SourceLocation::from_offsets(0, input.len()),
                ErrorContext::new(input, 0..input.len(), input),
                "Failed to build main tier from parse tree",
            ));
        }
        return Err(errors);
    }

    main_tier.ok_or_else(|| {
        let mut errors = ParseErrors::new();
        errors.push(ParseError::new(
            ErrorCode::MissingMainTier,
            Severity::Error,
            SourceLocation::from_offsets(0, input.len()),
            ErrorContext::new(input, 0..input.len(), input),
            "Failed to build main tier from parse tree",
        ));
        errors
    })
}
