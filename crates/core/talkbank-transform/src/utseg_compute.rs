//! Utterance segmentation assignment algorithm.
//!
//! Port of the Python `compute_assignments()` function from `utseg.py`.
//! Takes constituency tree bracket strings and produces word-to-group assignments.

use std::collections::HashSet;

use crate::constituency::{parse_bracket_notation, parse_tree_indices};

/// 0-based phrase group identifier assigned during utterance segmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PhraseId(usize);

/// Compute word-to-utterance-group assignments from constituency tree strings.
///
/// Takes one or more tree bracket notation strings (one per Stanza sentence)
/// and the total number of words. Returns a `Vec<usize>` parallel to the words,
/// where each element is a 0-based group ID.
///
/// Algorithm:
/// 1. Parse tree, extract S-level phrase ranges via coordination detection
/// 2. De-duplicate ranges (remove subsets of existing ranges)
/// 3. Assign words to phrase groups
/// 4. Fill unassigned words (forward then backward)
/// 5. Merge small groups (< 3 words) into neighbors
pub fn compute_assignments(trees: &[String], num_words: usize) -> Vec<usize> {
    if num_words <= 1 {
        return vec![0; num_words];
    }

    // Parse all trees and extract phrase ranges
    let mut phrase_ranges: Vec<Vec<usize>> = Vec::new();
    for tree_str in trees {
        match parse_bracket_notation(tree_str) {
            Ok(tree) => {
                phrase_ranges.extend(parse_tree_indices(&tree, 0));
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse constituency tree, skipping");
            }
        }
    }

    // Sort by length (ascending)
    phrase_ranges.sort_by_key(|r| r.len());

    // De-duplicate: process from longest to shortest, removing subsets
    let mut unique_ranges: Vec<Vec<usize>> = Vec::new();
    let full_range: Vec<usize> = (0..num_words).collect();

    for rng in phrase_ranges
        .iter()
        .rev()
        .chain(std::iter::once(&full_range))
    {
        let mut rng_set: HashSet<usize> = rng.iter().copied().collect();

        for existing in &unique_ranges {
            let existing_set: HashSet<usize> = existing.iter().copied().collect();
            rng_set = &rng_set - &existing_set;
        }

        if !rng_set.is_empty()
            && !unique_ranges
                .iter()
                .any(|x| rng_set.is_subset(&x.iter().copied().collect()))
        {
            let mut sorted: Vec<usize> = rng_set.into_iter().collect();
            sorted.sort_unstable();
            unique_ranges.push(sorted);
        }
    }
    unique_ranges.reverse();

    // Filter out single-element ranges
    unique_ranges.retain(|r| r.len() > 1);

    if unique_ranges.is_empty() {
        return vec![0; num_words];
    }

    // Assign words to phrase groups
    let mut word_to_phrase: Vec<Option<PhraseId>> = vec![None; num_words];
    for (phrase_id, indices) in unique_ranges.iter().enumerate() {
        for &idx in indices {
            if idx < num_words {
                word_to_phrase[idx] = Some(PhraseId(phrase_id));
            }
        }
    }

    // Fill unassigned words: forward fill
    for i in 0..num_words {
        if word_to_phrase[i].is_some() {
            continue;
        }
        // Look forward
        let mut found = false;
        for j in (i + 1)..num_words {
            if word_to_phrase[j].is_some() {
                word_to_phrase[i] = word_to_phrase[j];
                found = true;
                break;
            }
        }
        if !found {
            // Look backward
            for j in (0..i).rev() {
                if word_to_phrase[j].is_some() {
                    word_to_phrase[i] = word_to_phrase[j];
                    break;
                }
            }
        }
    }

    if word_to_phrase.iter().any(|x| x.is_none()) {
        return vec![0; num_words];
    }

    // Group consecutive words by phrase ID
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = vec![0];
    for i in 1..num_words {
        if word_to_phrase[i] == word_to_phrase[i - 1] {
            current_group.push(i);
        } else {
            groups.push(current_group);
            current_group = vec![i];
        }
    }
    groups.push(current_group);

    // Merge small groups (< 3 words) into neighbors
    let mut merged: Vec<Vec<usize>> = Vec::new();
    let mut pending: Vec<usize> = Vec::new();

    for grp in groups {
        if grp.len() < 3 {
            pending.extend(grp);
        } else {
            let mut combined = pending;
            combined.extend(grp);
            merged.push(combined);
            pending = Vec::new();
        }
    }
    if !pending.is_empty() {
        if let Some(last) = merged.last_mut() {
            last.extend(pending);
        } else {
            merged.push(pending);
        }
    }

    if merged.is_empty() {
        return vec![0; num_words];
    }

    // Build final assignments
    let mut assignments = vec![0usize; num_words];
    for (group_id, group_indices) in merged.iter().enumerate() {
        for &idx in group_indices {
            assignments[idx] = group_id;
        }
    }

    assignments
}

/// Compute assignments from a single tree, used by the utseg orchestrator.
///
/// Convenience wrapper that takes a single tree string.
pub fn compute_assignments_single(tree_str: &str, num_words: usize) -> Vec<usize> {
    compute_assignments(&[tree_str.to_string()], num_words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_word() {
        assert_eq!(compute_assignments(&[], 1), vec![0]);
        assert_eq!(compute_assignments(&[], 0), Vec::<usize>::new());
    }

    #[test]
    fn test_no_coordination() {
        // Simple tree with no coordination — all words in group 0
        let tree = "(S (NP (DT the) (NN cat)) (VP (VBD sat)))".to_string();
        assert_eq!(compute_assignments(&[tree], 3), vec![0, 0, 0]);
    }

    #[test]
    fn test_coordination_split() {
        // "I eat and he runs" — two coordinated S clauses
        let tree =
            "(ROOT (S (S (NP (PRP I)) (VP (VBP eat))) (CC and) (S (NP (PRP he)) (VP (VBZ runs)))))"
                .to_string();
        let assignments = compute_assignments(&[tree], 5);
        // Should split into two groups: [I eat and] and [he runs]
        // The "and" is unassigned and gets filled forward or merged
        assert!(assignments[0] == assignments[1]); // I, eat in same group
        assert!(assignments[3] == assignments[4]); // he, runs in same group
    }

    #[test]
    fn test_all_same_group_after_merge() {
        // Very short utterance — merging should collapse everything
        let tree = "(ROOT (S (NP (PRP I)) (VP (VBP go))))".to_string();
        let assignments = compute_assignments(&[tree], 2);
        assert_eq!(assignments, vec![0, 0]);
    }

    #[test]
    fn test_invalid_tree_falls_back() {
        let assignments = compute_assignments(&["not a tree".to_string()], 3);
        assert_eq!(assignments, vec![0, 0, 0]);
    }
}
