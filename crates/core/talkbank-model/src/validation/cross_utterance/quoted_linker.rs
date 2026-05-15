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
use crate::model::{Terminator, Utterance};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

/// Validates the `+"` linker against surrounding same-speaker quotation context.
///
/// This rule checks whether the linker participates in either a backward
/// quotation-follows chain or a forward quotation-precedes chain.
#[allow(dead_code)]
pub(super) fn check_quoted_linker(utterances: &[Utterance], idx: usize) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let utterance = &utterances[idx];
    let speaker = utterance.main.speaker.as_str();

    // Check Pattern A: Look backward for +"/. or +"
    let mut pattern_a_valid = false;
    for prev_utt in utterances[..idx].iter().rev() {
        if prev_utt.main.speaker.as_str() == speaker {
            // Check if previous same-speaker utterance ended with +"/.
            if let Some(ref term) = prev_utt.main.content.terminator
                && matches!(term, Terminator::QuotedNewLine { .. })
            {
                pattern_a_valid = true;
                break;
            }
            // Check if previous same-speaker utterance had +" linker
            if has_quoted_linker(prev_utt) {
                pattern_a_valid = true;
                break;
            }
            // Found same-speaker without pattern A markers - stop
            break;
        }
    }

    // Check Pattern B: Look forward for +".
    let mut pattern_b_valid = false;
    for next_utt in utterances[idx + 1..].iter() {
        if next_utt.main.speaker.as_str() == speaker {
            // Check if this or a future same-speaker utterance ends with +".
            if let Some(ref term) = next_utt.main.content.terminator
                && matches!(term, Terminator::QuotedPeriodSimple { .. })
            {
                pattern_b_valid = true;
                break;
            }
            // If same-speaker but no +" linker, check its terminator
            if !has_quoted_linker(next_utt)
                && let Some(ref term) = next_utt.main.content.terminator
                && matches!(term, Terminator::QuotedPeriodSimple { .. })
            {
                pattern_b_valid = true;
                break;
            }
        }
    }

    // If in Pattern B, verify it eventually ends with +".
    if !pattern_a_valid && !pattern_b_valid {
        errors.push(
            ParseError::new(
                ErrorCode::UnmatchedContentAnnotationEnd,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: +\" ...", speaker),
                    utterance.main.span,
                    "quoted utterance linker",
                ),
                format!(
                    "Quotation precedes pattern missing required terminator (+\". ) from speaker {}",
                    speaker
                ),
            )
            .with_suggestion(format!(
                "End the quotation sequence with +\". terminator in a later utterance by {}",
                speaker
            ))
        );
    }

    errors
}
