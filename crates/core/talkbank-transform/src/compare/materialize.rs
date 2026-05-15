use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::dependent_tier::{Mor, MorTier};
use talkbank_model::model::{ChatFile, DependentTier, Line, Utterance};
use talkbank_model::{UtteranceIdx, WriteChat};

use crate::dependent_tiers::replace_or_add_tier;
use crate::diff::copy_dependent_tiers;
use crate::diff::preserve::TierKind;
use crate::extract::{self, ExtractedUtterance};

use super::engine::{collect_utterance_terminators, is_punct_or_filler};
use super::model::{CompareStatus, ComparisonBundle, UtteranceComparison};
use super::serialize::{
    CompareSerializationError, CompareTierLabel, CompareUserDefinedTier, XsmorTierContent,
    XsrepTierContent,
};

fn compared_word_counts(utts: &[ExtractedUtterance]) -> Vec<usize> {
    utts.iter()
        .map(|utt| {
            utt.words
                .iter()
                .filter(|word| !is_punct_or_filler(word.text.as_str()))
                .count()
        })
        .collect()
}

fn alignable_word_counts(utts: &[ExtractedUtterance]) -> Vec<usize> {
    utts.iter().map(|utt| utt.words.len()).collect()
}

fn collect_mor_items(chat_file: &ChatFile) -> Vec<Vec<Mor>> {
    let mut utterance_items = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            let mor_items = utt
                .mor_tier()
                .map(|mor| mor.items().iter().cloned().collect())
                .unwrap_or_default();
            utterance_items.push(mor_items);
        }
    }
    utterance_items
}

fn exact_projection_source(
    bundle: &ComparisonBundle,
    gold_utterance_index: usize,
    main_compared_word_counts: &[usize],
    gold_compared_word_counts: &[usize],
    main_alignable_word_counts: &[usize],
    gold_alignable_word_counts: &[usize],
) -> Option<usize> {
    let gold_word_count = *gold_compared_word_counts.get(gold_utterance_index)?;
    if gold_word_count == 0 {
        return None;
    }

    let matches: Vec<_> = bundle
        .gold_word_matches
        .iter()
        .copied()
        .filter(|item| item.gold_utterance_index == gold_utterance_index)
        .collect();
    if matches.len() != gold_word_count {
        return None;
    }

    let mut gold_positions: Vec<_> = matches.iter().map(|item| item.gold_word_position).collect();
    gold_positions.sort_unstable();
    gold_positions.dedup();
    if gold_positions.len() != gold_word_count {
        return None;
    }

    let mut main_pairs: Vec<_> = matches
        .iter()
        .map(|item| (item.main_utterance_index, item.main_word_position))
        .collect();
    main_pairs.sort_unstable();
    main_pairs.dedup();
    if main_pairs.len() != gold_word_count {
        return None;
    }

    let mut main_utterance_indices: Vec<_> = matches
        .iter()
        .map(|item| item.main_utterance_index)
        .collect();
    main_utterance_indices.sort_unstable();
    main_utterance_indices.dedup();
    if main_utterance_indices.len() != 1 {
        return None;
    }

    let main_utterance_index = main_utterance_indices[0];
    if main_compared_word_counts.get(main_utterance_index).copied() != Some(gold_word_count) {
        return None;
    }
    if main_alignable_word_counts
        .get(main_utterance_index)
        .copied()
        != gold_alignable_word_counts
            .get(gold_utterance_index)
            .copied()
    {
        return None;
    }

    let gold_tokens = bundle.gold_utterances.get(gold_utterance_index)?;
    if gold_tokens
        .tokens
        .iter()
        .any(|token| token.status != CompareStatus::Match)
    {
        return None;
    }

    Some(main_utterance_index)
}

fn build_projected_mor_tier(
    bundle: &ComparisonBundle,
    gold_utterance_index: usize,
    gold_word_count: usize,
    main_mor_items: &[Vec<Mor>],
    gold_mor_items: &[Vec<Mor>],
    gold_terminators: &[Option<String>],
) -> Option<MorTier> {
    if gold_word_count == 0 {
        return None;
    }

    let matches: Vec<_> = bundle
        .gold_word_matches
        .iter()
        .copied()
        .filter(|item| item.gold_utterance_index == gold_utterance_index)
        .collect();
    if matches.is_empty() {
        return None;
    }

    let mut gold_positions: Vec<_> = matches.iter().map(|item| item.gold_word_position).collect();
    gold_positions.sort_unstable();
    gold_positions.dedup();
    if gold_positions.len() != gold_word_count {
        return None;
    }

    let mut main_pairs: Vec<_> = matches
        .iter()
        .map(|item| (item.main_utterance_index, item.main_word_position))
        .collect();
    main_pairs.sort_unstable();
    main_pairs.dedup();
    if main_pairs.len() != matches.len() {
        return None;
    }

    let mut projected = gold_mor_items
        .get(gold_utterance_index)
        .filter(|items| items.len() == gold_word_count)
        .map(|items| items.iter().cloned().map(Some).collect())
        .unwrap_or_else(|| vec![None; gold_word_count]);

    for matched in matches {
        let mor = main_mor_items
            .get(matched.main_utterance_index)?
            .get(matched.main_word_position)?
            .clone();
        *projected.get_mut(matched.gold_word_position)? = Some(mor);
    }

    let items: Vec<Mor> = projected.into_iter().collect::<Option<Vec<_>>>()?;
    // Gold data may have no terminator at this position (legacy or
    // incomplete gold). MorTier requires a typed terminator, so we
    // return None when gold has none — the caller treats absent
    // gold-mor as "no comparison MorTier available," which is the
    // semantically correct behavior here. Stringly terminators in
    // gold are lifted to typed via Terminator::try_from_chat_str.
    let raw_terminator: String = gold_terminators
        .get(gold_utterance_index)
        .cloned()
        .flatten()?;
    let typed_terminator = talkbank_model::Terminator::try_from_chat_str(raw_terminator.trim())?;
    Some(MorTier::new_mor(items, typed_terminator))
}

fn replace_or_add_mor_tier(chat_file: &mut ChatFile, utterance_index: usize, mor: MorTier) {
    let mut utterance_count = 0usize;
    for line in chat_file.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            if utterance_count == utterance_index {
                replace_or_add_tier(&mut utt.dependent_tiers, DependentTier::Mor(mor));
                break;
            }
            utterance_count += 1;
        }
    }
}

/// Project structurally safe `%mor` / `%gra` / `%wor` annotations from main onto gold.
///
/// This keeps compare projection in the CHAT AST:
/// - exact utterance matches copy aligned dependent tiers wholesale
/// - full gold-word coverage without exact utterance identity still projects `%mor`
pub fn project_gold_structurally(
    main_file: &ChatFile,
    gold_file: &ChatFile,
    bundle: &ComparisonBundle,
) -> ChatFile {
    let mut projected = gold_file.clone();
    let main_utts = extract::extract_words(main_file, TierDomain::Mor);
    let gold_utts = extract::extract_words(gold_file, TierDomain::Mor);
    let main_compared_word_counts = compared_word_counts(&main_utts);
    let gold_compared_word_counts = compared_word_counts(&gold_utts);
    let main_alignable_word_counts = alignable_word_counts(&main_utts);
    let gold_alignable_word_counts = alignable_word_counts(&gold_utts);
    let main_mor_items = collect_mor_items(main_file);
    let gold_mor_items = collect_mor_items(gold_file);
    let gold_terminators = collect_utterance_terminators(gold_file);

    for gold_utterance_index in 0..gold_utts.len() {
        if let Some(main_utterance_index) = exact_projection_source(
            bundle,
            gold_utterance_index,
            &main_compared_word_counts,
            &gold_compared_word_counts,
            &main_alignable_word_counts,
            &gold_alignable_word_counts,
        ) {
            copy_dependent_tiers(
                main_file,
                UtteranceIdx(main_utterance_index),
                &mut projected,
                UtteranceIdx(gold_utterance_index),
                &[TierKind::Mor, TierKind::Gra, TierKind::Wor],
            );
            continue;
        }

        if let Some(projected_mor) = build_projected_mor_tier(
            bundle,
            gold_utterance_index,
            gold_compared_word_counts[gold_utterance_index],
            &main_mor_items,
            &gold_mor_items,
            &gold_terminators,
        ) {
            replace_or_add_mor_tier(&mut projected, gold_utterance_index, projected_mor);
        }
    }

    projected
}

/// Inject comparison results into a CHAT file as `%xsrep` and `%xsmor` tiers.
///
/// For each [`UtteranceComparison`], finds the corresponding utterance in the
/// file (by `utterance_index`) and adds user-defined tiers containing
/// the formatted comparison annotations.
///
/// Uses `replace_or_add_tier` to ensure idempotent injection.
pub fn inject_comparison(
    chat_file: &mut ChatFile,
    utterances: &[UtteranceComparison],
) -> Result<(), CompareSerializationError> {
    let mut utt_line_indices: Vec<usize> = Vec::new();
    for (line_idx, line) in chat_file.lines.iter().enumerate() {
        if matches!(line, Line::Utterance(_)) {
            utt_line_indices.push(line_idx);
        }
    }

    for utt_comparison in utterances {
        if utt_comparison.tokens.is_empty() {
            continue;
        }

        let utt_idx = utt_comparison.utterance_index;
        if utt_idx >= utt_line_indices.len() {
            tracing::warn!(
                utt_idx,
                num_utterances = utt_line_indices.len(),
                "Compare utterance_index out of range"
            );
            continue;
        }

        let line_idx = utt_line_indices[utt_idx];
        let xsrep_tier = CompareUserDefinedTier {
            label: CompareTierLabel::xsrep(),
            content: XsrepTierContent::try_from(utt_comparison)?,
        };
        let xsmor_tier = CompareUserDefinedTier {
            label: CompareTierLabel::xsmor(),
            content: XsmorTierContent::try_from(utt_comparison)?,
        };

        if let Some(Line::Utterance(utt)) = chat_file.lines.get_mut(line_idx) {
            replace_or_add_user_defined_tier(utt, xsrep_tier)?;
            replace_or_add_user_defined_tier(utt, xsmor_tier)?;
        }
    }

    Ok(())
}

fn replace_or_add_user_defined_tier<T: WriteChat>(
    utterance: &mut Utterance,
    tier: CompareUserDefinedTier<T>,
) -> Result<(), CompareSerializationError> {
    let new_tier = tier.into_dependent_tier()?;
    replace_or_add_tier(&mut utterance.dependent_tiers, new_tier);
    Ok(())
}

/// Remove existing `%xsrep` and `%xsmor` tiers from all utterances.
pub fn clear_comparison(chat_file: &mut ChatFile) {
    for line in chat_file.lines.iter_mut() {
        if let Line::Utterance(utt) = line {
            utt.dependent_tiers.retain(|tier| {
                !matches!(
                    tier,
                    DependentTier::UserDefined(ud)
                        if ud.label.as_str() == "xsrep" || ud.label.as_str() == "xsmor"
                )
            });
        }
    }
}
