//! Compound word merging for ASR post-processing.
//!
//! Merges adjacent ASR tokens that form known English compound words
//! (e.g. "air" + "plane" → "airplane"). The compound list is loaded from
//! `data/compounds.json` at compile time (3,660 raw entries, 3,584 unique
//! pairs after `HashSet` dedup).

use std::collections::HashSet;
use std::sync::LazyLock;

use super::{AsrElement, AsrElementKind, AsrRawText};

/// Compound word pairs loaded from the JSON data file at compile time.
///
/// Each pair is `(first_word, second_word)` where adjacent occurrence
/// in ASR output should be merged into a single compound.
// Data is compile-time-constant: `include_str!` embeds the JSON at build time.
#[allow(clippy::unwrap_used)]
static COMPOUNDS: LazyLock<HashSet<(&'static str, &'static str)>> = LazyLock::new(|| {
    let data: Vec<[&str; 2]> =
        serde_json::from_str(include_str!("../../data/compounds.json")).unwrap();
    data.into_iter().map(|[a, b]| (a, b)).collect()
});

/// Merge adjacent ASR elements that form known compound words.
///
/// Uses a sliding window of size 2. When two adjacent tokens match a known
/// compound pair, they are merged: values concatenated (no space), timing
/// from the first element, kind set to `Text`.
///
/// # Example
///
/// ```text
/// ["air", "plane", "is", "here"] → ["airplane", "is", "here"]
/// ```
pub fn merge_compounds(elements: &[AsrElement]) -> Vec<AsrElement> {
    if elements.len() < 2 {
        return elements.to_vec();
    }

    let mut result: Vec<AsrElement> = Vec::with_capacity(elements.len());
    let mut i = 0;

    while i < elements.len() {
        if i + 1 < elements.len() {
            let w1 = elements[i].value.as_str();
            let w2 = elements[i + 1].value.as_str();
            if COMPOUNDS.contains(&(w1, w2)) {
                // Merge: concatenate values, take timing from first
                result.push(AsrElement {
                    value: AsrRawText::new(format!("{w1}{w2}")),
                    ts: elements[i].ts,
                    end_ts: elements[i].end_ts,
                    kind: AsrElementKind::Text,
                });
                i += 2;
                continue;
            }
        }
        result.push(elements[i].clone());
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr_postprocess::AsrTimestampSecs;

    fn elem(value: &str, ts: f64, end_ts: f64) -> AsrElement {
        AsrElement {
            value: AsrRawText::new(value),
            ts: AsrTimestampSecs(ts),
            end_ts: AsrTimestampSecs(end_ts),
            kind: AsrElementKind::Text,
        }
    }

    #[test]
    fn test_compounds_loaded() {
        // Python list has 3660 entries but 76 are duplicates → 3584 unique pairs
        assert_eq!(COMPOUNDS.len(), 3584);
    }

    #[test]
    fn test_airplane_compound() {
        assert!(COMPOUNDS.contains(&("air", "plane")));
    }

    #[test]
    fn test_merge_basic() {
        let elems = vec![
            elem("air", 0.0, 0.5),
            elem("plane", 0.5, 1.0),
            elem("is", 1.0, 1.3),
            elem("here", 1.3, 1.5),
        ];
        let merged = merge_compounds(&elems);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].value, "airplane");
        assert_eq!(merged[0].ts, 0.0);
        assert_eq!(merged[0].end_ts, 0.5); // timing from first
        assert_eq!(merged[1].value, "is");
        assert_eq!(merged[2].value, "here");
    }

    #[test]
    fn test_no_merge() {
        let elems = vec![elem("the", 0.0, 0.3), elem("cat", 0.3, 0.6)];
        let merged = merge_compounds(&elems);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].value, "the");
        assert_eq!(merged[1].value, "cat");
    }

    #[test]
    fn test_empty_input() {
        let merged = merge_compounds(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_single_element() {
        let elems = vec![elem("hello", 0.0, 0.5)];
        let merged = merge_compounds(&elems);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].value, "hello");
    }

    // -- Verb-particle merge policy regression tests --
    //
    // `data/compounds.json` contains `["come","back"]` and `["put","down"]`,
    // and `merge_compounds()` applies them unconditionally with no POS /
    // phrase-vs-noun disambiguation. The empirical CHAT corpora strongly
    // favor the open form for these verb-particle pairs
    // (see `book/src/decisions/asr-compound-merging.md`).
    //
    // These tests encode the policy that the bare 2-token sequence
    // should survive as two tokens when the usage is verb-particle.
    // They are `#[ignore]` pending the compound-list audit proposed in
    // the decision document.

    #[test]
    #[ignore = "pending compound-list audit: verb-particle 'come back' should not merge to 'comeback'"]
    fn come_back_not_merged() {
        let elems = vec![elem("come", 0.0, 0.5), elem("back", 0.5, 1.0)];
        let merged = merge_compounds(&elems);
        assert_eq!(
            merged.len(),
            2,
            "'come back' (verb + particle) must stay two tokens; got {:?}",
            merged.iter().map(|e| e.value.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(merged[0].value, "come");
        assert_eq!(merged[1].value, "back");
    }

    #[test]
    #[ignore = "pending compound-list audit: verb-particle 'put down' should not merge to 'putdown'"]
    fn put_down_not_merged() {
        let elems = vec![elem("put", 0.0, 0.5), elem("down", 0.5, 1.0)];
        let merged = merge_compounds(&elems);
        assert_eq!(
            merged.len(),
            2,
            "'put down' (verb + particle) must stay two tokens; got {:?}",
            merged.iter().map(|e| e.value.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(merged[0].value, "put");
        assert_eq!(merged[1].value, "down");
    }

    #[test]
    fn test_consecutive_compounds() {
        // "air" + "plane" + "thunder" + "clap" → "airplane" + "thunderclap"
        let elems = vec![
            elem("air", 0.0, 0.3),
            elem("plane", 0.3, 0.6),
            elem("thunder", 0.6, 0.9),
            elem("clap", 0.9, 1.2),
        ];
        let merged = merge_compounds(&elems);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].value, "airplane");
        assert_eq!(merged[1].value, "thunderclap");
    }

    // --- property tests ---

    use proptest::prelude::*;

    /// Words that are unlikely to form compound pairs.
    fn non_compound_word() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("the".to_string()),
            Just("cat".to_string()),
            Just("sat".to_string()),
            Just("is".to_string()),
            Just("very".to_string()),
            Just("happy".to_string()),
            Just("here".to_string()),
            Just("now".to_string()),
            "[a-z]{7,10}".prop_map(|s| s), // long random words unlikely to be compounds
        ]
    }

    fn elem_vec(max_len: usize) -> impl Strategy<Value = Vec<AsrElement>> {
        prop::collection::vec(
            non_compound_word().prop_map(|w| elem(&w, 0.0, 1.0)),
            0..=max_len,
        )
    }

    proptest! {
        /// Output length is always ≤ input length (merges reduce count).
        #[test]
        fn output_never_grows(elems in elem_vec(12)) {
            let merged = merge_compounds(&elems);
            prop_assert!(
                merged.len() <= elems.len(),
                "output {} > input {}", merged.len(), elems.len()
            );
        }

        /// Concatenation of all values is preserved (merging joins, never drops text).
        #[test]
        fn text_content_preserved(elems in elem_vec(10)) {
            let before: String = elems.iter().map(|e| e.value.as_str()).collect();
            let merged = merge_compounds(&elems);
            let after: String = merged.iter().map(|e| e.value.as_str()).collect();
            prop_assert_eq!(before, after, "text content changed during merge");
        }

        /// Merging is idempotent: merge(merge(x)) == merge(x).
        /// After one pass, merged words won't form new compound pairs.
        #[test]
        fn merge_is_idempotent(elems in elem_vec(10)) {
            let once = merge_compounds(&elems);
            let twice = merge_compounds(&once);
            let once_vals: Vec<&str> = once.iter().map(|e| e.value.as_str()).collect();
            let twice_vals: Vec<&str> = twice.iter().map(|e| e.value.as_str()).collect();
            prop_assert_eq!(once_vals, twice_vals, "merge is not idempotent");
        }
    }
}
