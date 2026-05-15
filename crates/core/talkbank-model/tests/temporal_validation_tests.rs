//! Integration tests for temporal validation (E701, E704)
//!
//! Tests CLAN CHECK command behavior:
//! - E701 (Error 83): Global timeline monotonicity
//! - E704 (Error 133): Per-speaker overlap with 500ms tolerance

use talkbank_model::ErrorCode;
use talkbank_model::ErrorCollector;
use talkbank_model::Span;
use talkbank_model::content::TierContentItems;
use talkbank_model::model::{
    Bullet, ChatFile, Header, Line, MainTier, ParseHealthState, SpeakerCode, Terminator,
    TierContent, Utterance, UtteranceContent, UtteranceLanguage, UtteranceLanguageMetadata, Word,
};

/// Helper to create a simple ChatFile for testing
fn create_test_file(lines: Vec<Line>) -> ChatFile {
    ChatFile::new(lines)
}

/// Helper to create a main tier with bullet
fn main_tier_with_bullet(speaker: &str, start_ms: u64, end_ms: u64) -> MainTier {
    MainTier {
        speaker: SpeakerCode::new(speaker),
        content: TierContent {
            linkers: Default::default(),
            language_code: None,
            content: TierContentItems::new(vec![UtteranceContent::Word(Box::new(
                Word::new_unchecked("foo", "foo"),
            ))]),
            terminator: Some(Terminator::Period { span: Span::DUMMY }),
            postcodes: Default::default(),
            bullet: Some(Bullet::new(start_ms, end_ms)),
            content_span: None,
        },
        span: Span::DUMMY,
        speaker_span: Span::DUMMY,
    }
}

/// Helper to create a main tier with a single untranscribed "www" word and bullet
fn main_tier_with_untranscribed_www_bullet(speaker: &str, start_ms: u64, end_ms: u64) -> MainTier {
    MainTier {
        speaker: SpeakerCode::new(speaker),
        content: TierContent {
            linkers: Default::default(),
            language_code: None,
            content: TierContentItems::new(vec![UtteranceContent::Word(Box::new(
                Word::new_unchecked("www", "www"),
            ))]),
            terminator: Some(Terminator::Period { span: Span::DUMMY }),
            postcodes: Default::default(),
            bullet: Some(Bullet::new(start_ms, end_ms)),
            content_span: None,
        },
        span: Span::DUMMY,
        speaker_span: Span::DUMMY,
    }
}

/// Helper to create an utterance
fn utterance(main: MainTier) -> Utterance {
    Utterance {
        preceding_headers: Default::default(),
        main,
        dependent_tiers: Default::default(),
        alignments: None,
        alignment_diagnostics: Vec::new(),
        parse_health: ParseHealthState::Clean,
        utterance_language: UtteranceLanguage::Uncomputed,
        language_metadata: UtteranceLanguageMetadata::Uncomputed,
    }
}

/// Tests e701 global timeline monotonicity violation.
#[test]
fn test_e701_global_timeline_monotonicity_violation() {
    // Test E701: Same speaker's second utterance starts before their first.
    // E701 is scoped to per-speaker (cross-speaker non-monotonicity is normal
    // conversational overlap, not an error).
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 2000))), // CHI: 1000-2000ms
        Line::utterance(utterance(main_tier_with_bullet("MOT", 3000, 4000))), // MOT: 3000-4000ms
        Line::utterance(utterance(main_tier_with_bullet("CHI", 500, 1500))), // CHI: 500ms < CHI's 1000ms
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should have E701 error for same-speaker non-monotonicity
    assert!(
        error_vec
            .iter()
            .any(|e| e.code == ErrorCode::TierBeginTimeNotMonotonic),
        "Expected E701 error for same-speaker non-monotonic timeline, got: {:#?}",
        error_vec
    );
}

/// Tests e701 cross-speaker non-monotonicity does NOT fire (normal overlap).
#[test]
fn test_e701_cross_speaker_non_monotonic_does_not_fire() {
    // Cross-speaker non-monotonicity is normal conversational overlap.
    // MOT starts at 500ms while CHI started at 1000ms — this is just
    // two speakers talking at the same time, not an error.
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 2000))), // CHI: 1000-2000ms
        Line::utterance(utterance(main_tier_with_bullet("MOT", 500, 1500))), // MOT: 500ms (overlap)
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should NOT have E701 — different speakers overlapping is fine
    assert!(
        !error_vec
            .iter()
            .any(|e| e.code == ErrorCode::TierBeginTimeNotMonotonic),
        "Cross-speaker non-monotonicity should NOT fire E701, got: {:#?}",
        error_vec
    );
}

/// Tests e701 same-speaker monotonic passes.
#[test]
fn test_e701_same_speaker_monotonic_passes() {
    // Same speaker with monotonically increasing start times — no error.
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 2000))), // 1000-2000ms
        Line::utterance(utterance(main_tier_with_bullet("MOT", 2000, 3000))), // 2000-3000ms
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should NOT have E701 error
    assert!(
        !error_vec
            .iter()
            .any(|e| e.code == ErrorCode::TierBeginTimeNotMonotonic),
        "Should not have E701 error for monotonic timeline, got: {:#?}",
        error_vec
    );
}

/// Tests e704 speaker self overlap exceeds tolerance.
#[test]
fn test_e704_speaker_self_overlap_exceeds_tolerance() {
    // Test E704: Same speaker overlaps with self beyond 500ms tolerance
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 3000))), // CHI: 1000-3000ms
        Line::utterance(utterance(main_tier_with_bullet("CHI", 2000, 4000))), // CHI: 2000-4000ms (1000ms overlap)
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should have E704 error (1000ms overlap exceeds 500ms tolerance)
    assert!(
        error_vec
            .iter()
            .any(|e| e.code == ErrorCode::SpeakerSelfOverlap),
        "Expected E704 error for speaker self-overlap exceeding tolerance, got: {:#?}",
        error_vec
    );
}

/// Tests e704 speaker self overlap within tolerance.
#[test]
fn test_e704_speaker_self_overlap_within_tolerance() {
    // Test E704: Same speaker overlaps with self within 500ms tolerance (should pass)
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 2000))), // CHI: 1000-2000ms
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1600, 2600))), // CHI: 1600-2600ms (400ms overlap)
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should NOT have E704 error (400ms overlap is within 500ms tolerance)
    assert!(
        !error_vec
            .iter()
            .any(|e| e.code == ErrorCode::SpeakerSelfOverlap),
        "Should not have E704 error for overlap within tolerance, got: {:#?}",
        error_vec
    );
}

/// Tests different speakers can overlap.
#[test]
fn test_different_speakers_can_overlap() {
    // Test that different speakers can overlap without error
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_bullet("CHI", 1000, 3000))), // CHI: 1000-3000ms
        Line::utterance(utterance(main_tier_with_bullet("MOT", 2000, 4000))), // MOT: 2000-4000ms
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should NOT have E704 error (different speakers)
    assert!(
        !error_vec
            .iter()
            .any(|e| e.code == ErrorCode::SpeakerSelfOverlap),
        "Different speakers should be able to overlap, got: {:#?}",
        error_vec
    );
}

/// Tests e704 ignores untranscribed only www turns.
#[test]
fn test_e704_ignores_untranscribed_only_www_turns() {
    // Mirrors real corpus pattern: repeated INV "www" bullets can overlap,
    // but CHECK does not treat these as self-overlap violations.
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(utterance(main_tier_with_untranscribed_www_bullet(
            "INV", 355_600, 653_182,
        ))),
        Line::utterance(utterance(main_tier_with_untranscribed_www_bullet(
            "INV", 562_690, 729_500,
        ))),
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    assert!(
        !error_vec
            .iter()
            .any(|e| e.code == ErrorCode::SpeakerSelfOverlap),
        "Untranscribed-only www turns should be ignored for E704, got: {:#?}",
        error_vec
    );
}

/// Tests no bullets no errors.
#[test]
fn test_no_bullets_no_errors() {
    // Test that files without bullets don't trigger temporal errors
    let file = create_test_file(vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::utterance(Utterance {
            preceding_headers: Default::default(),
            main: MainTier {
                speaker: SpeakerCode::new("CHI"),
                content: TierContent {
                    linkers: Default::default(),
                    language_code: None,
                    content: TierContentItems::new(Vec::new()),
                    terminator: Some(Terminator::Period { span: Span::DUMMY }),
                    postcodes: Default::default(),
                    bullet: None, // No bullet
                    content_span: None,
                },
                span: Span::DUMMY,
                speaker_span: Span::DUMMY,
            },
            dependent_tiers: Default::default(),
            alignments: None,
            alignment_diagnostics: Vec::new(),
            parse_health: ParseHealthState::Clean,
            utterance_language: UtteranceLanguage::Uncomputed,
            language_metadata: UtteranceLanguageMetadata::Uncomputed,
        }),
        Line::header(Header::End),
    ]);

    let errors = ErrorCollector::new();
    file.validate(&errors, None);
    let error_vec = errors.into_vec();

    // Should not have temporal errors
    assert!(
        !error_vec.iter().any(|e| matches!(
            e.code,
            ErrorCode::TierBeginTimeNotMonotonic | ErrorCode::SpeakerSelfOverlap
        )),
        "Files without bullets should not have temporal errors, got: {:#?}",
        error_vec
    );
}
