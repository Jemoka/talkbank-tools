//! Generic dependent-tier replacement helpers.

use smallvec::SmallVec;
use talkbank_model::model::DependentTier;

/// Replace an existing tier of the same variant or append a new one.
pub fn replace_or_add_tier(tiers: &mut SmallVec<[DependentTier; 3]>, new_tier: DependentTier) {
    let variant_matches = |existing: &DependentTier, new: &DependentTier| -> bool {
        match (existing, new) {
            (DependentTier::Mor(_), DependentTier::Mor(_)) => true,
            (DependentTier::Gra(_), DependentTier::Gra(_)) => true,
            (DependentTier::Wor(_), DependentTier::Wor(_)) => true,
            (DependentTier::UserDefined(a), DependentTier::UserDefined(b)) => a.label == b.label,
            _ => false,
        }
    };

    for tier in tiers.iter_mut() {
        if variant_matches(tier, &new_tier) {
            *tier = new_tier;
            return;
        }
    }
    tiers.push(new_tier);
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::{NonEmptyString, UserDefinedDependentTier, WorTier};

    #[test]
    fn replace_or_add_tier_user_defined_matches_by_label() {
        let mut tiers = smallvec::smallvec![];

        let xtra1 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: NonEmptyString::new("first").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra1);
        assert_eq!(tiers.len(), 1);

        let xtra2 = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xtra").unwrap(),
            content: NonEmptyString::new("second").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xtra2);
        assert_eq!(tiers.len(), 1);

        let DependentTier::UserDefined(ud) = &tiers[0] else {
            panic!("expected UserDefined tier");
        };
        assert_eq!(ud.content.as_ref(), "second");

        let xcod = DependentTier::UserDefined(UserDefinedDependentTier {
            label: NonEmptyString::new("xcod").unwrap(),
            content: NonEmptyString::new("code").unwrap(),
            span: talkbank_model::Span::DUMMY,
        });
        replace_or_add_tier(&mut tiers, xcod);
        assert_eq!(tiers.len(), 2);
    }

    #[test]
    fn replace_or_add_tier_replaces_existing_wor() {
        let mut tiers = smallvec::smallvec![DependentTier::Wor(WorTier::default())];
        let replacement = DependentTier::Wor(WorTier::from_words(vec![
            talkbank_model::model::Word::simple("hello"),
        ]));

        replace_or_add_tier(&mut tiers, replacement);

        assert_eq!(tiers.len(), 1);
        let DependentTier::Wor(wor) = &tiers[0] else {
            panic!("expected %wor tier");
        };
        assert_eq!(wor.words().count(), 1);
    }
}
