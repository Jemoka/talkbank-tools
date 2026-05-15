//! WER benchmark computation: hypothesis + reference → error metrics.
//!
//! Moves the pure-computation logic from Python `benchmark.py` to Rust.
//! Calls [`wer_conform::conform_words`] and [`dp_align::align`] internally.

use crate::dp_align::{self, AlignResult, MatchMode};
use crate::wer_conform;

/// WER computation result.
#[derive(Debug, Clone, PartialEq)]
pub struct WerResult {
    /// Word Error Rate (0.0 to 1.0+).
    pub wer: f64,
    /// Number of reference words.
    pub total: usize,
    /// Number of matching words.
    pub matches: usize,
    /// Human-readable diff string.
    pub diff: String,
}

/// Pre-normalize a single word for WER comparison.
fn normalize_wer_word(word: &str) -> String {
    word.to_lowercase().replace(['(', ')'], "")
}

/// Remove dashes from all words.
fn remove_dashes(words: &[String]) -> Vec<String> {
    words.iter().map(|w| w.replace('-', "")).collect()
}

/// Combine consecutive single-letter tokens (non-Chinese).
fn combine_single_letters(words: &[String], is_chinese: bool) -> Vec<String> {
    if is_chinese {
        return words.to_vec();
    }
    let mut result: Vec<String> = Vec::new();
    let mut sticky = String::new();
    for w in words {
        if w.len() == 1 && w.chars().next().is_some_and(|c| c.is_alphabetic()) {
            sticky.push_str(w);
        } else {
            if !sticky.is_empty() {
                result.push(std::mem::take(&mut sticky));
            }
            result.push(w.clone());
        }
    }
    if !sticky.is_empty() {
        result.push(sticky);
    }
    result
}

/// Decompose words into individual characters (for Chinese WER).
fn decompose_chars(words: &[String]) -> Vec<String> {
    words
        .iter()
        .flat_map(|w| w.chars().map(|c| c.to_string()))
        .collect()
}

/// Compute WER from pre-extracted word lists.
///
/// `langs` is a list of ISO-639-3 codes (e.g. `["eng"]`, `["zho"]`).
/// Returns a [`WerResult`] with WER score, match count, total, and diff.
pub fn compute_wer(hypothesis: &[String], reference: &[String], langs: &[String]) -> WerResult {
    if reference.is_empty() {
        return WerResult {
            wer: 0.0,
            total: 0,
            matches: 0,
            diff: String::new(),
        };
    }

    let is_chinese = langs.iter().any(|l| l == "zho");

    // 1. Dash removal
    let hyp = remove_dashes(hypothesis);
    let ref_ = remove_dashes(reference);

    // 2. Single-letter combining (non-Chinese)
    let hyp = combine_single_letters(&hyp, is_chinese);

    // 3. Chinese character-level decomposition
    let (hyp_final, ref_final) = if is_chinese {
        (decompose_chars(&hyp), decompose_chars(&ref_))
    } else {
        (hyp, ref_)
    };

    // 4. WER conform (contraction expansion, filler normalization, etc.)
    let hyp_conformed = wer_conform::conform_words(&hyp_final);
    let ref_conformed = wer_conform::conform_words(&ref_final);

    // 5. Normalize for comparison
    let hyp_normalized: Vec<String> = hyp_conformed
        .iter()
        .map(|w| normalize_wer_word(w))
        .collect();
    let ref_normalized: Vec<String> = ref_conformed
        .iter()
        .map(|w| normalize_wer_word(w))
        .collect();

    // 6. DP alignment
    let alignment = dp_align::align(&hyp_normalized, &ref_normalized, MatchMode::Exact);

    // 7. Error counting state machine (ported from Python)
    let mut sub = 0usize;
    let mut del = 0usize;
    let mut ins = 0usize;
    let mut prev_error: Option<&str> = None;
    let mut cleaned: Vec<&AlignResult> = Vec::new();
    let mut anticipating_payload = false;

    for entry in &alignment {
        match entry {
            AlignResult::ExtraReference { key, .. } => {
                // Check for "name" heuristic
                if key.contains("name") && !key.starts_with("name") {
                    if let Some(last) = cleaned.last() {
                        if matches!(last, AlignResult::ExtraPayload { .. }) {
                            cleaned.pop();
                        } else {
                            anticipating_payload = true;
                        }
                    } else {
                        anticipating_payload = true;
                    }
                    // Treat as match (skip error counting)
                    cleaned.push(entry);
                    prev_error = None;
                    continue;
                }

                if let Some(prev) = prev_error {
                    if prev != "extra_reference" {
                        // Previous was extra_payload → substitution
                        sub += 1;
                        del -= 1;
                        prev_error = None;
                    } else {
                        ins += 1;
                        prev_error = Some("extra_reference");
                    }
                } else {
                    ins += 1;
                    prev_error = Some("extra_reference");
                }
                cleaned.push(entry);
            }
            AlignResult::ExtraPayload { .. } => {
                if anticipating_payload {
                    anticipating_payload = false;
                    continue;
                }

                if let Some(prev) = prev_error {
                    if prev != "extra_payload" {
                        // Previous was extra_reference → substitution
                        sub += 1;
                        ins -= 1;
                        prev_error = None;
                    } else {
                        del += 1;
                        prev_error = Some("extra_payload");
                    }
                } else {
                    del += 1;
                    prev_error = Some("extra_payload");
                }
                cleaned.push(entry);
            }
            AlignResult::Match { .. } => {
                prev_error = None;
                cleaned.push(entry);
            }
        }
    }

    // 8. Build diff string
    let mut diff_lines: Vec<String> = Vec::new();
    for entry in &cleaned {
        match entry {
            AlignResult::ExtraReference { key, .. } => {
                diff_lines.push(format!("+ {key}"));
            }
            AlignResult::ExtraPayload { key, .. } => {
                diff_lines.push(format!("- {key}"));
            }
            AlignResult::Match { key, .. } => {
                diff_lines.push(format!("  {key}"));
            }
        }
    }

    let total_errors = sub + del + ins;
    let wer = total_errors as f64 / reference.len() as f64;
    let matches = reference.len().saturating_sub(total_errors);

    WerResult {
        wer,
        total: reference.len(),
        matches,
        diff: diff_lines.join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn identical_words() {
        let r = compute_wer(
            &s(&["hello", "world"]),
            &s(&["hello", "world"]),
            &s(&["eng"]),
        );
        assert_eq!(r.wer, 0.0);
        assert_eq!(r.matches, 2);
        assert_eq!(r.total, 2);
    }

    #[test]
    fn empty_reference() {
        let r = compute_wer(&s(&["hello"]), &s(&[]), &s(&["eng"]));
        assert_eq!(r.wer, 0.0);
        assert_eq!(r.total, 0);
    }

    #[test]
    fn empty_hypothesis() {
        let r = compute_wer(&s(&[]), &s(&["hello", "world"]), &s(&["eng"]));
        assert_eq!(r.wer, 1.0);
        assert_eq!(r.matches, 0);
        assert_eq!(r.total, 2);
    }

    #[test]
    fn one_substitution() {
        let r = compute_wer(
            &s(&["hello", "earth"]),
            &s(&["hello", "world"]),
            &s(&["eng"]),
        );
        assert!(r.wer > 0.0);
        assert!(r.diff.contains("+ world") || r.diff.contains("- earth"));
    }

    #[test]
    fn dash_removal() {
        let r = compute_wer(&s(&["ice-cream"]), &s(&["icecream"]), &s(&["eng"]));
        assert_eq!(r.wer, 0.0);
    }

    #[test]
    fn single_letter_combining() {
        // "a" "b" "c" should combine to "abc"
        let r = compute_wer(&s(&["a", "b", "c"]), &s(&["abc"]), &s(&["eng"]));
        assert_eq!(r.wer, 0.0);
    }

    #[test]
    fn chinese_char_decomposition() {
        let r = compute_wer(&s(&["你好"]), &s(&["你好"]), &s(&["zho"]));
        assert_eq!(r.wer, 0.0);
        assert_eq!(r.total, 1); // total is original reference word count
    }

    #[test]
    fn chinese_char_mismatch() {
        let r = compute_wer(&s(&["你好"]), &s(&["你们"]), &s(&["zho"]));
        // 2 chars, 1 matches (你), 1 substitution → wer = 1/1 = 1.0
        assert!(r.wer > 0.0);
    }

    #[test]
    fn case_insensitive_comparison() {
        let r = compute_wer(
            &s(&["Hello", "WORLD"]),
            &s(&["hello", "world"]),
            &s(&["eng"]),
        );
        assert_eq!(r.wer, 0.0);
    }

    #[test]
    fn paren_removal() {
        let r = compute_wer(
            &s(&["(hello)", "world"]),
            &s(&["hello", "world"]),
            &s(&["eng"]),
        );
        assert_eq!(r.wer, 0.0);
    }
}
