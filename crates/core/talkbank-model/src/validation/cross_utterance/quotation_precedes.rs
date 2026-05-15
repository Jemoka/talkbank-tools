//! Cross-utterance validation patterns
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use super::helpers::has_quoted_linker;
use crate::model::Utterance;
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

/// Validates `+".` quotation-precedes terminators against prior `+"` linkers.
///
/// The rule requires at least one preceding same-speaker utterance marked as
/// quoted speech, otherwise the terminator is considered structurally orphaned.
pub(super) fn check_quotation_precedes(utterances: &[Utterance], idx: usize) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let utterance = &utterances[idx];
    let speaker = utterance.main.speaker.as_str();

    // Find previous utterance(s) by same speaker
    let mut found_quoted = false;
    for prev_utt in utterances[..idx].iter().rev() {
        if prev_utt.main.speaker.as_str() == speaker {
            if has_quoted_linker(prev_utt) {
                found_quoted = true;
                break;
            } else {
                // Found same-speaker utterance without +" - stop search
                break;
            }
        }
    }

    if !found_quoted {
        errors.push(
            ParseError::new(
                ErrorCode::InvalidContentAnnotationNesting,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ... +\". ", speaker),
                    utterance.main.span,
                    "quotation precedes terminator",
                ),
                format!(
                    "Quotation precedes terminator (+\". ) without preceding quoted utterances (+\") from same speaker ({})",
                    speaker
                ),
            )
            .with_suggestion(format!(
                "Add +\" linker to preceding utterance(s) by {} to mark them as quoted speech",
                speaker
            ))
        );
    }

    errors
}
