//! Dispatch for simple text-like dependent tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::model::dependent_tier::DependentTier;
use crate::model::{TextTier, Utterance};
use crate::node_types::*;
use talkbank_model::ParseOutcome;
use talkbank_model::model::dependent_tier::{
    PhoalnTier, SylTier, SylTierType, parse_phoaln_content, parse_syl_content,
};
use tree_sitter::Node;

use super::helpers::extract_unparsed_tier_content;

/// Apply a raw (text) tier to the utterance.
///
/// Returns `true` if this tier type was handled (even if content extraction failed).
/// Returns `false` if this is not a raw tier type.
///
/// If content extraction fails (None), the tier is NOT added to the utterance
/// and an error has already been reported.
pub(super) fn apply_raw_tier(
    utterance: &mut Utterance,
    tier_kind: &str,
    tier_node: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> bool {
    let span = Span::new(tier_node.start_byte() as u32, tier_node.end_byte() as u32);

    match tier_kind {
        ORT_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Ort(TextTier::new(content).with_span(span)));
            }
        }
        ENG_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Eng(TextTier::new(content).with_span(span)));
            }
        }
        GLS_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Gls(TextTier::new(content).with_span(span)));
            }
        }
        ALT_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Alt(TextTier::new(content).with_span(span)));
            }
        }
        COH_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Coh(TextTier::new(content).with_span(span)));
            }
        }
        DEF_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Def(TextTier::new(content).with_span(span)));
            }
        }
        ERR_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Err(TextTier::new(content).with_span(span)));
            }
        }
        FAC_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Fac(TextTier::new(content).with_span(span)));
            }
        }
        FLO_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Flo(TextTier::new(content).with_span(span)));
            }
        }
        PAR_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance
                    .dependent_tiers
                    .push(DependentTier::Par(TextTier::new(content).with_span(span)));
            }
        }
        TIM_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                utterance.dependent_tiers.push(DependentTier::Tim(
                    crate::model::dependent_tier::TimTier::from_text(content).with_span(span),
                ));
            }
        }
        MODSYL_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                let words = parse_syl_content(content.as_str());
                utterance.dependent_tiers.push(DependentTier::Modsyl(
                    SylTier::new(SylTierType::Modsyl, words).with_span(span),
                ));
            }
        }
        PHOSYL_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                let words = parse_syl_content(content.as_str());
                utterance.dependent_tiers.push(DependentTier::Phosyl(
                    SylTier::new(SylTierType::Phosyl, words).with_span(span),
                ));
            }
        }
        PHOALN_DEPENDENT_TIER => {
            if let ParseOutcome::Parsed(content) =
                extract_unparsed_tier_content(tier_node, input, errors)
            {
                match parse_phoaln_content(content.as_str()) {
                    Ok(words) => {
                        utterance.dependent_tiers.push(DependentTier::Phoaln(
                            PhoalnTier::new(words).with_span(span),
                        ));
                    }
                    Err(e) => {
                        errors.report(ParseError::new(
                            ErrorCode::InvalidDependentTier,
                            Severity::Error,
                            SourceLocation::from_offsets(
                                tier_node.start_byte(),
                                tier_node.end_byte(),
                            ),
                            ErrorContext::new(
                                input,
                                tier_node.start_byte()..tier_node.end_byte(),
                                "%phoaln",
                            ),
                            format!("malformed %phoaln content: {}", e),
                        ));
                    }
                }
            }
        }
        _ => return false,
    }

    true
}
