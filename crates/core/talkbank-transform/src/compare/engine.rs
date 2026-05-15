use std::cmp::Reverse;
use std::collections::HashMap;

use talkbank_model::WriteChat;
use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::model::{ChatFile, DependentTier, Line};

use crate::dp_align::{self, AlignResult, MatchMode};
use crate::extract::{self, ExtractedUtterance};
use crate::wer_conform;

use super::metrics::MetricAccumulator;
use super::model::{
    CompareStatus, CompareToken, ComparisonBundle, GoldWordMatch, UtteranceComparison,
};

#[derive(Debug, Clone)]
struct FlattenedWordInfo {
    utterance_index: usize,
    word_position: usize,
    compare_position: usize,
    pos: Option<String>,
}

/// Punctuation and fillers to exclude from comparison (matching BA2 behavior).
///
/// Terminators are recognized via the typed `Terminator` enum so the set
/// stays in lockstep with the grammar. Separators (`,`, `‡`, `„`) are
/// additionally excluded because BA2's compare skipped them too.
pub(in crate::compare) fn is_punct_or_filler(word: &str) -> bool {
    static FILLERS: &[&str] = &["um", "uhm", "em", "mhm", "uhhm", "eh", "uh", "hm"];

    let w = word.trim();
    talkbank_model::model::content::Terminator::is_chat_terminator(w)
        || matches!(w, "," | "‡" | "„")
        || FILLERS.contains(&w.to_lowercase().as_str())
}

fn is_punct_pos(pos: Option<&str>) -> bool {
    pos.is_some_and(|value| value.eq_ignore_ascii_case("PUNCT"))
}

/// Apply conform_words per word, returning expanded tokens and an index
/// mapping back to the original word list.
///
/// `mapping[j]` = index into the original `words` list that `conformed[j]`
/// originated from.
pub(in crate::compare) fn conform_with_mapping(words: &[String]) -> (Vec<String>, Vec<usize>) {
    let mut conformed = Vec::new();
    let mut mapping = Vec::new();
    for (idx, word) in words.iter().enumerate() {
        let expanded = wer_conform::conform_words(std::slice::from_ref(word));
        for token in expanded {
            conformed.push(token);
            mapping.push(idx);
        }
    }
    (conformed, mapping)
}

/// Find the best local main-token window for one gold utterance.
///
/// This follows the BA2 compare engine's rough-pass strategy:
/// - compare contiguous windows using bag-of-words overlap
/// - only consider windows near the gold utterance length
/// - prefer better overlap, then more aligner matches (so order is respected
///   and cross-utterance fragments that happen to be dense don't beat
///   in-utterance matches), then fewer wasted tokens, then the latest window
///
/// Tiebreaker order matches BA2-master 86230ef (2026-04-17,
/// "fix part 2 of compare").
pub(in crate::compare) fn find_best_segment(
    gold_tokens: &[String],
    main_tokens: &[String],
) -> (usize, usize) {
    if gold_tokens.is_empty() || main_tokens.is_empty() {
        return (0, 0);
    }

    let gold_len = gold_tokens.len();
    let main_len = main_tokens.len();
    let min_window = std::cmp::max(1, gold_len.saturating_sub(2));
    let max_window = std::cmp::min(main_len, gold_len + 2);
    let gold_counts = token_counts(gold_tokens);

    // Comparing `overlap` is equivalent to BA2's float `score = overlap /
    // gold_len` because `gold_len` is constant within the call — and lets us
    // collapse all four tiebreaker axes into one tuple comparison. `Reverse`
    // flips the lower-is-better waste axis.
    let mut best_window = (0usize, std::cmp::min(main_len, gold_len));
    let mut best_key: Option<(usize, usize, Reverse<usize>, usize)> = None;

    for span in min_window..=max_window {
        for start in 0..=(main_len - span) {
            let end = start + span;
            let overlap = token_overlap(&main_tokens[start..end], &gold_counts);
            let waste = span.saturating_sub(overlap);
            let align_matches = count_alignment_matches(&main_tokens[start..end], gold_tokens);

            let key = (overlap, align_matches, Reverse(waste), end);
            if best_key.is_none_or(|best| key > best) {
                best_window = (start, end);
                best_key = Some(key);
            }
        }
    }

    // No tokens overlap at all → return an empty window so the caller doesn't
    // consume main tokens that belong to a later gold utterance.
    if best_key.is_none_or(|(overlap, ..)| overlap == 0) {
        return (0, 0);
    }

    best_window
}

fn count_alignment_matches(window: &[String], gold_tokens: &[String]) -> usize {
    dp_align::align(window, gold_tokens, MatchMode::CaseInsensitive)
        .into_iter()
        .filter(|item| matches!(item, AlignResult::Match { .. }))
        .count()
}

fn best_rotation(window_tokens: &[String], gold_tokens: &[String]) -> usize {
    if window_tokens.len() <= 1 {
        return 0;
    }

    let mut best_rotation = 0usize;
    let mut best_matches = 0usize;
    for rotation in 0..window_tokens.len() {
        let rotated: Vec<String> = window_tokens[rotation..]
            .iter()
            .chain(window_tokens[..rotation].iter())
            .cloned()
            .collect();
        let matches = count_alignment_matches(&rotated, gold_tokens);
        if matches > best_matches {
            best_matches = matches;
            best_rotation = rotation;
        }
    }
    best_rotation
}

fn token_counts(tokens: &[String]) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for token in tokens {
        *counts.entry(token.as_str()).or_insert(0) += 1;
    }
    counts
}

fn token_overlap(window: &[String], gold_counts: &HashMap<&str, usize>) -> usize {
    let mut window_counts = HashMap::new();
    for token in window {
        *window_counts.entry(token.as_str()).or_insert(0) += 1;
    }

    window_counts
        .iter()
        .map(|(token, count)| std::cmp::min(*count, *gold_counts.get(token).unwrap_or(&0)))
        .sum()
}

/// Compare a main transcript against a gold-standard reference.
///
/// Both inputs are parsed CHAT files. Words are extracted from the Mor
/// domain (excluding punctuation and fillers), normalized via
/// `conform_words`, then aligned with the Hirschberg DP aligner.
///
/// Returns per-utterance comparison annotations and aggregate metrics.
pub fn compare(main_file: &ChatFile, gold_file: &ChatFile) -> ComparisonBundle {
    // 1. Extract words from both files
    let main_utts = extract::extract_words(main_file, TierDomain::Mor);
    let gold_utts = extract::extract_words(gold_file, TierDomain::Mor);

    // 2. Flatten words, filtering punctuation and fillers
    let (main_words, main_info) = flatten_words(main_file, &main_utts);
    let (gold_words, gold_info) = flatten_words(gold_file, &gold_utts);

    // 3. Apply conform with index mapping
    let (conformed_main, main_map) = conform_with_mapping(&main_words);
    let (conformed_gold, gold_map) = conform_with_mapping(&gold_words);

    // 4. Partition conformed gold tokens by utterance so compare can work
    // sequentially, one gold utterance at a time.
    let mut gold_utt_tokens: Vec<Vec<String>> = vec![Vec::new(); gold_utts.len()];
    let mut gold_utt_maps: Vec<Vec<usize>> = vec![Vec::new(); gold_utts.len()];
    for (conformed_idx, token) in conformed_gold.iter().enumerate() {
        let orig_gold_idx = gold_map[conformed_idx];
        let gold_utt_idx = gold_info[orig_gold_idx].utterance_index;
        gold_utt_tokens[gold_utt_idx].push(token.clone());
        gold_utt_maps[gold_utt_idx].push(orig_gold_idx);
    }

    // 5. Align each gold utterance against the best local main window.
    //
    // Matching batchalign2-master matters more here than "fixing" its
    // semantics: compare only aligns inside the selected window and does not
    // surface skipped main tokens that fall outside that window as insertions.
    let mut main_positioned: Vec<Vec<(f64, CompareToken)>> = vec![Vec::new(); main_utts.len()];
    let mut gold_positioned: Vec<Vec<(f64, CompareToken)>> = vec![Vec::new(); gold_utts.len()];
    let mut gold_word_matches = Vec::new();
    let mut metrics = MetricAccumulator::default();
    let mut search_start = 0usize;
    let mut last_global_main_anchor: Option<(usize, usize)> = None;

    for gold_utt_idx in 0..gold_utts.len() {
        let g_tokens = &gold_utt_tokens[gold_utt_idx];
        let g_maps = &gold_utt_maps[gold_utt_idx];
        if g_tokens.is_empty() {
            continue;
        }

        let remaining_main = &conformed_main[search_start..];
        let (win_start, win_end) = find_best_segment(g_tokens, remaining_main);
        let abs_start = search_start + win_start;
        let abs_end = search_start + win_end;

        let window_main = &conformed_main[abs_start..abs_end];
        let window_len = window_main.len();
        let rotation = best_rotation(window_main, g_tokens);
        let rotated_window: Vec<String> = if rotation == 0 {
            window_main.to_vec()
        } else {
            window_main[rotation..]
                .iter()
                .chain(window_main[..rotation].iter())
                .cloned()
                .collect()
        };
        let default_main_anchor = (window_len > 0)
            .then(|| main_map[abs_start + rotation % window_len])
            .map(|orig_idx| {
                let info = &main_info[orig_idx];
                (info.utterance_index, info.word_position)
            });
        let utt_alignment = dp_align::align(&rotated_window, g_tokens, MatchMode::CaseInsensitive);
        let mut local_main_cursor = 0usize;
        let mut local_gold_cursor = 0usize;
        let mut last_gold_word_position: Option<usize> = None;
        let mut local_main_anchor: Option<(usize, usize)> = None;

        for item in utt_alignment {
            match item {
                AlignResult::Match { key, .. } => {
                    let global_main_idx = abs_start + ((local_main_cursor + rotation) % window_len);
                    let orig_main_idx = main_map[global_main_idx];
                    let main_word = &main_info[orig_main_idx];
                    let orig_gold_idx = g_maps[local_gold_cursor];
                    let gold_word = &gold_info[orig_gold_idx];

                    let token = CompareToken {
                        text: key,
                        pos: main_word.pos.clone(),
                        status: CompareStatus::Match,
                    };
                    metrics.record(&token);
                    main_positioned[main_word.utterance_index]
                        .push((main_word.word_position as f64, token.clone()));
                    gold_positioned[gold_utt_idx].push((gold_word.word_position as f64, token));

                    let structural_match = GoldWordMatch {
                        gold_utterance_index: gold_utt_idx,
                        gold_word_position: gold_word.compare_position,
                        main_utterance_index: main_word.utterance_index,
                        main_word_position: main_word.compare_position,
                    };
                    if gold_word_matches.last() != Some(&structural_match) {
                        gold_word_matches.push(structural_match);
                    }

                    local_main_anchor = Some((main_word.utterance_index, main_word.word_position));
                    last_global_main_anchor = local_main_anchor;
                    last_gold_word_position = Some(gold_word.word_position);
                    local_main_cursor += 1;
                    local_gold_cursor += 1;
                }
                AlignResult::ExtraPayload { key, .. } => {
                    let global_main_idx = abs_start + ((local_main_cursor + rotation) % window_len);
                    let orig_main_idx = main_map[global_main_idx];
                    let main_word = &main_info[orig_main_idx];

                    let token = CompareToken {
                        text: key,
                        pos: main_word.pos.clone(),
                        status: CompareStatus::ExtraMain,
                    };
                    metrics.record(&token);
                    main_positioned[main_word.utterance_index]
                        .push((main_word.word_position as f64, token.clone()));
                    gold_positioned[gold_utt_idx].push((
                        last_gold_word_position.map_or(-0.5, |pos| pos as f64 + 0.5),
                        token,
                    ));

                    local_main_anchor = Some((main_word.utterance_index, main_word.word_position));
                    last_global_main_anchor = local_main_anchor;
                    local_main_cursor += 1;
                }
                AlignResult::ExtraReference { key, .. } => {
                    let orig_gold_idx = g_maps[local_gold_cursor];
                    let gold_word = &gold_info[orig_gold_idx];

                    let token = CompareToken {
                        text: key,
                        pos: gold_word.pos.clone(),
                        status: CompareStatus::ExtraGold,
                    };
                    metrics.record(&token);
                    gold_positioned[gold_utt_idx]
                        .push((gold_word.word_position as f64, token.clone()));

                    if let Some((target_utt, target_word_pos)) = local_main_anchor
                        .or(default_main_anchor)
                        .or(last_global_main_anchor)
                        && let Some(target_tokens) = main_positioned.get_mut(target_utt)
                    {
                        target_tokens.push((target_word_pos as f64 + 0.5, token));
                    }

                    last_gold_word_position = Some(gold_word.word_position);
                    local_gold_cursor += 1;
                }
            }
        }

        search_start = abs_end;
    }

    // 6. Append the gold utterance terminator as a PUNCT token so gold-projected
    // `%xsrep` / `%xsmor` lines match batchalign2-master output shape.
    for (gold_utt_idx, terminator) in collect_utterance_terminators(gold_file)
        .into_iter()
        .enumerate()
    {
        let Some(terminator) = terminator else {
            continue;
        };
        gold_positioned[gold_utt_idx].push((
            gold_utt_tokens[gold_utt_idx].len() as f64,
            CompareToken {
                text: terminator,
                pos: Some("PUNCT".to_string()),
                status: CompareStatus::Match,
            },
        ));
    }

    // 7. Stabilize per-utterance token order.
    for tokens in &mut main_positioned {
        tokens.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }
    for tokens in &mut gold_positioned {
        tokens.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    let main_utterances = build_utterance_comparisons(&main_utts, main_positioned);
    let gold_utterances = build_utterance_comparisons(&gold_utts, gold_positioned);

    ComparisonBundle {
        main_utterances,
        gold_utterances,
        gold_word_matches,
        metrics: metrics.finish(),
    }
}

fn build_utterance_comparisons(
    utterances: &[ExtractedUtterance],
    positioned: Vec<Vec<(f64, CompareToken)>>,
) -> Vec<UtteranceComparison> {
    utterances
        .iter()
        .enumerate()
        .map(|(utt_idx, utt)| UtteranceComparison {
            utterance_index: utt_idx,
            speaker: utt.speaker.as_str().to_string(),
            tokens: positioned[utt_idx]
                .iter()
                .map(|(_, token)| token.clone())
                .collect(),
        })
        .collect()
}

/// Flatten extracted utterances into a word list and info vector.
///
/// Returns:
/// - `words`: cleaned text for each non-punct/non-filler word
/// - `info`: word position and `%mor`-derived metadata for each word
fn flatten_words(
    chat_file: &ChatFile,
    utts: &[ExtractedUtterance],
) -> (Vec<String>, Vec<FlattenedWordInfo>) {
    let mut words = Vec::new();
    let mut info = Vec::new();
    let mor_positions = collect_mor_pos_labels(chat_file);

    for utt in utts {
        let mut compare_position = 0usize;
        for extracted in &utt.words {
            let text = extracted.text.as_str();
            let pos = mor_positions
                .get(utt.utterance_index.0)
                .and_then(|positions| positions.get(extracted.utterance_word_index.0))
                .cloned()
                .flatten();
            if is_punct_or_filler(text) || is_punct_pos(pos.as_deref()) {
                continue;
            }
            words.push(text.to_string());
            info.push(FlattenedWordInfo {
                utterance_index: utt.utterance_index.0,
                word_position: extracted.utterance_word_index.0,
                compare_position,
                pos,
            });
            compare_position += 1;
        }
    }

    (words, info)
}

fn collect_mor_pos_labels(chat_file: &ChatFile) -> Vec<Vec<Option<String>>> {
    let mut utterance_positions = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            let mor_positions = utt
                .dependent_tiers
                .iter()
                .find_map(|tier| match tier {
                    DependentTier::Mor(mor) => Some(
                        mor.items()
                            .iter()
                            .map(|item| Some(item.main.pos.to_string().to_uppercase()))
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            utterance_positions.push(mor_positions);
        }
    }
    utterance_positions
}

pub(in crate::compare) fn collect_utterance_terminators(
    chat_file: &ChatFile,
) -> Vec<Option<String>> {
    let mut terminators = Vec::new();
    for line in &chat_file.lines {
        if let Line::Utterance(utt) = line {
            terminators.push(
                utt.main
                    .content
                    .terminator
                    .as_ref()
                    .map(|term| term.to_chat_string()),
            );
        }
    }
    terminators
}
