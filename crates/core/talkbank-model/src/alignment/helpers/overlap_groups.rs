//! Cross-utterance overlap group analysis.
//!
//! Matches top overlap regions (⌈...⌉) with bottom overlap regions (⌊...⌋)
//! across utterances to form [`OverlapGroup`]s. Supports 1:N matching —
//! one top region from speaker A can be matched by bottom regions from
//! speakers B, C, etc. (multiple respondents to the same turn).
//!
//! The matching algorithm:
//! 1. Collect all top and bottom regions across all utterances
//! 2. For each bottom region, find the nearest preceding top region from a
//!    different speaker with the same index
//! 3. Group bottoms by their matched top
//! 4. Tops with no matching bottoms are orphaned
//! 5. Bottoms with no matching tops are orphaned
//!
//! ## Example
//!
//! ```text
//! *A: I was ⌈ saying that ⌉ .     → top region, unindexed
//! *B:       ⌊ yeah ⌋ .             → bottom, matches A's top
//! *C:       ⌊ right ⌋ .            → bottom, also matches A's top (1:N)
//! *A: and ⌈2 then ⌉2 .            → top region, index 2
//! *B:     ⌊2 mhm ⌋2 .             → bottom, matches A's index-2 top
//! ```
//!
//! Produces two groups: one with A's unindexed top + B and C's bottoms,
//! one with A's index-2 top + B's index-2 bottom.

use crate::model::Line;

use super::overlap::{OverlapMarkerInfo, OverlapRegion, OverlapRegionKind, extract_overlap_info};

/// An overlap region anchored to a specific utterance.
#[derive(Debug, Clone)]
pub struct OverlapAnchor {
    /// Index of the utterance in the file's utterance list (0-based).
    pub utterance_index: usize,
    /// Speaker code of the utterance.
    pub speaker: String,
    /// The overlap region within this utterance.
    pub region: OverlapRegion,
    /// Utterance-level timing bullet, if present.
    pub bullet: Option<(u64, u64)>,
}

/// A matched overlap group: one top region paired with 1..N bottom regions.
///
/// Represents the semantic relationship "speaker A was talking, and speakers
/// B (and possibly C, D, ...) started overlapping during A's turn."
#[derive(Debug, Clone)]
pub struct OverlapGroup {
    /// The top region (⌈...⌉) — the speaker who was talking first.
    pub top: OverlapAnchor,
    /// Bottom regions (⌊...⌋) from different speakers who overlapped.
    /// May be empty if the top has no matching bottoms (orphaned top that
    /// was grouped because it shares an index with other tops).
    pub bottoms: Vec<OverlapAnchor>,
}

/// Complete cross-utterance overlap analysis for a file.
#[derive(Debug, Clone)]
pub struct FileOverlapAnalysis {
    /// Matched overlap groups (1 top : N bottoms).
    pub groups: Vec<OverlapGroup>,
    /// Top regions with no matching bottom from any other speaker.
    pub orphaned_tops: Vec<OverlapAnchor>,
    /// Bottom regions with no matching top from any other speaker.
    pub orphaned_bottoms: Vec<OverlapAnchor>,
    /// Per-utterance overlap info (cached from extraction).
    pub per_utterance: Vec<PerUtteranceOverlap>,
}

/// Per-utterance overlap data extracted during analysis.
#[derive(Debug, Clone)]
pub struct PerUtteranceOverlap {
    /// Utterance index.
    pub utterance_index: usize,
    /// Speaker code.
    pub speaker: String,
    /// Overlap marker info for this utterance.
    pub info: OverlapMarkerInfo,
    /// Utterance-level timing bullet.
    pub bullet: Option<(u64, u64)>,
}

impl FileOverlapAnalysis {
    /// Total number of matched groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total number of bottom regions across all groups.
    pub fn total_bottoms(&self) -> usize {
        self.groups.iter().map(|g| g.bottoms.len()).sum()
    }

    /// Groups where the top has timing and at least one bottom has timing.
    pub fn timed_groups(&self) -> impl Iterator<Item = &OverlapGroup> {
        self.groups
            .iter()
            .filter(|g| g.top.bullet.is_some() && g.bottoms.iter().any(|b| b.bullet.is_some()))
    }

    /// Whether there are any overlap markers in the file at all.
    pub fn has_overlaps(&self) -> bool {
        !self.groups.is_empty()
            || !self.orphaned_tops.is_empty()
            || !self.orphaned_bottoms.is_empty()
    }
}

/// Analyze cross-utterance overlap groups for an entire file.
///
/// Extracts overlap regions from each utterance, then matches top regions
/// (⌈...⌉) with bottom regions (⌊...⌋) across speakers. Supports 1:N
/// matching — one top can have multiple bottoms from different speakers.
pub fn analyze_file_overlaps(lines: &[Line]) -> FileOverlapAnalysis {
    // Step 1: Extract per-utterance overlap info.
    let mut per_utterance: Vec<PerUtteranceOverlap> = Vec::new();
    for line in lines {
        if let Line::Utterance(utt) = line {
            let info = extract_overlap_info(&utt.main.content.content.0);
            let bullet = utt
                .main
                .content
                .bullet
                .as_ref()
                .map(|b| (b.timing.start_ms, b.timing.end_ms));
            per_utterance.push(PerUtteranceOverlap {
                utterance_index: per_utterance.len(),
                speaker: utt.main.speaker.to_string(),
                info,
                bullet,
            });
        }
    }

    // Step 2: Collect all top and bottom anchors.
    let mut tops: Vec<OverlapAnchor> = Vec::new();
    let mut bottoms: Vec<OverlapAnchor> = Vec::new();

    for pu in &per_utterance {
        for region in &pu.info.regions {
            let anchor = OverlapAnchor {
                utterance_index: pu.utterance_index,
                speaker: pu.speaker.clone(),
                region: region.clone(),
                bullet: pu.bullet,
            };
            match region.kind {
                OverlapRegionKind::Top if region.has_begin() => tops.push(anchor),
                OverlapRegionKind::Bottom if region.has_begin() => bottoms.push(anchor),
                _ => {} // Orphaned closings without openings — skip
            }
        }
    }

    // Step 3: Match each bottom to its nearest preceding top from a different
    // speaker with the same index. Build groups.
    let mut top_to_bottoms: Vec<Vec<OverlapAnchor>> = vec![Vec::new(); tops.len()];
    let mut bottom_matched: Vec<bool> = vec![false; bottoms.len()];

    for (bi, bottom) in bottoms.iter().enumerate() {
        // Search backward through tops for a match.
        let mut best_top: Option<usize> = None;
        for (ti, top) in tops.iter().enumerate().rev() {
            // Must be from a different speaker.
            if top.speaker == bottom.speaker {
                continue;
            }
            // Must have the same index (or both unindexed).
            if top.region.index != bottom.region.index {
                continue;
            }
            // Must precede or be at the same utterance position.
            if top.utterance_index > bottom.utterance_index {
                continue;
            }
            best_top = Some(ti);
            break;
        }

        if let Some(ti) = best_top {
            top_to_bottoms[ti].push(bottom.clone());
            bottom_matched[bi] = true;
        }
    }

    // Step 4: Build groups and collect orphans.
    let mut groups: Vec<OverlapGroup> = Vec::new();
    let mut orphaned_tops: Vec<OverlapAnchor> = Vec::new();

    for (ti, top) in tops.into_iter().enumerate() {
        let matched_bottoms = std::mem::take(&mut top_to_bottoms[ti]);
        if matched_bottoms.is_empty() {
            orphaned_tops.push(top);
        } else {
            groups.push(OverlapGroup {
                top,
                bottoms: matched_bottoms,
            });
        }
    }

    let orphaned_bottoms: Vec<OverlapAnchor> = bottoms
        .into_iter()
        .zip(bottom_matched)
        .filter(|(_, matched)| !*matched)
        .map(|(b, _)| b)
        .collect();

    FileOverlapAnalysis {
        groups,
        orphaned_tops,
        orphaned_bottoms,
        per_utterance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use crate::model::{
        MainTier, OverlapIndex, OverlapPoint, OverlapPointKind, Terminator, Utterance,
        UtteranceContent, Word,
    };

    fn make_utterance(speaker: &str, content: Vec<UtteranceContent>) -> Utterance {
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        Utterance::new(main)
    }

    fn word(text: &str) -> UtteranceContent {
        UtteranceContent::Word(Box::new(Word::new_unchecked(text, text)))
    }

    fn overlap(kind: OverlapPointKind) -> UtteranceContent {
        UtteranceContent::OverlapPoint(OverlapPoint::new(kind, None))
    }

    fn overlap_idx(kind: OverlapPointKind, idx: u32) -> UtteranceContent {
        UtteranceContent::OverlapPoint(OverlapPoint::new(kind, Some(OverlapIndex(idx))))
    }

    fn to_lines(utts: Vec<Utterance>) -> Vec<Line> {
        utts.into_iter()
            .map(|u| Line::Utterance(Box::new(u)))
            .collect()
    }

    #[test]
    fn test_simple_1to1_pairing() {
        // *A: ⌈ hello ⌉ .
        // *B: ⌊ hi ⌋ .
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::TopOverlapBegin),
                    word("hello"),
                    overlap(OverlapPointKind::TopOverlapEnd),
                ],
            ),
            make_utterance(
                "B",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("hi"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 1);
        assert_eq!(analysis.groups[0].top.speaker, "A");
        assert_eq!(analysis.groups[0].bottoms.len(), 1);
        assert_eq!(analysis.groups[0].bottoms[0].speaker, "B");
        assert!(analysis.orphaned_tops.is_empty());
        assert!(analysis.orphaned_bottoms.is_empty());
    }

    #[test]
    fn test_1_to_n_pairing() {
        // *A: ⌈ hello ⌉ .
        // *B: ⌊ yeah ⌋ .
        // *C: ⌊ right ⌋ .
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::TopOverlapBegin),
                    word("hello"),
                    overlap(OverlapPointKind::TopOverlapEnd),
                ],
            ),
            make_utterance(
                "B",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("yeah"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
            make_utterance(
                "C",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("right"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 1, "one top → one group");
        assert_eq!(analysis.groups[0].bottoms.len(), 2, "two bottoms matched");
        assert_eq!(analysis.groups[0].bottoms[0].speaker, "B");
        assert_eq!(analysis.groups[0].bottoms[1].speaker, "C");
        assert!(analysis.orphaned_tops.is_empty());
        assert!(analysis.orphaned_bottoms.is_empty());
    }

    #[test]
    fn test_indexed_pairing() {
        // *A: ⌈ one ⌉ ⌈2 two ⌉2 .
        // *B: ⌊ one ⌋ .
        // *B: ⌊2 two ⌋2 .
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::TopOverlapBegin),
                    word("one"),
                    overlap(OverlapPointKind::TopOverlapEnd),
                    overlap_idx(OverlapPointKind::TopOverlapBegin, 2),
                    word("two"),
                    overlap_idx(OverlapPointKind::TopOverlapEnd, 2),
                ],
            ),
            make_utterance(
                "B",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("one"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
            make_utterance(
                "B",
                vec![
                    overlap_idx(OverlapPointKind::BottomOverlapBegin, 2),
                    word("two"),
                    overlap_idx(OverlapPointKind::BottomOverlapEnd, 2),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 2, "two separate groups by index");
        // Unindexed group
        assert_eq!(analysis.groups[0].top.region.index, None);
        assert_eq!(analysis.groups[0].bottoms.len(), 1);
        // Indexed group
        assert_eq!(analysis.groups[1].top.region.index, Some(OverlapIndex(2)));
        assert_eq!(analysis.groups[1].bottoms.len(), 1);
    }

    #[test]
    fn test_same_speaker_not_matched() {
        // *A: ⌈ hello ⌉ .
        // *A: ⌊ nope ⌋ .  ← same speaker, should not match
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::TopOverlapBegin),
                    word("hello"),
                    overlap(OverlapPointKind::TopOverlapEnd),
                ],
            ),
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("nope"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 0);
        assert_eq!(analysis.orphaned_tops.len(), 1);
        assert_eq!(analysis.orphaned_bottoms.len(), 1);
    }

    #[test]
    fn test_orphaned_top() {
        // *A: ⌈ hello ⌉ .
        // *B: no overlap here .
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap(OverlapPointKind::TopOverlapBegin),
                    word("hello"),
                    overlap(OverlapPointKind::TopOverlapEnd),
                ],
            ),
            make_utterance("B", vec![word("no"), word("overlap")]),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 0);
        assert_eq!(analysis.orphaned_tops.len(), 1);
        assert_eq!(analysis.orphaned_tops[0].speaker, "A");
    }

    #[test]
    fn test_orphaned_bottom() {
        // *A: no overlap .
        // *B: ⌊ random ⌋ .  ← no preceding top from different speaker
        let lines = to_lines(vec![
            make_utterance("A", vec![word("no"), word("overlap")]),
            make_utterance(
                "B",
                vec![
                    overlap(OverlapPointKind::BottomOverlapBegin),
                    word("random"),
                    overlap(OverlapPointKind::BottomOverlapEnd),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 0);
        assert_eq!(analysis.orphaned_bottoms.len(), 1);
    }

    #[test]
    fn test_index_mismatch_not_matched() {
        // *A: ⌈2 hello ⌉2 .
        // *B: ⌊3 hi ⌋3 .  ← index 3 ≠ index 2
        let lines = to_lines(vec![
            make_utterance(
                "A",
                vec![
                    overlap_idx(OverlapPointKind::TopOverlapBegin, 2),
                    word("hello"),
                    overlap_idx(OverlapPointKind::TopOverlapEnd, 2),
                ],
            ),
            make_utterance(
                "B",
                vec![
                    overlap_idx(OverlapPointKind::BottomOverlapBegin, 3),
                    word("hi"),
                    overlap_idx(OverlapPointKind::BottomOverlapEnd, 3),
                ],
            ),
        ]);

        let analysis = analyze_file_overlaps(&lines);
        assert_eq!(analysis.groups.len(), 0, "index mismatch → no group");
        assert_eq!(analysis.orphaned_tops.len(), 1);
        assert_eq!(analysis.orphaned_bottoms.len(), 1);
    }
}
