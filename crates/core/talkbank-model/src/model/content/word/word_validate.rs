//! [`Validate`] implementations for [`Word`] and [`WordContents`].

use crate::validation::ValidationContext;
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

use super::content::WordContent;
use super::word_contents::WordContents;
use super::word_type::Word;

impl crate::validation::Validate for WordContents {
    /// Ensures words contain at least one content element and validates each element.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.0.is_empty() {
            let span = match context.field_span {
                Some(span) => span,
                None => crate::Span::from_usize(0, 0),
            };
            let location = match context.field_span {
                Some(span) => crate::SourceLocation::new(span),
                None => crate::SourceLocation::at_offset(0),
            };
            // DEFAULT: Missing field text is reported as empty to match the offending value.
            let source_text = context.field_text.clone().unwrap_or_default();
            // DEFAULT: Missing label falls back to "word" for error messaging.
            let label = context.field_label.unwrap_or("word");

            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::EmptyWordContent,
                    crate::Severity::Error,
                    location,
                    crate::ErrorContext::new(source_text.clone(), span, label),
                    "Word content cannot be empty",
                )
                .with_suggestion("Add at least one word content element"),
            );
            return;
        }

        for item in &self.0 {
            item.validate(context, errors);
        }
    }
}

impl crate::validation::Validate for Word {
    /// Validates structural, language, and mode-specific invariants for one parsed word token.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        use crate::validation::word::language::LanguageResolutionOutcome;
        use crate::validation::word::{language, resolve_word_language, structure};

        // E243: Check for illegal characters (whitespace, bullets, control chars)
        // This must run FIRST to catch parser bugs
        structure::check_word_characters(self, errors);

        // E203/E243/E248: Inline marker integrity checks.
        structure::check_inline_at_markers(self, errors);

        // E231: Check shortening marker balance
        structure::check_shortening_balance(self, errors);

        // E232-E233: Check compound marker validity
        structure::check_compound_markers(self, errors);

        // E244-E247, E250: Check prosodic marker placement and semantics
        structure::check_prosodic_markers(self, errors);

        // Validate word content elements via WordContents (text/shortenings)
        self.content.validate(
            &context
                .clone()
                .with_field_span(self.span)
                .with_field_text(self.raw_text.clone())
                .with_field_label("word"),
            errors,
        );

        // E209: Check if word has no spoken content
        let cleaned = self.cleaned_text();
        if !structure::has_spoken_material(self) && self.untranscribed().is_none() {
            errors.report(
                ParseError::new(
                    ErrorCode::EmptySpokenContent,
                    Severity::Error,
                    SourceLocation::new(self.span),
                    ErrorContext::new(cleaned, self.span, cleaned),
                    "Word has no spoken content",
                )
                .with_suggestion(
                    "Words must have phonetic content or be marked as untranscribed (xxx, yyy, www)",
                ),
            );
        }

        // E220: Language-specific word validation (numeric digits)
        let tier_language = context
            .tier_language
            .as_ref()
            .or(context.shared.default_language.as_ref());

        // Only validate digits if we have real language context
        if tier_language.is_some() {
            let LanguageResolutionOutcome {
                resolution,
                diagnostics,
            } = resolve_word_language(self, tier_language, &context.shared.declared_languages);
            errors.report_all(diagnostics);

            language::check_word_digits_multi(self, &resolution, errors);
        }

        // CA omission handling
        if matches!(self.category, Some(crate::model::WordCategory::CAOmission)) {
            if !context.shared.ca_mode {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidWordFormat,
                        Severity::Error,
                        SourceLocation::new(self.span),
                        ErrorContext::new(self.raw_text.as_str(), self.span, self.raw_text.as_str()),
                        "CA omission '(word)' used outside CA mode",
                    )
                    .with_suggestion(
                        "Use @Options: CA (or CA-Unicode) for CA omissions, or use 0word for omissions in standard CHAT",
                    ),
                );
            }

            let has_spoken_text = self
                .content
                .iter()
                .any(|item| matches!(item, WordContent::Text(text) if !text.as_ref().is_empty()));
            let has_invalid_lexical = self.content.iter().any(|item| {
                matches!(
                    item,
                    WordContent::Shortening(_) | WordContent::CompoundMarker(_)
                )
            });
            if !has_spoken_text || has_invalid_lexical {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidWordFormat,
                        Severity::Error,
                        SourceLocation::new(self.span),
                        ErrorContext::new(self.raw_text.as_str(), self.span, self.raw_text.as_str()),
                        "CA omission must include spoken text and must not contain shortenings",
                    )
                    .with_suggestion(
                        "Represent CA omission as text content (word) with optional non-lexical markers, not shortening or compound markers",
                    ),
                );
            }
        }

        if context.shared.ca_mode {
            let is_standalone_shortening =
                self.content.len() == 1 && matches!(self.content[0], WordContent::Shortening(_));
            if is_standalone_shortening {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidWordFormat,
                        Severity::Error,
                        SourceLocation::new(self.span),
                        ErrorContext::new(
                            self.raw_text.as_str(),
                            self.span,
                            self.raw_text.as_str(),
                        ),
                        "Standalone shortening should be represented as CA omission in CA mode",
                    )
                    .with_suggestion(
                        "Use CA omission semantics: category=ca_omission with plain text content",
                    ),
                );
            }
        }

        // E241: Check for illegal untranscribed types
        if context.shared.ca_mode {
            // CA mode treats omission markers differently; skip standard untranscribed checks.
        } else if let Some(suggested) = structure::get_illegal_untranscribed_suggestion(cleaned) {
            errors.report(
                ParseError::new(
                    ErrorCode::IllegalUntranscribed,
                    Severity::Error,
                    SourceLocation::new(self.span),
                    ErrorContext::new(cleaned, self.span, cleaned),
                    format!(
                        "\"{}\" is not legal; did you mean to use \"{}\"?",
                        cleaned, suggested
                    ),
                )
                .with_suggestion(format!(
                    "Use \"{}\" instead. Modern CHAT format requires lowercase triple letters for untranscribed words.",
                    suggested
                )),
            );
        }
    }
}
