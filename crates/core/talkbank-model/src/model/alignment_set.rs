//! Alignment metadata for tracking tier-to-tier correspondences.
//!
//! This module provides the [`AlignmentSet`] structure which stores pre-computed
//! alignment information between dependent tiers and their source content, plus
//! the alignable units used to derive those mappings.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::SpanShift;

use crate::Span;

/// Single alignable unit within a tier alignment domain.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct AlignmentUnit {
    /// Index in the alignment sequence (0-indexed).
    pub index: usize,

    /// Source span for this unit (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// Alignable units for each tier/domain pair.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SpanShift, Default)]
pub struct AlignmentUnits {
    /// Main-tier units for %mor alignment.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub main_mor: Vec<AlignmentUnit>,

    /// Main-tier units for %pho/%mod alignment.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub main_pho: Vec<AlignmentUnit>,

    /// Main-tier units for %sin alignment.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub main_sin: Vec<AlignmentUnit>,

    /// Main-tier units for %wor alignment.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub main_wor: Vec<AlignmentUnit>,

    /// %mor tier items.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub mor: Vec<AlignmentUnit>,

    /// %mor chunk units (for %gra alignment).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub mor_chunks: Vec<AlignmentUnit>,

    /// %gra relations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub gra: Vec<AlignmentUnit>,

    /// %pho items.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pho: Vec<AlignmentUnit>,

    /// %wor items (alignable content only).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wor: Vec<AlignmentUnit>,

    /// %mod items.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub mod_: Vec<AlignmentUnit>,

    /// %sin items.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sin: Vec<AlignmentUnit>,

    /// %modsyl words.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub modsyl: Vec<AlignmentUnit>,

    /// %phosyl words.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub phosyl: Vec<AlignmentUnit>,

    /// %phoaln words.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub phoaln: Vec<AlignmentUnit>,
}

/// Alignment metadata for an utterance's dependent tiers.
///
/// References:
/// - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
/// - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
/// - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SpanShift)]
pub struct AlignmentSet {
    /// Alignable unit lists for each tier/domain.
    pub units: AlignmentUnits,

    /// Main → %mor alignment (if %mor tier present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mor: Option<crate::alignment::MorAlignment>,

    /// %mor → %gra alignment (if both tiers present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gra: Option<crate::alignment::GraAlignment>,

    /// Main → %pho alignment (if %pho tier present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pho: Option<crate::alignment::PhoAlignment>,

    /// `%wor` timing sidecar (if `%wor` tier present).
    ///
    /// `%wor` is a timing-annotation record, not a structural alignment —
    /// see [`WorTimingSidecar`](crate::alignment::WorTimingSidecar) for the
    /// full rationale. The slot is `Some(Drifted { .. })` when a transcript
    /// has been edited since `align` ran; that is the normal stale state,
    /// not a validation error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wor_timings: Option<crate::alignment::WorTimingSidecar>,

    /// Main → %mod alignment (if %mod/%xpho tier present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mod_: Option<crate::alignment::PhoAlignment>,

    /// Main → %sin alignment (if %sin tier present).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sin: Option<crate::alignment::SinAlignment>,

    /// %modsyl → %mod alignment (tier-to-tier, word count).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modsyl: Option<crate::alignment::PhoAlignment>,

    /// %phosyl → %pho alignment (tier-to-tier, word count).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phosyl: Option<crate::alignment::PhoAlignment>,

    /// %phoaln → %mod & %pho alignment (tier-to-tier, word count).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phoaln: Option<crate::alignment::PhoAlignment>,
}

impl AlignmentSet {
    /// Creates a new alignment set from pre-built alignment units.
    pub fn new(units: AlignmentUnits) -> Self {
        Self {
            units,
            mor: None,
            gra: None,
            pho: None,
            wor_timings: None,
            mod_: None,
            sin: None,
            modsyl: None,
            phosyl: None,
            phoaln: None,
        }
    }

    /// Check if all tier alignments are error-free.
    ///
    /// `%wor` is intentionally absent from this computation: it is modeled as
    /// a timing sidecar ([`WorTimingSidecar`](crate::alignment::WorTimingSidecar))
    /// rather than a `TierAlignmentResult`, so it has no error stream to check.
    /// The `Drifted` variant is a data state, not a validation failure.
    pub fn is_error_free(&self) -> bool {
        let mor_ok = self.mor.as_ref().is_none_or(|a| a.is_error_free());
        let gra_ok = self.gra.as_ref().is_none_or(|a| a.is_error_free());
        let pho_ok = self.pho.as_ref().is_none_or(|a| a.is_error_free());
        let mod_ok = self.mod_.as_ref().is_none_or(|a| a.is_error_free());
        let sin_ok = self.sin.as_ref().is_none_or(|a| a.is_error_free());
        let modsyl_ok = self.modsyl.as_ref().is_none_or(|a| a.is_error_free());
        let phosyl_ok = self.phosyl.as_ref().is_none_or(|a| a.is_error_free());
        let phoaln_ok = self.phoaln.as_ref().is_none_or(|a| a.is_error_free());

        mor_ok && gra_ok && pho_ok && mod_ok && sin_ok && modsyl_ok && phosyl_ok && phoaln_ok
    }

    /// Collect all alignment errors for corpus validation output.
    ///
    /// `%wor` is absent from this collection for the same reason as
    /// [`is_error_free`](Self::is_error_free): it is a timing sidecar with
    /// no error stream. Drift is reported structurally via the
    /// [`WorTimingSidecar::Drifted`](crate::alignment::WorTimingSidecar::Drifted)
    /// variant, not via `ParseError`s.
    pub fn collect_errors(&self) -> Vec<&crate::ParseError> {
        let mut errors = Vec::new();

        if let Some(mor) = &self.mor {
            errors.extend(&mor.errors);
        }
        if let Some(gra) = &self.gra {
            errors.extend(&gra.errors);
        }
        if let Some(pho) = &self.pho {
            errors.extend(&pho.errors);
        }
        if let Some(mod_) = &self.mod_ {
            errors.extend(&mod_.errors);
        }
        if let Some(sin) = &self.sin {
            errors.extend(&sin.errors);
        }
        if let Some(modsyl) = &self.modsyl {
            errors.extend(&modsyl.errors);
        }
        if let Some(phosyl) = &self.phosyl {
            errors.extend(&phosyl.errors);
        }
        if let Some(phoaln) = &self.phoaln {
            errors.extend(&phoaln.errors);
        }

        errors
    }
}

impl Default for AlignmentSet {
    /// Builds an empty alignment set with no computed mappings.
    fn default() -> Self {
        Self::new(AlignmentUnits::default())
    }
}
