//! Intra-utterance quotation-postcode balance validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>

use crate::model::Utterance;
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

/// Validate quotation markers are balanced in an utterance.
///
/// CHAT format uses `+"/` to begin quotations and `+"/.` to end them.
/// Utterances with `+"` prefix continue a quotation from a previous utterance.
///
/// This validates that within a single utterance:
/// - Every `+"/` has a corresponding `+"/.`
/// - Quotation markers are balanced
///
/// Note: Cross-utterance quotation validation (with `+"` continuations)
/// is handled by cross-utterance validators, not this function.
pub(crate) fn check_quotation_balance(utterance: &Utterance, errors: &impl ErrorSink) {
    // Stack of spans for each open quotation-begin marker
    let mut begin_spans: Vec<Span> = Vec::new();

    for postcode in &utterance.main.content.postcodes {
        let text = postcode.text.as_str();
        if text == "\"/" {
            begin_spans.push(postcode.span);
        } else if text == "\"/." && begin_spans.pop().is_none() {
            errors.report(
                ParseError::at_span(
                    ErrorCode::UnbalancedQuotation,
                    Severity::Error,
                    postcode.span,
                    "Quotation end (+\"/.) without corresponding begin (+\"/)",
                )
                .with_suggestion(
                    "Ensure each quotation end (+\"/.) has a matching quotation begin (+\"/) before it",
                ),
            );
        }
    }

    // Check for unclosed quotations
    for begin_span in &begin_spans {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnbalancedQuotation,
                Severity::Error,
                *begin_span,
                "Unbalanced quotation: unclosed quotation begin marker (+\"/)",
            )
            .with_suggestion(
                "Ensure each quotation begin (+\"/) has a matching quotation end (+\"/.)",
            ),
        );
    }
}
