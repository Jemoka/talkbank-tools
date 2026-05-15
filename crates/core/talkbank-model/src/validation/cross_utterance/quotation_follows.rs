//! Cross-utterance validation for quotation-follows terminators.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>

use super::helpers::has_quoted_linker;
use crate::model::{Terminator, Utterance};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

/// Validate Pattern A quotation-follows sequencing for one utterance index.
///
/// Pattern: *SPK: attribution +"/. \n *SPK: +" quote.
///
/// After a `+"/.` terminator, the next utterance by the same speaker must
/// start with `+"`. The checker also rejects mixed sequences that combine
/// quotation-follows and quotation-precedes terminators in one chain.
pub(super) fn check_quotation_follows(utterances: &[Utterance], idx: usize) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let utterance = &utterances[idx];
    let speaker = utterance.main.speaker.as_str();

    // Find next utterance by same speaker
    let next_same_speaker = utterances[idx + 1..]
        .iter()
        .find(|u| u.main.speaker.as_str() == speaker);

    if let Some(next_utt) = next_same_speaker {
        // Check if it has +" linker
        if !has_quoted_linker(next_utt) {
            errors.push(
                ParseError::new(
                    ErrorCode::UnbalancedQuotationCrossUtterance,
                    Severity::Error,
                    SourceLocation::new(utterance.main.span),
                    ErrorContext::new(
                        format!("*{}: ... +\"/. ", speaker),
                        utterance.main.span,
                        "quotation follows terminator",
                    ),
                    format!(
                        "Quotation follows terminator (+\"/. ) not followed by quoted utterance (+\") from same speaker ({})",
                        speaker
                    ),
                )
                .with_suggestion(format!(
                    "Add +\" linker to the next utterance by {} to mark it as quoted speech",
                    speaker
                ))
            );
        } else {
            // Check for mixed patterns: ALL +" utterances after +"/. should not end with +".
            // Continue checking subsequent same-speaker +" utterances
            for check_utt in utterances[idx + 1..].iter() {
                if check_utt.main.speaker.as_str() != speaker {
                    continue; // Skip different speakers
                }

                if !has_quoted_linker(check_utt) {
                    break; // End of quotation sequence
                }

                // Check if this +" utterance ends with +".
                if let Some(ref term) = check_utt.main.content.terminator
                    && matches!(term, Terminator::QuotedPeriodSimple { .. })
                {
                    errors.push(
                        ParseError::new(
                            ErrorCode::UnbalancedQuotationCrossUtterance,
                            Severity::Error,
                            SourceLocation::new(check_utt.main.span),
                            ErrorContext::new(
                                format!("*{}: +\" ... +\". ", speaker),
                                check_utt.main.span,
                                "mixed quotation patterns",
                        ),
                        "Mixed quotation patterns - cannot use both +\"/. (quotation follows) and +\". (quotation precedes) in same sequence",
                    )
                    .with_suggestion("Use normal terminator (., ?, !) for quoted utterances in quotation follows pattern")
                    );
                    break; // Only report first mixed pattern
                }
            }
        }
    } else {
        // No next utterance by same speaker - error
        errors.push(
            ParseError::new(
                ErrorCode::UnbalancedQuotationCrossUtterance,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ... +\"/. ", speaker),
                    utterance.main.span,
                    "quotation follows terminator",
                ),
                format!(
                    "Quotation follows terminator (+\"/. ) not followed by any subsequent utterance from same speaker ({})",
                    speaker
                ),
            )
            .with_suggestion(format!(
                "Add at least one utterance by {} starting with +\" to provide the quoted content",
                speaker
            ))
        );
    }

    errors
}
