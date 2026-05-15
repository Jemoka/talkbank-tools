//! Post-FA bullet repair: fix timing violations without destroying accuracy.
//!
//! Replaces CLAN's FIXBULLETS with three principled strategies:
//!
//! 1. **Same-speaker gap filling**: snap small gaps (500-1000ms) between
//!    consecutive same-speaker utterances.
//! 2. **Boundary averaging**: for small overlaps (≤ threshold), split the
//!    difference rather than clamping one side forward.
//! 3. **Selective timing removal via LIS**: for large violations, find the
//!    longest increasing subsequence of start times and strip timing from
//!    utterances outside the LIS.
//!
//! Design principle: every bullet either points to the correct audio location
//! or doesn't exist. No lying bullets.

use std::collections::HashMap;

use talkbank_model::model::{ChatFile, Line, Utterance};

use super::orchestrate::strip_timing_from_content;

/// Maximum overlap (ms) eligible for boundary averaging (Strategy 1).
/// Beyond this, the overlap is either genuine or a real alignment failure.
const BOUNDARY_AVERAGING_THRESHOLD_MS: u64 = 500;

/// Gap range (ms) eligible for same-speaker gap filling (Strategy 3).
const GAP_FILL_MAX_MS: u64 = 1000;

/// Statistics from a bullet repair pass.
#[derive(Debug, Clone, Default)]
pub struct RepairStats {
    /// Utterances whose bullets were adjusted by boundary averaging.
    pub boundary_averaged: usize,
    /// Utterances whose start was snapped to previous same-speaker end.
    pub gaps_filled: usize,
    /// Utterances whose timing was stripped by LIS removal.
    pub timing_stripped: usize,
    /// Total utterances with bullets before repair.
    pub total_bulleted: usize,
}

impl std::fmt::Display for RepairStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bullet repair: {} boundary-averaged, {} gaps-filled, {} timing-stripped \
             (of {} bulleted utterances)",
            self.boundary_averaged, self.gaps_filled, self.timing_stripped, self.total_bulleted
        )
    }
}

/// A single decision record from the repair pass.
///
/// Each record describes one action taken on one utterance, with enough
/// detail for the `%xalign` tier and for the evaluation harness.
#[derive(Debug, Clone)]
pub struct RepairDecision {
    /// Index into `chat_file.lines`.
    pub line_idx: usize,
    /// Speaker code.
    pub speaker: String,
    /// Typed FA strategy that produced this decision. Narrower than
    /// the crate-wide [`DecisionStrategy`](talkbank_transform::decisions::DecisionStrategy):
    /// this struct is FA-specific, so the strategy is constrained to
    /// [`FaStrategy`] at the point of construction.
    pub strategy: talkbank_transform::decisions::FaStrategy,
    /// Human-readable reason string for `%xalign`.
    pub reason: String,
    /// Whether this decision is low-confidence (should get `%xrev: [?]`).
    pub needs_review: bool,
}

/// Combined result of a repair pass: aggregate stats + per-utterance decisions.
#[derive(Debug, Clone, Default)]
pub struct RepairResult {
    /// Aggregate statistics.
    pub stats: RepairStats,
    /// Per-utterance decision log (one entry per action taken).
    pub decisions: Vec<RepairDecision>,
}

/// Info about an utterance's bullet, extracted for repair analysis.
struct BulletEntry {
    /// Index into `chat_file.lines` (the `Line::Utterance` position).
    line_idx: usize,
    /// Speaker code.
    speaker: String,
    /// Current bullet start_ms.
    start_ms: u64,
    /// Current bullet end_ms.
    end_ms: u64,
}

/// Apply all three repair strategies to a parsed CHAT file.
///
/// Strategies are applied in order:
/// 1. Same-speaker gap filling (conservative, always safe)
/// 2. Boundary averaging (for small overlaps)
/// 3. Selective timing removal via LIS (for large violations)
///
/// Returns statistics describing what was changed.
pub fn repair_bullets(chat_file: &mut ChatFile, dry_run: bool) -> RepairResult {
    let mut stats = RepairStats::default();
    let mut decisions = Vec::new();

    // Collect bullet entries in document order.
    let entries = collect_bullet_entries(chat_file);
    stats.total_bulleted = entries.len();

    if entries.is_empty() {
        return RepairResult { stats, decisions };
    }

    // Strategy 3: Same-speaker gap filling.
    // Must run first because it only narrows gaps, never creates new violations.
    let gap_fills = find_gap_fills(&entries);
    stats.gaps_filled = gap_fills.len();
    for &(line_idx, new_start_ms) in &gap_fills {
        // SAFETY: `line_idx` came from `find_gap_fills(&entries)`, which only
        // produces indices present in `entries`.
        #[allow(clippy::unwrap_used)]
        let entry = entries.iter().find(|e| e.line_idx == line_idx).unwrap();
        let gap = entry.start_ms - new_start_ms;
        decisions.push(RepairDecision {
            line_idx,
            speaker: entry.speaker.clone(),
            strategy: talkbank_transform::decisions::FaStrategy::GapFilled,
            reason: format!(
                "gap_filled gap={}ms same_speaker machine={}_{} snapped_start={}",
                gap, entry.start_ms, entry.end_ms, new_start_ms
            ),
            needs_review: true,
        });
    }

    // Strategy 1: Boundary averaging for small overlaps.
    // Run on document-order adjacent pairs (any speaker).
    let boundary_avgs = find_boundary_averages(&entries);
    stats.boundary_averaged = boundary_avgs.len();
    for &(earlier_line_idx, later_line_idx, _midpoint) in &boundary_avgs {
        // SAFETY: `earlier_line_idx` and `later_line_idx` came from
        // `find_boundary_averages(&entries)`, which only produces indices present
        // in `entries`.
        #[allow(clippy::unwrap_used)]
        let earlier = entries
            .iter()
            .find(|e| e.line_idx == earlier_line_idx)
            .unwrap();
        #[allow(clippy::unwrap_used)]
        let later = entries
            .iter()
            .find(|e| e.line_idx == later_line_idx)
            .unwrap();
        let overlap = earlier.end_ms - later.start_ms;
        decisions.push(RepairDecision {
            line_idx: later_line_idx,
            speaker: later.speaker.clone(),
            strategy: talkbank_transform::decisions::FaStrategy::BoundaryAveraged,
            reason: format!(
                "boundary_averaged overlap={}ms machine={}_{} adjacent={}:{}",
                overlap, later.start_ms, later.end_ms, earlier.speaker, earlier_line_idx
            ),
            needs_review: true,
        });
    }

    // Strategy 2: Selective timing removal via LIS.
    // Find utterances whose start times are non-monotonic and outside the LIS.
    let lis_removals = find_lis_removals(&entries);
    stats.timing_stripped = lis_removals.len();
    for &line_idx in &lis_removals {
        // SAFETY: `line_idx` came from `find_lis_removals(&entries)`, which only
        // produces indices present in `entries`.
        #[allow(clippy::unwrap_used)]
        let entry = entries.iter().find(|e| e.line_idx == line_idx).unwrap();
        decisions.push(RepairDecision {
            line_idx,
            speaker: entry.speaker.clone(),
            strategy: talkbank_transform::decisions::FaStrategy::LisRemoval,
            reason: format!(
                "lis_removal same_speaker_non_monotonic machine={}_{}",
                entry.start_ms, entry.end_ms
            ),
            needs_review: true,
        });
    }

    if dry_run {
        return RepairResult { stats, decisions };
    }

    // Apply changes. Order matters: gap fills and boundary averages modify
    // timestamps in place; LIS removals strip timing entirely.

    // Apply gap fills (modify start_ms of later utterance).
    for &(line_idx, new_start_ms) in &gap_fills {
        if let Some(utt) = get_utterance_mut(chat_file, line_idx)
            && let Some(ref mut bullet) = utt.main.content.bullet
        {
            bullet.timing.start_ms = new_start_ms;
        }
    }

    // Apply boundary averages (modify end_ms of earlier and start_ms of later).
    for &(earlier_line_idx, later_line_idx, midpoint) in &boundary_avgs {
        if let Some(utt) = get_utterance_mut(chat_file, earlier_line_idx)
            && let Some(ref mut bullet) = utt.main.content.bullet
        {
            bullet.timing.end_ms = midpoint;
        }
        if let Some(utt) = get_utterance_mut(chat_file, later_line_idx)
            && let Some(ref mut bullet) = utt.main.content.bullet
        {
            bullet.timing.start_ms = midpoint;
        }
    }

    // Apply LIS removals (strip timing entirely).
    for &line_idx in &lis_removals {
        if let Some(utt) = get_utterance_mut(chat_file, line_idx) {
            strip_utterance_timing(utt);
        }
    }

    RepairResult { stats, decisions }
}

/// Collect bullet entries from all main-tier utterances in document order.
fn collect_bullet_entries(chat_file: &ChatFile) -> Vec<BulletEntry> {
    let mut entries = Vec::new();

    for (line_idx, line) in chat_file.lines.iter().enumerate() {
        let Line::Utterance(utt) = line else {
            continue;
        };
        let Some(ref bullet) = utt.main.content.bullet else {
            continue;
        };
        entries.push(BulletEntry {
            line_idx,
            speaker: utt.main.speaker.to_string(),
            start_ms: bullet.timing.start_ms,
            end_ms: bullet.timing.end_ms,
        });
    }

    entries
}

/// Strategy 3: Find same-speaker gaps eligible for filling.
///
/// Returns `(line_idx, new_start_ms)` pairs — the later utterance's start
/// should be snapped to the previous same-speaker utterance's end.
fn find_gap_fills(entries: &[BulletEntry]) -> Vec<(usize, u64)> {
    let mut fills = Vec::new();
    // Track per-speaker last end time.
    let mut speaker_last_end: HashMap<&str, u64> = HashMap::new();

    for entry in entries {
        if let Some(&prev_end) = speaker_last_end.get(entry.speaker.as_str())
            && entry.start_ms > prev_end
        {
            let gap = entry.start_ms - prev_end;
            if gap <= GAP_FILL_MAX_MS {
                fills.push((entry.line_idx, prev_end));
            }
        }
        speaker_last_end.insert(&entry.speaker, entry.end_ms);
    }

    fills
}

/// Strategy 1: Find adjacent pairs with small overlap eligible for boundary
/// averaging.
///
/// Returns `(earlier_line_idx, later_line_idx, midpoint)` triples.
fn find_boundary_averages(entries: &[BulletEntry]) -> Vec<(usize, usize, u64)> {
    let mut averages = Vec::new();

    for pair in entries.windows(2) {
        let earlier = &pair[0];
        let later = &pair[1];

        // Check for overlap: later starts before earlier ends.
        if later.start_ms < earlier.end_ms {
            let overlap = earlier.end_ms - later.start_ms;
            if overlap <= BOUNDARY_AVERAGING_THRESHOLD_MS {
                // Split the difference.
                let midpoint = later.start_ms + overlap / 2;
                averages.push((earlier.line_idx, later.line_idx, midpoint));
            }
        }
    }

    averages
}

/// Strategy 2: Find utterances to strip timing from using per-speaker LIS.
///
/// For each speaker, computes the Longest Increasing Subsequence of start
/// times. Utterances NOT in their speaker's LIS have same-speaker
/// non-monotonic timing. Their timing is stripped rather than mangled.
///
/// Cross-speaker non-monotonicity is intentionally left alone — it
/// represents normal conversational overlap, not a data error.
///
/// Returns `line_idx` values of utterances to strip.
fn find_lis_removals(entries: &[BulletEntry]) -> Vec<usize> {
    // Group entry indices by speaker.
    let mut speaker_entries: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        speaker_entries
            .entry(entry.speaker.as_str())
            .or_default()
            .push(i);
    }

    let mut removals = Vec::new();

    for indices in speaker_entries.values() {
        if indices.len() <= 1 {
            continue;
        }

        // Extract start times for this speaker's utterances (in document order).
        let starts: Vec<u64> = indices.iter().map(|&i| entries[i].start_ms).collect();
        let lis = longest_increasing_subsequence(&starts);

        // Build set of positions (within this speaker's list) that are in the LIS.
        let mut in_lis = vec![false; indices.len()];
        for &pos in &lis {
            in_lis[pos] = true;
        }

        // Entries NOT in this speaker's LIS are stripped. They are the
        // minimal set whose removal makes the speaker's timeline monotonic.
        for (pos, &entry_idx) in indices.iter().enumerate() {
            if !in_lis[pos] {
                removals.push(entries[entry_idx].line_idx);
            }
        }
    }

    removals
}

/// Compute the Longest Increasing Subsequence (non-strictly increasing).
///
/// Returns the indices of elements in the LIS.
/// Uses the patience sorting / binary search algorithm: O(n log n).
fn longest_increasing_subsequence(values: &[u64]) -> Vec<usize> {
    if values.is_empty() {
        return Vec::new();
    }

    let n = values.len();
    // tails[i] = index of smallest tail element for IS of length i+1.
    let mut tails: Vec<usize> = Vec::new();
    // prev[i] = index of previous element in LIS ending at i.
    let mut prev: Vec<Option<usize>> = vec![None; n];

    for i in 0..n {
        // Binary search: find first tail >= values[i].
        let pos = tails.partition_point(|&t| values[t] <= values[i]);

        if pos == tails.len() {
            tails.push(i);
        } else {
            tails[pos] = i;
        }

        if pos > 0 {
            prev[i] = Some(tails[pos - 1]);
        }
    }

    // Reconstruct LIS from prev pointers.
    let mut result = Vec::with_capacity(tails.len());
    // SAFETY: `tails` is non-empty because the loop above always pushes at least
    // one element (input `starts` is non-empty, checked by caller).
    #[allow(clippy::unwrap_used)]
    let mut idx = *tails.last().unwrap();
    result.push(idx);
    while let Some(p) = prev[idx] {
        result.push(p);
        idx = p;
    }
    result.reverse();
    result
}

/// Get a mutable reference to an utterance by its line index.
fn get_utterance_mut(chat_file: &mut ChatFile, line_idx: usize) -> Option<&mut Utterance> {
    if let Some(Line::Utterance(utt)) = chat_file.lines.get_mut(line_idx) {
        Some(utt)
    } else {
        None
    }
}

/// Strip all timing from an utterance: bullet, inline word bullets, %wor tier.
fn strip_utterance_timing(utt: &mut Utterance) {
    // Remove utterance-level bullet.
    utt.main.content.bullet = None;

    // Remove inline word bullets (reuses the existing orchestrate helper).
    strip_timing_from_content(&mut utt.main.content.content.0);

    // Remove %wor tier.
    utt.dependent_tiers
        .retain(|tier| !matches!(tier, talkbank_model::model::DependentTier::Wor { .. }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test: parse a real trimmed CHAT file with E704 (same-speaker
    /// overlap) and E701 (cross-speaker non-monotonicity), run bullet repair,
    /// verify that boundary averaging and LIS removal produce correct results.
    #[test]
    fn test_repair_on_real_bre_fixture() {
        let chat_text = std::fs::read_to_string("../../resources/fixtures/bullet_repair_e704.cha")
            .expect("fixture file missing — run trim_chat_audio.py to regenerate");

        let parser = talkbank_transform::parse::TreeSitterParser::new().expect("parser init");
        let (mut chat_file, _errors) =
            talkbank_transform::parse::parse_lenient(&parser, &chat_text);

        // Dry-run first: verify we detect issues without modifying the file.
        let dry_result = repair_bullets(&mut chat_file, true);
        assert!(
            dry_result.stats.total_bulleted > 0,
            "fixture should have bulleted utterances"
        );
        // The fixture has cross-speaker overlaps that trigger boundary averaging,
        // and at least one same-speaker overlap. The exact counts may shift if
        // the fixture is re-trimmed, so just verify we found something to repair.
        assert!(
            !dry_result.decisions.is_empty(),
            "expected at least one repair decision"
        );

        // Now apply for real.
        let result = repair_bullets(&mut chat_file, false);
        assert_eq!(result.stats.total_bulleted, dry_result.stats.total_bulleted);
        // Decisions should include per-utterance records.
        assert!(!result.decisions.is_empty());
        // Every decision should have a non-empty reason.
        for d in &result.decisions {
            assert!(!d.reason.is_empty(), "decision has empty reason");
            assert!(d.needs_review, "repair decisions should need review");
        }

        // After repair: same-speaker bullets should be monotonically increasing.
        // Cross-speaker non-monotonicity is expected (normal conversational overlap).
        let entries_after = collect_bullet_entries(&chat_file);
        let mut speaker_last_start: HashMap<&str, u64> = HashMap::new();
        for entry in &entries_after {
            if let Some(&prev) = speaker_last_start.get(entry.speaker.as_str()) {
                assert!(
                    entry.start_ms >= prev,
                    "same-speaker non-monotonic after repair: {} starts at {}ms but \
                     previously started at {}ms",
                    entry.speaker,
                    entry.start_ms,
                    prev,
                );
            }
            speaker_last_start.insert(&entry.speaker, entry.start_ms);
        }
    }

    /// Verify that repair on a clean file (no violations) is a no-op.
    #[test]
    fn test_repair_noop_on_clean_file() {
        // Construct a minimal CHAT with two well-ordered bullets.
        // Gap > 1000ms so gap-filling doesn't trigger.
        let chat_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|2;0.0||||Target_Child|||
*CHI:\thello . \u{0015}1000_2000\u{0015}
*CHI:\tworld . \u{0015}4000_5000\u{0015}
@End
";
        let parser = talkbank_transform::parse::TreeSitterParser::new().expect("parser init");
        let (mut chat_file, _errors) = talkbank_transform::parse::parse_lenient(&parser, chat_text);

        let result = repair_bullets(&mut chat_file, false);
        assert_eq!(result.stats.boundary_averaged, 0);
        assert_eq!(result.stats.gaps_filled, 0);
        assert_eq!(result.stats.timing_stripped, 0);
        assert_eq!(result.stats.total_bulleted, 2);
        assert!(result.decisions.is_empty());
    }

    #[test]
    fn test_lis_simple() {
        let values = vec![3, 1, 2, 4, 3, 5];
        let lis = longest_increasing_subsequence(&values);
        // LIS: 1, 2, 3, 5 (indices 1, 2, 4, 5) or 1, 2, 4, 5 (indices 1, 2, 3, 5)
        assert_eq!(lis.len(), 4);
        // Verify it's actually increasing.
        for pair in lis.windows(2) {
            assert!(values[pair[0]] <= values[pair[1]]);
        }
    }

    #[test]
    fn test_lis_already_sorted() {
        let values = vec![1, 2, 3, 4, 5];
        let lis = longest_increasing_subsequence(&values);
        assert_eq!(lis.len(), 5);
    }

    #[test]
    fn test_lis_reverse_sorted() {
        let values = vec![5, 4, 3, 2, 1];
        let lis = longest_increasing_subsequence(&values);
        assert_eq!(lis.len(), 1);
    }

    #[test]
    fn test_lis_empty() {
        let values: Vec<u64> = vec![];
        let lis = longest_increasing_subsequence(&values);
        assert!(lis.is_empty());
    }
}

impl From<&RepairDecision> for talkbank_transform::decisions::DecisionRecord {
    fn from(d: &RepairDecision) -> Self {
        Self {
            line_idx: d.line_idx,
            speaker: d.speaker.clone(),
            strategy: talkbank_transform::decisions::DecisionStrategy::Fa(d.strategy),
            reason: d.reason.clone(),
            needs_review: d.needs_review,
        }
    }
}
