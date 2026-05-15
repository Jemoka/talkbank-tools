//! Utterance-scoped validation orchestration.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//!
//! This module acts as the canonical entrypoint for main-tier + dependent-tier
//! validation for a single utterance so callers (for example parsers or aligners)
//! can retain a single `ValidationContext`.

use super::Utterance;
use crate::ErrorSink;
use crate::model::ValidationTagged;
use crate::model::dependent_tier::DependentTier;
use crate::validation::Validate;
use crate::validation::ValidationContext;

impl Validate for Utterance {
    /// Performs utterance-local validation checks and reports diagnostics.
    ///
    /// This pass handles speaker consistency, main-tier structure, dependent-tier
    /// invariants, and already-computed alignment diagnostics for the utterance.
    /// Rule ordering is deliberate: foundational parse/speaker issues are
    /// reported first so later tier-level diagnostics have clearer context.
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
        // E522: speaker code used on `*SPEAKER:` must appear in @Participants.
        if !context.shared.participant_ids.is_empty()
            && !context.shared.participant_ids.contains(&self.main.speaker)
        {
            let speaker_str = self.main.speaker.as_str();
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::SpeakerNotDefined,
                    crate::Severity::Error,
                    crate::SourceLocation::new(self.main.speaker_span),
                    crate::ErrorContext::new(speaker_str, self.main.speaker_span, speaker_str),
                    format!(
                        "Speaker '{}' used on main tier but not declared in @Participants",
                        speaker_str
                    ),
                )
                .with_suggestion(format!(
                    "Add '{}' to the @Participants line, or correct the speaker code on this utterance",
                    speaker_str
                )),
            );
        }

        // Main-tier lexical and structural checks.
        self.main.validate(context, errors);

        // E242: Validate quotation balance
        crate::validation::utterance::check_quotation_balance(self, errors);

        // E356, E357: Validate underline markers are balanced
        crate::validation::utterance::check_underline_balance(self, errors);

        // E230: Validate CA delimiter balance
        crate::validation::utterance::check_ca_delimiter_balance(self, errors);

        // E373: Validate overlap index values
        // E348: Validate overlap marker pairing
        crate::validation::utterance::check_overlap_markers(self, context, errors);

        // E258: Validate no consecutive commas in document order
        crate::validation::utterance::check_consecutive_commas(self, errors);

        // E259: Validate commas are not preceded by non-spoken content
        crate::validation::utterance::check_comma_after_non_spoken(self, errors);

        // E401: Validate no duplicate dependent tiers
        crate::validation::utterance::check_no_duplicate_dependent_tiers(self, errors);

        // E604: `%gra` requires `%mor`.
        // Skip when `%mor` is parse-tainted: parser recovery may have dropped `%mor`
        // from the AST while still reporting the root parse issue elsewhere.
        let has_gra = self
            .dependent_tiers
            .iter()
            .any(|tier| matches!(tier, crate::model::dependent_tier::DependentTier::Gra(_)));
        let has_mor = self
            .dependent_tiers
            .iter()
            .any(|tier| matches!(tier, crate::model::dependent_tier::DependentTier::Mor(_)));
        let mor_tainted = self
            .parse_health
            .is_tier_tainted(crate::model::ParseHealthTier::Mor);
        if has_gra && !has_mor && !mor_tainted {
            errors.report(
                crate::ParseError::new(
                    crate::ErrorCode::GraWithoutMor,
                    crate::Severity::Error,
                    crate::SourceLocation::new(self.main.span),
                    crate::ErrorContext::new("", self.main.span, ""),
                    "%gra tier requires %mor tier to be present",
                )
                .with_suggestion("Add %mor tier before %gra, or remove %gra tier"),
            );
        }

        let gra_alignment_blocks_structure = self.alignment_diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                crate::ErrorCode::MorGraCountMismatch
                    | crate::ErrorCode::GraInvalidWordIndex
                    | crate::ErrorCode::GraInvalidHeadIndex
            )
        });

        // E721-E724: structural validation of `%gra` relations.
        //
        // If `%mor↔%gra` alignment has already established that the tier is
        // cardinality-broken or contains out-of-bounds indices, suppress the
        // downstream graph-structure checks. Otherwise we emit misleading
        // cascades like E713 + E722 for the same malformed relation set.
        if !gra_alignment_blocks_structure {
            for tier in &self.dependent_tiers {
                if let DependentTier::Gra(marker) = tier {
                    marker.validate_structure(errors);
                }
            }
        }

        // E711: `%mor` content validation (stems/suffixes/POS categories).
        for tier in &self.dependent_tiers {
            if let DependentTier::Mor(marker) = tier {
                marker.validate_content(errors);
            }
        }

        // Validate tier alignment diagnostics collected during alignment computation.
        if !self.alignment_diagnostics.is_empty() {
            errors.report_all(self.alignment_diagnostics.clone());
        }

        // Validate user-defined dependent tiers (e.g., `%xfoo`, `%xbar`).
        for tier in &self.dependent_tiers {
            if let DependentTier::UserDefined(tier) = tier {
                crate::validation::check_user_defined_tier_content(
                    &tier.label,
                    &tier.content,
                    tier.span,
                    errors,
                );
            }
        }

        // E603: Validate %tim tier content format.
        for tier in &self.dependent_tiers {
            if let DependentTier::Tim(tim_tier) = tier
                && tim_tier.has_validation_issue()
            {
                check_tim_tier_format(tim_tier.as_str(), tim_tier.span(), errors);
            }
        }

        // E605: Warn on unsupported (non-standard, non-%x) dependent tiers.
        for tier in &self.dependent_tiers {
            if let DependentTier::Unsupported(t) = tier {
                errors.report(
                    crate::ParseError::new(
                        crate::ErrorCode::UnsupportedDependentTier,
                        crate::Severity::Warning,
                        crate::SourceLocation::new(t.span),
                        crate::ErrorContext::new(
                            t.label.as_str(),
                            t.span,
                            t.label.as_str(),
                        ),
                        format!(
                            "Unsupported dependent tier '%{}'",
                            t.label
                        ),
                    )
                    .with_suggestion(
                        "Use a standard tier name (e.g., %mor, %gra) or prefix with 'x' for user-defined tiers (e.g., %xfoo)",
                    ),
                );
            }
        }
    }
}

/// E603: Emit warning for non-time-like `%tim` tier content.
fn check_tim_tier_format(content: &str, span: crate::Span, errors: &impl ErrorSink) {
    errors.report(
        crate::ParseError::new(
            crate::ErrorCode::InvalidTimTierFormat,
            crate::Severity::Warning,
            crate::SourceLocation::new(span),
            crate::ErrorContext::new(content, span, "tim"),
            format!("Invalid %tim tier format: '{}'", content.trim()),
        )
        .with_suggestion("Expected time format: HH:MM:SS or HH:MM:SS.mmm"),
    );
}

impl Utterance {
    /// Computes alignments first, then runs standard utterance validation.
    ///
    /// Use this entrypoint when validation results should include alignment
    /// diagnostics generated from current tier content. This mutates alignment
    /// caches before validation, so callers should treat it as a stateful pass.
    pub fn validate_with_alignment(
        &mut self,
        context: &ValidationContext,
        errors: &impl ErrorSink,
    ) {
        self.compute_alignments(context);
        self.validate(context, errors);
    }
}
