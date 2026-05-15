use super::diagnostics::build_count_mismatch_error;
use crate::Utterance;
use crate::alignment::indices::{MainWordIndex, PhoItemIndex, SinItemIndex};
use crate::{ErrorCode, ErrorContext, ParseError, Severity, Span};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use crate::model::{MainTier, Terminator, UtteranceContent, Word};

    /// Helper: two-word main tier.
    fn two_word_main() -> MainTier {
        MainTier::new(
            "CHI",
            vec![
                UtteranceContent::Word(Box::new(Word::new_unchecked("one", "one"))),
                UtteranceContent::Word(Box::new(Word::new_unchecked("two", "two"))),
            ],
            Terminator::Period { span: Span::DUMMY },
        )
        .with_span(Span::from_usize(0, 20))
    }

    /// Helper: one-word main tier.
    fn one_word_main() -> MainTier {
        MainTier::new(
            "CHI",
            vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
                "one", "one",
            )))],
            Terminator::Period { span: Span::DUMMY },
        )
        .with_span(Span::from_usize(0, 15))
    }

    /// `%mod` underflow must emit E733, not E714.
    ///
    /// Before the fix, `build_phonology_alignment_from_counts` hardcoded E714 for
    /// all phonology-family tiers. After the fix, the caller passes tier-specific
    /// codes and %mod gets its own E733/E734 range.
    #[test]
    fn mod_alignment_too_few_emits_e733_not_e714() {
        // Two-word main tier, %mod has only 1 item → underflow.
        let main = two_word_main();
        let alignment = build_phonology_alignment_from_counts(
            &main,
            1,
            Span::from_usize(21, 35),
            "%mod",
            ErrorCode::ModCountMismatchTooFew,
            ErrorCode::ModCountMismatchTooMany,
        );

        assert!(
            !alignment.is_error_free(),
            "Expected an underflow error for %mod"
        );
        let error = &alignment.errors[0];
        assert_eq!(
            error.code,
            ErrorCode::ModCountMismatchTooFew,
            "Expected E733 (ModCountMismatchTooFew), got {:?}",
            error.code
        );
    }

    /// `%mod` overflow must emit E734, not E715.
    #[test]
    fn mod_alignment_too_many_emits_e734_not_e715() {
        // One-word main tier, %mod has 2 items → overflow.
        let main = one_word_main();
        let alignment = build_phonology_alignment_from_counts(
            &main,
            2,
            Span::from_usize(16, 40),
            "%mod",
            ErrorCode::ModCountMismatchTooFew,
            ErrorCode::ModCountMismatchTooMany,
        );

        assert!(
            !alignment.is_error_free(),
            "Expected an overflow error for %mod"
        );
        let error = &alignment.errors[0];
        assert_eq!(
            error.code,
            ErrorCode::ModCountMismatchTooMany,
            "Expected E734 (ModCountMismatchTooMany), got {:?}",
            error.code
        );
    }

    /// `%pho` still emits E714/E715 — the parameterized API must not break %pho.
    #[test]
    fn pho_alignment_too_few_still_emits_e714() {
        let main = two_word_main();
        let alignment = build_phonology_alignment_from_counts(
            &main,
            1,
            Span::from_usize(21, 35),
            "%pho",
            ErrorCode::PhoCountMismatchTooFew,
            ErrorCode::PhoCountMismatchTooMany,
        );

        assert!(!alignment.is_error_free());
        assert_eq!(alignment.errors[0].code, ErrorCode::PhoCountMismatchTooFew);
    }
}

/// Build a `MorTier` from items, inheriting span and terminator from
/// the utterance's existing `%mor:` tier. Returns `None` when no
/// existing `%mor:` tier is present — there is no terminator or span
/// to inherit, and the constructor cannot synthesize them.
pub(super) fn build_mor_tier_from_items(
    utterance: &Utterance,
    items: &[crate::model::Mor],
) -> Option<crate::model::MorTier> {
    let existing = utterance.mor_tier()?;
    let mut tier = crate::model::MorTier::new_mor(items.to_vec(), existing.terminator.clone());
    tier.span = existing.span;
    Some(tier)
}

/// Build a [`PhoAlignment`] from raw counts, emitting tier-specific error codes.
///
/// Both `%pho` and `%mod` use this builder, but they report different error
/// codes so diagnostics are unambiguously scoped to a single tier:
///
/// | Tier  | `too_few_code` | `too_many_code` |
/// |-------|---------------|----------------|
/// | `%pho` | E714 (`PhoCountMismatchTooFew`) | E715 (`PhoCountMismatchTooMany`) |
/// | `%mod` | E733 (`ModCountMismatchTooFew`) | E734 (`ModCountMismatchTooMany`) |
///
/// Pass the appropriate codes at each call site in `compute.rs`.
pub(super) fn build_phonology_alignment_from_counts(
    main: &crate::model::MainTier,
    item_count: usize,
    tier_span: Span,
    tier_label: &str,
    too_few_code: ErrorCode,
    too_many_code: ErrorCode,
) -> crate::alignment::PhoAlignment {
    let mut alignment = crate::alignment::PhoAlignment::new();

    let main_count = crate::alignment::helpers::count_tier_positions(
        &main.content.content,
        crate::alignment::TierDomain::Pho,
    );

    let min_len = main_count.min(item_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
            Some(MainWordIndex::new(i)),
            Some(PhoItemIndex::new(i)),
        ));
    }

    if main_count > item_count {
        let error = ParseError::new(
            too_few_code,
            Severity::Error,
            main.span.into(),
            ErrorContext::new("", main.span.to_range(), ""),
            format!(
                "Main tier has more alignable content than {} tier: expected {} phonological tokens, found {}",
                tier_label, main_count, item_count
            ),
        )
        .with_suggestion(format!(
            "Add phonological tokens to {} tier to match main tier words",
            tier_label
        ));
        alignment = alignment.with_error(error);
        for i in item_count..main_count {
            alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
                Some(MainWordIndex::new(i)),
                None,
            ));
        }
    } else if item_count > main_count {
        let error = ParseError::new(
            too_many_code,
            Severity::Error,
            tier_span.into(),
            ErrorContext::new("", tier_span.to_range(), ""),
            format!(
                "{} tier is longer than main tier: expected {} phonological tokens, found {}",
                tier_label, main_count, item_count
            ),
        )
        .with_suggestion(format!(
            "Remove extra phonological tokens from {} tier",
            tier_label
        ));
        alignment = alignment.with_error(error);
        for i in main_count..item_count {
            alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
                None,
                Some(PhoItemIndex::new(i)),
            ));
        }
    }

    alignment
}

pub(super) fn build_sin_alignment_from_counts(
    main: &crate::model::MainTier,
    item_count: usize,
    tier_span: Span,
) -> crate::alignment::SinAlignment {
    let mut alignment = crate::alignment::SinAlignment::new();

    let main_count = crate::alignment::helpers::count_tier_positions(
        &main.content.content,
        crate::alignment::TierDomain::Sin,
    );

    let min_len = main_count.min(item_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
            Some(MainWordIndex::new(i)),
            Some(SinItemIndex::new(i)),
        ));
    }

    if main_count > item_count {
        let error = ParseError::new(
            ErrorCode::SinCountMismatchTooFew,
            Severity::Error,
            main.span.into(),
            ErrorContext::new("", main.span.to_range(), ""),
            format!(
                "Main tier has more alignable content than %sin tier: expected {} gesture/sign tokens, found {}",
                main_count, item_count
            ),
        )
        .with_suggestion("Add gesture/sign tokens to %sin tier to match main tier words");
        alignment = alignment.with_error(error);
        for i in item_count..main_count {
            alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
                Some(MainWordIndex::new(i)),
                None,
            ));
        }
    } else if item_count > main_count {
        let error = ParseError::new(
            ErrorCode::SinCountMismatchTooMany,
            Severity::Error,
            tier_span.into(),
            ErrorContext::new("", tier_span.to_range(), ""),
            format!(
                "%sin tier is longer than main tier: expected {} gesture/sign tokens, found {}",
                main_count, item_count
            ),
        )
        .with_suggestion("Remove extra gesture/sign tokens from %sin tier");
        alignment = alignment.with_error(error);
        for i in main_count..item_count {
            alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
                None,
                Some(SinItemIndex::new(i)),
            ));
        }
    }

    alignment
}

pub(super) fn build_tier_to_tier_alignment(
    source_count: usize,
    source_span: Span,
    source_label: &str,
    target_count: usize,
    target_span: Span,
    target_label: &str,
    mismatch_code: ErrorCode,
) -> crate::alignment::PhoAlignment {
    let mut alignment = crate::alignment::PhoAlignment::new();

    let min_len = source_count.min(target_count);
    for i in 0..min_len {
        alignment = alignment.with_pair(crate::alignment::AlignmentPair::new(
            Some(MainWordIndex::new(i)),
            Some(PhoItemIndex::new(i)),
        ));
    }

    if source_count != target_count {
        alignment = alignment.with_error(build_count_mismatch_error(
            source_count,
            source_span,
            source_label,
            target_count,
            target_span,
            target_label,
            mismatch_code,
        ));
    }

    alignment
}
