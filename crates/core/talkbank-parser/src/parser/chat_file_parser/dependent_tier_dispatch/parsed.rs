//! Dispatch for dependent tiers with dedicated typed parsers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::error::ErrorSink;
use crate::model::Utterance;
use crate::model::dependent_tier::DependentTier;
use crate::node_types::*;
use crate::parser::tier_parsers::act::parse_act_tier;
use crate::parser::tier_parsers::cod::parse_cod_tier;
use crate::parser::tier_parsers::gra::parse_gra_tier;
use crate::parser::tier_parsers::mor::parse_mor_tier;
use crate::parser::tier_parsers::pho::{parse_mod_tier, parse_pho_tier};
use crate::parser::tier_parsers::sin::parse_sin_tier;
use crate::parser::tier_parsers::text::{
    parse_add_tier, parse_com_tier, parse_exp_tier, parse_gpx_tier, parse_int_tier, parse_sit_tier,
    parse_spa_tier,
};
use crate::parser::tier_parsers::wor::parse_wor_tier;
use talkbank_model::model::Terminator;
use talkbank_model::model::dependent_tier::{GraTier, MorTier};
use tree_sitter::Node;

/// Parse and attach tiers handled by typed tier parsers (`%mor`, `%gra`, `%pho`, etc.).
pub(super) fn apply_parsed_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    match tier_kind {
        MOR_DEPENDENT_TIER => {
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "mor", errors);
                utterance
                    .dependent_tiers
                    .push(DependentTier::Mor(empty_mor_placeholder()));
            } else {
                // Diagnostics for Rejected go through ErrorSink; the
                // recovered utterance still keeps the %mor slot in place so
                // downstream regeneration can mutate it in place without
                // reordering against later tiers such as %wor.
                match parse_mor_tier(tier_node, input, errors) {
                    talkbank_model::ParseOutcome::Parsed(tier) => {
                        utterance.dependent_tiers.push(DependentTier::Mor(tier));
                    }
                    talkbank_model::ParseOutcome::Rejected => {
                        utterance
                            .dependent_tiers
                            .push(DependentTier::Mor(empty_mor_placeholder()));
                    }
                }
            }
        }
        GRA_DEPENDENT_TIER => {
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "gra", errors);
                utterance
                    .dependent_tiers
                    .push(DependentTier::Gra(empty_gra_placeholder()));
            } else {
                let tier = parse_gra_tier(tier_node, input, errors);
                utterance.dependent_tiers.push(DependentTier::Gra(tier));
            }
        }
        PHO_DEPENDENT_TIER => {
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "pho", errors);
            } else {
                let tier = parse_pho_tier(tier_node, input, errors);
                utterance.dependent_tiers.push(DependentTier::Pho(tier));
            }
        }
        MOD_DEPENDENT_TIER => {
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "mod", errors);
            } else {
                let tier = parse_mod_tier(tier_node, input, errors);
                utterance.dependent_tiers.push(DependentTier::Mod(tier));
            }
        }
        COM_DEPENDENT_TIER => {
            let tier = parse_com_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Com(tier));
        }
        EXP_DEPENDENT_TIER => {
            let tier = parse_exp_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Exp(tier));
        }
        ADD_DEPENDENT_TIER => {
            let tier = parse_add_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Add(tier));
        }
        SPA_DEPENDENT_TIER => {
            let tier = parse_spa_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Spa(tier));
        }
        SIT_DEPENDENT_TIER => {
            let tier = parse_sit_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Sit(tier));
        }
        SIN_DEPENDENT_TIER => {
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "sin", errors);
            } else {
                let tier = parse_sin_tier(tier_node, input, errors);
                utterance.dependent_tiers.push(DependentTier::Sin(tier));
            }
        }
        COD_DEPENDENT_TIER => {
            let tier = parse_cod_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Cod(tier));
        }
        ACT_DEPENDENT_TIER => {
            let tier = parse_act_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Act(tier));
        }
        INT_DEPENDENT_TIER => {
            let tier = parse_int_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Int(tier));
        }
        GPX_DEPENDENT_TIER => {
            let tier = parse_gpx_tier(tier_node, input, errors);
            utterance.dependent_tiers.push(DependentTier::Gpx(tier));
        }
        WOR_DEPENDENT_TIER => {
            // %wor is a generated tier — if it has parse errors (e.g., legacy
            // CLAN data with groups/retraces), drop it rather than failing.
            // The validator still reports the error; align regenerates %wor.
            if tier_node.has_error() {
                report_tier_parse_error(tier_node, input, "wor", errors);
            } else {
                let tier = parse_wor_tier(tier_node, input, errors);
                utterance.dependent_tiers.push(DependentTier::Wor(tier));
            }
        }
        _ => return false,
    }

    true
}

/// Report a single summary error for a dependent tier that has parse errors.
///
/// This implements fail-fast: instead of parsing a broken tier element-by-element
/// (which cascades into many errors), we report one error and drop the tier.
fn report_tier_parse_error(tier_node: Node, input: &str, tier_name: &str, errors: &impl ErrorSink) {
    use crate::parser::tree_parsing::parser_helpers::error_analysis::analyze_dependent_tier_error_with_context;

    // Count error nodes for the summary message
    let mut cursor = tier_node.walk();
    for child in tier_node.children(&mut cursor) {
        if child.is_error() || child.is_missing() {
            errors.report(analyze_dependent_tier_error_with_context(
                child,
                input,
                Some(tier_name),
            ));
        }
    }
}

fn empty_mor_placeholder() -> MorTier {
    MorTier::new_mor(
        Vec::new(),
        Terminator::Period {
            span: talkbank_model::Span::DUMMY,
        },
    )
}

fn empty_gra_placeholder() -> GraTier {
    GraTier::new_gra(Vec::new())
}
