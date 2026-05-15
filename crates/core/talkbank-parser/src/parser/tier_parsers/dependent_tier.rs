//! Generic dependent-tier parsing entrypoint.
//!
//! This parser extracts a `%label:\tcontent` line into `UserDefinedTier` without
//! knowing the tier semantics ahead of time. Typed tier parsers can then consume
//! the result when appropriate.
//!
//! ## Implementation Strategy
//!
//! Uses the **synthesis pattern**: wraps the tier in a minimal valid CHAT file,
//! parses with tree-sitter to get proper CST structure, then extracts the tier node.
//! This avoids all text hacking while providing granular parsing APIs.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use crate::node_types::{COLON, NEWLINE, SPACE, TAB, TIER_SEP, WHITESPACES};
use crate::parser::tree_parsing::parser_helpers::is_dependent_tier;
use talkbank_model::ParseOutcome;
use talkbank_model::model::UserDefinedTier;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Converts a dependent-tier line using the **synthesis pattern**.
///
/// Wraps the tier in a minimal valid CHAT file, parses with tree-sitter to get proper
/// CST structure, then extracts the tier node. This provides granular parsing without text hacking.
///
/// # Format
/// `%tiertype:\tcontent` (tab after colon)
///
/// # Common tier types
/// - `%mor`: Morphological analysis
/// - `%pho`: Phonological transcription
/// - `%gra`: Grammatical relations
/// - `%com`: Comment
/// - `%act`: Action descriptions
///
/// # Examples
///
/// ```ignore
/// use talkbank_model::ErrorCollector;
/// use talkbank_model::ParseOutcome;
///
/// let errors = ErrorCollector::new();
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%mor:\tpro|I v|be&1S .", &errors) {
///     assert_eq!(tier.label, "mor");
/// }
///
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%pho:\ta b c", &errors) {
///     assert_eq!(tier.label, "pho");
///     assert_eq!(tier.content, "a b c");
/// }
/// ```
///
/// # Implementation
///
/// Uses tree-sitter synthesis pattern:
/// 1. Wrap tier in minimal CHAT file
/// 2. Parse entire file with tree-sitter
/// 3. Extract dependent tier node from CST
/// 4. Parse node using existing CST traversal code
/// 5. Stream errors via ErrorSink (never fail-fast)
pub fn parse_dependent_tier(input: &str, errors: &impl ErrorSink) -> ParseOutcome<UserDefinedTier> {
    // Trim leading whitespace: CHAT requires '%' at column 0 for dependent tiers,
    // so leading spaces would prevent tree-sitter from recognizing the line.
    let trimmed = input.trim_start();

    // Synthesize minimal valid CHAT file with the tier
    let wrapped = format!(
        "@UTF8\n\
         @Begin\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n\
         *CHI:\tdummy .\n\
         {}\n\
         @End\n",
        trimmed
    );

    // Parse with tree-sitter to get proper CST structure
    let mut parser = tree_sitter::Parser::new();
    if let Err(e) = parser.set_language(&tree_sitter_talkbank::LANGUAGE.into()) {
        errors.report(ParseError::new(
            ErrorCode::ParseFailed,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            format!("Failed to set tree-sitter language: {}", e),
        ));
        return ParseOutcome::rejected();
    }

    let Some(tree) = parser.parse(&wrapped, None) else {
        errors.report(ParseError::new(
            ErrorCode::ParseFailed,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            "Tree-sitter failed to parse synthesized CHAT file",
        ));
        return ParseOutcome::rejected();
    };

    // Find the dependent tier node in the CST
    // It's the first tier after the main tier
    let root = tree.root_node();
    let Some(tier_node) = find_first_dependent_tier(root) else {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::at_offset(0),
            ErrorContext::new(input, 0..input.len(), input),
            "No dependent tier found in parsed structure",
        ));
        return ParseOutcome::rejected();
    };

    // Extract tier type and content from CST node (no text hacking!)
    extract_tier_from_node(tier_node, &wrapped, input, errors)
}

/// Find first dependent tier node in CST
fn find_first_dependent_tier(root: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if is_dependent_tier(root.kind()) {
        return Some(root);
    }

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if let Some(found) = find_first_dependent_tier(child) {
            return Some(found);
        }
    }

    None
}

/// Extract UserDefinedTier from CST node using proper tree traversal
fn extract_tier_from_node(
    node: tree_sitter::Node,
    source: &str,
    original_input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UserDefinedTier> {
    // The node is a tier node - extract label/content from CST without fabricated defaults.
    let mut tier_type: Option<String> = None;
    let mut content_parts: Vec<String> = Vec::new();
    let mut saw_content_node = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            TIER_SEP | COLON | TAB | SPACE | WHITESPACES | NEWLINE => {}
            // All tier prefixes (including x_tier_prefix "%xfoo" and
            // unsupported_tier_prefix "%custom") end with "_tier_prefix".
            // Strip the leading '%' to get the tier label.
            kind if kind.ends_with("_tier_prefix") => match child.utf8_text(source.as_bytes()) {
                Ok(text) => {
                    if let Some(label) = text.strip_prefix('%') {
                        if label.is_empty() {
                            errors.report(ParseError::new(
                                ErrorCode::InvalidDependentTier,
                                Severity::Error,
                                SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                                ErrorContext::new(
                                    original_input,
                                    0..original_input.len(),
                                    original_input,
                                ),
                                "Dependent tier label cannot be empty",
                            ));
                            return ParseOutcome::rejected();
                        }
                        tier_type = Some(label.to_string());
                    } else {
                        errors.report(ParseError::new(
                            ErrorCode::InvalidDependentTier,
                            Severity::Error,
                            SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                            ErrorContext::new(
                                original_input,
                                0..original_input.len(),
                                original_input,
                            ),
                            format!(
                                "Dependent tier prefix '{}' is missing leading '%' marker",
                                text
                            ),
                        ));
                        return ParseOutcome::rejected();
                    }
                }
                Err(_) => {
                    errors.report(ParseError::new(
                        ErrorCode::UnparsableContent,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(original_input, 0..original_input.len(), original_input),
                        "Unparsable content: dependent tier prefix is not valid UTF-8",
                    ));
                    return ParseOutcome::rejected();
                }
            },
            _ => match child.utf8_text(source.as_bytes()) {
                Ok(text) => {
                    saw_content_node = true;
                    content_parts.push(text.to_string());
                }
                Err(_) => {
                    errors.report(ParseError::new(
                        ErrorCode::UnparsableContent,
                        Severity::Error,
                        SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                        ErrorContext::new(original_input, 0..original_input.len(), original_input),
                        "Unparsable content: dependent tier content is not valid UTF-8",
                    ));
                    return ParseOutcome::rejected();
                }
            },
        }
    }

    let tier_type = match tier_type {
        Some(tier_type) => tier_type,
        None => {
            errors.report(ParseError::new(
                ErrorCode::InvalidDependentTier,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(original_input, 0..original_input.len(), original_input),
                "Could not extract dependent tier label from parsed structure",
            ));
            return ParseOutcome::rejected();
        }
    };

    if !saw_content_node {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Could not extract dependent tier content from parsed structure",
        ));
        return ParseOutcome::rejected();
    }

    let mut content = String::new();
    for part in content_parts {
        if !content.is_empty() && !part.is_empty() {
            content.push(' ');
        }
        content.push_str(&part);
    }

    if content.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::InvalidDependentTier,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(original_input, 0..original_input.len(), original_input),
            "Dependent tier content cannot be empty",
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed(UserDefinedTier::new(tier_type, content))
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use talkbank_model::{ErrorCode, ErrorCollector};

    /// Verifies known dependent tiers parse with label/content preserved verbatim.
    #[test]
    fn parses_known_tier_label_and_content_without_fabrication() -> Result<(), String> {
        let input = "%mor:\tpro|I v|go .";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected %mor tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "mor");
        assert_eq!(tier.content, "pro|I v|go .");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }

    /// Verifies `%x...` tiers parse without losing custom labels.
    #[test]
    fn parses_x_tier_label_and_content_without_fabrication() -> Result<(), String> {
        let input = "%xfoo:\tcustom value";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected %xfoo tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "xfoo");
        assert_eq!(tier.content, "custom value");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }

    /// Verifies empty dependent-tier content is rejected without placeholder fabrication.
    #[test]
    fn empty_content_is_rejected_without_placeholder_values() {
        let input = "%com:\t";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        assert!(
            result.is_none(),
            "Expected empty dependent tier content to fail"
        );
        assert!(
            errors
                .to_vec()
                .iter()
                .any(|err| err.code == ErrorCode::InvalidDependentTier),
            "Expected InvalidDependentTier diagnostic, got: {:?}",
            errors.to_vec()
        );
    }

    /// Verifies leading/trailing line whitespace does not prevent tier extraction.
    #[test]
    fn tier_with_leading_trailing_whitespace() -> Result<(), String> {
        let input = "  %pho:\ta b c  ";
        let errors = ErrorCollector::new();
        let result = parse_dependent_tier(input, &errors);
        let tier = result.ok_or_else(|| {
            format!(
                "Expected whitespace-padded tier to parse, got errors: {:?}",
                errors.to_vec()
            )
        })?;
        assert_eq!(tier.label.as_str(), "pho");
        // 3 trailing spaces: "a b c" from pho_groups + join separator + "  " from ERROR node
        // (trailing spaces in pho content are a tree-sitter ERROR, collected by synthesis pattern)
        assert_eq!(tier.content, "a b c   ");
        assert!(
            errors.is_empty(),
            "Unexpected diagnostics: {:?}",
            errors.into_vec()
        );
        Ok(())
    }
}
