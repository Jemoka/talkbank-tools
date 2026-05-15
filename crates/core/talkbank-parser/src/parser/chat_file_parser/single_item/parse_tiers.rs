//! Single dependent tier parsing.
//!
//! Provides parsing of a standalone dependent tier line like `%mor:\tdet|the n|cat .`
//! using the synthesis pattern: wrap in minimal CHAT context, parse, extract the tier.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::{
    ErrorCode, ErrorCollector, ErrorContext, OffsetAdjustingErrorSink, ParseError, ParseErrors,
    Severity, SourceLocation, Span, SpanShift,
};

use super::TreeSitterParser;
use crate::error::ParseResult;
use crate::model::DependentTier;

/// Parse a single dependent tier line.
///
/// Uses the synthesis pattern:
/// 1. Wraps the tier in a minimal CHAT file context
/// 2. Parses with tree-sitter
/// 3. Extracts the dependent tier from the first utterance
///
/// # Format
///
/// Dependent tiers follow the format: `%tiertype:\tcontent`
/// - Must start with `%`
/// - Tier type name (mor, gra, pho, etc.)
/// - Colon separator `:`
/// - Tab character (required)
/// - Tier content
///
/// # Examples
///
/// ```
/// use talkbank_parser::TreeSitterParser;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let parser = TreeSitterParser::new()
///     .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
///
/// // Parse a morphology tier
/// let result = parser.parse_tiers("%mor:\tpro|I v|want n|cookie-PL .");
/// assert!(result.is_ok());
///
/// // Parse a grammar tier
/// let result = parser.parse_tiers("%gra:\t1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT");
/// assert!(result.is_ok());
/// # Ok(())
/// # }
/// ```
pub fn parse_tiers(parser: &TreeSitterParser, input: &str) -> ParseResult<DependentTier> {
    // MINIMAL WRAPPER PATTERN:
    // Tree-sitter requires a complete CHAT file to parse dependent tiers.
    // We wrap the input in a minimal valid CHAT context.
    //
    // Wrapper format:
    //   PREFIX = "@UTF8\n@Begin\n*CHI:\ta b c d e f g h i j k l m n o p q r s t u v w x y z .\n"
    //   INPUT = user's tier content
    //   SUFFIX = "\n@End"
    //
    // ERROR HANDLING:
    // - OffsetAdjustingErrorSink: Adjusts error spans from wrapper coordinate space to input coordinate space
    // - tier_sink: Collects errors to check if parsing succeeded
    let prefix = "@UTF8\n@Begin\n*CHI:\ta b c d e f g h i j k l m n o p q r s t u v w x y z .\n";
    let wrapped = format!("{}{}\n@End", prefix, input);

    // Use OffsetAdjustingErrorSink to fix error offsets
    let tier_sink = ErrorCollector::new();
    let offset = prefix.len();
    let adjusting_sink = OffsetAdjustingErrorSink::new(&tier_sink, offset, input);

    let file = parser.parse_chat_file_streaming(&wrapped, &adjusting_sink);
    let error_vec = tier_sink.into_vec();

    // Extract the dependent tier from the parsed file
    // We prioritize extracting the tier even if there were some parsing errors,
    // since the tier content itself may have parsed successfully.
    if let Some(utterance) = file.utterances().next() {
        if let Some(tier) = utterance.dependent_tiers.first() {
            // Tier was extracted successfully
            // Adjust spans: remove wrapper offset so spans are relative to input (0-based)
            let mut adjusted_tier = tier.clone();
            let prefix_len = prefix.len();
            adjusted_tier.shift_spans_after(0, -(prefix_len as i32));

            Ok(adjusted_tier)
        } else {
            // No dependent tier found - this means parsing actually failed
            if error_vec.is_empty() {
                let span = Span::from(0..input.len());
                let err = ParseError::new(
                    ErrorCode::new("E999"),
                    Severity::Error,
                    SourceLocation::new(span),
                    ErrorContext::new(input, 0..input.len(), ""),
                    "No dependent tier found in parsed output",
                );
                Err(ParseErrors::from(vec![err]))
            } else {
                Err(ParseErrors::from(error_vec))
            }
        }
    } else {
        // No utterance found
        if error_vec.is_empty() {
            let span = Span::from(0..input.len());
            let err = ParseError::new(
                ErrorCode::new("E999"),
                Severity::Error,
                SourceLocation::new(span),
                ErrorContext::new(input, 0..input.len(), ""),
                "No utterance found in parsed output",
            );
            Err(ParseErrors::from(vec![err]))
        } else {
            Err(ParseErrors::from(error_vec))
        }
    }
}
