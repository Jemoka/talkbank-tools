//! Tier-kind and span helpers for `DependentTier`.
//!
//! These helpers provide stable identifiers used by duplicate detection,
//! indexing, and diagnostics without exposing enum-internal pattern matching
//! at call sites.
//! The `kind` accessor normalizes user-defined labels so `%xfoo` tiers do not
//! collide with standard names.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::DependentTier;

impl DependentTier {
    /// Returns the canonical dependent-tier identifier used in CHAT tags.
    ///
    /// For standard tiers this is the lowercase suffix (`"mor"`, `"gra"`, ...).
    /// For user-defined tiers this returns the stored custom label (including
    /// the leading `x`), preserving `%x*` namespace semantics. Callers should
    /// prefer this helper over ad-hoc pattern matching when building maps or
    /// duplicate-detection keys.
    pub fn kind(&self) -> &str {
        match self {
            DependentTier::Mor(_) => "mor",
            DependentTier::Gra(_) => "gra",
            DependentTier::Pho(_) => "pho",
            DependentTier::Mod(_) => "mod",
            DependentTier::Sin(_) => "sin",
            DependentTier::Act(_) => "act",
            DependentTier::Cod(_) => "cod",
            DependentTier::Add(_) => "add",
            DependentTier::Com(_) => "com",
            DependentTier::Exp(_) => "exp",
            DependentTier::Gpx(_) => "gpx",
            DependentTier::Int(_) => "int",
            DependentTier::Sit(_) => "sit",
            DependentTier::Spa(_) => "spa",
            DependentTier::Alt(_) => "alt",
            DependentTier::Coh(_) => "coh",
            DependentTier::Def(_) => "def",
            DependentTier::Eng(_) => "eng",
            DependentTier::Err(_) => "err",
            DependentTier::Fac(_) => "fac",
            DependentTier::Flo(_) => "flo",
            DependentTier::Modsyl(_) => "modsyl",
            DependentTier::Phosyl(_) => "phosyl",
            DependentTier::Phoaln(_) => "phoaln",
            DependentTier::Gls(_) => "gls",
            DependentTier::Ort(_) => "ort",
            DependentTier::Par(_) => "par",
            DependentTier::Tim(_) => "tim",
            DependentTier::Wor(_) => "wor",
            // User-defined tiers: label already includes 'x' prefix
            // e.g., %xmor stores label="xmor" to avoid collision with %mor
            DependentTier::UserDefined(tier) => &tier.label,
            // Unsupported tiers: label is the raw tier name (e.g., "foo" for %foo)
            DependentTier::Unsupported(tier) => &tier.label,
        }
    }

    /// Returns the source span associated with this dependent-tier value.
    ///
    /// This is used for diagnostics and provenance tracking; it does not affect
    /// semantic equality or serialization output. All variants expose spans
    /// through this accessor so caller code can stay enum-shape agnostic.
    pub fn span(&self) -> crate::Span {
        match self {
            DependentTier::Mor(t) => t.span,
            DependentTier::Gra(t) => t.span,
            DependentTier::Pho(t) => t.span,
            DependentTier::Mod(t) => t.span,
            DependentTier::Sin(t) => t.span,
            DependentTier::Act(t) => t.span,
            DependentTier::Cod(t) => t.span,
            DependentTier::Add(t) => t.span,
            DependentTier::Com(t) => t.span,
            DependentTier::Exp(t) => t.span,
            DependentTier::Gpx(t) => t.span,
            DependentTier::Int(t) => t.span,
            DependentTier::Sit(t) => t.span,
            DependentTier::Spa(t) => t.span,
            DependentTier::Alt(t) => t.span,
            DependentTier::Coh(t) => t.span,
            DependentTier::Def(t) => t.span,
            DependentTier::Eng(t) => t.span,
            DependentTier::Err(t) => t.span,
            DependentTier::Fac(t) => t.span,
            DependentTier::Flo(t) => t.span,
            DependentTier::Modsyl(t) => t.span,
            DependentTier::Phosyl(t) => t.span,
            DependentTier::Phoaln(t) => t.span,
            DependentTier::Gls(t) => t.span,
            DependentTier::Ort(t) => t.span,
            DependentTier::Par(t) => t.span,
            DependentTier::Tim(t) => t.span(),
            DependentTier::Wor(t) => t.span,
            DependentTier::UserDefined(t) => t.span,
            DependentTier::Unsupported(t) => t.span,
        }
    }
}
