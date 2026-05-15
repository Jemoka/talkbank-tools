//! OVERLAP-AUDIT — CA overlap marker analysis.
//!
//! Analyzes CA overlap markers (⌈⌉⌊⌋) across utterances: counts markers,
//! matches top↔bottom pairs (1:N), reports orphans, checks temporal
//! consistency for files with timing.
//!
//! Uses [`analyze_file_overlaps`] from `talkbank-model` for cross-utterance
//! matching with proper 1:N support and index-aware pairing.
//!
//! # Output
//!
//! Per file:
//! - Number of overlap groups (matched top↔bottom)
//! - Orphaned tops and bottoms
//! - Pairing quality classification
//! - Temporal consistency (for timed files)
//!
//! Plus a corpus-wide summary.

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::Utterance;
use talkbank_model::alignment::helpers::overlap_groups::{
    FileOverlapAnalysis, analyze_file_overlaps,
};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section,
};

/// Configuration for the OVERLAP-AUDIT command.
#[derive(Debug, Clone, Default)]
pub struct OverlapAuditConfig {}

/// Per-file overlap results.
#[derive(Debug, Clone, Serialize)]
struct FileResult {
    filename: String,
    total_utterances: usize,
    overlap_groups: usize,
    total_bottoms: usize,
    orphaned_tops: usize,
    orphaned_bottoms: usize,
    timed_groups: usize,
    temporally_consistent: usize,
    quality: String,
}

/// Corpus-wide summary.
#[derive(Debug, Clone, Serialize)]
struct CorpusSummary {
    files_total: usize,
    files_with_overlaps: usize,
    total_groups: usize,
    total_bottoms: usize,
    total_orphaned_tops: usize,
    total_orphaned_bottoms: usize,
    timed_groups: usize,
    temporally_consistent: usize,
}

/// Typed output for the OVERLAP-AUDIT command.
#[derive(Debug, Clone, Serialize)]
pub struct OverlapAuditResult {
    files: Vec<FileResult>,
    summary: CorpusSummary,
}

impl OverlapAuditResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("overlap-audit");

        for f in &self.files {
            if f.overlap_groups == 0 && f.orphaned_tops == 0 && f.orphaned_bottoms == 0 {
                continue; // Skip files with no overlaps
            }
            let mut fields = IndexMap::new();
            fields.insert("Groups".to_owned(), f.overlap_groups.to_string());
            fields.insert("Bottoms".to_owned(), f.total_bottoms.to_string());
            fields.insert("Orphaned tops".to_owned(), f.orphaned_tops.to_string());
            fields.insert(
                "Orphaned bottoms".to_owned(),
                f.orphaned_bottoms.to_string(),
            );
            fields.insert("Quality".to_owned(), f.quality.clone());
            if f.timed_groups > 0 {
                fields.insert(
                    "Temporal".to_owned(),
                    format!("{}/{} consistent", f.temporally_consistent, f.timed_groups),
                );
            }
            result.add_section(Section::with_fields(f.filename.clone(), fields));
        }

        let s = &self.summary;
        let mut fields = IndexMap::new();
        fields.insert(
            "Files with overlaps".to_owned(),
            s.files_with_overlaps.to_string(),
        );
        fields.insert("Total groups".to_owned(), s.total_groups.to_string());
        fields.insert("Total bottoms".to_owned(), s.total_bottoms.to_string());
        fields.insert(
            "Orphaned tops".to_owned(),
            s.total_orphaned_tops.to_string(),
        );
        fields.insert(
            "Orphaned bottoms".to_owned(),
            s.total_orphaned_bottoms.to_string(),
        );
        if s.timed_groups > 0 {
            let pct = s.temporally_consistent as f64 / s.timed_groups as f64 * 100.0;
            fields.insert(
                "Temporal consistency".to_owned(),
                format!(
                    "{}/{} ({:.0}%)",
                    s.temporally_consistent, s.timed_groups, pct
                ),
            );
        }
        result.add_section(Section::with_fields("Summary".to_owned(), fields));

        result
    }
}

impl CommandOutput for OverlapAuditResult {
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    fn render_clan(&self) -> String {
        self.render_text()
    }
}

/// Accumulated state for OVERLAP-AUDIT across files.
#[derive(Debug, Default)]
pub struct OverlapAuditState {
    files: Vec<FileResult>,
}

/// OVERLAP-AUDIT command implementation.
#[derive(Debug, Clone, Default)]
pub struct OverlapAuditCommand;

/// Classify pairing quality.
fn classify_quality(analysis: &FileOverlapAnalysis) -> String {
    if !analysis.has_overlaps() {
        return "none".to_owned();
    }
    if analysis.orphaned_tops.is_empty() && analysis.orphaned_bottoms.is_empty() {
        return "fully_paired".to_owned();
    }
    let total =
        analysis.groups.len() + analysis.orphaned_tops.len() + analysis.orphaned_bottoms.len();
    let orphan_fraction =
        (analysis.orphaned_tops.len() + analysis.orphaned_bottoms.len()) as f64 / total as f64;
    /// Fraction of orphaned overlaps above which the file is classified as
    /// "open_only" (mostly unpaired overlap markers, not matched groups).
    const ORPHAN_CLASSIFICATION_THRESHOLD: f64 = 0.8;
    if orphan_fraction > ORPHAN_CLASSIFICATION_THRESHOLD {
        "open_only".to_owned()
    } else {
        "mixed".to_owned()
    }
}

/// Check temporal consistency for a group: does the bottom utterance's
/// timing overlap with the top utterance's timing?
fn is_temporally_consistent(
    top_bullet: Option<(u64, u64)>,
    bottom_bullet: Option<(u64, u64)>,
) -> Option<bool> {
    let (top_start, top_end) = top_bullet?;
    let (bottom_start, _bottom_end) = bottom_bullet?;
    let tolerance_ms: u64 = 2000;
    Some(bottom_start <= top_end + tolerance_ms && bottom_start + tolerance_ms >= top_start)
}

impl AnalysisCommand for OverlapAuditCommand {
    type Config = OverlapAuditConfig;
    type State = OverlapAuditState;
    type Output = OverlapAuditResult;

    /// No per-utterance processing — all work happens in `end_file`.
    fn process_utterance(
        &self,
        _utterance: &Utterance,
        _file_context: &FileContext<'_>,
        _state: &mut Self::State,
    ) {
    }

    /// Run cross-utterance overlap analysis on the full file.
    fn end_file(&self, file_context: &FileContext<'_>, state: &mut Self::State) {
        let analysis = analyze_file_overlaps(&file_context.chat_file.lines);

        let total_utterances = file_context
            .chat_file
            .lines
            .iter()
            .filter(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .count();

        // Check temporal consistency for timed groups.
        let mut timed_groups = 0;
        let mut temporally_consistent = 0;
        for group in &analysis.groups {
            for bottom in &group.bottoms {
                if let Some(consistent) = is_temporally_consistent(group.top.bullet, bottom.bullet)
                {
                    timed_groups += 1;
                    if consistent {
                        temporally_consistent += 1;
                    }
                }
            }
        }

        let quality = classify_quality(&analysis);

        state.files.push(FileResult {
            filename: file_context.filename.to_owned(),
            total_utterances,
            overlap_groups: analysis.groups.len(),
            total_bottoms: analysis.total_bottoms(),
            orphaned_tops: analysis.orphaned_tops.len(),
            orphaned_bottoms: analysis.orphaned_bottoms.len(),
            timed_groups,
            temporally_consistent,
            quality,
        });
    }

    fn finalize(&self, state: Self::State) -> OverlapAuditResult {
        let files_with_overlaps = state
            .files
            .iter()
            .filter(|f| f.overlap_groups > 0 || f.orphaned_tops > 0 || f.orphaned_bottoms > 0)
            .count();

        let summary = CorpusSummary {
            files_total: state.files.len(),
            files_with_overlaps,
            total_groups: state.files.iter().map(|f| f.overlap_groups).sum(),
            total_bottoms: state.files.iter().map(|f| f.total_bottoms).sum(),
            total_orphaned_tops: state.files.iter().map(|f| f.orphaned_tops).sum(),
            total_orphaned_bottoms: state.files.iter().map(|f| f.orphaned_bottoms).sum(),
            timed_groups: state.files.iter().map(|f| f.timed_groups).sum(),
            temporally_consistent: state.files.iter().map(|f| f.temporally_consistent).sum(),
        };

        OverlapAuditResult {
            files: state.files,
            summary,
        }
    }
}
