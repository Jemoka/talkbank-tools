//! CSV and Markdown report writers.
//!
//! Four artifacts per run:
//! - `per-word.csv` — one row per `@s` word (flat, easy to sort/filter)
//! - `per-pair.csv` — one row per language pair (aggregate stats)
//! - `flagged.csv` — subset of `per-word.csv` where at least one flag fired
//! - `summary.md` — narrative report with the gate decisions
//!
//! Column names and ordering match the Python analyzer so that existing
//! downstream notebooks and the 2026-04-15 baseline diff cleanly against
//! this Rust port's output.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use csv::Writer;

use super::types::{AtSAnalysis, AtSStatus, FileAnalysis, HeuristicFlag, PairKey};

// ---------------------------------------------------------------------------
// Report-writer domain errors
// ---------------------------------------------------------------------------

/// Errors produced when writing report artifacts.
#[derive(thiserror::Error, Debug)]
pub enum ReportError {
    /// Filesystem failure.
    #[error("failed to write {path}: {source}")]
    Io {
        /// Output path.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: io::Error,
    },
    /// CSV-serialization failure.
    #[error("csv serialization failed for {path}: {source}")]
    Csv {
        /// Output path.
        path: PathBuf,
        /// Underlying csv crate error.
        #[source]
        source: csv::Error,
    },
}

impl ReportError {
    fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> ReportError {
        let path = path.into();
        move |source| ReportError::Io { path, source }
    }

    fn csv(path: impl Into<PathBuf>) -> impl FnOnce(csv::Error) -> ReportError {
        let path = path.into();
        move |source| ReportError::Csv { path, source }
    }
}

// ---------------------------------------------------------------------------
// Per-pair aggregate model
// ---------------------------------------------------------------------------

/// Aggregate counts for one language pair, accumulated across files.
///
/// `BTreeMap` is used for flag and POS counters so CSV column order is
/// deterministic across runs (important for diffing vs the Python baseline
/// and for reproducibility).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PairAggregate {
    /// Language-pair key.
    pub pair_key: PairKey,
    /// Number of files contributing to this pair.
    pub files: u64,
    /// Total `@s` word count.
    pub at_s_total: u64,
    /// Count with status `Spliced`.
    pub spliced: u64,
    /// Count with status `L2Xxx`.
    pub l2xxx: u64,
    /// Count with status `MissingMor`.
    pub missing_mor: u64,
    /// Count of records with zero heuristic flags.
    pub heuristic_clean: u64,
    /// Per-flag counts (BTreeMap for determinism).
    pub flag_counts: BTreeMap<HeuristicFlag, u64>,
    /// Top-POS distribution (BTreeMap for determinism).
    pub pos_counts: BTreeMap<String, u64>,
    /// Post-morphotag outcome distribution across all utterances in all
    /// files for this pair (Wave 4 of the morphotag reconciliation
    /// architecture). Sum of these four should equal the total number
    /// of utterances in all files for this pair.
    pub outcomes: OutcomeCounts,
}

/// Per-pair outcome distribution for morphotag post-hoc classification.
///
/// See [`UtteranceOutcome`](super::types::UtteranceOutcome) for the
/// four-way classification this aggregates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutcomeCounts {
    /// Utterances with `UtteranceOutcome::NotApplicable`.
    pub not_applicable: u64,
    /// Utterances with `UtteranceOutcome::Aligned`.
    pub aligned: u64,
    /// Utterances with `UtteranceOutcome::CountMismatchInFile`.
    pub count_mismatch_in_file: u64,
    /// Utterances with `UtteranceOutcome::PipelineAbsorbedFailure`.
    pub pipeline_absorbed_failure: u64,
}

impl OutcomeCounts {
    /// Total utterances counted across all four variants.
    pub fn total(&self) -> u64 {
        self.not_applicable
            + self.aligned
            + self.count_mismatch_in_file
            + self.pipeline_absorbed_failure
    }

    /// Count of anomaly outcomes (not-applicable and aligned are expected;
    /// the other two are pipeline anomalies that require attention).
    pub fn anomalies(&self) -> u64 {
        self.count_mismatch_in_file + self.pipeline_absorbed_failure
    }

    /// Fraction of utterances that are anomalies. Zero when no
    /// utterances were seen.
    pub fn anomaly_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.anomalies() as f64 / total as f64
        }
    }
}

impl PairAggregate {
    /// Ratio of spliced `@s` words to total — pessimistic because any
    /// `missing_mor` reduces the numerator even when the feature itself
    /// succeeded. With the AST walker missing_mor should be rare.
    pub fn splice_rate(&self) -> f64 {
        if self.at_s_total == 0 {
            0.0
        } else {
            self.spliced as f64 / self.at_s_total as f64
        }
    }

    /// L2-dispatch success rate: `1 - L2xxx / total`. Counts `missing_mor`
    /// as NOT a dispatch failure — this is the metric gated against the
    /// pre-registered 99% threshold.
    pub fn dispatch_rate(&self) -> f64 {
        if self.at_s_total == 0 {
            0.0
        } else {
            1.0 - (self.l2xxx as f64 / self.at_s_total as f64)
        }
    }

    /// Fraction of records with zero heuristic flags.
    pub fn heuristic_clean_rate(&self) -> f64 {
        if self.at_s_total == 0 {
            0.0
        } else {
            self.heuristic_clean as f64 / self.at_s_total as f64
        }
    }
}

/// Roll up per-file analyses into per-pair aggregates.
pub fn aggregate_by_pair(files: &[FileAnalysis]) -> BTreeMap<PairKey, PairAggregate> {
    use super::types::UtteranceOutcome;
    let mut out: BTreeMap<PairKey, PairAggregate> = BTreeMap::new();
    for file in files {
        let agg = out
            .entry(file.pair_key.clone())
            .or_insert_with(|| PairAggregate {
                pair_key: file.pair_key.clone(),
                ..PairAggregate::default()
            });
        agg.files += 1;
        for outcome in &file.utterance_outcomes {
            match outcome {
                UtteranceOutcome::NotApplicable => agg.outcomes.not_applicable += 1,
                UtteranceOutcome::Aligned { .. } => agg.outcomes.aligned += 1,
                UtteranceOutcome::CountMismatchInFile { .. } => {
                    agg.outcomes.count_mismatch_in_file += 1
                }
                UtteranceOutcome::PipelineAbsorbedFailure { .. } => {
                    agg.outcomes.pipeline_absorbed_failure += 1
                }
            }
        }
        for a in &file.analyses {
            agg.at_s_total += 1;
            match a.status {
                AtSStatus::Spliced => {
                    agg.spliced += 1;
                    if let Some(pos) = &a.pos {
                        *agg.pos_counts.entry(pos.as_str().to_string()).or_insert(0) += 1;
                    }
                }
                AtSStatus::L2Xxx => agg.l2xxx += 1,
                AtSStatus::MissingMor => agg.missing_mor += 1,
            }
            if a.flags.is_empty() {
                agg.heuristic_clean += 1;
            }
            for f in &a.flags {
                *agg.flag_counts.entry(*f).or_insert(0) += 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// CSV writers
// ---------------------------------------------------------------------------

/// Per-word CSV field order — MUST match the Python analyzer's
/// `PER_WORD_FIELDS` for downstream compatibility.
pub const PER_WORD_FIELDS: &[&str] = &[
    "file",
    "pair_key",
    "effective_lang",
    "surface",
    "mor_item",
    "pos",
    "lemma",
    "features",
    "gra_deprel",
    "status",
    "flags",
];

/// Per-pair CSV field order.
///
/// The first 15 columns (through `top_pos`) match the Python analyzer's
/// `PER_PAIR_FIELDS` for downstream-tooling compatibility. The last five
/// columns (prefixed `outcome_` / `anomaly_`) are added by Wave 4 of the
/// morphotag reconciliation architecture and carry the per-pair
/// distribution of `UtteranceOutcome` variants.
pub const PER_PAIR_FIELDS: &[&str] = &[
    "pair_key",
    "files",
    "at_s_total",
    "spliced",
    "splice_rate",
    "l2xxx",
    "dispatch_rate",
    "missing_mor",
    "heuristic_clean",
    "heuristic_clean_rate",
    "flag_L2Xxx",
    "flag_MissingMor",
    "flag_PropnForFunctionWord",
    "flag_FeaturePosMismatch",
    "top_pos",
    "outcome_not_applicable",
    "outcome_aligned",
    "outcome_count_mismatch_in_file",
    "outcome_pipeline_absorbed_failure",
    "anomaly_rate",
];

fn write_header(w: &mut Writer<File>, fields: &[&str]) -> Result<(), csv::Error> {
    w.write_record(fields)
}

/// Write `per-word.csv` — one row per `@s` word.
pub fn write_per_word(path: &Path, analyses: &[&AtSAnalysis]) -> Result<(), ReportError> {
    let file = File::create(path).map_err(ReportError::io(path))?;
    let mut w = Writer::from_writer(file);
    write_header(&mut w, PER_WORD_FIELDS).map_err(ReportError::csv(path))?;
    for a in analyses {
        w.write_record(per_word_row(a))
            .map_err(ReportError::csv(path))?;
    }
    w.flush().map_err(ReportError::io(path))?;
    Ok(())
}

/// Write `flagged.csv` — subset of `per-word.csv` where any flag fired.
pub fn write_flagged(path: &Path, analyses: &[&AtSAnalysis]) -> Result<(), ReportError> {
    let file = File::create(path).map_err(ReportError::io(path))?;
    let mut w = Writer::from_writer(file);
    write_header(&mut w, PER_WORD_FIELDS).map_err(ReportError::csv(path))?;
    for a in analyses {
        if a.flags.is_empty() {
            continue;
        }
        w.write_record(per_word_row(a))
            .map_err(ReportError::csv(path))?;
    }
    w.flush().map_err(ReportError::io(path))?;
    Ok(())
}

/// Write `per-pair.csv` — one row per language pair.
pub fn write_per_pair(
    path: &Path,
    pairs: &BTreeMap<PairKey, PairAggregate>,
) -> Result<(), ReportError> {
    let file = File::create(path).map_err(ReportError::io(path))?;
    let mut w = Writer::from_writer(file);
    write_header(&mut w, PER_PAIR_FIELDS).map_err(ReportError::csv(path))?;
    for agg in pairs.values() {
        w.write_record(per_pair_row(agg))
            .map_err(ReportError::csv(path))?;
    }
    w.flush().map_err(ReportError::io(path))?;
    Ok(())
}

fn per_word_row(a: &AtSAnalysis) -> Vec<String> {
    let file = a.occurrence.file.display().to_string();
    let flags = a
        .flags
        .iter()
        .map(|f| f.name().to_string())
        .collect::<Vec<_>>()
        .join(";");
    vec![
        file,
        a.occurrence.pair_key.to_string(),
        a.occurrence.effective_lang.as_str().to_string(),
        a.occurrence.surface.to_string(),
        a.occurrence
            .mor_item
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_default(),
        a.pos
            .as_ref()
            .map(|p| p.as_str().to_string())
            .unwrap_or_default(),
        a.lemma.clone().unwrap_or_default(),
        a.features
            .as_ref()
            .map(|f| f.as_str().to_string())
            .unwrap_or_default(),
        a.gra_deprel
            .as_ref()
            .map(|d| d.as_str().to_string())
            .unwrap_or_default(),
        a.status.as_csv_str().to_string(),
        flags,
    ]
}

fn per_pair_row(agg: &PairAggregate) -> Vec<String> {
    let mut top_pos_items: Vec<(&String, &u64)> = agg.pos_counts.iter().collect();
    top_pos_items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    top_pos_items.truncate(5);
    let top_pos = top_pos_items
        .iter()
        .map(|(p, c)| format!("{p}:{c}"))
        .collect::<Vec<_>>()
        .join(", ");

    let flag = |f: HeuristicFlag| agg.flag_counts.get(&f).copied().unwrap_or(0).to_string();

    vec![
        agg.pair_key.to_string(),
        agg.files.to_string(),
        agg.at_s_total.to_string(),
        agg.spliced.to_string(),
        format!("{:.3}", agg.splice_rate()),
        agg.l2xxx.to_string(),
        format!("{:.3}", agg.dispatch_rate()),
        agg.missing_mor.to_string(),
        agg.heuristic_clean.to_string(),
        format!("{:.3}", agg.heuristic_clean_rate()),
        flag(HeuristicFlag::L2Xxx),
        flag(HeuristicFlag::MissingMor),
        flag(HeuristicFlag::PropnForFunctionWord),
        flag(HeuristicFlag::FeaturePosMismatch),
        top_pos,
        agg.outcomes.not_applicable.to_string(),
        agg.outcomes.aligned.to_string(),
        agg.outcomes.count_mismatch_in_file.to_string(),
        agg.outcomes.pipeline_absorbed_failure.to_string(),
        format!("{:.3}", agg.outcomes.anomaly_rate()),
    ]
}

// ---------------------------------------------------------------------------
// Summary Markdown writer
// ---------------------------------------------------------------------------

/// Thresholds matching the pre-registered ungating criteria from the plan.
const SPLICE_GATE: f64 = 0.99;
/// Per-pair heuristic-clean threshold.
const CLEAN_GATE_PER_PAIR: f64 = 0.85;
/// Aggregate heuristic-clean threshold.
const CLEAN_GATE_AGGREGATE: f64 = 0.90;

/// Compose the human-readable `summary.md` from per-pair aggregates.
fn pct(numer: u64, denom: u64) -> f64 {
    if denom == 0 {
        0.0
    } else {
        numer as f64 / denom as f64 * 100.0
    }
}

/// Render a Markdown summary of the per-pair L2 morphotag evaluation
/// outcomes.
///
/// Includes an overall header (eval-set path, morphotag output dir,
/// aggregate counts), a pair-by-pair table with splice/dispatch/
/// clean-heuristic percentages, and an anomaly breakdown when any
/// pair recorded absorbed failures or count mismatches.
pub fn summary_markdown(
    pairs: &BTreeMap<PairKey, PairAggregate>,
    eval_set_path: &Path,
    morphotag_dir: &Path,
) -> String {
    let total_files: u64 = pairs.values().map(|a| a.files).sum();
    let total_at_s: u64 = pairs.values().map(|a| a.at_s_total).sum();
    let total_spliced: u64 = pairs.values().map(|a| a.spliced).sum();
    let total_l2xxx: u64 = pairs.values().map(|a| a.l2xxx).sum();
    let total_clean: u64 = pairs.values().map(|a| a.heuristic_clean).sum();

    let (agg_splice, agg_dispatch, agg_clean) = if total_at_s == 0 {
        (0.0, 0.0, 0.0)
    } else {
        let t = total_at_s as f64;
        (
            total_spliced as f64 / t,
            1.0 - (total_l2xxx as f64 / t),
            total_clean as f64 / t,
        )
    };

    let gate_str = |pass: bool| if pass { "PASS" } else { "FAIL" };

    let mut out = String::new();
    out.push_str("# L2 Morphotag Aggregate Evaluation\n\n");
    out.push_str(&format!(
        "**Eval set:** `{}`\n**Morphotag output:** `{}`\n\n",
        eval_set_path.display(),
        morphotag_dir.display()
    ));
    out.push_str(&format!("- Files: **{}**\n", total_files));
    out.push_str(&format!("- `@s` words: **{}**\n", total_at_s));
    out.push_str(&format!(
        "- Aggregate **dispatch rate**: **{:.2}%** (gate ≥{:.0}%: {}) — counts only `L2|xxx` fallbacks against the feature.\n",
        agg_dispatch * 100.0,
        SPLICE_GATE * 100.0,
        gate_str(agg_dispatch >= SPLICE_GATE)
    ));
    out.push_str(&format!(
        "- Aggregate splice rate: {:.2}% — includes `missing_mor` as failures (pessimistic floor; AST walker should keep this near 0).\n",
        agg_splice * 100.0
    ));
    out.push_str(&format!(
        "- Aggregate heuristic-clean rate: **{:.1}%** (gate ≥{:.0}%: {})\n\n",
        agg_clean * 100.0,
        CLEAN_GATE_AGGREGATE * 100.0,
        gate_str(agg_clean >= CLEAN_GATE_AGGREGATE)
    ));

    out.push_str("## Per-pair\n\n");
    out.push_str(
        "| Pair | Files | @s | Dispatch | Splice | L2\\|xxx | MissingMor | Clean | Gate |\n",
    );
    out.push_str(
        "|------|------:|---:|---------:|-------:|--------:|-----------:|------:|:----:|\n",
    );
    for agg in pairs.values() {
        let pair_gate =
            agg.dispatch_rate() >= SPLICE_GATE && agg.heuristic_clean_rate() >= CLEAN_GATE_PER_PAIR;
        out.push_str(&format!(
            "| `{}` | {} | {} | {:.2}% | {:.1}% | {} | {} | {:.1}% | {} |\n",
            agg.pair_key,
            agg.files,
            agg.at_s_total,
            agg.dispatch_rate() * 100.0,
            agg.splice_rate() * 100.0,
            agg.l2xxx,
            agg.missing_mor,
            agg.heuristic_clean_rate() * 100.0,
            gate_str(pair_gate),
        ));
    }
    out.push('\n');

    out.push_str("## Flag distribution\n\n");
    let mut total_flags: BTreeMap<HeuristicFlag, u64> = BTreeMap::new();
    for agg in pairs.values() {
        for (flag, count) in &agg.flag_counts {
            *total_flags.entry(*flag).or_insert(0) += *count;
        }
    }
    if total_flags.is_empty() {
        out.push_str("_No flags fired._\n");
    } else {
        out.push_str("| Flag | Count | % of @s |\n");
        out.push_str("|------|------:|--------:|\n");
        for (flag, count) in &total_flags {
            let pct = if total_at_s == 0 {
                0.0
            } else {
                *count as f64 / total_at_s as f64
            };
            out.push_str(&format!(
                "| `{}` | {} | {:.1}% |\n",
                flag.name(),
                count,
                pct * 100.0
            ));
        }
    }
    out.push('\n');

    // Wave 4 (morphotag reconciliation architecture): per-utterance
    // outcome distribution, aggregated across all utterances in all files
    // for all pairs. Anomaly rate > 0 means either the pipeline absorbed
    // a MisalignmentBug (no %mor emitted despite alignable content) or
    // the output file has a count mismatch someone stamped in manually.
    let total_outcomes: u64 = pairs.values().map(|a| a.outcomes.total()).sum();
    let total_not_applicable: u64 = pairs.values().map(|a| a.outcomes.not_applicable).sum();
    let total_aligned: u64 = pairs.values().map(|a| a.outcomes.aligned).sum();
    let total_count_mismatch: u64 = pairs
        .values()
        .map(|a| a.outcomes.count_mismatch_in_file)
        .sum();
    let total_absorbed: u64 = pairs
        .values()
        .map(|a| a.outcomes.pipeline_absorbed_failure)
        .sum();
    let total_anomalies: u64 = total_count_mismatch + total_absorbed;
    let anomaly_rate_pct = if total_outcomes == 0 {
        0.0
    } else {
        total_anomalies as f64 / total_outcomes as f64 * 100.0
    };

    out.push_str("## Per-utterance outcome distribution\n\n");
    out.push_str(&format!(
        "Across **{total_outcomes} utterances** (all pairs, all files):\n\n"
    ));
    out.push_str(&format!(
        "- `Aligned`: {total_aligned} ({:.1}%) — `%mor` matched CHAT alignable count.\n",
        pct(total_aligned, total_outcomes)
    ));
    out.push_str(&format!(
        "- `NotApplicable`: {total_not_applicable} ({:.1}%) — no Mor-alignable content; correctly no `%mor`.\n",
        pct(total_not_applicable, total_outcomes)
    ));
    out.push_str(&format!(
        "- `CountMismatchInFile`: {total_count_mismatch} ({:.1}%) — `%mor` size ≠ alignable count. Post-fix this should be 0.\n",
        pct(total_count_mismatch, total_outcomes)
    ));
    out.push_str(&format!(
        "- `PipelineAbsorbedFailure`: {total_absorbed} ({:.1}%) — alignable content present but no `%mor`; pipeline absorbed a `MisalignmentBug`.\n",
        pct(total_absorbed, total_outcomes)
    ));
    out.push_str(&format!(
        "- **Anomaly rate: {anomaly_rate_pct:.2}%** (sum of last two; non-zero means a bug to investigate).\n\n"
    ));
    if total_anomalies > 0 {
        out.push_str("### Per-pair anomalies\n\n");
        out.push_str(
            "| Pair | Utts | Aligned | NotApp | CountMismatch | AbsorbedFailure | Anomaly% |\n",
        );
        out.push_str(
            "|------|-----:|--------:|-------:|--------------:|----------------:|---------:|\n",
        );
        let mut pairs_sorted: Vec<&PairAggregate> = pairs.values().collect();
        pairs_sorted.sort_by_key(|a| std::cmp::Reverse(a.outcomes.anomalies()));
        for agg in pairs_sorted {
            if agg.outcomes.anomalies() == 0 {
                continue;
            }
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} | {:.2}% |\n",
                agg.pair_key,
                agg.outcomes.total(),
                agg.outcomes.aligned,
                agg.outcomes.not_applicable,
                agg.outcomes.count_mismatch_in_file,
                agg.outcomes.pipeline_absorbed_failure,
                agg.outcomes.anomaly_rate() * 100.0,
            ));
        }
        out.push('\n');
        out.push_str(
            "See [`book/src/architecture/morphotag-invariants.md`](../../../architecture/morphotag-invariants.md) \
             for how to investigate anomalies.\n\n",
        );
    }

    out.push_str("## Reproducing this evaluation\n\n");
    out.push_str("```bash\n");
    out.push_str(&format!(
        "batchalign3 eval l2-morphotag \\\n    --eval-set {} \\\n    --morphotag-output {} \\\n    --output <report-dir>/\n",
        eval_set_path.display(),
        morphotag_dir.display()
    ));
    out.push_str("```\n");

    out
}

/// Write `summary.md` to `path`.
pub fn write_summary(
    path: &Path,
    pairs: &BTreeMap<PairKey, PairAggregate>,
    eval_set_path: &Path,
    morphotag_dir: &Path,
) -> Result<(), ReportError> {
    let body = summary_markdown(pairs, eval_set_path, morphotag_dir);
    fs::write(path, body).map_err(ReportError::io(path))?;
    Ok(())
}
