//! Dependent-tier sanitization.

use talkbank_model::dependent_tier::mor::analysis::MorStem;
use talkbank_model::{BulletContent, DependentTier, NonEmptyString, TextTier};

use super::REDACTED_TEXT;
use super::placeholder::{PlaceholderState, PlaceholderToken};

/// Returns `false` when the tier should be dropped entirely.
///
/// Phonological tiers leak narrow IPA which is itself a clinical
/// fingerprint in dementia / aphasia corpora. Dropping them is more
/// conservative than placeholdering.
pub(crate) fn keep_dependent_tier(tier: &DependentTier) -> bool {
    !matches!(
        tier,
        DependentTier::Pho(_)
            | DependentTier::Mod(_)
            | DependentTier::Sin(_)
            | DependentTier::Modsyl(_)
            | DependentTier::Phosyl(_)
            | DependentTier::Phoaln(_)
    )
}

/// Sanitizes a dependent tier in place. Caller filters out tiers that
/// [`keep_dependent_tier`] returns `false` for.
pub(crate) fn sanitize_dependent_tier(tier: &mut DependentTier, state: &mut PlaceholderState) {
    match tier {
        DependentTier::Mor(mor) => redact_mor_tier(mor, state),
        DependentTier::Add(t) => t.content = redacted_bullet(),
        DependentTier::Com(t) => t.content = redacted_bullet(),
        DependentTier::Exp(t) => t.content = redacted_bullet(),
        DependentTier::Gpx(t) => t.content = redacted_bullet(),
        DependentTier::Int(t) => t.content = redacted_bullet(),
        DependentTier::Sit(t) => t.content = redacted_bullet(),
        DependentTier::Spa(t) => t.content = redacted_bullet(),
        DependentTier::Alt(t)
        | DependentTier::Coh(t)
        | DependentTier::Def(t)
        | DependentTier::Eng(t)
        | DependentTier::Err(t)
        | DependentTier::Fac(t)
        | DependentTier::Flo(t)
        | DependentTier::Gls(t)
        | DependentTier::Ort(t)
        | DependentTier::Par(t) => redact_text_tier(t),
        DependentTier::UserDefined(t) | DependentTier::Unsupported(t) => {
            if let Some(redacted) = NonEmptyString::new(REDACTED_TEXT) {
                t.content = redacted;
            }
        }
        // Numeric / structural tiers — no lexical content.
        DependentTier::Gra(_) | DependentTier::Wor(_) | DependentTier::Tim(_) => {}
        // Phonological tiers — should already be filtered by `keep_dependent_tier`.
        DependentTier::Pho(_)
        | DependentTier::Mod(_)
        | DependentTier::Sin(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_) => {}
        // Inline-bullet items; per-item redaction deferred.
        DependentTier::Act(_) | DependentTier::Cod(_) => {}
    }
}

fn redacted_bullet() -> BulletContent {
    BulletContent::from_text(REDACTED_TEXT)
}

fn redact_text_tier(tier: &mut TextTier) {
    if let Some(redacted) = NonEmptyString::new(REDACTED_TEXT) {
        tier.content = redacted;
    }
}

fn redact_mor_tier(mor: &mut talkbank_model::MorTier, state: &mut PlaceholderState) {
    for item in mor.items_mut().iter_mut() {
        replace_lemma(&mut item.main, state);
        for clitic in item.post_clitics.iter_mut() {
            replace_lemma(clitic, state);
        }
    }
}

fn replace_lemma(
    word: &mut talkbank_model::dependent_tier::mor::word::MorWord,
    state: &mut PlaceholderState,
) {
    word.lemma = MorStem::new(PlaceholderToken::lemma(state.next()).as_str());
}
