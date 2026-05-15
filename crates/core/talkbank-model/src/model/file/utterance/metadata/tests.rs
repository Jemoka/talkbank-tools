//! Tests for utterance language metadata and alignment metadata behavior.
//!
//! This suite guards metadata derivation rules (tier vs word language sources)
//! and interaction points with alignment units and parse-health handling.

use super::super::Utterance;
use crate::Severity;
use crate::model::dependent_tier::DependentTier;
use crate::model::language_metadata::WordLanguages;
use crate::model::{
    Action, AlignmentUnits, Annotated, BracketedContent, BracketedItem, Bullet, GraTier,
    GrammaticalRelation, Group, LanguageCode, LanguageSource, MainTier, Mor, MorTier, MorWord,
    ParseHealthTier, PhoItem, PhoTier, PhoWord, PosCategory, SinItem, SinTier, SinToken,
    Terminator, UtteranceContent, UtteranceLanguage, UtteranceLanguageMetadata, WorTier, Word,
    WordLanguageMarker,
};
use crate::validation::ValidationContext;
use crate::{ErrorCode, Span};

/// Builds `LanguageCode` values for test fixtures.
fn codes(list: &[&str]) -> Vec<LanguageCode> {
    list.iter().map(|code| LanguageCode::new(*code)).collect()
}

/// Short helper for constructing one `LanguageCode`.
fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code)
}

/// Default-language resolution populates utterance and per-word metadata.
///
/// This is the baseline path when neither tier-scoped nor word-scoped overrides are present.
#[test]
fn test_compute_language_metadata() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\tni3 hao3 .

    let word1 = Word::new_unchecked("ni3", "ni3");
    let word2 = Word::new_unchecked("hao3", "hao3");

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&utterance.utterance_language),
        crate::model::ValidationTag::Clean
    );
    assert_eq!(metadata.tier_language, Some(lc("zho")));
    assert_eq!(metadata.word_languages.len(), 2);

    // Both words should resolve to zho with Default source
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[1].source, LanguageSource::Default);

    // Not code-switching
    assert!(!metadata.is_code_switching());
    Ok(())
}

/// Word-level shortcuts trigger code-switching metadata.
///
/// The test checks both language assignment provenance and aggregate switching detection.
#[test]
fn test_compute_language_metadata_code_switching() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\tni3 hello@s .
    // First word zho, second word eng (via @s)

    let word1 = Word::new_unchecked("ni3", "ni3");

    let mut word2 = Word::new_unchecked("hello@s", "hello");
    word2.lang = Some(WordLanguageMarker::Shortcut);

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(metadata.word_languages.len(), 2);

    // First word: zho (default)
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    // Second word: eng (via @s shortcut)
    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::WordShortcut
    );

    // This IS code-switching
    assert!(metadata.is_code_switching());

    // Count by language
    let counts = metadata.count_by_language();
    assert_eq!(counts.get(&lc("zho")), Some(&1));
    assert_eq!(counts.get(&lc("eng")), Some(&1));
    Ok(())
}

/// Tier-scoped `[- lang]` overrides become the utterance baseline language.
///
/// All words should inherit the tier language when no per-word overrides are present.
#[test]
fn test_compute_language_metadata_tier_scoped() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\t[- eng] hello world .

    let word1 = Word::new_unchecked("hello", "hello");
    let word2 = Word::new_unchecked("world", "world");

    let mut main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    main_tier.content.language_code = Some(LanguageCode::new("eng"));

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedTierScoped { code: lc("eng") }
    );
    assert_eq!(metadata.tier_language, Some(lc("eng"))); // Tier override

    // Both words should resolve to eng with TierScoped source
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::TierScoped
    );

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::TierScoped
    );
    Ok(())
}

/// Language extraction recurses through grouped content.
///
/// Group-internal words must contribute to the same flat alignable-word metadata sequence.
#[test]
fn test_compute_language_metadata_recurses_into_groups() -> Result<(), String> {
    let grouped_default = Word::new_unchecked("ni3", "ni3");

    let mut grouped_switched = Word::new_unchecked("hello@s", "hello");
    grouped_switched.lang = Some(WordLanguageMarker::Shortcut);

    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(Box::new(grouped_default)),
        BracketedItem::Word(Box::new(grouped_switched)),
    ]));

    let trailing_word = Word::new_unchecked("hao3", "hao3");
    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Group(group),
            UtteranceContent::Word(Box::new(trailing_word)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;

    assert_eq!(metadata.word_languages.len(), 3);
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::WordShortcut
    );

    assert_eq!(
        metadata.word_languages[2].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[2].source, LanguageSource::Default);
    assert!(metadata.is_code_switching());
    Ok(())
}

/// Missing tier/default language leaves utterance language unresolved.
///
/// The unresolved status should propagate to per-word metadata entries.
#[test]
fn test_compute_language_metadata_unresolved_without_tier_or_default() -> Result<(), String> {
    let word = Word::new_unchecked("hello", "hello");
    let main_tier = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(word))],
        Terminator::Period { span: Span::DUMMY },
    );
    let mut utterance = Utterance::new(main_tier);

    let declared_languages: Vec<LanguageCode> = vec![];
    let default_language: Option<&LanguageCode> = None;
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(utterance.utterance_language, UtteranceLanguage::Unresolved);
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&utterance.utterance_language),
        crate::model::ValidationTag::Error
    );
    assert_eq!(metadata.tier_language, None);
    assert_eq!(metadata.word_languages.len(), 1);
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Unresolved
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::Unresolved
    );
    Ok(())
}

/// `UtteranceLanguage::Uncomputed` maps to warning-level validation state.
///
/// This keeps "not yet computed" distinct from true parse/semantic errors.
#[test]
fn test_utterance_language_uncomputed_is_warning_tag() {
    let state = UtteranceLanguage::Uncomputed;
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
    assert!(crate::model::ValidationTagged::is_validation_warning(
        &state
    ));
}

/// Default language metadata state starts as uncomputed warning.
///
/// The default communicates "analysis pending" rather than invalid transcript content.
#[test]
fn test_language_metadata_state_defaults_to_uncomputed_warning() {
    let state = UtteranceLanguageMetadata::default();
    assert!(matches!(state, UtteranceLanguageMetadata::Uncomputed));
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
}

/// Builds alignment fixture utterance for downstream use.
fn build_alignment_fixture_utterance() -> Utterance {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::simple("hello")))],
        None::<Terminator>,
    );

    let mor_item = Mor::new(MorWord::new(PosCategory::new("noun"), "hello"));
    let mor = MorTier::new_mor(
        vec![mor_item],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 0, "ROOT"),
        GrammaticalRelation::new(2, 1, "PUNCT"),
    ]);
    let pho = PhoTier::new_pho(vec![PhoItem::Word(PhoWord::new("helo"))]);
    let wor = WorTier::from_words(vec![Word::simple("hello")]);

    Utterance::new(main)
        .with_mor(mor)
        .with_gra(gra)
        .with_pho(pho)
        .add_dependent_tier(DependentTier::Wor(wor))
}

/// Alignment computation produces both main↔`%mor` and `%mor`↔`%gra` mappings.
///
/// This integration test ensures downstream consumers can rely on both alignment layers.
#[test]
fn compute_alignments_produces_mor_and_gra_alignment() -> Result<(), String> {
    let mut utterance = build_alignment_fixture_utterance();
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    // main <-> %mor alignment should be present and error-free
    let mor = alignments
        .mor
        .as_ref()
        .ok_or_else(|| "Expected main↔%mor alignment".to_string())?;
    assert!(
        mor.is_error_free(),
        "main↔%mor alignment should have no errors"
    );
    assert!(
        !mor.pairs.is_empty(),
        "main↔%mor alignment should have pairs"
    );

    // %mor <-> %gra alignment should be present and error-free
    let gra = alignments
        .gra
        .as_ref()
        .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?;
    assert!(
        gra.is_error_free(),
        "%mor↔%gra alignment should have no errors"
    );
    assert!(
        !gra.pairs.is_empty(),
        "%mor↔%gra alignment should have pairs"
    );

    Ok(())
}

/// Alignment computation produces `%wor` mapping while preserving inline bullets.
///
/// The test verifies both alignment-pair output and that timing bullets remain
/// attached to `%wor` words after computation.
#[test]
fn compute_alignments_produces_wor_alignment_with_inline_bullets() -> Result<(), String> {
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("one"))),
            UtteranceContent::Word(Box::new(Word::simple("two"))),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut timed_word = Word::simple("one");
    timed_word.inline_bullet = Some(Bullet::new(100, 220));
    let wor = WorTier::from_words(vec![timed_word, Word::simple("two")]);

    let mut utterance = Utterance::new(main).add_dependent_tier(DependentTier::Wor(wor));
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    let wor_sidecar = alignments
        .wor_timings
        .as_ref()
        .ok_or_else(|| "Expected main↔%wor timing sidecar".to_string())?;
    assert_eq!(
        *wor_sidecar,
        crate::alignment::WorTimingSidecar::Positional { count: 2 },
        "main↔%wor timing sidecar should be positional with count 2",
    );

    // Verify inline_bullet is preserved on the wor tier word
    let wor_words: Vec<&Word> = utterance
        .dependent_tiers
        .iter()
        .filter_map(|dt| match dt {
            DependentTier::Wor(wor) => Some(wor.words()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(wor_words.len(), 2);
    assert_eq!(
        wor_words[0].inline_bullet,
        Some(Bullet::new(100, 220)),
        "First wor word should have inline_bullet"
    );
    assert!(
        wor_words[1].inline_bullet.is_none(),
        "Second wor word should have no inline_bullet"
    );

    Ok(())
}

/// Confirms `%sin` alignment-unit counting includes annotated actions on the main tier.
#[test]
fn alignment_units_count_annotated_action_for_sin_domain() {
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::AnnotatedAction(Annotated::new(Action::new())),
            UtteranceContent::Word(Box::new(Word::new_unchecked("word", "word"))),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let sin = SinTier::new(vec![
        SinItem::Token(SinToken::new_unchecked("0")),
        SinItem::Token(SinToken::new_unchecked("0")),
    ]);

    let utterance = Utterance::new(main).with_sin(sin);
    let units = AlignmentUnits::from_utterance(&utterance, &ValidationContext::default());

    assert_eq!(
        units.main_sin.len(),
        2,
        "main_sin must count annotated action"
    );
    assert_eq!(
        units.sin.len(),
        2,
        "%sin units should reflect tier item count"
    );
}

/// Parse-health taint on `%gra` suppresses only `%gra`-dependent alignment paths.
///
/// Other alignment domains should remain available and error-free.
#[test]
fn parse_health_taints_only_gra_alignment_when_gra_tier_is_tainted() -> Result<(), String> {
    let mut utterance = build_alignment_fixture_utterance();
    utterance.mark_parse_taint(ParseHealthTier::Gra);
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    assert!(
        alignments
            .mor
            .as_ref()
            .ok_or_else(|| "Expected main↔%mor alignment".to_string())?
            .is_error_free()
    );
    assert!(
        alignments
            .pho
            .as_ref()
            .ok_or_else(|| "Expected main↔%pho alignment".to_string())?
            .is_error_free()
    );
    // `%wor` is a sidecar — presence of `Positional` is the analogue of
    // the old `is_error_free()` check on the other alignments.
    assert!(
        alignments
            .wor_timings
            .as_ref()
            .ok_or_else(|| "Expected main↔%wor timing sidecar".to_string())?
            .is_positional()
    );

    let gra = alignments
        .gra
        .as_ref()
        .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?;
    assert_eq!(gra.errors.len(), 1);
    assert_eq!(gra.errors[0].code, ErrorCode::TierValidationError);
    assert_eq!(gra.errors[0].severity, Severity::Warning);
    assert!(gra.errors[0].message.contains(
        "Tier validation warning: skipped %mor↔%gra alignment because %gra tier had parse errors during recovery"
    ));
    Ok(())
}

/// Main-tier parse taint suppresses main-dependent alignments but keeps `%mor↔%gra`.
///
/// This guards the contract that `%mor↔%gra` can still run when `%mor/%gra`
/// are clean, even if main-tier recovery marked `%mor/%pho/%wor` as skipped.
#[test]
fn parse_health_taints_main_dependent_alignments_but_keeps_mor_gra_alignment() -> Result<(), String>
{
    let mut utterance = build_alignment_fixture_utterance();
    utterance.mark_parse_taint(ParseHealthTier::Main);
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    let mor = alignments
        .mor
        .as_ref()
        .ok_or_else(|| "Expected main↔%mor alignment".to_string())?;
    assert_eq!(mor.errors.len(), 1);
    assert_eq!(mor.errors[0].code, ErrorCode::TierValidationError);
    assert!(mor.errors[0].message.contains(
        "Tier validation warning: skipped main↔%mor alignment because main tier had parse errors during recovery"
    ));

    let pho = alignments
        .pho
        .as_ref()
        .ok_or_else(|| "Expected main↔%pho alignment".to_string())?;
    assert_eq!(pho.errors.len(), 1);
    assert_eq!(pho.errors[0].code, ErrorCode::TierValidationError);
    assert!(pho.errors[0].message.contains(
        "Tier validation warning: skipped main↔%pho alignment because main tier had parse errors during recovery"
    ));

    // `%wor` is a timing sidecar, not a `TierAlignmentResult`. On parse-taint
    // the slot stays `None` — taint context lives on `ParseHealth`, not in
    // fabricated error-shaped alignments.
    assert!(
        alignments.wor_timings.is_none(),
        "%wor timing sidecar must be absent when main tier parse is tainted"
    );

    assert!(
        alignments
            .gra
            .as_ref()
            .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?
            .is_error_free(),
        "main-tier taint must not block %mor↔%gra alignment"
    );
    Ok(())
}
