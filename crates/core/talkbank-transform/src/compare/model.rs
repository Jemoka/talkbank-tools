use std::collections::BTreeMap;

/// Status of a compared token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareStatus {
    /// Word matches between main and gold.
    Match,
    /// Word present in main but not in gold (insertion).
    ExtraMain,
    /// Word present in gold but not in main (deletion).
    ExtraGold,
}

/// A single token in the comparison output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareToken {
    /// The word text.
    pub text: String,
    /// Uppercased part-of-speech tag when `%mor` data is available.
    pub pos: Option<String>,
    /// Match status.
    pub status: CompareStatus,
}

/// Per-utterance comparison result.
#[derive(Debug, Clone)]
pub struct UtteranceComparison {
    /// Zero-based utterance index in the main file.
    pub utterance_index: usize,
    /// Speaker code.
    pub speaker: String,
    /// Comparison tokens (matches, insertions, deletions).
    pub tokens: Vec<CompareToken>,
}

/// Aggregate comparison metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareMetrics {
    /// Word Error Rate: (insertions + deletions) / total_gold_words.
    pub wer: f64,
    /// 1.0 - wer (clamped to [0, 1]).
    pub accuracy: f64,
    /// Number of matching words.
    pub matches: usize,
    /// Words in main but not in gold.
    pub insertions: usize,
    /// Words in gold but not in main.
    pub deletions: usize,
    /// Total words in the gold transcript (matches + deletions).
    pub total_gold_words: usize,
    /// Total words in the main transcript (matches + insertions).
    pub total_main_words: usize,
    /// Per-POS error breakdown keyed by uppercased POS label.
    pub pos_counts: BTreeMap<String, PosErrorCounts>,
}

/// Per-POS compare counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PosErrorCounts {
    /// Number of matching tokens for this POS.
    pub matches: usize,
    /// Number of insertion tokens for this POS.
    pub insertions: usize,
    /// Number of deletion tokens for this POS.
    pub deletions: usize,
}

/// Full comparison bundle.
///
/// This is the internal workflow artifact produced by transcript comparison.
/// It can later support multiple materialization paths (main-annotated output,
/// gold-projected output, metrics sidecars, debugging views) without forcing
/// the compare stage itself to decide the final output shape.
#[derive(Debug, Clone)]
pub struct ComparisonBundle {
    /// Main-anchored per-utterance comparison annotations.
    pub main_utterances: Vec<UtteranceComparison>,
    /// Gold-anchored per-utterance comparison annotations.
    pub gold_utterances: Vec<UtteranceComparison>,
    /// Structural word matches from gold back to the matched main word.
    pub gold_word_matches: Vec<GoldWordMatch>,
    /// Aggregate metrics.
    pub metrics: CompareMetrics,
}

/// Compatibility alias retained while the compare pipeline is refactored toward
/// workflow bundles plus explicit materializers.
pub type CompareResult = ComparisonBundle;

/// A structural match between one gold word slot and one main word slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldWordMatch {
    /// Gold utterance containing the matched word.
    pub gold_utterance_index: usize,
    /// Zero-based compared-word position within the gold utterance.
    pub gold_word_position: usize,
    /// Main utterance supplying the matched word.
    pub main_utterance_index: usize,
    /// Zero-based compared-word position within the main utterance.
    pub main_word_position: usize,
}
