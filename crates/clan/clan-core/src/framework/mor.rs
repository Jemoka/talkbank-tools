//! Shared `%mor` tier utilities for analysis commands.
//!
//! Several CLAN commands need to extract and process `%mor` dependent tier
//! text from utterances. This module centralizes:
//!
//! - [`extract_mor_text`] — extracts the `%mor` tier content from an utterance
//! - [`MorPosCount`] — POS/inflection counters for morphological classification
//! - [`classify_mor_item`] — classifies a typed `%mor` item into grammatical categories
//! - [`count_morphemes_typed`] — counts morphemes in a typed `%mor` item

use talkbank_model::{DependentTier, Mor, MorTier, MorWord, Utterance};

use crate::framework::dependent_tier_content_text;

/// Per-speaker POS and inflectional morphology counters.
///
/// Extracted from analysis accumulators so that multiple commands (EVAL,
/// KIDEVAL, etc.) can share the classification logic without duplicating
/// the 14 counter fields.
#[derive(Debug, Default, Clone)]
pub struct MorPosCount {
    /// Nouns.
    pub nouns: u64,
    /// Verbs (all types except auxiliaries).
    pub verbs: u64,
    /// Auxiliary verbs.
    pub auxiliaries: u64,
    /// Modal verbs.
    pub modals: u64,
    /// Prepositions.
    pub prepositions: u64,
    /// Adjectives.
    pub adjectives: u64,
    /// Adverbs.
    pub adverbs: u64,
    /// Conjunctions.
    pub conjunctions: u64,
    /// Determiners.
    pub determiners: u64,
    /// Pronouns.
    pub pronouns: u64,
    /// Plurals.
    pub plurals: u64,
    /// Past tense.
    pub past_tense: u64,
    /// Present participle (-ing).
    pub present_participle: u64,
    /// Past participle.
    pub past_participle: u64,
}

/// Extract the typed `%mor` tier from an utterance.
///
/// Returns a reference to the `MorTier` if present and non-empty, or `None`.
pub fn extract_mor_tier(utterance: &Utterance) -> Option<&MorTier> {
    for dep in &utterance.dependent_tiers {
        if let DependentTier::Mor(tier) = dep {
            if !tier.items().is_empty() {
                return Some(tier);
            }
            return None;
        }
    }
    None
}

/// Classify a typed `%mor` item and increment POS/inflection counts.
///
/// Typed alternative to [`classify_mor_token`] that operates on the parsed
/// `Mor` item directly instead of re-tokenizing serialized text.
pub fn classify_mor_item(item: &Mor, counts: &mut MorPosCount) {
    classify_mor_word(&item.main, counts);
    for clitic in &item.post_clitics {
        classify_mor_word(clitic, counts);
    }
}

/// Classify a single `MorWord` (POS + features) into counts.
///
/// Handles both modern UD POS tags (`noun`, `verb`, `propn`, `adp`, `cconj`)
/// and legacy CLAN tags (`n`, `v`, `n:prop`, `prep`, `conj`).
fn classify_mor_word(word: &MorWord, counts: &mut MorPosCount) {
    let pos = word.pos.as_str();
    let pos_lower = pos.to_lowercase();
    let pos_ref = pos_lower.as_str();

    // POS classification — accepts both UD and legacy tags.
    // Order matters: exact matches before prefix matches to avoid
    // "propn" hitting the "pro" prefix intended for pronouns.
    match pos_ref {
        // Nouns: UD "noun"/"propn", legacy "n"/"n:prop"
        "noun" | "propn" => counts.nouns += 1,
        p if p.starts_with("n:prop") || p == "n" => counts.nouns += 1,

        // Auxiliaries: UD "aux", legacy "v:aux" (must precede verb catch-all)
        "aux" | "v:aux" => counts.auxiliaries += 1,

        // Verbs: UD "verb", legacy "v" (excluding "v:aux" already matched above)
        "verb" => counts.verbs += 1,
        p if p.starts_with("v") => counts.verbs += 1,

        // Modals: legacy "mod" (UD folds modals into "aux" or "verb")
        "mod" => counts.modals += 1,

        // Prepositions: UD "adp", legacy "prep"
        "adp" | "prep" => counts.prepositions += 1,

        // Adjectives
        "adj" => counts.adjectives += 1,

        // Adverbs
        p if p.starts_with("adv") => counts.adverbs += 1,

        // Conjunctions: UD "cconj"/"sconj", legacy "conj"/"conj:*"
        "cconj" | "sconj" => counts.conjunctions += 1,
        p if p.starts_with("conj") => counts.conjunctions += 1,

        // Determiners
        p if p.starts_with("det") => counts.determiners += 1,

        // Pronouns: UD "pron", legacy "pro"/"pro:sub"/etc.
        // Note: "propn" is already matched above as noun, so won't reach here.
        "pron" => counts.pronouns += 1,
        p if p.starts_with("pro") => counts.pronouns += 1,

        _ => {}
    }

    // Feature/inflection classification.
    // Handles both UD features (Plur, Past, Part-Past, Part-Pres, Ger)
    // and legacy features (PL, PAST, PASTP, PRESP).
    classify_features(&word.features, counts);
}

/// Classify morphological features into inflection counts.
///
/// UD features use names like `Plur`, `Past`, `Part-Past`, `Part-Pres`, `Ger`.
/// Legacy features use `PL`, `PAST`, `PASTP`, `PRESP`.
/// We check each feature individually rather than substring-matching across
/// the full feature list, to avoid false positives.
fn classify_features(features: &[talkbank_model::MorFeature], counts: &mut MorPosCount) {
    // Track per-word (not per-feature) to avoid double-counting when
    // multiple features match the same category.
    let mut has_plural = false;
    let mut has_past = false;
    let mut has_presp = false;
    let mut has_pastp = false;

    for feature in features {
        let val = feature.value().to_lowercase();
        match val.as_str() {
            // Exact matches for common feature values
            "plur" | "pl" => has_plural = true,
            "past" => has_past = true,
            "presp" => has_presp = true,
            "pastp" => has_pastp = true,
            "ger" => has_presp = true, // UD gerund = present participle

            // UD compound features: "Part-Past", "Part-Pres"
            // Note: has_pastp already increments past_tense in finalization,
            // so we don't set has_past here to avoid double-counting.
            v if v.contains("part") && v.contains("past") => {
                has_pastp = true;
            }
            v if v.contains("part") && v.contains("pres") => has_presp = true,

            // Substring fallbacks for other legacy feature names
            v if v.contains("plur") || v == "pl" => has_plural = true,
            _ => {}
        }
    }

    if has_plural {
        counts.plurals += 1;
    }
    if has_past {
        counts.past_tense += 1;
    }
    if has_presp {
        counts.present_participle += 1;
    }
    if has_pastp {
        counts.past_participle += 1;
        // Past participles also increment past_tense (matching legacy CLAN behavior
        // where -PASTP contains -PAST as substring).
        counts.past_tense += 1;
    }
}

/// Count morphemes in a typed `%mor` item.
///
/// Each `MorWord` contributes 1 base morpheme plus one per feature.
/// Post-clitics each contribute their own morpheme count.
pub fn count_morphemes_typed(item: &Mor) -> u64 {
    count_word_morphemes(&item.main)
        + item
            .post_clitics
            .iter()
            .map(count_word_morphemes)
            .sum::<u64>()
}

/// Count morphemes in a single `MorWord`: 1 base + number of features.
fn count_word_morphemes(word: &MorWord) -> u64 {
    1 + word.features.len() as u64
}

/// Check if a `%mor` pattern matches any word in a `Mor` item (main + clitics).
///
/// Pattern formats used by DSS and IPSYN rule files:
/// - `"pos_prefix"` — matches if any word's POS starts with prefix (e.g., `"pro:sub"`, `"v"`)
/// - `"pos_prefix|"` — same, with explicit IPSYN-style trailing separator
/// - `"POS-FEATURE"` — matches POS prefix **and** feature value (e.g., `"v-PAST"`)
/// - `"FEATURE"` — matches any feature value when no POS prefix matches (e.g., `"POSS"`)
///
/// All matching is case-insensitive.
pub fn mor_pattern_matches(item: &Mor, pattern: &str) -> bool {
    mor_word_pattern_matches(&item.main, pattern)
        || item
            .post_clitics
            .iter()
            .any(|w| mor_word_pattern_matches(w, pattern))
}

/// Check if a single `MorWord` matches a %mor pattern string.
fn mor_word_pattern_matches(word: &MorWord, pattern: &str) -> bool {
    let pat = pattern.trim_end_matches('|');
    if pat.is_empty() {
        return false;
    }
    let pat_lower = pat.to_lowercase();
    let pos_lower = word.pos.as_str().to_lowercase();

    // Compound "POS-FEATURE" pattern (e.g., "v-PAST")
    if let Some((pos_part, feature_part)) = pat_lower.split_once('-') {
        return pos_lower.starts_with(pos_part)
            && word
                .features
                .iter()
                .any(|f| f.value().to_lowercase().contains(feature_part));
    }

    // POS prefix match (e.g., "pro:sub", "v", "det:art")
    if pos_lower.starts_with(&pat_lower) {
        return true;
    }

    // Feature value match (e.g., "POSS", "PAST")
    word.features
        .iter()
        .any(|f| f.value().to_lowercase().contains(&pat_lower))
}

/// Check if any word in a slice of `Mor` items has a POS matching the given prefix.
///
/// Used by `is_complete_sentence` where exact POS matching (not feature fallback)
/// is needed. Checks main word and all post-clitics.
pub fn any_item_has_pos(items: &[Mor], pos_prefix: &str) -> bool {
    let prefix = pos_prefix.to_lowercase();
    items.iter().any(|item| {
        let check = |w: &MorWord| w.pos.as_str().to_lowercase().starts_with(&prefix);
        check(&item.main) || item.post_clitics.iter().any(check)
    })
}

/// Extract the `%mor` tier text content from an utterance.
///
/// Searches the utterance's dependent tiers for the `%mor` tier, extracts
/// the text after the tab character (stripping the `%mor:\t` prefix), and
/// returns it. Returns `None` if no `%mor` tier is present or if its
/// content is empty.
///
/// This replaces the 4-way duplicated `dep.kind() == "mor"` loop found
/// in eval.rs, kideval.rs, dss.rs, and ipsyn.rs.
pub fn extract_mor_text(utterance: &Utterance) -> Option<String> {
    for dep in &utterance.dependent_tiers {
        if let DependentTier::Mor(_) = dep {
            let content = dependent_tier_content_text(dep);
            if !content.is_empty() {
                return Some(content);
            }
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use smallvec::smallvec;
    use talkbank_model::MorFeature;

    /// Helper to build a typed `Mor` item for testing.
    fn mor(pos: &str, lemma: &str, features: &[&str]) -> Mor {
        let mut word = MorWord::new(pos, lemma);
        word.features = features.iter().map(MorFeature::new).collect();
        Mor {
            main: word,
            post_clitics: smallvec![],
        }
    }

    #[test]
    fn count_morphemes_typed_basic() {
        assert_eq!(count_morphemes_typed(&mor("n", "dog", &[])), 1);
        assert_eq!(count_morphemes_typed(&mor("n", "dog", &["PL"])), 2);
        assert_eq!(count_morphemes_typed(&mor("v", "walk", &["PAST"])), 2);
        assert_eq!(count_morphemes_typed(&mor("v", "walk", &["3S", "PAST"])), 3);
    }

    #[test]
    fn classify_mor_item_categories() {
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("n", "dog", &[]), &mut counts);
        classify_mor_item(&mor("v", "walk", &[]), &mut counts);
        classify_mor_item(&mor("adj", "big", &[]), &mut counts);
        classify_mor_item(&mor("det:art", "the", &[]), &mut counts);
        assert_eq!(counts.nouns, 1);
        assert_eq!(counts.verbs, 1);
        assert_eq!(counts.adjectives, 1);
        assert_eq!(counts.determiners, 1);
    }

    #[test]
    fn classify_mor_item_inflections() {
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("n", "dog", &["PL"]), &mut counts);
        assert_eq!(counts.plurals, 1);

        let mut counts2 = MorPosCount::default();
        classify_mor_item(&mor("v", "walk", &["PAST"]), &mut counts2);
        assert_eq!(counts2.past_tense, 1);

        let mut counts3 = MorPosCount::default();
        classify_mor_item(&mor("v", "run", &["PRESP"]), &mut counts3);
        assert_eq!(counts3.present_participle, 1);

        // Note: -PASTP also matches -PAST (substring), so past_tense is incremented too.
        // This matches the original eval.rs behavior.
        let mut counts4 = MorPosCount::default();
        classify_mor_item(&mor("v", "eat", &["PASTP"]), &mut counts4);
        assert_eq!(counts4.past_participle, 1);
        assert_eq!(counts4.past_tense, 1);
    }

    #[test]
    fn classify_auxiliaries_and_modals() {
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("aux", "be", &[]), &mut counts);
        classify_mor_item(&mor("v:aux", "have", &[]), &mut counts);
        classify_mor_item(&mor("mod", "can", &[]), &mut counts);
        assert_eq!(counts.auxiliaries, 2);
        assert_eq!(counts.modals, 1);
        assert_eq!(counts.verbs, 0);
    }

    #[test]
    fn extract_mor_text_returns_none_when_absent() {
        use talkbank_model::Span;
        use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let utt = Utterance::new(main);
        assert!(extract_mor_text(&utt).is_none());
    }

    // --- mor_pattern_matches tests ---

    #[test]
    fn pattern_matches_pos_prefix() {
        let item = mor("pro:sub", "I", &[]);
        assert!(mor_pattern_matches(&item, "pro:sub"));
        assert!(mor_pattern_matches(&item, "pro"));
        assert!(!mor_pattern_matches(&item, "v"));
    }

    #[test]
    fn pattern_matches_ipsyn_trailing_pipe() {
        let item = mor("n", "dog", &[]);
        assert!(mor_pattern_matches(&item, "n|"));
        assert!(!mor_pattern_matches(&item, "v|"));
    }

    #[test]
    fn pattern_matches_compound_pos_feature() {
        let item = mor("v", "walk", &["PAST"]);
        assert!(mor_pattern_matches(&item, "v-PAST"));
        assert!(!mor_pattern_matches(&item, "v-POSS"));
        assert!(!mor_pattern_matches(&item, "n-PAST"));
    }

    #[test]
    fn pattern_matches_feature_only() {
        let item = mor("n", "dog", &["POSS"]);
        assert!(mor_pattern_matches(&item, "POSS"));
        assert!(!mor_pattern_matches(&item, "PAST"));
    }

    #[test]
    fn pattern_matches_case_insensitive() {
        let item = mor("Pro:Sub", "I", &["Nom"]);
        assert!(mor_pattern_matches(&item, "pro:sub"));
        assert!(mor_pattern_matches(&item, "PRO:SUB"));
        assert!(mor_pattern_matches(&item, "nom"));
    }

    #[test]
    fn pattern_matches_post_clitic() {
        let main_word = MorWord::new("pro:sub", "I");
        let clitic = MorWord::new("aux", "be");
        let item = Mor {
            main: main_word,
            post_clitics: smallvec![clitic],
        };
        assert!(mor_pattern_matches(&item, "aux"));
        assert!(mor_pattern_matches(&item, "pro:sub"));
    }

    #[test]
    fn any_item_has_pos_basic() {
        let items = vec![
            mor("pro:sub", "I", &[]),
            mor("v", "want", &[]),
            mor("n", "ball", &[]),
        ];
        assert!(any_item_has_pos(&items, "pro:sub"));
        assert!(any_item_has_pos(&items, "v"));
        assert!(any_item_has_pos(&items, "n"));
        assert!(!any_item_has_pos(&items, "adj"));
    }

    // --- UD format tests ---

    #[test]
    fn classify_ud_pos_tags() {
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("noun", "dog", &[]), &mut counts);
        classify_mor_item(&mor("propn", "John", &[]), &mut counts);
        classify_mor_item(&mor("verb", "walk", &[]), &mut counts);
        classify_mor_item(&mor("adp", "in", &[]), &mut counts);
        classify_mor_item(&mor("cconj", "and", &[]), &mut counts);
        classify_mor_item(&mor("sconj", "because", &[]), &mut counts);
        classify_mor_item(&mor("pron", "I", &[]), &mut counts);
        classify_mor_item(&mor("det", "the", &[]), &mut counts);
        classify_mor_item(&mor("adv", "quickly", &[]), &mut counts);
        classify_mor_item(&mor("aux", "will", &[]), &mut counts);
        assert_eq!(counts.nouns, 2, "noun + propn");
        assert_eq!(counts.verbs, 1);
        assert_eq!(counts.prepositions, 1);
        assert_eq!(counts.conjunctions, 2, "cconj + sconj");
        assert_eq!(counts.pronouns, 1);
        assert_eq!(counts.determiners, 1);
        assert_eq!(counts.adverbs, 1);
        assert_eq!(counts.auxiliaries, 1);
    }

    #[test]
    fn classify_ud_propn_not_pronoun() {
        // Critical: "propn" must count as noun, NOT pronoun.
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("propn", "Mommy", &[]), &mut counts);
        assert_eq!(counts.nouns, 1);
        assert_eq!(counts.pronouns, 0);
    }

    #[test]
    fn classify_ud_features() {
        // UD "Plur" feature
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("noun", "cookie", &["Plur"]), &mut counts);
        assert_eq!(counts.plurals, 1);

        // UD "Past" feature
        let mut counts2 = MorPosCount::default();
        classify_mor_item(&mor("verb", "walk", &["Past"]), &mut counts2);
        assert_eq!(counts2.past_tense, 1);

        // UD compound "Part-Past" (past participle)
        let mut counts3 = MorPosCount::default();
        classify_mor_item(&mor("verb", "eat", &["Part-Past"]), &mut counts3);
        assert_eq!(counts3.past_participle, 1);
        assert_eq!(counts3.past_tense, 1);

        // UD compound "Part-Pres" (present participle / gerund)
        let mut counts4 = MorPosCount::default();
        classify_mor_item(&mor("verb", "run", &["Part-Pres"]), &mut counts4);
        assert_eq!(counts4.present_participle, 1);

        // UD "Ger" (gerund = present participle)
        let mut counts5 = MorPosCount::default();
        classify_mor_item(&mor("verb", "go", &["Ger"]), &mut counts5);
        assert_eq!(counts5.present_participle, 1);
    }

    #[test]
    fn classify_ud_multi_feature_no_double_count() {
        // UD features like "Fin-Ind-Pres-S3" — individual features, none should double-count
        let mut counts = MorPosCount::default();
        classify_mor_item(
            &mor("verb", "walk", &["Fin", "Ind", "Pres", "S3"]),
            &mut counts,
        );
        assert_eq!(counts.past_tense, 0);
        assert_eq!(counts.plurals, 0);
        assert_eq!(counts.present_participle, 0);
        assert_eq!(counts.past_participle, 0);
    }

    #[test]
    fn classify_mixed_legacy_and_ud() {
        // Verify both formats work in the same counter
        let mut counts = MorPosCount::default();
        classify_mor_item(&mor("n", "dog", &["PL"]), &mut counts); // legacy
        classify_mor_item(&mor("noun", "cat", &["Plur"]), &mut counts); // UD
        classify_mor_item(&mor("v", "run", &["PAST"]), &mut counts); // legacy
        classify_mor_item(&mor("verb", "walk", &["Past"]), &mut counts); // UD
        assert_eq!(counts.nouns, 2);
        assert_eq!(counts.verbs, 2);
        assert_eq!(counts.plurals, 2);
        assert_eq!(counts.past_tense, 2);
    }
}
