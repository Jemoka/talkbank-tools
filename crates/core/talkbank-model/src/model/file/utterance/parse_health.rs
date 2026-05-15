//! Parse-health state used to gate cross-tier alignment checks.
//!
//! When parsers recover from malformed content, they may still produce a partial
//! tier value so downstream consumers can continue. `ParseHealth` records which
//! tiers were recovered (tainted) so alignment validation can skip comparisons
//! that would otherwise produce false positives from corrupted intermediate data.
//!
//! CHAT reference anchors:
//! - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [Morphological tier](https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier)
//! - [Grammatical relations tier](https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier)
//! - [Phonology tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology)
//! - [Word tier](https://talkbank.org/0info/manuals/CHAT.html#Word_Tier)
//! - [Gestures](https://talkbank.org/0info/manuals/CHAT.html#Gestures)

/// Runtime parse provenance for one utterance.
///
/// `Unknown` means the current utterance value did not come from a parser path
/// that explicitly established whether recovery was needed, so downstream
/// alignment code must not quietly assume the content is parse-clean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParseHealthState {
    /// No explicit parse provenance has been attached yet.
    #[default]
    Unknown,
    /// Parser-backed provenance established that no tracked tier required recovery.
    Clean,
    /// Parser-backed provenance established that one or more tracked tiers were recovered.
    Tainted(ParseHealth),
}

/// Runtime parse provenance used to decide whether alignment comparisons are trustworthy.
///
/// The value stores one taint bit per [`ParseHealthTier`]. `Default` means no
/// taint bits are set, so all tracked tiers are considered clean until a parser
/// marks recovery-tainted domains.
///
/// # References
///
/// - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseHealth {
    /// Bitset of recovery-tainted tiers keyed by [`ParseHealthTier`].
    tainted_tiers: u16,
}

/// Tier domains tracked by parse-health taint bits.
///
/// # References
///
/// - [Dependent tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseHealthTier {
    /// Main utterance tier.
    Main,
    /// Morphological analysis tier (%mor).
    Mor,
    /// Grammatical relations tier (%gra).
    Gra,
    /// Phonological transcription tier (%pho).
    Pho,
    /// Word-level timing tier (%wor).
    Wor,
    /// Model phonology tier (%mod).
    Mod,
    /// Gesture/sign tier (%sin).
    Sin,
    /// Syllabified model phonology tier (%modsyl).
    Modsyl,
    /// Syllabified actual phonology tier (%phosyl).
    Phosyl,
    /// Phone alignment tier (%phoaln).
    Phoaln,
}

impl ParseHealthTier {
    /// Return the taint bit corresponding to this tier.
    const fn bit(self) -> u16 {
        1u16 << (self as u8)
    }
}

impl Default for ParseHealth {
    /// Starts all tiers as clean until a parser marks recovery-tainted domains.
    fn default() -> Self {
        Self { tainted_tiers: 0 }
    }
}

impl ParseHealth {
    /// Build explicit runtime provenance from one concrete parse-health bitset.
    pub fn into_state(self) -> ParseHealthState {
        if self.is_clean() {
            ParseHealthState::Clean
        } else {
            ParseHealthState::Tainted(self)
        }
    }

    /// Returns `true` when no tracked tier has been tainted.
    ///
    /// This is a coarse health gate useful for quick checks before running any
    /// cross-tier validations.
    pub fn is_clean(&self) -> bool {
        self.tainted_tiers == 0
    }

    /// Returns `true` when the given tier has not been tainted.
    pub fn is_tier_clean(&self, tier: ParseHealthTier) -> bool {
        self.tainted_tiers & tier.bit() == 0
    }

    /// Returns `true` when the given tier has already been tainted.
    pub fn is_tier_tainted(&self, tier: ParseHealthTier) -> bool {
        !self.is_tier_clean(tier)
    }

    /// Mark one tier as tainted due to parse recovery.
    ///
    /// Taint marks signal that downstream alignment on that tier may produce
    /// misleading diagnostics and should be conditionally skipped.
    pub fn taint(&mut self, tier: ParseHealthTier) {
        self.tainted_tiers |= tier.bit();
    }

    /// Mark all dependent-tier alignment domains as tainted.
    ///
    /// Used when parser recovery cannot isolate impact to a single dependent
    /// tier family (for example, malformed unknown dependent-tier content that
    /// can shift parsing boundaries for following dependent tiers).
    pub fn taint_all_alignment_dependents(&mut self) {
        self.tainted_tiers |= ParseHealthTier::Mor.bit()
            | ParseHealthTier::Gra.bit()
            | ParseHealthTier::Pho.bit()
            | ParseHealthTier::Wor.bit()
            | ParseHealthTier::Mod.bit()
            | ParseHealthTier::Sin.bit()
            | ParseHealthTier::Modsyl.bit()
            | ParseHealthTier::Phosyl.bit()
            | ParseHealthTier::Phoaln.bit();
    }

    /// Returns whether main-tier to `%mor` alignment checks are safe to run.
    ///
    /// Requires both source (main) and target (`%mor`) tiers to be parse-clean.
    pub fn can_align_main_to_mor(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Main) && self.is_tier_clean(ParseHealthTier::Mor)
    }

    /// Returns whether `%mor` to `%gra` alignment checks are safe to run.
    ///
    /// This gate prevents cascading errors when either morphology or dependency
    /// parsing has recovered from malformed input.
    pub fn can_align_mor_to_gra(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Mor) && self.is_tier_clean(ParseHealthTier::Gra)
    }

    /// Returns whether main-tier to `%pho` alignment checks are safe to run.
    ///
    /// Both the main tier and `%pho` tier must be untainted.
    pub fn can_align_main_to_pho(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Main) && self.is_tier_clean(ParseHealthTier::Pho)
    }

    /// Returns whether main-tier to `%wor` alignment checks are safe to run.
    ///
    /// `%wor` alignment is skipped when parse recovery may have shifted tier slots.
    pub fn can_resolve_wor_timing_sidecar(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Main) && self.is_tier_clean(ParseHealthTier::Wor)
    }

    /// Returns whether main-tier to `%mod` alignment checks are safe to run.
    ///
    /// `%mod` is treated symmetrically with `%pho` for parse-health gating.
    pub fn can_align_main_to_mod(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Main) && self.is_tier_clean(ParseHealthTier::Mod)
    }

    /// Returns whether main-tier to `%sin` alignment checks are safe to run.
    ///
    /// Gesture/sign alignment should be suppressed whenever either side is tainted.
    pub fn can_align_main_to_sin(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Main) && self.is_tier_clean(ParseHealthTier::Sin)
    }

    /// Returns whether `%modsyl` to `%mod` alignment checks are safe to run.
    ///
    /// Both tiers must be parse-clean for tier-to-tier word count comparison.
    pub fn can_align_modsyl_to_mod(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Modsyl) && self.is_tier_clean(ParseHealthTier::Mod)
    }

    /// Returns whether `%phosyl` to `%pho` alignment checks are safe to run.
    pub fn can_align_phosyl_to_pho(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Phosyl) && self.is_tier_clean(ParseHealthTier::Pho)
    }

    /// Returns whether `%phoaln` to `%mod` and `%pho` alignment checks are safe to run.
    ///
    /// `%phoaln` aligns with both `%mod` and `%pho`, so all three must be clean.
    pub fn can_align_phoaln(&self) -> bool {
        self.is_tier_clean(ParseHealthTier::Phoaln)
            && self.is_tier_clean(ParseHealthTier::Mod)
            && self.is_tier_clean(ParseHealthTier::Pho)
    }
}

impl ParseHealthState {
    /// Returns `true` when parse provenance is missing.
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Returns `true` when parser-backed provenance marked every tracked tier clean.
    pub const fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }

    /// Returns `true` when the given tier is known clean.
    pub fn is_tier_clean(self, tier: ParseHealthTier) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.is_tier_clean(tier),
        }
    }

    /// Returns `true` when the given tier is known tainted.
    pub fn is_tier_tainted(self, tier: ParseHealthTier) -> bool {
        match self {
            Self::Unknown | Self::Clean => false,
            Self::Tainted(health) => health.is_tier_tainted(tier),
        }
    }

    /// Returns `true` when main-tier to `%mor` alignment checks are safe to run.
    pub fn can_align_main_to_mor(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_main_to_mor(),
        }
    }

    /// Returns `true` when `%mor` to `%gra` alignment checks are safe to run.
    pub fn can_align_mor_to_gra(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_mor_to_gra(),
        }
    }

    /// Returns `true` when main-tier to `%pho` alignment checks are safe to run.
    pub fn can_align_main_to_pho(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_main_to_pho(),
        }
    }

    /// Returns `true` when main-tier to `%wor` alignment checks are safe to run.
    pub fn can_resolve_wor_timing_sidecar(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_resolve_wor_timing_sidecar(),
        }
    }

    /// Returns `true` when main-tier to `%mod` alignment checks are safe to run.
    pub fn can_align_main_to_mod(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_main_to_mod(),
        }
    }

    /// Returns `true` when main-tier to `%sin` alignment checks are safe to run.
    pub fn can_align_main_to_sin(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_main_to_sin(),
        }
    }

    /// Returns `true` when `%modsyl` to `%mod` alignment checks are safe to run.
    pub fn can_align_modsyl_to_mod(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_modsyl_to_mod(),
        }
    }

    /// Returns `true` when `%phosyl` to `%pho` alignment checks are safe to run.
    pub fn can_align_phosyl_to_pho(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_phosyl_to_pho(),
        }
    }

    /// Returns `true` when `%phoaln` cross-tier checks are safe to run.
    pub fn can_align_phoaln(self) -> bool {
        match self {
            Self::Unknown => false,
            Self::Clean => true,
            Self::Tainted(health) => health.can_align_phoaln(),
        }
    }

    /// Mark one tier as tainted due to parser recovery.
    pub fn taint(&mut self, tier: ParseHealthTier) {
        match self {
            Self::Unknown | Self::Clean => {
                let mut health = ParseHealth::default();
                health.taint(tier);
                *self = Self::Tainted(health);
            }
            Self::Tainted(health) => health.taint(tier),
        }
    }

    /// Mark all dependent-tier alignment domains as tainted.
    pub fn taint_all_alignment_dependents(&mut self) {
        match self {
            Self::Unknown | Self::Clean => {
                let mut health = ParseHealth::default();
                health.taint_all_alignment_dependents();
                *self = Self::Tainted(health);
            }
            Self::Tainted(health) => health.taint_all_alignment_dependents(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseHealth, ParseHealthState, ParseHealthTier};

    /// The default parse-health state marks every tracked tier as clean.
    #[test]
    fn default_health_is_clean() {
        let health = ParseHealth::default();

        assert!(health.is_clean());
        assert!(health.is_tier_clean(ParseHealthTier::Main));
        assert!(health.is_tier_clean(ParseHealthTier::Mor));
        assert!(health.is_tier_clean(ParseHealthTier::Phoaln));
    }

    /// Tainting one tier affects only that tier.
    #[test]
    fn taint_marks_only_the_requested_tier() {
        let mut health = ParseHealth::default();
        health.taint(ParseHealthTier::Gra);

        assert!(health.is_tier_clean(ParseHealthTier::Main));
        assert!(health.is_tier_tainted(ParseHealthTier::Gra));
        assert!(health.is_tier_clean(ParseHealthTier::Mor));
    }

    /// Unknown dependent-tier recovery taints every dependent alignment tier.
    #[test]
    fn taint_all_alignment_dependents_leaves_main_clean() {
        let mut health = ParseHealth::default();
        health.taint_all_alignment_dependents();

        assert!(health.is_tier_clean(ParseHealthTier::Main));
        assert!(health.is_tier_tainted(ParseHealthTier::Mor));
        assert!(health.is_tier_tainted(ParseHealthTier::Gra));
        assert!(health.is_tier_tainted(ParseHealthTier::Phoaln));
    }

    /// Unknown parse provenance must not be treated as trusted-clean.
    #[test]
    fn unknown_state_is_not_clean() {
        let state = ParseHealthState::Unknown;

        assert!(state.is_unknown());
        assert!(!state.is_clean());
        assert!(!state.can_align_main_to_mor());
        assert!(!state.is_tier_clean(ParseHealthTier::Main));
    }

    /// Clean parse provenance allows alignment checks without carrying taint bits.
    #[test]
    fn clean_health_becomes_clean_state() {
        let state = ParseHealth::default().into_state();

        assert_eq!(state, ParseHealthState::Clean);
        assert!(state.can_align_main_to_mor());
        assert!(state.is_tier_clean(ParseHealthTier::Gra));
    }

    /// Taint helpers promote clean or unknown states into explicit tainted provenance.
    #[test]
    fn state_tainting_promotes_into_tainted_state() {
        let mut state = ParseHealthState::Unknown;
        state.taint(ParseHealthTier::Gra);

        assert!(state.is_tier_tainted(ParseHealthTier::Gra));
        assert!(!state.can_align_mor_to_gra());
    }
}
