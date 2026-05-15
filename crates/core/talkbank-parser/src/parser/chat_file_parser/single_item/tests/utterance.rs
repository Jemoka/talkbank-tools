//! Test module for utterance in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use super::{parse_main_tier, parse_utterance, with_snapshot_settings};
use crate::model::{DependentTier, Separator, UtteranceContent};

// ✅ SUCCESS CASE - Simplest valid utterance
/// Parses the minimal valid main tier (`*SPK:\tword .`) and snapshots the structured result.
#[test]
fn simplest_success() {
    let result = parse_utterance("*CHI:\thello .");
    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("utterance_parsing_tests__simplest_success", result);
    });
}

/// Regression: isolated `parse_utterance()` must preserve attached dependent tiers.
#[test]
fn preserves_dependent_tiers() {
    let result = parse_utterance("*CHI:\tI want .\n%mor:\tpro|I v|want .\n");
    let utterance = result.expect("expected utterance parse to succeed");

    assert_eq!(utterance.dependent_tiers.len(), 1);
    assert!(matches!(
        utterance.dependent_tiers[0],
        DependentTier::Mor(_)
    ));
}

/// Regression: isolated `parse_utterance()` must preserve both main-tier and dependent-tier bullets.
#[test]
fn preserves_main_and_dependent_bullets() {
    let result = parse_utterance(
        "*CHI:\thello there . \u{15}2041689_2042652\u{15}\n%cod:\tthis is junk \u{15}2041689_2042652\u{15}\n",
    );
    let utterance = result.expect("expected utterance parse to succeed");

    assert!(utterance.main.content.bullet.is_some());
    assert_eq!(utterance.dependent_tiers.len(), 1);
    match &utterance.dependent_tiers[0] {
        DependentTier::Cod(tier) => assert!(
            tier.content
                .segments
                .iter()
                .any(|segment| matches!(segment, crate::model::BulletContentSegment::Bullet(_))),
            "expected %cod bullet segment to be preserved"
        ),
        other => panic!("expected %cod tier, got {other:?}"),
    }
}

// ❌ ERROR CASE - Missing terminator
/// Verifies the parser reports an error when a main tier omits its required terminator.
#[test]
fn error_missing_terminator() {
    let result = parse_main_tier("*CHI:\thello");

    // Check critical invariant: should have at least one error
    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty(), "Expected at least 1 error");
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("utterance_parsing_tests__error_missing_terminator", result);
    });
}

// ❌ ERROR CASE - Space instead of tab
/// Verifies the parser rejects a speaker line that uses a space instead of the required tab after `:`.
#[test]
fn error_space_instead_of_tab() {
    let result = parse_main_tier("*CHI: hello .");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty());
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(
            "utterance_parsing_tests__error_space_instead_of_tab",
            result
        );
    });
}

// ❌ ERROR CASE - Empty speaker
/// Verifies an empty speaker code is reported as an error.
#[test]
fn error_empty_speaker() {
    let result = parse_main_tier("*:\thello .");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty());
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("utterance_parsing_tests__error_empty_speaker", result);
    });
}

// ❌ ERROR CASE - Multiple errors (CRITICAL: no fail-fast)
/// Confirms we still collect diagnostics when a line has multiple structural problems.
#[test]
fn error_multiple_problems() {
    // Space instead of tab + missing terminator
    let result = parse_main_tier("*CHI: hello");

    if let Err(errors) = &result {
        // CRITICAL: Should find errors (may be 1 or more depending on parser)
        assert!(!errors.errors.is_empty(), "Should collect errors");
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("utterance_parsing_tests__error_multiple_problems", result);
    });
}

// ❌ ERROR CASE - Invalid terminator
/// Verifies invalid utterance-end punctuation is flagged as a terminator error.
#[test]
fn error_invalid_terminator() {
    // Semicolon is not a valid CHAT terminator
    let result = parse_main_tier("*CHI:\thello ;");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for invalid terminator"
        );
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!("utterance_parsing_tests__error_invalid_terminator", result);
    });
}

// ❌ ERROR CASE - Missing tab detected by tree-sitter
/// Verifies tree-sitter error recovery still yields diagnostics for missing speaker/tab separator.
#[test]
fn error_missing_tab_treesitter() {
    let result = parse_main_tier("*CHI hello .");

    if let Err(errors) = &result {
        assert!(!errors.errors.is_empty(), "Expected error for missing tab");
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(
            "utterance_parsing_tests__error_missing_tab_treesitter",
            result
        );
    });
}

// ❌ ERROR CASE - Test E305: Invalid terminator in tree-sitter parser
/// Regression test: invalid terminator should still surface parser diagnostics in tree-sitter path.
#[test]
fn error_e305_invalid_terminator_treesitter() {
    // Use a character that's definitely not a valid terminator
    let result = parse_main_tier("*CHI:\thello ;");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for invalid terminator"
        );
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(
            "utterance_parsing_tests__error_e305_invalid_terminator_treesitter",
            result
        );
    });
}

// ❌ ERROR CASE - Test E305: Missing terminator in tree-sitter parser
/// Regression test: missing terminator should still surface parser diagnostics in tree-sitter path.
#[test]
fn error_e305_missing_terminator_treesitter() {
    let result = parse_main_tier("*CHI:\thello");

    if let Err(errors) = &result {
        assert!(
            !errors.errors.is_empty(),
            "Expected error for missing terminator"
        );
    }

    with_snapshot_settings(|| {
        insta::assert_debug_snapshot!(
            "utterance_parsing_tests__error_e305_missing_terminator_treesitter",
            result
        );
    });
}

/// Regression: trailing CA arrows stay in main-tier content as separators.
#[test]
fn trailing_ca_arrow_stays_separator() {
    let result = parse_main_tier("*CHI:\tlevel pitch →");
    let main_tier = result.expect("expected CA arrow main tier to parse");

    assert!(
        main_tier.content.terminator.is_none(),
        "trailing CA arrow must not be promoted to terminator"
    );
    assert!(matches!(
        main_tier.content.content.last(),
        Some(UtteranceContent::Separator(Separator::Level { .. }))
    ));
}

/// Regression: trailing CA no-break markers stay in main-tier content as separators.
#[test]
fn trailing_ca_no_break_stays_separator() {
    let result = parse_main_tier("*CHI:\tno break ≈");
    let main_tier = result.expect("expected CA no-break main tier to parse");

    assert!(
        main_tier.content.terminator.is_none(),
        "trailing CA no-break must not be promoted to terminator"
    );
    assert!(matches!(
        main_tier.content.content.last(),
        Some(UtteranceContent::Separator(Separator::CaNoBreak { .. }))
    ));
}

/// Regression: trailing CA technical-break markers stay in main-tier content as separators.
#[test]
fn trailing_ca_technical_break_stays_separator() {
    let result = parse_main_tier("*CHI:\ttechnical break ≋");
    let main_tier = result.expect("expected CA technical-break main tier to parse");

    assert!(
        main_tier.content.terminator.is_none(),
        "trailing CA technical-break must not be promoted to terminator"
    );
    assert!(matches!(
        main_tier.content.content.last(),
        Some(UtteranceContent::Separator(
            Separator::CaTechnicalBreak { .. }
        ))
    ));
}
