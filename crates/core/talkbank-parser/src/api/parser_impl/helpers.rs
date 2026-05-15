//! Shared implementation helpers behind `TreeSitterParser` tier fragment methods.
//!
//! The key mechanism here is the "minimal CHAT wrapper" pattern: synthesize a
//! tiny valid file around one tier, parse it through the full parser pipeline,
//! and then project just the requested tier back out with corrected spans.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ParseOutcome;
use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::{ChatFile, Line};
use talkbank_model::{
    ErrorCollector, ErrorSink, OffsetAdjustingErrorSink, SpanShift, TeeErrorSink,
};

use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};

// =========================================================================
// Wrapper Parsing Infrastructure
// =========================================================================

/// Main tier line used in minimal wrappers.
const MAIN_TIER_LINE: &str = "*CHI:\thello .\n";

/// Parse a dependent tier using the minimal wrapper pattern.
///
/// # The Minimal Wrapper Pattern
///
/// Tree-sitter requires a complete, valid CHAT file to parse dependent tiers.
/// We wrap the tier content in a minimal valid CHAT structure:
///
/// ```text
/// @UTF8
/// @Begin
/// @Languages:  eng
/// *CHI:    hello .
/// %xxx:    <INPUT>
/// @End
/// ```
///
/// # Dual Error Handling
///
/// We use TWO error sinks:
/// 1. `OffsetAdjustingErrorSink`: Adjusts error spans from wrapper coordinates
///    to document coordinates, streaming to the user's ErrorSink.
/// 2. `ErrorCollector`: Collects errors to determine success/failure.
///
/// `TeeErrorSink` streams each error to both sinks simultaneously.
///
/// # Span Adjustment
///
/// Parsed objects have spans relative to the wrapper. We adjust them:
/// 1. Subtract wrapper prefix length → input-relative spans (0-based)
/// 2. Add user offset → document-absolute spans
///
/// # Type Parameters
///
/// * `T` - The tier type to extract (must implement `SpanShift`)
/// * `F` - Extractor function that pulls the tier from a `DependentTier`
///
/// # Arguments
///
/// * `parser` - The TreeSitterParser instance
/// * `tier_header` - The tier marker including tab (e.g., `"%mor:\t"`)
/// * `input` - The tier content to parse (without the header)
/// * `offset` - Byte offset in the original document
/// * `errors` - User's error sink for real-time reporting
/// * `extractor` - Function to extract the specific tier type from `DependentTier`
pub(crate) fn wrapper_parse_tier<T, F>(
    parser: &TreeSitterParser,
    tier_header: &str,
    input: &str,
    offset: usize,
    errors: &impl ErrorSink,
    extractor: F,
) -> ParseOutcome<T>
where
    T: SpanShift,
    F: FnOnce(DependentTier) -> Option<T>,
{
    // Build the minimal CHAT wrapper
    let chat = format!(
        "{}{}{}{}\n{}",
        MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE, tier_header, input, MINIMAL_CHAT_SUFFIX
    );

    // Set up dual error handling
    let tier_sink = ErrorCollector::new();
    let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
    let tee = TeeErrorSink::new(&adjusting_sink, &tier_sink);

    // Parse the wrapper
    let chat_file = parser.parse_chat_file_streaming(&chat, &tee);

    // Check for errors
    if !tier_sink.is_empty() {
        return ParseOutcome::rejected();
    }

    // Calculate prefix length for span adjustment
    // Formula: -(prefix_len) + offset converts wrapper-relative to document-absolute
    let prefix_len = MINIMAL_CHAT_PREFIX.len() + MAIN_TIER_LINE.len() + tier_header.len();

    // Extract the dependent tier from the parsed file
    let Some(tier) = extract_first_dependent_tier(chat_file) else {
        return ParseOutcome::rejected();
    };

    // Apply the extractor to get the specific tier type
    let Some(mut extracted) = extractor(tier) else {
        return ParseOutcome::rejected();
    };

    // Adjust spans from wrapper-relative to document-absolute
    extracted.shift_spans_after(0, -(prefix_len as i32) + offset as i32);
    ParseOutcome::parsed(extracted)
}

/// Parse a generic dependent tier (where input includes the header).
///
/// Unlike `wrapper_parse_tier`, this function expects the input to already
/// include the tier header (e.g., `"%mor:\tpro|I v|want ."`).
///
/// Used by `parse_dependent_tier()` for parsing unknown tier types.
pub(crate) fn wrapper_parse_generic_tier(
    parser: &TreeSitterParser,
    input: &str,
    offset: usize,
    errors: &impl ErrorSink,
) -> ParseOutcome<DependentTier> {
    // Build wrapper (input already has tier header)
    let chat = format!(
        "{}{}{}\n{}",
        MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE, input, MINIMAL_CHAT_SUFFIX
    );

    // Set up dual error handling
    let tier_sink = ErrorCollector::new();
    let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
    let tee = TeeErrorSink::new(&adjusting_sink, &tier_sink);

    // Parse the wrapper
    let chat_file = parser.parse_chat_file_streaming(&chat, &tee);

    // Check for errors
    if !tier_sink.is_empty() {
        return ParseOutcome::rejected();
    }

    // Calculate prefix length (no tier_header since it's in input)
    let prefix_len = MINIMAL_CHAT_PREFIX.len() + MAIN_TIER_LINE.len();

    // Extract and return the first dependent tier
    for line in chat_file.lines {
        if let Line::Utterance(utterance) = line
            && let Some(mut tier) = utterance.dependent_tiers.into_iter().next()
        {
            tier.shift_spans_after(0, -(prefix_len as i32) + offset as i32);
            return ParseOutcome::parsed(tier);
        }
    }

    ParseOutcome::rejected()
}

/// Extract the first dependent tier from a parsed ChatFile.
///
/// Returns the first dependent tier found in the parsed file (there should
/// only be one since we parse a minimal wrapper with a single tier).
fn extract_first_dependent_tier(chat_file: ChatFile) -> Option<DependentTier> {
    for line in chat_file.lines {
        if let Line::Utterance(utterance) = line {
            return utterance.dependent_tiers.into_iter().next();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::MorTier;

    /// Tests wrapper parse tier mor.
    #[test]
    fn test_wrapper_parse_tier_mor() -> Result<(), String> {
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let errors = ErrorCollector::new();

        let result =
            wrapper_parse_tier(
                &parser,
                "%mor:\t",
                "pro|I v|want .",
                0,
                &errors,
                |tier| match tier {
                    DependentTier::Mor(tier) => Some(tier),
                    _ => None,
                },
            );

        assert!(result.is_parsed());
        let mor: MorTier = result
            .into_option()
            .ok_or_else(|| "Expected MOR tier to parse".to_string())?;
        assert!(!mor.items().is_empty());
        Ok(())
    }
}
