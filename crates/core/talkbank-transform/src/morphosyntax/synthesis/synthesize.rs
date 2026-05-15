//! Pure-function `Mor` builder for non-analyzable special-form words.

use talkbank_model::model::FormType;
use talkbank_model::model::dependent_tier::mor::{Mor, MorStem, MorWord, PosCategory};

use super::super::mor_word::clean_lemma;
use super::table::scat_synthesis;

/// Build a synthesized `Mor` for a special-form word.
///
/// `surface` is the cleaned text of the word (the `@<letter>` suffix
/// already stripped — typically `ExtractedWord::text`). The lemma is
/// the surface text after the project's standard lemma sanitization
/// (`clean_lemma` from `mor_word.rs`), which strips leading/trailing
/// hyphens and rewrites inner hyphens as en-dashes (U+2013) per the
/// CHAT `%mor` grammar convention. Features are form-type-derived.
///
/// Without `clean_lemma`, surfaces like `ie-@u` produced lemmas
/// ending in `-` (e.g., `uni|ie-`) which the `%mor` parser rejected
/// with E316. See `contract_s2_*` tests below.
pub fn synthesize_special_form_mor(form_type: &FormType, surface: &str) -> Mor {
    let rule = scat_synthesis(form_type);
    // `clean_lemma(lemma, text)` falls back to `text` when `lemma`
    // is unrecoverable; for synthesis the caller's `surface` is both
    // the lemma source and the fallback, so we pass it twice.
    let (cleaned, _is_unknown) = clean_lemma(surface, surface);
    let mut word = MorWord::new(PosCategory::new(rule.scat), MorStem::new(&cleaned));
    for feature in rule.features {
        word = word.with_feature(feature);
    }
    Mor::new(word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::WriteChat as _;

    fn render(form_type: FormType, surface: &str) -> String {
        let mut buf = String::new();
        synthesize_special_form_mor(&form_type, surface)
            .write_chat(&mut buf)
            .unwrap();
        buf
    }

    /// Table-driven check covering every `FormType` row whose synthesis
    /// is fully determined by scat alone (no extra features). Adding a
    /// row to `table.rs` without updating this is the explicit bridge
    /// between code and tests; the exhaustive match in `table.rs`
    /// already enforces compile-time coverage.
    #[test]
    fn scat_emitted_for_each_simple_form_type() {
        let cases: &[(FormType, &str, &str)] = &[
            (FormType::A, "ba", "unk|ba"),
            (FormType::B, "baba", "bab|baba"),
            (FormType::C, "wuggies", "chi|wuggies"),
            (FormType::D, "younz", "dia|younz"),
            (FormType::F, "mama", "fam|mama"),
            (FormType::FP, "um", "co|um"),
            (FormType::G, "gongga", "unk|gongga"),
            (FormType::I, "uhuh", "co|uhuh"),
            (FormType::K, "abcd", "n:let|abcd"),
            (FormType::L, "a", "n:let|a"),
            (FormType::N, "glanglo", "neo|glanglo"),
            (FormType::O, "woof", "on|woof"),
            (FormType::P, "ba", "phon|ba"),
            (FormType::Q, "cat", "meta|cat"),
            (FormType::SAS, "hello", "sas|hello"),
            (FormType::SI, "lala", "sing|lala"),
            (FormType::SL, "hello", "sign|hello"),
            (FormType::T, "wug", "test|wug"),
            (FormType::U, "vɔleɪ", "uni|vɔleɪ"),
            (FormType::WP, "cattywumpus", "wplay|cattywumpus"),
            (FormType::X, "stuff", "unk|stuff"),
        ];
        for (ft, surface, expected) in cases {
            assert_eq!(render(ft.clone(), surface), *expected);
        }
    }

    /// `@n` lemma is the surface form, not Stanza's hallucinated lemma.
    /// Email-thread example: `sprakin(g)@n` should NOT carry `sprake`.
    #[test]
    fn neologism_lemma_is_surface_not_stanza_guess() {
        assert_eq!(render(FormType::N, "sprakin"), "neo|sprakin");
    }

    /// `[: x@n]` placeholder pattern: lemma is whatever the surface is
    /// (here, the literal `x` placeholder).
    #[test]
    fn neologism_with_x_placeholder_lemma() {
        assert_eq!(render(FormType::N, "x"), "neo|x");
    }

    /// `@ls` carries the form-type-derived flat `-Plur` feature so
    /// CLAN's KIDEVAL `+-Plur` matcher (`kideval.cpp:4399`) catches it.
    #[test]
    fn letter_plural_carries_flat_plur_feature() {
        assert_eq!(render(FormType::LS, "a"), "n:let|a-Plur");
    }

    /// `@z:xxx` carries the user code as a flat feature so tools that
    /// know the code can recover it; CLAN ignores unknown flat features.
    #[test]
    fn user_defined_carries_user_code_as_flat_feature() {
        assert_eq!(
            render(FormType::UserDefined("rtfd".to_string()), "word"),
            "unk|word-rtfd"
        );
    }

    // ============================================================
    // Synthesis-layer contract tests written 2026-05-01 from
    // first-principles claims about what the synthesis layer must do.
    // No implementation code consulted while writing these. Tests
    // that fail are evidence the synthesis layer violates the
    // contract — not evidence the test is wrong (refine if proved
    // otherwise).
    // ============================================================

    /// Contract S2 — Surface text containing characters that have
    /// `%mor` grammar meaning must not produce an unparseable lemma.
    /// The grammar uses `-` as the feature separator (`verb|see-Past`),
    /// so a lemma ending in `-` opens a suffix that never starts and
    /// the parser rejects with E316. The synthesis layer must either
    /// sanitize trailing `-` OR return an error — silent emission of
    /// `<scat>|<text>-` is broken output.
    ///
    /// Operational evidence (2026-05-01 push of childes-other-data):
    /// `Japanese/MiiPro/Nanami/31112.cha` lines 2468 and 5326 contain
    /// `*CHI:	ie-@u …` which the synthesis layer turned into
    /// `%mor:	uni|ie- …`, blocking the push via E316 on
    /// `chatter validate`.
    ///
    /// Expectation: SHOULD FAIL TODAY.
    #[test]
    fn contract_s2_unibet_with_trailing_hyphen_surface_must_not_produce_unparseable_lemma() {
        // Surface "ie-" simulates what the form-marker stripper passes
        // to synthesis after dropping `@u` from input "ie-@u".
        let output = render(FormType::U, "ie-");
        let lemma = output
            .split('|')
            .nth(1)
            .expect("synthesis output must have scat|lemma form");
        assert!(
            !lemma.ends_with('-'),
            "Contract S2 violated: synthesis produced '{output}' for surface 'ie-' (FormType::U); \
             lemma '{lemma}' ends in '-' which the %mor parser rejects with E316. \
             Either sanitize trailing '-' from the lemma or have construction fail."
        );
    }

    /// Contract S2 (companion) — same constraint for `@n` neologism
    /// form. A neologism token written as `coast-@n` (a hyphenated
    /// neologism) would hit the same bug.
    ///
    /// Expectation: SHOULD FAIL TODAY (same root cause).
    #[test]
    fn contract_s2_neologism_with_trailing_hyphen_surface_must_not_produce_unparseable_lemma() {
        let output = render(FormType::N, "coast-");
        let lemma = output
            .split('|')
            .nth(1)
            .expect("synthesis output must have scat|lemma form");
        assert!(
            !lemma.ends_with('-'),
            "Contract S2 violated for FormType::N: '{output}' has lemma '{lemma}' ending in '-'"
        );
    }
}
