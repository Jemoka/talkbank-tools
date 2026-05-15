//! Builder-style constructors for assembling `Utterance` values.
//!
//! These helpers prioritize ergonomics for tests and programmatic transcript
//! construction. All `with_*` helpers append tiers in call order, which is
//! also the order preserved during CHAT serialization.
//! The builders never mutate existing tiers—each call returns a fresh `Utterance`
//! with the new tier appended so callers can chain safely even inside `map`.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::Utterance;
use crate::model::dependent_tier::wor::WorTier;
use crate::model::dependent_tier::*;
use crate::model::{
    Header, MainTier, NonEmptyString, ParseHealthState, ParseHealthTier, UtteranceLanguage,
    UtteranceLanguageMetadata,
};
use smallvec::SmallVec;

impl Utterance {
    /// Create a new utterance with only its required main tier.
    ///
    /// Runtime-derived metadata fields start in their uncomputed/empty state.
    /// Callers can then append dependent tiers in explicit serialization order
    /// via the `with_*`/`add_dependent_tier` helpers.
    pub fn new(main: MainTier) -> Self {
        Self {
            preceding_headers: SmallVec::new(),
            main,
            dependent_tiers: SmallVec::new(),
            alignments: None,
            alignment_diagnostics: Vec::new(),
            parse_health: ParseHealthState::Clean,
            utterance_language: UtteranceLanguage::Uncomputed,
            language_metadata: UtteranceLanguageMetadata::Uncomputed,
        }
    }

    /// Marks one tier as parse-tainted for downstream alignment gating.
    ///
    /// Use this after parser recovery when a specific tier parsed with damage
    /// but the utterance should still flow through later pipeline stages.
    pub fn mark_parse_taint(&mut self, tier: ParseHealthTier) {
        self.parse_health.taint(tier);
    }

    /// Marks all alignment-relevant dependent tiers as parse-tainted.
    ///
    /// This is the coarse fallback used when recovery quality is unclear and
    /// alignments should prefer "skip with diagnostics" behavior.
    pub fn mark_all_dependent_alignment_taint(&mut self) {
        self.parse_health.taint_all_alignment_dependents();
    }

    /// Replaces preceding interstitial headers (`@Comment`, `@Bg`, `@Eg`, `@G`).
    ///
    /// Header order is preserved exactly and emitted before the main tier when
    /// serializing this utterance.
    pub fn with_preceding_headers(mut self, headers: impl Into<SmallVec<[Header; 2]>>) -> Self {
        self.preceding_headers = headers.into();
        self
    }

    /// Appends one dependent tier in serialization order.
    ///
    /// Ordering is significant for roundtrip fidelity and duplicate-tier diagnostics.
    pub fn add_dependent_tier(mut self, tier: DependentTier) -> Self {
        self.dependent_tiers.push(tier);
        self
    }

    // ========================================================================
    // Convenience appenders for specific tier types (backward compatibility).
    // ========================================================================

    /// Append a `%mor` tier.
    pub fn with_mor(self, mor: MorTier) -> Self {
        self.add_dependent_tier(DependentTier::Mor(mor))
    }

    /// Append a `%gra` tier.
    pub fn with_gra(self, gra: GraTier) -> Self {
        self.add_dependent_tier(DependentTier::Gra(gra))
    }

    /// Append a `%pho` tier.
    pub fn with_pho(self, pho: PhoTier) -> Self {
        self.add_dependent_tier(DependentTier::Pho(pho))
    }

    /// Append a `%mod` tier.
    pub fn with_mod_tier(self, mod_tier: PhoTier) -> Self {
        self.add_dependent_tier(DependentTier::Mod(mod_tier))
    }

    /// Append a `%sin` tier.
    pub fn with_sin(self, sin: SinTier) -> Self {
        self.add_dependent_tier(DependentTier::Sin(sin))
    }

    /// Append a `%wor` tier.
    pub fn with_wor(self, wor: WorTier) -> Self {
        self.add_dependent_tier(DependentTier::Wor(wor))
    }

    /// Append a `%act` tier.
    pub fn with_act(self, act: ActTier) -> Self {
        self.add_dependent_tier(DependentTier::Act(act))
    }

    /// Append a `%cod` tier.
    pub fn with_cod(self, cod: CodTier) -> Self {
        self.add_dependent_tier(DependentTier::Cod(cod))
    }

    /// Append a `%com` tier.
    pub fn with_com(self, com: ComTier) -> Self {
        self.add_dependent_tier(DependentTier::Com(com))
    }

    /// Append a `%exp` tier.
    pub fn with_exp(self, exp: ExpTier) -> Self {
        self.add_dependent_tier(DependentTier::Exp(exp))
    }

    /// Append a `%add` tier.
    pub fn with_add(self, add: AddTier) -> Self {
        self.add_dependent_tier(DependentTier::Add(add))
    }

    /// Append a `%spa` tier.
    pub fn with_spa(self, spa: SpaTier) -> Self {
        self.add_dependent_tier(DependentTier::Spa(spa))
    }

    /// Append a `%sit` tier.
    pub fn with_sit(self, sit: SitTier) -> Self {
        self.add_dependent_tier(DependentTier::Sit(sit))
    }

    /// Append a `%gpx` tier.
    pub fn with_gpx(self, gpx: GpxTier) -> Self {
        self.add_dependent_tier(DependentTier::Gpx(gpx))
    }

    /// Append a `%int` tier.
    pub fn with_int(self, int: IntTier) -> Self {
        self.add_dependent_tier(DependentTier::Int(int))
    }

    /// Append a `%ort` tier.
    pub fn with_ort(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Ort(content))
    }

    /// Append a `%eng` tier.
    pub fn with_eng(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Eng(content))
    }

    /// Append a `%gls` tier.
    pub fn with_gls(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Gls(content))
    }

    /// Append a `%alt` tier.
    pub fn with_alt(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Alt(content))
    }

    /// Append a `%coh` tier.
    pub fn with_coh(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Coh(content))
    }

    /// Append a `%def` tier.
    pub fn with_def(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Def(content))
    }

    /// Append a `%err` tier.
    pub fn with_err(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Err(content))
    }

    /// Append a `%fac` tier.
    pub fn with_fac(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Fac(content))
    }

    /// Append a `%flo` tier.
    pub fn with_flo(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Flo(content))
    }

    /// Append a `%par` tier.
    pub fn with_par(self, content: TextTier) -> Self {
        self.add_dependent_tier(DependentTier::Par(content))
    }

    /// Append a `%tim` tier.
    pub fn with_tim(self, content: crate::model::dependent_tier::TimTier) -> Self {
        self.add_dependent_tier(DependentTier::Tim(content))
    }

    /// Appends a user-defined dependent tier (`%xLABEL`).
    ///
    /// The label is stored as provided (after non-empty validation) so custom
    /// project tiers can roundtrip without schema changes.
    pub fn with_user_defined(self, label: NonEmptyString, content: NonEmptyString) -> Self {
        self.add_dependent_tier(DependentTier::UserDefined(UserDefinedDependentTier {
            label,
            content,
            span: crate::Span::DUMMY,
        }))
    }
}
