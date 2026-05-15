//! Digit-policy validation for words under resolved language context.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>

use crate::model::Word;
use crate::model::content::word::WordCategory;
use crate::validation::context::contains_digits;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

use super::helpers::mixed_language_allows_numbers;
use super::resolve::LanguageResolution;

/// Validate whether digits are allowed for a word under resolved language context.
///
/// Current policy is permissive for mixed/ambiguous codes: if any candidate
/// language allows digits, the token is accepted. Single-language words reduce
/// to the same check over one candidate.
///
/// # Behavior
/// - Skips validation for Omission words (0 prefix is valid CHAT)
/// - If word contains digits, checks candidate languages via `resolution`
/// - Emits `E220` only when no candidate language permits digits
pub(crate) fn check_word_digits_multi(
    word: &Word,
    resolution: &LanguageResolution,
    errors: &impl ErrorSink,
) {
    // Skip validation for Omission words (0word pattern) - the 0 prefix is valid CHAT
    if word.category == Some(WordCategory::Omission) {
        return;
    }

    if !contains_digits(word.cleaned_text()) {
        return;
    }

    // For mixed/ambiguous language markers, allow digits if at least one candidate
    // language allows them. This matches permissive CHAT usage in reference data.
    let allows_digits = if resolution.languages().is_empty() {
        false
    } else {
        resolution
            .languages()
            .iter()
            .any(|lang| mixed_language_allows_numbers(lang.as_str()))
    };

    if !allows_digits {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalDigits,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                format!(
                    "\"{}\" is not a legal word in language(s) \"{}\": numeric digits not allowed",
                    word.cleaned_text(),
                    resolution.as_display_string()
                ),
            )
            .with_suggestion(
                "Languages that allow numbers: zho, cym, vie, tha, nan, yue, min, hak".to_string(),
            ),
        );
    }
}
