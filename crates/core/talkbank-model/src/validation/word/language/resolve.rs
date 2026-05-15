//! Resolves effective word language(s) before language-sensitive checks run.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>

use crate::model::{LanguageCode, Word, WordLanguageMarker};
use crate::{ErrorCode, ParseError, Severity, SourceLocation};
use talkbank_derive::ValidationTagged;

use super::helpers::{get_other_language, is_tertiary_language};

/// Resolution of a word's applicable languages for validation purposes.
///
/// Different language marker forms resolve to different language sets:
/// - **Single**: One definitive language (explicit marker, shortcut resolved, or tier default)
/// - **Multiple**: Code-mixed content with multiple languages.
/// - **Ambiguous**: Ambiguous between multiple languages.
/// - **Unresolved**: No language context is available, so language-specific checks are skipped.
///
/// Downstream validators choose their own quantification policy (for example
/// "valid in all" vs permissive "valid in any") over the resolved language set.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Multiple>
/// - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Ambiguous>
#[derive(Debug, Clone, PartialEq, Eq, ValidationTagged, serde::Serialize, serde::Deserialize)]
pub enum LanguageResolution {
    /// Single definite language
    Single(LanguageCode),
    /// Multiple languages (code-mixing): @s:eng+fra
    Multiple(Vec<LanguageCode>),
    /// Ambiguous between languages: @s:eng&spa
    Ambiguous(Vec<LanguageCode>),
    /// No language could be resolved
    #[validation_tag(error)]
    Unresolved,
}

impl LanguageResolution {
    /// Return the resolved language set used by downstream validators.
    ///
    /// For `Single`, this is a one-element slice. For `Multiple`/`Ambiguous`,
    /// this includes all member languages in marker order.
    pub fn languages(&self) -> &[LanguageCode] {
        match self {
            Self::Single(code) => std::slice::from_ref(code),
            Self::Multiple(codes) | Self::Ambiguous(codes) => codes,
            Self::Unresolved => &[],
        }
    }

    /// Apply an "all languages must pass" predicate to this resolution.
    ///
    /// This is a reusable helper for strict validators. Rules that need a
    /// different policy (such as "any language may pass") should iterate
    /// `languages()` directly.
    ///
    /// # Arguments
    /// * `check_fn` - Function that returns true if a language accepts the content
    pub fn is_valid_in_all<F>(&self, check_fn: F) -> bool
    where
        F: Fn(&LanguageCode) -> bool,
    {
        self.languages().iter().all(check_fn)
    }

    /// Render as a human-readable language identifier for error messages.
    pub fn as_display_string(&self) -> String {
        match self {
            Self::Single(code) => code.as_str().to_string(),
            Self::Multiple(codes) => codes
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join("+"),
            Self::Ambiguous(codes) => codes
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join("&"),
            Self::Unresolved => "<unresolved>".to_string(),
        }
    }
}

/// Outcome of word-language resolution.
///
/// Replaces the pre-2026-05-01 `(LanguageResolution, Vec<ParseError>)`
/// tuple seam with a named struct. The two fields express orthogonal
/// facts: what the resolver concluded, and which surface-syntax issues
/// surfaced during resolution. They are *not* a value/error pair —
/// every legal CHAT input still produces a resolution (possibly
/// `Unresolved`) plus zero or more diagnostics.
///
/// # Rule 6d compliance
///
/// When resolution genuinely cannot succeed (tertiary `@s` shortcut,
/// missing secondary language, no language context), the field
/// `resolution` is [`LanguageResolution::Unresolved`] — never a
/// fabricated fallback to the tier language. Callers that need a
/// concrete language for downstream validation must inspect the
/// variant and decide explicitly (skip the check, use a configured
/// default with provenance, etc.) rather than receiving a sentinel.
#[derive(Debug, Clone)]
pub struct LanguageResolutionOutcome {
    /// The resolver's verdict for this word.
    pub resolution: LanguageResolution,
    /// Surface-syntax diagnostics produced during resolution.
    /// May be non-empty even when `resolution` is a concrete language
    /// (e.g., `@s` in tertiary context emits a warning but resolves
    /// honestly to `Unresolved`).
    pub diagnostics: Vec<ParseError>,
}

/// Resolve the effective language set for one word token.
///
/// Returns a [`LanguageResolutionOutcome`] carrying the resolved
/// language (or `Unresolved`) and any surface-syntax diagnostics.
///
/// # Semantics
/// - **Explicit language**: Returns that language as Single
/// - **Shortcut @s**: Resolves to the "other" language in a dual-language context
/// - **Multiple languages @s:eng+fra**: Returns all listed languages as Multiple
/// - **Ambiguous languages @s:eng&spa**: Returns all listed languages as Ambiguous
/// - **No marker**: Returns tier language or `Unresolved` if no language context available
///
/// # Rule 6d
///
/// When `@s` cannot resolve (tertiary tier, missing secondary, no
/// context), the result is `LanguageResolution::Unresolved` together
/// with a populated `diagnostics` vec — not a fabricated `Single(tier)`.
pub fn resolve_word_language(
    word: &Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
) -> LanguageResolutionOutcome {
    let mut diagnostics = Vec::new();

    let resolution = match word.lang.as_ref() {
        Some(WordLanguageMarker::Shortcut) => {
            if let Some(current_lang) = tier_language {
                if is_tertiary_language(current_lang, declared_languages) {
                    diagnostics.push(
                        ParseError::new(
                            ErrorCode::TertiaryLanguageNeedsExplicitCode,
                            Severity::Error,
                            SourceLocation::new(word.span),
                            None,
                            format!(
                                "Language '{}' is tertiary, so @s shortcut needs explicit language code (e.g., @s:eng)",
                                current_lang.as_str()
                            ),
                        )
                    );
                    LanguageResolution::Unresolved
                } else {
                    match get_other_language(current_lang, declared_languages) {
                        Some(other_lang) => LanguageResolution::Single(other_lang),
                        None => {
                            diagnostics.push(
                                ParseError::new(
                                    ErrorCode::MissingLanguageContext,
                                    Severity::Error,
                                    SourceLocation::new(word.span),
                                    None,
                                    "No secondary language available for @s shortcut",
                                )
                                .with_suggestion("Either add a second language to @Languages header or use explicit language code (e.g., @s:spa)")
                            );
                            LanguageResolution::Unresolved
                        }
                    }
                }
            } else {
                diagnostics.push(ParseError::new(
                    ErrorCode::MissingLanguageContext,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    None,
                    "Cannot use @s shortcut: no language context available",
                ));
                LanguageResolution::Unresolved
            }
        }
        Some(WordLanguageMarker::Explicit(code)) => {
            if !declared_languages.is_empty() && !declared_languages.iter().any(|decl| decl == code)
            {
                diagnostics.push(
                    ParseError::new(
                        ErrorCode::UndeclaredExplicitWordLanguage,
                        Severity::Warning,
                        SourceLocation::new(word.span),
                        None,
                        format!(
                            "Explicit word language '{}' is not listed in @Languages",
                            code.as_str()
                        ),
                    )
                    .with_suggestion(format!(
                        "Add '{}' to @Languages or confirm the word-level override is intentional",
                        code.as_str()
                    )),
                );
            }
            LanguageResolution::Single(code.clone())
        }
        Some(WordLanguageMarker::Multiple(codes)) => {
            // Multiple languages mixed together (code-mixing)
            // Content must be valid in ALL component languages
            LanguageResolution::Multiple(codes.clone())
        }
        Some(WordLanguageMarker::Ambiguous(codes)) => {
            // Ambiguous between languages
            // Content must be valid in ALL possibilities
            LanguageResolution::Ambiguous(codes.clone())
        }
        None => {
            if let Some(tier_lang) = tier_language {
                LanguageResolution::Single(tier_lang.clone())
            } else {
                // No marker and no tier/default language context.
                // This is not necessarily a word-level error, but it must not fabricate a language.
                LanguageResolution::Unresolved
            }
        }
    };

    LanguageResolutionOutcome {
        resolution,
        diagnostics,
    }
}
