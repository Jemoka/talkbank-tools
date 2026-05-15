//! Tests for word-language resolution behavior.
//!
//! The cases cover fallback semantics, shortcut edge conditions, and explicit
//! marker behavior so language-resolution policy stays stable over refactors.

use super::{LanguageResolution, LanguageResolutionOutcome, resolve_word_language};
use crate::model::{LanguageCode, ValidationTag, ValidationTagged, Word, WordLanguageMarker};

/// Builds `LanguageCode` values for test fixtures.
fn codes(list: &[&str]) -> Vec<LanguageCode> {
    list.iter().map(|code| LanguageCode::new(*code)).collect()
}

/// Short helper for constructing one `LanguageCode`.
fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code)
}

/// Words without markers inherit the default/tier language.
///
/// This is the baseline resolution path when no explicit word-level override is present.
#[test]
fn test_resolve_word_language_default() {
    let word1 = Word::new_unchecked("ni3", "ni3");
    let word2 = Word::new_unchecked("hao3", "hao3");

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.first();

    let LanguageResolutionOutcome {
        resolution: lang1,
        diagnostics: errors1,
    } = resolve_word_language(&word1, tier_language, &declared_languages);
    assert_eq!(lang1, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors1.len(), 0);

    let LanguageResolutionOutcome {
        resolution: lang2,
        diagnostics: errors2,
    } = resolve_word_language(&word2, tier_language, &declared_languages);
    assert_eq!(lang2, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors2.len(), 0);
}

/// Tier-scoped language overrides apply uniformly to unmarked words.
///
/// Both words should resolve to the tier language with no diagnostics.
#[test]
fn test_resolve_word_language_tier_scoped() {
    let word1 = Word::new_unchecked("ni3", "ni3");
    let word2 = Word::new_unchecked("hao3", "hao3");

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.get(1);

    let LanguageResolutionOutcome {
        resolution: lang1,
        diagnostics: errors1,
    } = resolve_word_language(&word1, tier_language, &declared_languages);
    assert_eq!(lang1, LanguageResolution::Single(lc("eng")));
    assert_eq!(errors1.len(), 0);

    let LanguageResolutionOutcome {
        resolution: lang2,
        diagnostics: errors2,
    } = resolve_word_language(&word2, tier_language, &declared_languages);
    assert_eq!(lang2, LanguageResolution::Single(lc("eng")));
    assert_eq!(errors2.len(), 0);
}

/// **Rule 6d canonical regression.** When `Shortcut` is given an
/// undeclared `tier_language`, the resolver must return `Unresolved`
/// with `E249`, never fabricate `Single(tier_language)` as a fallback.
#[test]
fn dona_at_s_with_undeclared_tier_language_must_be_unresolved_not_eng_sentinel() {
    let mut word = Word::new_unchecked("dona", "dona");
    word.lang = Some(WordLanguageMarker::Shortcut);

    let declared = codes(&["cat", "spa"]);
    let wrong_tier = LanguageCode::new("eng"); // simulates primary_lang=eng leaking in

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, Some(&wrong_tier), &declared);

    assert_eq!(
        lang,
        LanguageResolution::Unresolved,
        "Shortcut @s with undeclared tier language must NOT fabricate \
         Single(tier_lang) — that was the dona@s bug. Honest answer is \
         Unresolved + diagnostic.",
    );
    assert_eq!(errors.len(), 1, "expected one diagnostic, got {errors:?}");
    assert_eq!(
        errors[0].code.as_str(),
        "E249",
        "diagnostic must be MissingLanguageContext (E249), naming the \
         real failure mode rather than masking it with a fake-eng",
    );
}

/// `Shortcut` on the primary tier of a two-language doc resolves to
/// the secondary declared language.
#[test]
fn dona_at_s_in_cat_spa_doc_resolves_to_spa_not_eng() {
    let mut word = Word::new_unchecked("dona", "dona");
    word.lang = Some(WordLanguageMarker::Shortcut);

    let declared = codes(&["cat", "spa"]);
    let tier_language = declared.first(); // cat

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared);

    assert_eq!(
        lang,
        LanguageResolution::Single(lc("spa")),
        "Shortcut @s on cat-tier with declared [cat, spa] must resolve to spa, never eng",
    );
    assert!(errors.is_empty(), "expected zero errors, got {errors:?}");
}

/// `@s` shortcut resolves to the secondary declared language.
///
/// This test covers the common two-language code-switching shorthand behavior.
#[test]
fn test_resolve_word_language_word_shortcut() {
    let mut word1 = Word::new_unchecked("ni3@s", "ni3");
    word1.lang = Some(WordLanguageMarker::Shortcut);

    let mut word2 = Word::new_unchecked("hao3@s", "hao3");
    word2.lang = Some(WordLanguageMarker::Shortcut);

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.get(1);

    let LanguageResolutionOutcome {
        resolution: lang1,
        diagnostics: errors1,
    } = resolve_word_language(&word1, tier_language, &declared_languages);
    assert_eq!(lang1, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors1.len(), 0);

    let LanguageResolutionOutcome {
        resolution: lang2,
        diagnostics: errors2,
    } = resolve_word_language(&word2, tier_language, &declared_languages);
    assert_eq!(lang2, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors2.len(), 0);
}

/// Explicit `@s:code` markers override tier/default language.
///
/// Resolution should return the explicit code directly with no warnings.
#[test]
fn test_resolve_word_language_word_explicit() {
    let mut word1 = Word::new_unchecked("ni3@s:zho", "ni3");
    word1.lang = Some(WordLanguageMarker::explicit("zho"));

    let mut word2 = Word::new_unchecked("hao3@s:zho", "hao3");
    word2.lang = Some(WordLanguageMarker::explicit("zho"));

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.get(1);

    let LanguageResolutionOutcome {
        resolution: lang1,
        diagnostics: errors1,
    } = resolve_word_language(&word1, tier_language, &declared_languages);
    assert_eq!(lang1, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors1.len(), 0);

    let LanguageResolutionOutcome {
        resolution: lang2,
        diagnostics: errors2,
    } = resolve_word_language(&word2, tier_language, &declared_languages);
    assert_eq!(lang2, LanguageResolution::Single(lc("zho")));
    assert_eq!(errors2.len(), 0);
}

/// Shortcut `@s` without a secondary language emits `E249` and resolves
/// to `Unresolved`.
///
/// Pre-2026-05-02 this fell back to the tier language; that violated
/// rule 6d (no fabricated sentinel values). The test now asserts the
/// honest typed outcome.
#[test]
fn test_resolve_word_language_no_secondary() {
    let mut word = Word::new_unchecked("ni3@s", "ni3");
    word.lang = Some(WordLanguageMarker::Shortcut);

    let declared_languages = codes(&["zho"]);
    let tier_language = declared_languages.first();

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Unresolved);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E249");
}

/// Shortcut `@s` in 3+ language context emits `E248` and resolves to
/// `Unresolved`.
///
/// Pre-2026-05-02 this fell back to the tier language; that violated
/// rule 6d. The test now asserts the honest typed outcome.
#[test]
fn test_resolve_word_language_tertiary() {
    let mut word = Word::new_unchecked("word@s", "word");
    word.lang = Some(WordLanguageMarker::Shortcut);

    let declared_languages = codes(&["zho", "eng", "spa"]);
    let tier_language = declared_languages.get(2);

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Unresolved);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E248");
}

/// Explicit language codes are allowed even if absent from `@Languages`.
///
/// The resolver stays permissive, but it should warn so transcribers notice the
/// missing header declaration.
#[test]
fn test_resolve_word_language_undeclared_explicit_code() {
    let mut word = Word::new_unchecked("ciao@s:ita", "ciao");
    word.lang = Some(WordLanguageMarker::explicit("ita"));

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.first();

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Single(lc("ita")));
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].severity, crate::Severity::Warning);
}

/// Explicit language codes declared in `@Languages` resolve cleanly.
///
/// No diagnostics should be produced in this straightforward case.
#[test]
fn test_resolve_word_language_declared_explicit_code() {
    // Valid case: explicit code that IS in @Languages header
    let mut word = Word::new_unchecked("hello@s:eng", "hello");
    word.lang = Some(WordLanguageMarker::explicit("eng"));

    let declared_languages = codes(&["zho", "eng"]);
    let tier_language = declared_languages.first();

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Single(lc("eng")));
    assert_eq!(errors.len(), 0); // No error - code is declared
}

/// Explicit markers still resolve when `@Languages` is missing.
///
/// File-level validation handles missing declarations; resolver accepts explicit word codes.
#[test]
fn test_resolve_word_language_explicit_with_no_declared_languages() {
    // Edge case: No @Languages header at all
    // (file-level validation should catch missing @Languages)
    let mut word = Word::new_unchecked("hello@s:eng", "hello");
    word.lang = Some(WordLanguageMarker::explicit("eng"));

    let declared_languages: Vec<LanguageCode> = vec![];
    let tier_language: Option<&LanguageCode> = None;

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Single(lc("eng")));
    assert_eq!(errors.len(), 0);
}

/// Shortcut `@s` without any language context resolves as unresolved.
///
/// The resolver should emit `E249` because no secondary language can be inferred.
#[test]
fn test_resolve_word_language_shortcut_without_context_is_unresolved() {
    let mut word = Word::new_unchecked("word@s", "word");
    word.lang = Some(WordLanguageMarker::Shortcut);

    let declared_languages: Vec<LanguageCode> = vec![];
    let tier_language: Option<&LanguageCode> = None;

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Unresolved);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code.as_str(), "E249");
}

/// Unmarked words with no language context resolve as unresolved.
///
/// Unlike shortcut failures, this path currently emits no additional diagnostics.
#[test]
fn test_resolve_word_language_no_marker_without_context_is_unresolved() {
    let word = Word::new_unchecked("word", "word");
    let declared_languages: Vec<LanguageCode> = vec![];
    let tier_language: Option<&LanguageCode> = None;

    let LanguageResolutionOutcome {
        resolution: lang,
        diagnostics: errors,
    } = resolve_word_language(&word, tier_language, &declared_languages);
    assert_eq!(lang, LanguageResolution::Unresolved);
    assert_eq!(errors.len(), 0);
}

/// Language-resolution variants map to expected validation tags.
///
/// Resolved variants are clean, while unresolved state is treated as an error.
#[test]
fn test_language_resolution_validation_tags() {
    assert_eq!(
        LanguageResolution::Single(lc("eng")).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Multiple(codes(&["eng", "spa"])).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Ambiguous(codes(&["eng", "spa"])).validation_tag(),
        ValidationTag::Clean
    );
    assert_eq!(
        LanguageResolution::Unresolved.validation_tag(),
        ValidationTag::Error
    );
}
