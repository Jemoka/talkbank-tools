use crate::Utterance;
use crate::model::{AlignmentUnit, AlignmentUnits};
use crate::validation::ValidationContext;

impl AlignmentUnits {
    /// Build alignment unit inventories for every alignable tier in an utterance.
    pub fn from_utterance(utterance: &Utterance, _context: &ValidationContext) -> Self {
        let mut units = AlignmentUnits {
            main_mor: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Mor,
            ),
            main_pho: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Pho,
            ),
            main_sin: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Sin,
            ),
            main_wor: build_main_units(
                &utterance.main.content.content,
                crate::alignment::TierDomain::Wor,
            ),
            ..Default::default()
        };

        if let Some(tier) = utterance.mor_tier() {
            let item_count = tier.items.0.len();
            units.mor = (0..item_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
            let chunk_count = tier.count_chunks();
            units.mor_chunks = (0..chunk_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.gra_tier() {
            let item_count = tier.relations.0.len();
            units.gra = (0..item_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.pho_tier() {
            let pho_count = tier.items.0.len();
            units.pho = (0..pho_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.mod_tier() {
            let mod_count = tier.items.0.len();
            units.mod_ = (0..mod_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.wor_tier() {
            let wor_count = tier
                .items
                .iter()
                .filter(|item| matches!(item, crate::model::dependent_tier::WorItem::Word(_)))
                .count();
            units.wor = (0..wor_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.sin_tier() {
            let sin_count = tier.items.0.len();
            units.sin = (0..sin_count)
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.modsyl_tier() {
            units.modsyl = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.phosyl_tier() {
            units.phosyl = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        if let Some(tier) = utterance.phoaln_tier() {
            units.phoaln = (0..tier.word_count())
                .map(|index| AlignmentUnit { index, span: None })
                .collect();
        }

        units
    }
}

fn build_main_units(
    content: &[crate::model::UtteranceContent],
    domain: crate::alignment::TierDomain,
) -> Vec<AlignmentUnit> {
    let mut units = Vec::new();
    let mut index = 0;
    for item in content {
        let count = count_main_item_units(item, domain, false);
        if count > 0 {
            for _ in 0..count {
                units.push(AlignmentUnit { index, span: None });
                index += 1;
            }
        }
    }
    units
}

fn count_main_item_units(
    item: &crate::model::UtteranceContent,
    domain: crate::alignment::TierDomain,
    in_retrace: bool,
) -> usize {
    use crate::alignment::helpers::{annotations_have_alignment_ignore, is_tag_marker_separator};
    use crate::alignment::helpers::{counts_for_tier_in_context, should_skip_group};
    use crate::model::UtteranceContent;

    match item {
        UtteranceContent::Word(word) => {
            usize::from(counts_for_tier_in_context(word, domain, in_retrace))
        }
        UtteranceContent::AnnotatedWord(annotated) => {
            if domain == crate::alignment::TierDomain::Mor
                && annotations_have_alignment_ignore(&annotated.scoped_annotations)
            {
                0
            } else {
                usize::from(counts_for_tier_in_context(
                    &annotated.inner,
                    domain,
                    in_retrace,
                ))
            }
        }
        UtteranceContent::ReplacedWord(replaced) => match domain {
            crate::alignment::TierDomain::Mor => {
                if annotations_have_alignment_ignore(&replaced.scoped_annotations) {
                    0
                } else if !replaced.replacement.words.is_empty() {
                    replaced
                        .replacement
                        .words
                        .iter()
                        .filter(|word| counts_for_tier_in_context(word, domain, in_retrace))
                        .count()
                } else {
                    usize::from(counts_for_tier_in_context(
                        &replaced.word,
                        domain,
                        in_retrace,
                    ))
                }
            }
            crate::alignment::TierDomain::Wor => usize::from(counts_for_tier_in_context(
                &replaced.word,
                domain,
                in_retrace,
            )),
            crate::alignment::TierDomain::Pho | crate::alignment::TierDomain::Sin => usize::from(
                crate::alignment::helpers::should_align_replaced_word_in_pho_sin(
                    &replaced.word,
                    !replaced.replacement.words.is_empty(),
                ),
            ),
        },
        UtteranceContent::Group(group) => {
            count_bracketed_units(&group.content.content, domain, in_retrace)
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            if should_skip_group(&annotated.scoped_annotations, domain) {
                0
            } else {
                count_bracketed_units(&annotated.inner.content.content, domain, in_retrace)
            }
        }
        UtteranceContent::PhoGroup(pho) => match domain {
            crate::alignment::TierDomain::Mor | crate::alignment::TierDomain::Wor => {
                count_bracketed_units(&pho.content.content, domain, in_retrace)
            }
            crate::alignment::TierDomain::Pho => 1,
            crate::alignment::TierDomain::Sin => 0,
        },
        UtteranceContent::SinGroup(sin) => match domain {
            crate::alignment::TierDomain::Mor | crate::alignment::TierDomain::Wor => {
                count_bracketed_units(&sin.content.content, domain, in_retrace)
            }
            crate::alignment::TierDomain::Sin => 1,
            crate::alignment::TierDomain::Pho => 0,
        },
        UtteranceContent::Quotation(quotation) => {
            count_bracketed_units(&quotation.content.content, domain, in_retrace)
        }
        UtteranceContent::Separator(sep) => {
            usize::from(domain == crate::alignment::TierDomain::Mor && is_tag_marker_separator(sep))
        }
        UtteranceContent::Pause(_) => usize::from(domain == crate::alignment::TierDomain::Pho),
        UtteranceContent::AnnotatedAction(_) => {
            usize::from(domain == crate::alignment::TierDomain::Sin)
        }
        UtteranceContent::Retrace(retrace) => {
            if domain == crate::alignment::TierDomain::Mor {
                0
            } else {
                count_bracketed_units(&retrace.content.content, domain, true)
            }
        }
        _ => 0,
    }
}

fn count_bracketed_units(
    items: &[crate::model::BracketedItem],
    domain: crate::alignment::TierDomain,
    in_retrace: bool,
) -> usize {
    use crate::alignment::helpers::counts_for_tier_in_context;
    use crate::alignment::helpers::{annotations_have_alignment_ignore, is_tag_marker_separator};
    use crate::alignment::helpers::{should_align_replaced_word_in_pho_sin, should_skip_group};
    use crate::model::BracketedItem;

    items
        .iter()
        .map(|item| match item {
            BracketedItem::Word(word) => {
                usize::from(counts_for_tier_in_context(word, domain, in_retrace))
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if domain == crate::alignment::TierDomain::Mor
                    && annotations_have_alignment_ignore(&annotated.scoped_annotations)
                {
                    0
                } else {
                    usize::from(counts_for_tier_in_context(
                        &annotated.inner,
                        domain,
                        in_retrace,
                    ))
                }
            }
            BracketedItem::ReplacedWord(replaced) => match domain {
                crate::alignment::TierDomain::Mor => {
                    if annotations_have_alignment_ignore(&replaced.scoped_annotations) {
                        0
                    } else if !replaced.replacement.words.is_empty() {
                        replaced
                            .replacement
                            .words
                            .iter()
                            .filter(|word| counts_for_tier_in_context(word, domain, in_retrace))
                            .count()
                    } else {
                        usize::from(counts_for_tier_in_context(
                            &replaced.word,
                            domain,
                            in_retrace,
                        ))
                    }
                }
                crate::alignment::TierDomain::Wor => usize::from(counts_for_tier_in_context(
                    &replaced.word,
                    domain,
                    in_retrace,
                )),
                crate::alignment::TierDomain::Pho | crate::alignment::TierDomain::Sin => {
                    usize::from(should_align_replaced_word_in_pho_sin(
                        &replaced.word,
                        !replaced.replacement.words.is_empty(),
                    ))
                }
            },
            BracketedItem::AnnotatedGroup(annotated) => {
                if should_skip_group(&annotated.scoped_annotations, domain) {
                    0
                } else {
                    count_bracketed_units(&annotated.inner.content.content, domain, in_retrace)
                }
            }
            BracketedItem::PhoGroup(pho) => match domain {
                crate::alignment::TierDomain::Mor | crate::alignment::TierDomain::Wor => {
                    count_bracketed_units(&pho.content.content, domain, in_retrace)
                }
                crate::alignment::TierDomain::Pho => 1,
                crate::alignment::TierDomain::Sin => 0,
            },
            BracketedItem::SinGroup(sin) => match domain {
                crate::alignment::TierDomain::Mor | crate::alignment::TierDomain::Wor => {
                    count_bracketed_units(&sin.content.content, domain, in_retrace)
                }
                crate::alignment::TierDomain::Sin => 1,
                crate::alignment::TierDomain::Pho => 0,
            },
            BracketedItem::Quotation(quotation) => {
                count_bracketed_units(&quotation.content.content, domain, in_retrace)
            }
            BracketedItem::Separator(sep) => usize::from(
                domain == crate::alignment::TierDomain::Mor && is_tag_marker_separator(sep),
            ),
            BracketedItem::Pause(_) => 0,
            BracketedItem::AnnotatedAction(_) | BracketedItem::Action(_) => 0,
            BracketedItem::Retrace(retrace) => {
                if domain == crate::alignment::TierDomain::Mor {
                    0
                } else {
                    count_bracketed_units(&retrace.content.content, domain, true)
                }
            }
            _ => 0,
        })
        .sum()
}
