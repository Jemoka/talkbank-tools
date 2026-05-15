//! Supertype matcher for all dependent-tier node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

/// Check if a node kind is a `dependent_tier` subtype
///
/// **Subtypes:** mor_dependent_tier, gra_dependent_tier, pho_dependent_tier, etc.
pub fn is_dependent_tier(kind: &str) -> bool {
    use crate::node_types::{
        ACT_DEPENDENT_TIER, ADD_DEPENDENT_TIER, ALT_DEPENDENT_TIER, COD_DEPENDENT_TIER,
        COH_DEPENDENT_TIER, COM_DEPENDENT_TIER, DEF_DEPENDENT_TIER, DEPENDENT_TIER,
        ENG_DEPENDENT_TIER, ERR_DEPENDENT_TIER, EXP_DEPENDENT_TIER, FAC_DEPENDENT_TIER,
        FLO_DEPENDENT_TIER, GLS_DEPENDENT_TIER, GPX_DEPENDENT_TIER, GRA_DEPENDENT_TIER,
        INT_DEPENDENT_TIER, MOD_DEPENDENT_TIER, MODSYL_DEPENDENT_TIER, MOR_DEPENDENT_TIER,
        ORT_DEPENDENT_TIER, PAR_DEPENDENT_TIER, PHO_DEPENDENT_TIER, PHOALN_DEPENDENT_TIER,
        PHOSYL_DEPENDENT_TIER, SIN_DEPENDENT_TIER, SIT_DEPENDENT_TIER, SPA_DEPENDENT_TIER,
        TIM_DEPENDENT_TIER, UNSUPPORTED_DEPENDENT_TIER, WOR_DEPENDENT_TIER, X_DEPENDENT_TIER,
    };

    matches!(
        kind,
        DEPENDENT_TIER
            | ACT_DEPENDENT_TIER
            | ADD_DEPENDENT_TIER
            | ALT_DEPENDENT_TIER
            | COD_DEPENDENT_TIER
            | COH_DEPENDENT_TIER
            | COM_DEPENDENT_TIER
            | DEF_DEPENDENT_TIER
            | ENG_DEPENDENT_TIER
            | ERR_DEPENDENT_TIER
            | EXP_DEPENDENT_TIER
            | FAC_DEPENDENT_TIER
            | FLO_DEPENDENT_TIER
            | GLS_DEPENDENT_TIER
            | GPX_DEPENDENT_TIER
            | GRA_DEPENDENT_TIER
            | INT_DEPENDENT_TIER
            | MOD_DEPENDENT_TIER
            | MODSYL_DEPENDENT_TIER
            | MOR_DEPENDENT_TIER
            | ORT_DEPENDENT_TIER
            | PHOALN_DEPENDENT_TIER
            | PHOSYL_DEPENDENT_TIER
            | PAR_DEPENDENT_TIER
            | PHO_DEPENDENT_TIER
            | SIN_DEPENDENT_TIER
            | SIT_DEPENDENT_TIER
            | SPA_DEPENDENT_TIER
            | TIM_DEPENDENT_TIER
            | UNSUPPORTED_DEPENDENT_TIER
            | WOR_DEPENDENT_TIER
            | X_DEPENDENT_TIER
    )
}
