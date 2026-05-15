//! WER word conforming for benchmark evaluation.
//!
//! Normalizes word lists before Word Error Rate (WER) comparison by applying
//! deterministic transformations: compound splitting, contraction expansion,
//! filler normalization, name replacement, abbreviation expansion, and special
//! word handling.
//!
//! This is the Rust replacement for the Python `_conform()` function from
//! `inference/benchmark.py`, exposed to Python via the
//! [`batchalign_core.wer_conform()`] PyO3 function.
//!
//! # Data files
//!
//! The module loads four embedded JSON data files at first use via [`LazyLock`]:
//!
//! - `compounds.json` — 3,584 compound word pairs (shared with [`asr_postprocess`])
//! - `names.json` — ~6,700 proper names (lowercased)
//! - `abbrev.json` — ~400 abbreviations (original case)
//!
//! [`asr_postprocess`]: crate::asr_postprocess

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Compound word lookup: maps joined compound → (part_a, part_b) for O(1) splitting.
///
/// Reuses the same data file as [`asr_postprocess::compounds`](crate::asr_postprocess).
// Data is compile-time-constant: `include_str!` embeds the JSON at build time.
#[allow(clippy::expect_used)]
static COMPOUND_MAP: LazyLock<HashMap<String, (String, String)>> = LazyLock::new(|| {
    let data: Vec<[String; 2]> = serde_json::from_str(include_str!("../data/compounds.json"))
        .expect("embedded compounds.json is valid");
    data.into_iter()
        .map(|[a, b]| {
            let joined = format!("{a}{b}");
            (joined, (a, b))
        })
        .collect()
});

/// Known proper names (lowercased). Replaced with `"name"` during WER evaluation
/// to avoid penalizing name recognition errors.
// Data is compile-time-constant: `include_str!` embeds the JSON at build time.
#[allow(clippy::expect_used)]
static NAMES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let data: Vec<String> = serde_json::from_str(include_str!("../data/names.json"))
        .expect("embedded names.json is valid");
    data.into_iter().collect()
});

/// Known abbreviations (original case). Letter-expanded during WER evaluation
/// (e.g., `"FBI"` → `["F", "B", "I"]`).
///
/// Python checks abbreviations in original case (`i.strip() in abbrev`), not
/// lowercased, so we preserve that behavior here.
// Data is compile-time-constant: `include_str!` embeds the JSON at build time.
#[allow(clippy::expect_used)]
static ABBREV: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let data: Vec<String> = serde_json::from_str(include_str!("../data/abbrev.json"))
        .expect("embedded abbrev.json is valid");
    data.into_iter().collect()
});

/// Common speech fillers, all normalized to `"um"` during WER evaluation.
static FILLERS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["um", "uhm", "em", "mhm", "uhhm", "eh", "uh", "hm"]
        .into_iter()
        .collect()
});

/// Normalize a word list for WER comparison.
///
/// Each input word is lowercased and then checked against a priority-ordered
/// rule chain. The first matching rule produces the output token(s) for that
/// word. Rules are applied per-word — the output may contain more tokens than
/// the input (compound splits, contraction expansions, etc.).
///
/// # Transformation rules (in priority order)
///
/// 1. **Compound splitting** — known compound words are split into their
///    constituent parts (e.g., `"airplane"` → `["air", "plane"]`).
/// 2. **Abbreviation letter expansion** — known abbreviations are expanded to
///    individual letters in original case (e.g., `"FBI"` → `["F", "B", "I"]`).
/// 3. **Contraction expansion** — English contractions are split and expanded
///    (`'s` → `is`, `'ve` → `have`, `'d` → `had`, `'m` → `am`).
/// 4. **Filler normalization** — common fillers (`um`, `uhm`, `eh`, `mhm`,
///    etc.) are all normalized to `"um"`.
/// 5. **Hyphen splitting** — hyphenated words are split at hyphens.
/// 6. **Special word expansions** — colloquial forms are expanded
///    (`gimme` → `give me`, `wanna` → `want to`, `gonna` → `going to`, etc.).
/// 7. **Name replacement** — known proper names are replaced with `"name"`.
/// 8. **Specific acronym expansion** — selected acronyms are letter-expanded
///    (`mba`, `tli`, `bbc`, `ai`, `aa`, `ii`).
/// 9. **Underscore splitting** — underscore-joined words are split.
/// 10. **Passthrough** — unrecognized words pass through lowercased.
pub fn conform_words(words: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(words.len());

    for word in words {
        let trimmed = word.trim();
        let w = trimmed.to_lowercase();

        if let Some((a, b)) = COMPOUND_MAP.get(&w) {
            result.push(a.clone());
            result.push(b.clone());
        } else if ABBREV.contains(trimmed) {
            // Python checks abbreviations in original case
            for ch in trimmed.chars() {
                result.push(ch.to_string());
            }
        } else if w.contains("'s") {
            result.push(w.split('\'').next().unwrap_or("").to_string());
            result.push("is".to_string());
        } else if w.contains("'ve") {
            result.push(w.split('\'').next().unwrap_or("").to_string());
            result.push("have".to_string());
        } else if w.contains("'d") {
            result.push(w.split('\'').next().unwrap_or("").to_string());
            result.push("had".to_string());
        } else if w.contains("'m") {
            result.push(w.split('\'').next().unwrap_or("").to_string());
            result.push("am".to_string());
        } else if FILLERS.contains(w.as_str()) {
            result.push("um".to_string());
        } else if w.contains('-') {
            for part in w.split('-') {
                result.push(part.trim().to_string());
            }
        } else if w == "ok" {
            result.push("okay".to_string());
        } else if w == "gimme" {
            result.extend(["give", "me"].map(String::from));
        } else if w == "hafta" || w == "havta" {
            result.extend(["have", "to"].map(String::from));
        } else if NAMES.contains(&w) {
            result.push("name".to_string());
        } else if w == "dunno" {
            result.extend(["don't", "know"].map(String::from));
        } else if w == "wanna" {
            result.extend(["want", "to"].map(String::from));
        } else if w == "gonna" {
            result.extend(["going", "to"].map(String::from));
        } else if w == "gotta" {
            result.extend(["got", "to"].map(String::from));
        } else if w == "kinda" {
            result.extend(["kind", "of"].map(String::from));
        } else if w == "sorta" {
            result.extend(["sort", "of"].map(String::from));
        } else if w == "shoulda" {
            result.extend(["should", "have"].map(String::from));
        } else if w == "sposta" {
            result.extend(["supposed", "to"].map(String::from));
        } else if w == "hadta" {
            result.extend(["had", "to"].map(String::from));
        } else if w == "alright" || w == "alrightie" {
            result.extend(["all", "right"].map(String::from));
        } else if w == "i'd" {
            result.extend(["i", "had"].map(String::from));
        } else if w == "this'll" {
            result.extend(["this", "will"].map(String::from));
        } else if w == "farmhouse" {
            result.extend(["farm", "house"].map(String::from));
        } else if w == "mm" || w == "hmm" {
            result.push("hm".to_string());
        } else if w == "em" {
            result.push("them".to_string());
        } else if w == "eh" {
            result.push("uh".to_string());
        } else if w == "til" {
            result.push("until".to_string());
        } else if w == "ed" {
            result.push("education".to_string());
        } else if matches!(w.as_str(), "mba" | "tli" | "bbc" | "ai" | "aa" | "ii") {
            for ch in w.chars() {
                result.push(ch.to_string());
            }
        } else if w.contains('_') {
            for part in w.split('_') {
                result.push(part.to_string());
            }
        } else {
            result.push(w);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn test_data_loaded() {
        assert!(NAMES.len() > 5000);
        assert!(ABBREV.len() > 100);
        assert!(COMPOUND_MAP.len() > 3000);
    }

    #[test]
    fn test_compound_split() {
        let result = conform_words(&s(&["airplane"]));
        assert_eq!(result, s(&["air", "plane"]));
    }

    #[test]
    fn test_contraction_expansion() {
        assert_eq!(conform_words(&s(&["he's"])), s(&["he", "is"]));
        assert_eq!(conform_words(&s(&["I've"])), s(&["i", "have"]));
        assert_eq!(conform_words(&s(&["she'd"])), s(&["she", "had"]));
        assert_eq!(conform_words(&s(&["I'm"])), s(&["i", "am"]));
    }

    #[test]
    fn test_filler_normalization() {
        assert_eq!(conform_words(&s(&["uhm"])), s(&["um"]));
        assert_eq!(conform_words(&s(&["mhm"])), s(&["um"]));
    }

    #[test]
    fn test_name_replacement() {
        // "aaron" is a common name that should be in the list
        assert_eq!(conform_words(&s(&["Aaron"])), s(&["name"]));
    }

    #[test]
    fn test_special_words() {
        assert_eq!(conform_words(&s(&["ok"])), s(&["okay"]));
        assert_eq!(conform_words(&s(&["gimme"])), s(&["give", "me"]));
        assert_eq!(conform_words(&s(&["wanna"])), s(&["want", "to"]));
        assert_eq!(conform_words(&s(&["gonna"])), s(&["going", "to"]));
        assert_eq!(conform_words(&s(&["dunno"])), s(&["don't", "know"]));
        assert_eq!(conform_words(&s(&["alright"])), s(&["all", "right"]));
    }

    #[test]
    fn test_hyphen_split() {
        assert_eq!(conform_words(&s(&["well-known"])), s(&["well", "known"]));
    }

    #[test]
    fn test_underscore_split() {
        assert_eq!(conform_words(&s(&["ice_cream"])), s(&["ice", "cream"]));
    }

    #[test]
    fn test_abbreviation_expansion() {
        // Python iterates original-case chars, so "FBI" → "F", "B", "I"
        assert_eq!(conform_words(&s(&["FBI"])), s(&["F", "B", "I"]));
    }

    #[test]
    fn test_acronym_expansion() {
        assert_eq!(conform_words(&s(&["mba"])), s(&["m", "b", "a"]));
        assert_eq!(conform_words(&s(&["ai"])), s(&["a", "i"]));
    }

    #[test]
    fn test_passthrough() {
        assert_eq!(
            conform_words(&s(&["hello", "world"])),
            s(&["hello", "world"])
        );
    }

    #[test]
    fn test_empty() {
        let result = conform_words(&s(&[]));
        assert!(result.is_empty());
    }

    #[test]
    fn test_mixed() {
        let result = conform_words(&s(&["Aaron", "he's", "gonna", "ok"]));
        assert_eq!(result, s(&["name", "he", "is", "going", "to", "okay"]));
    }

    // --- property tests ---

    use proptest::prelude::*;

    fn word_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            // Common words (passthrough)
            Just("hello".to_string()),
            Just("world".to_string()),
            Just("the".to_string()),
            Just("cat".to_string()),
            // Special words (expansion)
            Just("gonna".to_string()),
            Just("wanna".to_string()),
            Just("gimme".to_string()),
            Just("ok".to_string()),
            Just("dunno".to_string()),
            // Contractions
            Just("he's".to_string()),
            Just("I've".to_string()),
            Just("she'd".to_string()),
            // Fillers
            Just("um".to_string()),
            Just("uhm".to_string()),
            Just("mhm".to_string()),
            // Hyphenated
            Just("well-known".to_string()),
            // Random lowercase words
            "[a-z]{1,6}".prop_map(|s| s),
        ]
    }

    fn word_vec(max_len: usize) -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec(word_strategy(), 0..=max_len)
    }

    proptest! {
        /// Output length is always >= input length (transforms expand, never reduce).
        #[test]
        fn output_never_shrinks(words in word_vec(10)) {
            let result = conform_words(&words);
            prop_assert!(
                result.len() >= words.len(),
                "output {} < input {}", result.len(), words.len()
            );
        }

        /// No empty strings in output when input has no empty strings.
        #[test]
        fn no_empty_output_tokens(words in word_vec(10)) {
            let non_empty: Vec<String> = words.into_iter()
                .filter(|w| !w.trim().is_empty())
                .collect();
            let result = conform_words(&non_empty);
            for (i, token) in result.iter().enumerate() {
                prop_assert!(
                    !token.is_empty(),
                    "Empty token at index {} from input {:?}", i, non_empty
                );
            }
        }

        /// Applying conform twice is a fixed point: conform(conform(x)) == conform(conform(conform(x))).
        /// The first application may change case (abbreviation expansion preserves original case),
        /// but the second application normalizes to lowercase, which is stable thereafter.
        #[test]
        fn double_application_is_fixed_point(words in word_vec(8)) {
            let once = conform_words(&words);
            let twice = conform_words(&once);
            let thrice = conform_words(&twice);
            prop_assert_eq!(
                &twice, &thrice,
                "Not a fixed point at depth 2: {:?} -> {:?} -> {:?}",
                once, twice, thrice
            );
        }

        /// Empty input always produces empty output.
        #[test]
        fn empty_input_empty_output(_dummy in 0..1u8) {
            let result = conform_words(&[]);
            prop_assert!(result.is_empty());
        }

        /// Each input word produces at least one output word.
        /// This verifies no words are silently dropped.
        #[test]
        fn every_word_produces_output(words in word_vec(10)) {
            let non_empty: Vec<String> = words.into_iter()
                .filter(|w| !w.trim().is_empty())
                .collect();
            let result = conform_words(&non_empty);
            prop_assert!(
                result.len() >= non_empty.len(),
                "Some words were dropped: {} input, {} output",
                non_empty.len(), result.len()
            );
        }
    }
}
