//! TIERORDER -- reorder dependent tiers to canonical order.
//!
//! Reimplements CLAN's TIERORDER command, which sorts dependent tiers on each
//! utterance according to a canonical ordering. Utterances with only one
//! dependent tier are skipped (already trivially ordered).
//!
//! # Default tier order
//!
//! The canonical order follows CHAT convention, grouping tiers by function:
//!
//! 1. **Linguistic analysis tiers** (0--5):
//!    `%mor` --> `%gra` --> `%pho` --> `%mod` --> `%wor` --> `%sin`
//! 2. **Behavioral/descriptive tiers** (10--18):
//!    `%act` --> `%cod` --> `%com` --> `%spa` --> `%gpx` --> `%sit` -->
//!    `%exp` --> `%int` --> `%add`
//! 3. **Simple text tiers** (20--30):
//!    `%alt` --> `%coh` --> `%def` --> `%eng` --> `%err` --> `%fac` -->
//!    `%flo` --> `%gls` --> `%ort` --> `%par` --> `%tim`
//! 4. **User-defined tiers** (100): `%x*` (sorted last)
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Sorts using typed `DependentTier` variants with a numeric key function
//!   instead of matching raw `%`-prefixed line text.

use talkbank_model::{ChatFile, DependentTier, Line};

use crate::framework::{TransformCommand, TransformError};

/// TIERORDER transform: reorder dependent tiers.
pub struct TierorderCommand;

impl TransformCommand for TierorderCommand {
    type Config = ();

    /// Sort dependent tiers on each utterance according to canonical order.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        for line in file.lines.iter_mut() {
            if let Line::Utterance(utt) = line
                && utt.dependent_tiers.len() > 1
            {
                sort_tiers(&mut utt.dependent_tiers);
            }
        }
        Ok(())
    }
}

/// Sort dependent tiers according to canonical order.
fn sort_tiers(tiers: &mut smallvec::SmallVec<[DependentTier; 3]>) {
    tiers.sort_by_key(tier_order);
}

/// Return a numeric sort key for canonical tier ordering.
fn tier_order(tier: &DependentTier) -> u32 {
    match tier {
        // Linguistic analysis tiers first
        DependentTier::Mor(_) => 0,
        DependentTier::Gra(_) => 1,
        DependentTier::Pho(_) => 2,
        DependentTier::Mod(_) => 3,
        DependentTier::Wor(_) => 4,
        DependentTier::Sin(_) => 5,

        // Behavioral/descriptive tiers
        DependentTier::Act(_) => 10,
        DependentTier::Cod(_) => 11,
        DependentTier::Com(_) => 12,
        DependentTier::Spa(_) => 13,
        DependentTier::Gpx(_) => 14,
        DependentTier::Sit(_) => 15,
        DependentTier::Exp(_) => 16,
        DependentTier::Int(_) => 17,
        DependentTier::Add(_) => 18,

        // Simple text tiers
        DependentTier::Alt(_) => 20,
        DependentTier::Coh(_) => 21,
        DependentTier::Def(_) => 22,
        DependentTier::Eng(_) => 23,
        DependentTier::Err(_) => 24,
        DependentTier::Fac(_) => 25,
        DependentTier::Flo(_) => 26,
        // Phon project syllabification/alignment tiers — after %pho/%mod
        DependentTier::Modsyl(_) => 6,
        DependentTier::Phosyl(_) => 7,
        DependentTier::Phoaln(_) => 8,
        DependentTier::Gls(_) => 27,
        DependentTier::Ort(_) => 28,
        DependentTier::Par(_) => 29,
        DependentTier::Tim(_) => 30,

        // User-defined tiers last
        DependentTier::UserDefined(_) => 100,
        // Unsupported tiers after user-defined
        DependentTier::Unsupported(_) => 101,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{
        MainTier, NonEmptyString, Terminator, TextTier, Utterance, UtteranceContent, Word,
    };

    fn make_utterance_with_tiers(tiers: Vec<DependentTier>) -> Utterance {
        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);
        for t in tiers {
            utt.dependent_tiers.push(t);
        }
        utt
    }

    #[test]
    fn sorts_tiers_canonically() {
        let eng = DependentTier::Eng(TextTier::new(NonEmptyString::new("hello").unwrap()));
        let alt = DependentTier::Alt(TextTier::new(NonEmptyString::new("hi").unwrap()));

        // Insert in reverse order
        let mut utt = make_utterance_with_tiers(vec![eng, alt]);
        sort_tiers(&mut utt.dependent_tiers);

        // Alt (20) should come before Eng (23)
        assert!(matches!(utt.dependent_tiers[0], DependentTier::Alt(_)));
        assert!(matches!(utt.dependent_tiers[1], DependentTier::Eng(_)));
    }

    #[test]
    fn user_defined_tiers_sort_last() {
        let alt = DependentTier::Alt(TextTier::new(NonEmptyString::new("hi").unwrap()));
        let xmor = DependentTier::UserDefined(talkbank_model::UserDefinedDependentTier {
            label: NonEmptyString::new("xmor").unwrap(),
            content: NonEmptyString::new("custom").unwrap(),
            span: Span::DUMMY,
        });

        let mut utt = make_utterance_with_tiers(vec![xmor, alt]);
        sort_tiers(&mut utt.dependent_tiers);

        assert!(matches!(utt.dependent_tiers[0], DependentTier::Alt(_)));
        assert!(matches!(
            utt.dependent_tiers[1],
            DependentTier::UserDefined(_)
        ));
    }
}
