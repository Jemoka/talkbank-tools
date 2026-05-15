//! Generic dependent tier parsing
//!
//! This module parses an arbitrary dependent-tier line into [`UserDefinedTier`]
//! `(label + raw content)` using the same tree-sitter pipeline as the typed tier
//! parsers. It does not perform tier-specific semantic decoding.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#User_Defined_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::ParseOutcome;
use talkbank_model::model::UserDefinedTier;

/// Parse a generic dependent-tier line.
///
/// The parser wraps the input in a minimal CHAT document, parses through the
/// regular CST path, then extracts `%label:\tcontent` as a [`UserDefinedTier`].
///
/// # Format
///
/// Dependent tiers follow the format: `%tiertype:\tcontent`
/// - Must start with `%`
/// - Tier type name (mor, pho, gra, etc.)
/// - Colon separator `:`
/// - Tab character (required)
/// - Tier content
///
/// # Examples
///
/// ```ignore
/// use talkbank_parser::parse_dependent_tier;
/// use talkbank_model::ErrorCollector;
/// use talkbank_model::ParseOutcome;
///
/// // Parse a morphology tier
/// let errors = ErrorCollector::new();
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%mor:\tpro|I v|go&PAST .", &errors) {
///     assert_eq!(tier.label, "mor");
///     assert_eq!(tier.content, "pro|I v|go&PAST .");
/// }
///
/// // Parse a comment tier
/// if let ParseOutcome::Parsed(tier) = parse_dependent_tier("%com:\tthis is a comment", &errors) {
///     assert_eq!(tier.label, "com");
/// }
/// ```
///
/// # Error Streaming
///
/// Errors are streamed via the ErrorSink parameter:
/// - The line doesn't start with `%` (E601)
/// - The line is missing the colon separator (E602)
/// - Invalid format that tree-sitter can't parse (E601)
pub fn parse_dependent_tier(input: &str, errors: &impl ErrorSink) -> ParseOutcome<UserDefinedTier> {
    crate::parser::tier_parsers::parse_dependent_tier(input, errors)
}
