//! Shared evaluators for the four drift-invariant
//! [`FixtureAssertion`](crate::common::regression_manifest::FixtureAssertion)
//! variants. This module is the **single source of truth** for these four
//! predicates; both the regression-fixture harness
//! (`tests/ml_golden/regression_fixtures/harness.rs`) and the env-gated
//! real-file drift integration tests
//! (`tests/ml_golden/align/drift_integration.rs` + `drift_runner.rs`) dispatch
//! through the free functions exported here.
//!
//! Each check takes `&ChatFile` (the parsed output CHAT) because the drift
//! invariants are all expressible as purely structural properties of the typed
//! AST. No check needs raw CHAT text or the input fixture.
//!
//! Each returns `Result<(), String>` where `Err` carries the assertion-failure
//! message, ready to be formatted into a panic report.
//!
//! ## Invariant
//!
//! The behavior of these checks must remain byte-identical to the pre-
//! consolidation inline bodies in `harness.rs::run_one_assertion` (pre-commit
//! 8d7f9b4f) so that the 7 existing regression-fixture tests continue to
//! behave identically after the consolidation.

use batchalign::chat_ops::{ChatFile, DependentTier, UserDefinedDependentTier, Utterance};

use crate::common::regression_manifest::FixtureAssertion;

/// Dispatch one of the four drift assertions to its dedicated checker.
///
/// Returns `Err(..)` for any non-drift variant — callers in this crate only
/// pass the four supported variants, so the catch-all is a defensive
/// programming hook rather than a routine code path.
pub fn evaluate_drift_assertion(
    parsed: &ChatFile,
    assertion: &FixtureAssertion,
) -> Result<(), String> {
    match assertion {
        FixtureAssertion::NoFaGroupInvalidAudioWindow => {
            check_no_fa_group_invalid_audio_window(parsed)
        }
        FixtureAssertion::NoMonotonicityRescueEmitted => {
            check_no_monotonicity_rescue_emitted(parsed)
        }
        FixtureAssertion::UtteranceBulletMonotonicityPreserved => {
            check_utterance_bullet_monotonicity_preserved(parsed)
        }
        FixtureAssertion::NoSilentTimingStrip => check_no_silent_timing_strip(parsed),
        other => Err(format!(
            "evaluate_drift_assertion: unsupported assertion variant {other:?}; drift helper \
             only covers NoFaGroupInvalidAudioWindow, NoMonotonicityRescueEmitted, \
             UtteranceBulletMonotonicityPreserved, NoSilentTimingStrip"
        )),
    }
}

/// Build-time FA group windows are not surfaced to the harness; the only
/// observable proxy in the output CHAT is an explicit invalid-window marker
/// tier. Walk the typed AST looking for `%xrev` tiers whose body carries the
/// spec-forward marker string.
pub fn check_no_fa_group_invalid_audio_window(parsed: &ChatFile) -> Result<(), String> {
    let offenders: Vec<String> = parsed
        .utterances()
        .enumerate()
        .flat_map(|(i, utt)| {
            user_defined_tiers_with_label(utt, "xrev")
                .filter(|t| t.content.as_str().contains("fa_group_invalid_audio_window"))
                .map(move |t| format!("utterance #{}: %xrev: {}", i, t.content.as_str()))
        })
        .take(5)
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    Err(format!(
        "no_fa_group_invalid_audio_window: output has {} offending tier(s):\n      {}",
        offenders.len(),
        offenders.join("\n      "),
    ))
}

/// `%xalign` tier lines carrying a `monotonicity:` payload indicate the legacy
/// rescue layer fired. Once the rescue layer is deleted, this assertion becomes
/// a load-bearing negative contract. Walk the typed AST rather than the
/// serialized CHAT text.
pub fn check_no_monotonicity_rescue_emitted(parsed: &ChatFile) -> Result<(), String> {
    let offenders: Vec<String> = parsed
        .utterances()
        .enumerate()
        .flat_map(|(i, utt)| {
            user_defined_tiers_with_label(utt, "xalign")
                .filter(|t| t.content.as_str().contains("monotonicity:"))
                .map(move |t| format!("utterance #{}: %xalign: {}", i, t.content.as_str()))
        })
        .take(5)
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    Err(format!(
        "no_monotonicity_rescue_emitted: output has {} offending tier(s):\n      {}",
        offenders.len(),
        offenders.join("\n      "),
    ))
}

/// Walk every timed main-tier utterance in document order and verify strict
/// start-time monotonicity. Utterances without a bullet are skipped (they
/// carry no timing to compare).
///
/// Overlap-continuation utterances — those carrying a `+<` LazyOverlapPrecedes
/// linker or a ⌊ CA bottom-overlap marker — legitimately share start timing
/// with their predecessor by design. They are skipped from BOTH the comparison
/// (their start is not a violation) AND the `prev_start` baseline (their start
/// must not become the lower bound the next non-overlap utterance is compared
/// against). Detection mirrors `batchalign::chat_ops::fa::utr::select_strategy`
/// — the canonical overlap-aware pattern.
pub fn check_utterance_bullet_monotonicity_preserved(parsed: &ChatFile) -> Result<(), String> {
    use batchalign::chat_ops::Linker;
    use batchalign::chat_ops::fa::utr::overlap_markers;
    let mut prev_start: Option<u64> = None;
    let mut prev_index: usize = 0;
    for (i, utt) in parsed.utterances().enumerate() {
        let Some(bullet) = utt.main.content.bullet.as_ref() else {
            continue;
        };
        let is_overlap_continuation = utt
            .main
            .content
            .linkers
            .0
            .contains(&Linker::LazyOverlapPrecedes)
            || overlap_markers::extract_overlap_info(&utt.main.content.content.0)
                .has_bottom_overlap();
        if is_overlap_continuation {
            // Do not compare against prev_start, and do not update prev_start
            // from this utterance.
            continue;
        }
        let this_start = bullet.timing.start_ms;
        if let Some(p) = prev_start
            && this_start <= p
        {
            return Err(format!(
                "utterance_bullet_monotonicity_preserved: utterance #{} start {} ms \
                 does not advance past utterance #{} start {} ms",
                i, this_start, prev_index, p,
            ));
        }
        prev_start = Some(this_start);
        prev_index = i;
    }
    Ok(())
}

/// Detect the timing_stripped rescue path: an utterance with NO main-tier
/// bullet that also carries a sibling `%xrev` tier containing `[?]`. The
/// pairing is what distinguishes a silent strip from a legitimately untimed
/// utterance. Walk the typed AST — bullet presence lives on
/// `utt.main.content.bullet`, and the sibling xrev tier lives in
/// `utt.dependent_tiers`.
pub fn check_no_silent_timing_strip(parsed: &ChatFile) -> Result<(), String> {
    let offenders: Vec<String> = parsed
        .utterances()
        .enumerate()
        .filter(|(_, utt)| utt.main.content.bullet.is_none())
        .flat_map(|(i, utt)| {
            user_defined_tiers_with_label(utt, "xrev")
                .filter(|t| t.content.as_str().contains("[?]"))
                .map(move |t| {
                    format!(
                        "utterance #{}: bulletless main tier paired with %xrev: {}",
                        i,
                        t.content.as_str(),
                    )
                })
        })
        .take(5)
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    Err(format!(
        "no_silent_timing_strip: output has {} bulletless utterance(s) paired with [?] xrev marker:\n      {}",
        offenders.len(),
        offenders.join("\n      "),
    ))
}

/// Iterator over the `UserDefinedDependentTier` children of one utterance
/// whose label matches the requested string (pass `"xrev"` to match `%xrev`
/// tiers). User-defined and unsupported tiers both live as
/// [`UserDefinedDependentTier`] in the model; matching by label keeps drift
/// checks operating on the typed AST rather than raw CHAT text.
fn user_defined_tiers_with_label<'a>(
    utt: &'a Utterance,
    label: &'a str,
) -> impl Iterator<Item = &'a UserDefinedDependentTier> + 'a {
    utt.dependent_tiers.iter().filter_map(move |t| match t {
        DependentTier::UserDefined(u) | DependentTier::Unsupported(u)
            if u.label.as_str() == label =>
        {
            Some(u)
        }
        _ => None,
    })
}
