//! Dependent tier preservation for incremental processing.
//!
//! When the diff engine determines that an utterance is unchanged, its
//! dependent tiers (%mor, %gra, %wor, etc.) from the "before" file can be
//! copied to the "after" file, avoiding unnecessary reprocessing.

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{ChatFile, DependentTier, Line};

use crate::dependent_tiers::replace_or_add_tier;

/// Which dependent tier kinds to copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierKind {
    /// %mor tier.
    Mor,
    /// %gra tier.
    Gra,
    /// %wor tier.
    Wor,
}

/// Check if a dependent tier matches any of the requested kinds.
fn tier_matches(tier: &DependentTier, kinds: &[TierKind]) -> bool {
    for kind in kinds {
        match kind {
            TierKind::Mor if matches!(tier, DependentTier::Mor(_)) => return true,
            TierKind::Gra if matches!(tier, DependentTier::Gra(_)) => return true,
            TierKind::Wor if matches!(tier, DependentTier::Wor(_)) => return true,
            _ => {}
        }
    }
    false
}

/// Copy specified dependent tiers from a "before" utterance to an "after" utterance.
///
/// Uses `replace_or_add_tier` for idempotent insertion — safe to call multiple times.
///
/// Returns the number of tiers copied.
pub fn copy_dependent_tiers(
    before: &ChatFile,
    before_idx: UtteranceIdx,
    after: &mut ChatFile,
    after_idx: UtteranceIdx,
    kinds: &[TierKind],
) -> usize {
    // First, collect the tiers to copy from the "before" file.
    let tiers_to_copy: Vec<DependentTier> = {
        let mut utt_count = 0usize;
        let mut result = Vec::new();
        for line in &before.lines {
            if let Line::Utterance(utt) = line {
                if utt_count == before_idx.raw() {
                    for tier in utt.dependent_tiers.iter() {
                        if tier_matches(tier, kinds) {
                            result.push(tier.clone());
                        }
                    }
                    break;
                }
                utt_count += 1;
            }
        }
        result
    };

    let copied = tiers_to_copy.len();

    // Then inject them into the "after" file.
    let mut utt_count = 0usize;
    for line in after.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            if utt_count == after_idx.raw() {
                for tier in tiers_to_copy {
                    replace_or_add_tier(&mut utt.dependent_tiers, tier);
                }
                break;
            }
            utt_count += 1;
        }
    }

    copied
}
