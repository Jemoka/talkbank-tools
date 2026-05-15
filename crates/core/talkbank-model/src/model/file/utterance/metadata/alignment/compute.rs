use super::count_based::{
    build_mor_tier_from_items, build_phonology_alignment_from_counts,
    build_sin_alignment_from_counts, build_tier_to_tier_alignment,
};
use super::diagnostics::{
    first_non_dummy_span, skipped_alignment_warning, unknown_alignment_warning,
};
use crate::alignment::indices::{MainWordIndex, PhoItemIndex};
use crate::model::{AlignmentSet, AlignmentUnits, ParseHealthState, ParseHealthTier};
use crate::validation::ValidationContext;
use crate::{ErrorCode, ParseError, Span, Utterance};

/// One side of a tier alignment relationship (label, span, tier identity).
struct TierSide<'a> {
    label: &'a str,
    span: Span,
    tier: ParseHealthTier,
}

fn alignment_blocked_warning(
    health: ParseHealthState,
    alignment_name: &str,
    left: TierSide<'_>,
    right: TierSide<'_>,
) -> ParseError {
    match health {
        ParseHealthState::Unknown => unknown_alignment_warning(
            alignment_name,
            left.label,
            left.span,
            right.label,
            right.span,
        ),
        _ => skipped_alignment_warning(
            alignment_name,
            left.label,
            health.is_tier_clean(left.tier),
            left.span,
            right.label,
            health.is_tier_clean(right.tier),
            right.span,
        ),
    }
}

fn grouped_alignment_blocked_warning(
    health: ParseHealthState,
    alignment_name: &str,
    left: TierSide<'_>,
    right_label: &str,
    right_span: Span,
    right_clean: bool,
) -> ParseError {
    match health {
        ParseHealthState::Unknown => unknown_alignment_warning(
            alignment_name,
            left.label,
            left.span,
            right_label,
            right_span,
        ),
        _ => skipped_alignment_warning(
            alignment_name,
            left.label,
            health.is_tier_clean(left.tier),
            left.span,
            right_label,
            right_clean,
            right_span,
        ),
    }
}

impl Utterance {
    /// Recompute all derived alignment metadata for this utterance.
    pub fn compute_alignments(&mut self, context: &ValidationContext) {
        self.alignment_diagnostics.clear();

        let units = AlignmentUnits::from_utterance(self, context);
        let mut metadata = AlignmentSet::new(units);
        let health = self.parse_health;

        let (mor_items, mor_span) = if let Some(tier) = self.mor_tier() {
            (Some(tier.items.0.clone()), tier.span)
        } else {
            (None, Span::DUMMY)
        };
        let (gra_relations, gra_span) = if let Some(tier) = self.gra_tier() {
            (Some(tier.relations.0.clone()), tier.span)
        } else {
            (None, Span::DUMMY)
        };
        let pho_span = self.pho_tier().map_or(Span::DUMMY, |t| t.span);
        let mod_span = self.mod_tier().map_or(Span::DUMMY, |t| t.span);
        let sin_span = self.sin_tier().map_or(Span::DUMMY, |t| t.span);

        if let Some(items) = mor_items.as_ref() {
            // build_mor_tier_from_items returns None when the utterance has
            // no existing %mor: tier to inherit terminator/span from. In
            // that case alignment metadata is also absent — there's nothing
            // to align against the main tier.
            if let Some(mor) = build_mor_tier_from_items(self, items) {
                if health.can_align_main_to_mor() {
                    metadata.mor = Some(crate::alignment::align_main_to_mor(&self.main, &mor));
                } else {
                    metadata.mor = Some(crate::alignment::MorAlignment::new().with_error(
                        alignment_blocked_warning(
                            health,
                            "main↔%mor",
                            TierSide {
                                label: "main tier",
                                span: self.main.span,
                                tier: ParseHealthTier::Main,
                            },
                            TierSide {
                                label: "%mor tier",
                                span: mor_span,
                                tier: ParseHealthTier::Mor,
                            },
                        ),
                    ));
                }
            }
        }

        if let (Some(items), Some(relations)) = (mor_items.as_ref(), gra_relations.as_ref()) {
            if health.can_align_mor_to_gra() {
                if let Some(mor) = build_mor_tier_from_items(self, items) {
                    let gra = crate::model::GraTier::new_gra(relations.clone()).with_span(gra_span);
                    metadata.gra = Some(crate::alignment::align_mor_to_gra(&mor, &gra));
                }
            } else {
                metadata.gra = Some(crate::alignment::GraAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%mor↔%gra",
                        TierSide {
                            label: "%mor tier",
                            span: mor_span,
                            tier: ParseHealthTier::Mor,
                        },
                        TierSide {
                            label: "%gra tier",
                            span: gra_span,
                            tier: ParseHealthTier::Gra,
                        },
                    ),
                ));
            }
        }

        if let Some(tier) = self.pho_tier() {
            let item_count = tier.items.0.len();
            if health.can_align_main_to_pho() {
                metadata.pho = Some(build_phonology_alignment_from_counts(
                    &self.main,
                    item_count,
                    pho_span,
                    "%pho",
                    crate::ErrorCode::PhoCountMismatchTooFew,
                    crate::ErrorCode::PhoCountMismatchTooMany,
                ));
            } else {
                metadata.pho = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%pho",
                        TierSide {
                            label: "main tier",
                            span: self.main.span,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%pho tier",
                            span: pho_span,
                            tier: ParseHealthTier::Pho,
                        },
                    ),
                ));
            }
        }

        if let Some(wor) = self.wor_tier().cloned() {
            // `%wor` is a timing sidecar, not a `TierAlignmentResult`. On
            // parse-taint (`!can_resolve_wor_timing_sidecar()`) we leave the slot as
            // `None`; unlike the structural alignments above there is no
            // error stream to populate, and the sidecar has nothing
            // meaningful to report for tainted input. Callers that need
            // per-tier taint context should consult `ParseHealth` directly.
            if health.can_resolve_wor_timing_sidecar() {
                metadata.wor_timings = Some(crate::alignment::resolve_wor_timing_sidecar(
                    &self.main, &wor,
                ));
            }
        }

        if let Some(tier) = self.mod_tier() {
            let item_count = tier.items.0.len();
            if health.can_align_main_to_mod() {
                metadata.mod_ = Some(build_phonology_alignment_from_counts(
                    &self.main,
                    item_count,
                    mod_span,
                    "%mod",
                    crate::ErrorCode::ModCountMismatchTooFew,
                    crate::ErrorCode::ModCountMismatchTooMany,
                ));
            } else {
                metadata.mod_ = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%mod",
                        TierSide {
                            label: "main tier",
                            span: self.main.span,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%mod tier",
                            span: mod_span,
                            tier: ParseHealthTier::Mod,
                        },
                    ),
                ));
            }
        }

        if let Some(tier) = self.sin_tier() {
            let item_count = tier.items.0.len();
            if health.can_align_main_to_sin() {
                metadata.sin = Some(build_sin_alignment_from_counts(
                    &self.main, item_count, sin_span,
                ));
            } else {
                metadata.sin = Some(crate::alignment::SinAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "main↔%sin",
                        TierSide {
                            label: "main tier",
                            span: self.main.span,
                            tier: ParseHealthTier::Main,
                        },
                        TierSide {
                            label: "%sin tier",
                            span: sin_span,
                            tier: ParseHealthTier::Sin,
                        },
                    ),
                ));
            }
        }

        let modsyl_span = self.modsyl_tier().map_or(Span::DUMMY, |t| t.span);
        let phosyl_span = self.phosyl_tier().map_or(Span::DUMMY, |t| t.span);
        let phoaln_span = self.phoaln_tier().map_or(Span::DUMMY, |t| t.span);

        if let (Some(modsyl), Some(mod_tier)) = (self.modsyl_tier(), self.mod_tier()) {
            if health.can_align_modsyl_to_mod() {
                metadata.modsyl = Some(build_tier_to_tier_alignment(
                    modsyl.word_count(),
                    modsyl_span,
                    "%modsyl",
                    mod_tier.items.0.len(),
                    mod_span,
                    "%mod",
                    ErrorCode::ModsylModCountMismatch,
                ));
            } else {
                metadata.modsyl = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%modsyl↔%mod",
                        TierSide {
                            label: "%modsyl tier",
                            span: modsyl_span,
                            tier: ParseHealthTier::Modsyl,
                        },
                        TierSide {
                            label: "%mod tier",
                            span: mod_span,
                            tier: ParseHealthTier::Mod,
                        },
                    ),
                ));
            }
        }

        if let (Some(phosyl), Some(pho_tier)) = (self.phosyl_tier(), self.pho_tier()) {
            if health.can_align_phosyl_to_pho() {
                metadata.phosyl = Some(build_tier_to_tier_alignment(
                    phosyl.word_count(),
                    phosyl_span,
                    "%phosyl",
                    pho_tier.items.0.len(),
                    pho_span,
                    "%pho",
                    ErrorCode::PhosylPhoCountMismatch,
                ));
            } else {
                metadata.phosyl = Some(crate::alignment::PhoAlignment::new().with_error(
                    alignment_blocked_warning(
                        health,
                        "%phosyl↔%pho",
                        TierSide {
                            label: "%phosyl tier",
                            span: phosyl_span,
                            tier: ParseHealthTier::Phosyl,
                        },
                        TierSide {
                            label: "%pho tier",
                            span: pho_span,
                            tier: ParseHealthTier::Pho,
                        },
                    ),
                ));
            }
        }

        if let Some(phoaln) = self.phoaln_tier() {
            let phoaln_wc = phoaln.word_count();
            if health.can_align_phoaln() {
                let mut alignment = crate::alignment::PhoAlignment::new();
                let mod_count = self.mod_tier().map(|t| t.items.0.len());
                let pho_count = self.pho_tier().map(|t| t.items.0.len());

                if let Some(mc) = mod_count
                    && phoaln_wc != mc
                {
                    alignment =
                        alignment.with_error(super::diagnostics::build_count_mismatch_error(
                            phoaln_wc,
                            phoaln_span,
                            "%phoaln",
                            mc,
                            mod_span,
                            "%mod",
                            ErrorCode::PhoalnModCountMismatch,
                        ));
                }
                if let Some(pc) = pho_count
                    && phoaln_wc != pc
                {
                    alignment =
                        alignment.with_error(super::diagnostics::build_count_mismatch_error(
                            phoaln_wc,
                            phoaln_span,
                            "%phoaln",
                            pc,
                            pho_span,
                            "%pho",
                            ErrorCode::PhoalnPhoCountMismatch,
                        ));
                }

                let effective_count = mod_count
                    .unwrap_or(phoaln_wc)
                    .min(pho_count.unwrap_or(phoaln_wc))
                    .min(phoaln_wc);
                for i in 0..effective_count {
                    alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
                        Some(MainWordIndex::new(i)),
                        Some(PhoItemIndex::new(i)),
                    ));
                }

                metadata.phoaln = Some(alignment);
            } else {
                metadata.phoaln = Some(crate::alignment::PhoAlignment::new().with_error(
                    grouped_alignment_blocked_warning(
                        health,
                        "%phoaln↔%mod/%pho",
                        TierSide {
                            label: "%phoaln tier",
                            span: phoaln_span,
                            tier: ParseHealthTier::Phoaln,
                        },
                        "%mod/%pho tiers",
                        first_non_dummy_span([mod_span, pho_span]),
                        health.is_tier_clean(ParseHealthTier::Mod)
                            && health.is_tier_clean(ParseHealthTier::Pho),
                    ),
                ));
            }
        }

        self.alignment_diagnostics = metadata.collect_errors().into_iter().cloned().collect();
        self.alignments = Some(metadata);
    }

    /// Recompute alignments using a default validation context.
    pub fn compute_alignments_default(&mut self) {
        self.compute_alignments(&ValidationContext::default());
    }

    /// Return `true` when no alignment diagnostics are currently recorded.
    pub fn alignments_valid(&self) -> bool {
        self.alignment_diagnostics.is_empty()
    }

    /// Return borrowed alignment diagnostics currently attached to the utterance.
    pub fn collect_alignment_errors(&self) -> Vec<&crate::ParseError> {
        self.alignment_diagnostics.iter().collect()
    }
}
