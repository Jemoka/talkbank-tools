//! Structural/prosodic validators for main-tier word tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Words>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#WordInternalPause_Marker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>

use crate::model::{Word, WordContent, WordStressMarkerType};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Enforce character-level hygiene for the normalized word surface.
///
/// Words may NOT contain:
/// - Whitespace (spaces, tabs, newlines)
/// - Bullet markers (U+0015 / byte 0x15)
/// - Other control characters
///
/// NOTE: Validates cleaned_text, NOT raw_text. Raw text may contain formatting markers
/// (underline U+0001, U+0002, etc.) that are parsed into word content structure.
///
/// This validation catches parser bugs where word boundaries are incorrectly determined.
pub(crate) fn check_word_characters(word: &Word, errors: &impl ErrorSink) {
    let cleaned = word.cleaned_text();

    // Check for whitespace
    if cleaned.chars().any(|c| c.is_whitespace()) {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains illegal whitespace characters",
            )
            .with_suggestion(
                "Words must not contain spaces, tabs, or newlines. Check word boundaries in %wor tiers and main tier.",
            ),
        );
    }

    // Check for bullet marker (unit separator U+0015)
    if cleaned.as_bytes().contains(&0x15) {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(cleaned, word.span, cleaned),
                "Word contains illegal bullet marker (U+0015)",
            )
            .with_suggestion(
                "Bullet markers should not be part of word text. This is likely a parser bug.",
            ),
        );
    }

    // Check for other control characters (excluding those that are part of CHAT syntax)
    for (idx, ch) in cleaned.char_indices() {
        if ch.is_control() && ch != '\u{0015}' {
            // Already checked bullet separately
            errors.report(
                ParseError::new(
                    ErrorCode::IllegalCharactersInWord,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new(cleaned, word.span, cleaned),
                    format!("Word contains illegal control character U+{:04X} at position {}", ch as u32, idx),
                )
                .with_suggestion(
                    "Words must contain only printable characters (Unicode alphabetic, numbers, and CHAT-allowed symbols).",
                ),
            );
        }
    }
}

/// Validate that shortening markers use properly nested parentheses.
///
/// Uses stack-based validation to ensure proper pairing, not just counting.
pub(crate) fn check_shortening_balance(word: &Word, errors: &impl ErrorSink) {
    let mut depth = 0i32;

    // Use raw_text to preserve parser-recovered boundary information.
    for ch in word.raw_text.chars() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth < 0 {
                errors.report(
                    ParseError::new(
                        ErrorCode::UnbalancedShortening,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Closing parenthesis ')' without corresponding opening '('",
                    )
                    .with_suggestion(
                        "Ensure each closing ')' has a matching opening '(' before it",
                    ),
                );
                // Reset depth to prevent cascading errors
                depth = 0;
            }
        }
    }

    // Check for unclosed parentheses
    if depth > 0 {
        errors.report(
            ParseError::new(
                ErrorCode::UnbalancedShortening,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                format!(
                    "Unbalanced shortening markers: {} unclosed opening '('",
                    depth
                ),
            )
            .with_suggestion("Ensure each opening '(' has a matching closing ')'"),
        );
    }
}

/// Validate `+` compound marker placement within a token.
///
/// Compound markers must separate non-empty lexical segments, so leading,
/// trailing, or doubled markers are all rejected.
pub(crate) fn check_compound_markers(word: &Word, errors: &impl ErrorSink) {
    if matches!(word.content.first(), Some(WordContent::CompoundMarker(_))) {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidCompoundMarkerPosition,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot start a word",
            )
            .with_suggestion("Remove the leading '+' or attach it to the previous word"),
        );
    }

    if matches!(word.content.last(), Some(WordContent::CompoundMarker(_))) {
        errors.report(
            ParseError::new(
                ErrorCode::EmptyCompoundPart,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot have an empty trailing part",
            )
            .with_suggestion("Add content after '+' or remove the trailing marker"),
        );
    }

    if word.content.windows(2).any(|window| {
        matches!(
            window,
            [
                WordContent::CompoundMarker(_),
                WordContent::CompoundMarker(_)
            ]
        )
    }) {
        errors.report(
            ParseError::new(
                ErrorCode::EmptyCompoundPart,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.cleaned_text(), word.span, word.cleaned_text()),
                "Compound marker '+' cannot have empty parts (++)",
            )
            .with_suggestion("Remove one '+' or add content between compound markers"),
        );
    }
}

/// Map common non-canonical untranscribed placeholders to canonical forms.
///
/// Returns the recommended CHAT token (for example `xxx`, `yyy`, `www`) when
/// the input matches a known uppercase/short variant.
pub fn get_illegal_untranscribed_suggestion(text: &str) -> Option<&'static str> {
    match text {
        "xx" | "XXX" => Some("xxx"),
        "yy" | "YYY" => Some("yyy"),
        "WWW" => Some("www"),
        _ => None,
    }
}

/// Return whether a stress marker is primary (`ˈ`).
///
/// Small helper to keep pattern checks readable in prosodic validation.
fn is_primary_stress(marker_type: WordStressMarkerType) -> bool {
    matches!(marker_type, WordStressMarkerType::Primary)
}

/// Return whether a stress marker is secondary (`ˌ`).
///
/// Small helper to keep pattern checks readable in prosodic validation.
fn is_secondary_stress(marker_type: WordStressMarkerType) -> bool {
    matches!(marker_type, WordStressMarkerType::Secondary)
}

/// Validate prosodic marker placement in word content.
///
/// Rules:
/// - E244: Multiple consecutive stress markers are invalid (ˈˌtest)
/// - E245: Stress must be before spoken material, not at word end or before another marker
/// - E246: Lengthening (colon) must be after spoken material, not at word start
/// - E247: Only one primary stress per word allowed
/// - E250: Secondary stress requires primary stress in the same word
/// - E252: Syllable pause (^) must be between spoken material
pub(crate) fn check_prosodic_markers(word: &Word, errors: &impl ErrorSink) {
    let content = &word.content;

    // Count stress markers for E247 and E250
    let mut primary_stress_count = 0;
    let mut secondary_stress_count = 0;

    for item in content.iter() {
        if let WordContent::StressMarker(marker) = item {
            if is_primary_stress(marker.marker_type) {
                primary_stress_count += 1;
            } else if is_secondary_stress(marker.marker_type) {
                secondary_stress_count += 1;
            }
        }
    }

    // E247: Only one primary stress per word
    if primary_stress_count > 1 {
        errors.report(
            ParseError::new(
                ErrorCode::MultiplePrimaryStress,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                format!(
                    "Word has {} primary stress markers, but only one is allowed",
                    primary_stress_count
                ),
            )
            .with_suggestion("A word can have at most one primary stress (ˈ)"),
        );
    }

    // E250: Secondary stress requires primary stress
    if secondary_stress_count > 0 && primary_stress_count == 0 {
        errors.report(
            ParseError::new(
                ErrorCode::SecondaryStressWithoutPrimary,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Word has secondary stress (ˌ) but no primary stress (ˈ)",
            )
            .with_suggestion(
                "Secondary stress only makes sense when there is also a primary stress marker",
            ),
        );
    }

    for (i, item) in content.iter().enumerate() {
        // E244: Check for consecutive stress markers
        if matches!(item, WordContent::StressMarker(_)) {
            if matches!(content.get(i + 1), Some(WordContent::StressMarker(_))) {
                errors.report(
                    ParseError::new(
                        ErrorCode::ConsecutiveStressMarkers,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Multiple consecutive stress markers",
                    )
                    .with_suggestion(
                        "A syllable can only have one stress marker (primary ˈ or secondary ˌ)",
                    ),
                );
            }

            // E245: Stress must be followed by spoken material
            let has_following_text = content[i + 1..].iter().any(is_spoken_material);

            if !has_following_text {
                errors.report(
                    ParseError::new(
                        ErrorCode::StressNotBeforeSpokenMaterial,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Stress marker not followed by spoken material",
                    )
                    .with_suggestion("Stress markers (ˈ ˌ) must precede the syllable they mark"),
                );
            }
        }

        // E246: Lengthening must be after spoken material
        if let WordContent::Lengthening(_) = item {
            let has_preceding_text = content[..i].iter().any(is_spoken_material);

            if !has_preceding_text {
                errors.report(
                    ParseError::new(
                        ErrorCode::LengtheningNotAfterSpokenMaterial,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Lengthening marker (:) not after spoken material",
                    )
                    .with_suggestion(
                        "Lengthening marker (:) must follow the syllable it lengthens (e.g., bana:nas)",
                    ),
                );
            }
        }

        // E252: Syllable pause must be between spoken material
        if let WordContent::SyllablePause(_) = item {
            let has_preceding_text = content[..i].iter().any(is_spoken_material);
            let has_following_text = content[i + 1..].iter().any(is_spoken_material);

            if !has_preceding_text || !has_following_text {
                errors.report(
                    ParseError::new(
                        ErrorCode::SyllablePauseNotBetweenSpokenMaterial,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Syllable pause marker (^) must be between spoken material",
                    )
                    .with_suggestion(
                        "Syllable pause (^) must occur between syllables (e.g., rhi^noceros)",
                    ),
                );
            }
        }
    }
}

/// Return whether a word-content item contributes spoken lexical material.
///
/// Prosodic placement checks use this to distinguish markers from segment text.
fn is_spoken_material(content: &WordContent) -> bool {
    matches!(content, WordContent::Text(text) if !text.as_ref().is_empty())
}

/// Return whether the word contains at least one spoken lexical segment.
///
/// This is used by higher-level validators that need to gate marker checks on
/// actual spoken content presence.
pub(crate) fn has_spoken_material(word: &Word) -> bool {
    word.content.iter().any(is_spoken_material)
}

/// Validate inline `@...` marker integrity from raw text.
///
/// This catches parser-recovery cases where malformed marker suffixes are split
/// into standalone ERROR nodes and would otherwise be lost.
pub(crate) fn check_inline_at_markers(word: &Word, errors: &impl ErrorSink) {
    let at_count = word
        .raw_text
        .as_bytes()
        .iter()
        .filter(|&&b| b == b'@')
        .count();
    if at_count == 0 {
        return;
    }

    if word.raw_text.ends_with('@') {
        errors.report(
            ParseError::new(
                ErrorCode::IllegalCharactersInWord,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Dangling '@' marker in word",
            )
            .with_suggestion("Remove '@' or provide a valid marker suffix"),
        );
    }

    if at_count > 1 {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidFormType,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Malformed form marker suffix",
            )
            .with_suggestion("Use exactly one valid form marker (e.g., @b, @c, @z:label)"),
        );
        return;
    }

    if let Some(form_type) = &word.form_type {
        let marker = format!("@{}", form_type.to_chat_marker());
        if let Some(marker_pos) = word.raw_text.rfind(&marker) {
            let trailing = &word.raw_text[marker_pos + marker.len()..];
            if !trailing.is_empty() && !trailing.starts_with('@') && !trailing.starts_with('$') {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidFormType,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                        "Invalid characters after form marker",
                    )
                    .with_suggestion(
                        "Use a valid marker suffix only (e.g., @b, @c, @s:eng, @z:label)",
                    ),
                );
            }
        }
    }

    if word.form_type.is_none() && word.lang.is_none() && !word.raw_text.ends_with('@') {
        errors.report(
            ParseError::new(
                ErrorCode::InvalidFormType,
                Severity::Error,
                SourceLocation::new(word.span),
                ErrorContext::new(word.raw_text(), word.span, word.raw_text()),
                "Unknown '@' marker suffix",
            )
            .with_suggestion("Use a valid marker like @b, @c, @s:eng, or @z:label"),
        );
    }
}
