//! `batchalign3 eval l2-morphotag` — port of the Python analyzer at
//! `scripts/l2-eval/analyze.py` to a proper Rust Clap subcommand.
//!
//! The Python analyzer uses regexes over serialized CHAT to walk `@s` words
//! and pair them with `%mor` / `%gra` items by token position. That
//! approach mis-counts positions under CHAT retrace markers
//! (`[/]`, `[//]`, `<foo bar> [//]`), producing a persistent ~2%
//! `missing_mor` noise floor that had to be explained away in the summary.
//!
//! This Rust port drives off the typed `talkbank-model` AST via
//! [`walk_words(TierDomain::Mor)`](talkbank_model::alignment::helpers::walk::walk_words).
//! The walker yields word-like items in exactly the order that
//! `mor_tier.items` aligns to, eliminating the retrace off-by-one noise by
//! construction.
//!
//! The module is split along four responsibilities:
//!
//! | Module | Role |
//! |--------|------|
//! | [`types`] | All newtypes (`PairKey`, `SurfaceWord`, `MorItemText`, `GraItemText`, `FeatureSet`) and domain enums (`AtSStatus`, `HeuristicFlag`, `LanguageMarkerKind`) plus the per-word and file records |
//! | [`analysis`] | CHAT-AST walker: `analyze_file` / `analyze_chat_file`, plus serialized-form helpers `extract_pos_lemma_features` / `extract_gra_deprel` for test convenience |
//! | [`heuristics`] | `flags_for(&AtSAnalysis)` — the rule-based suspicious-output detectors |
//! | [`report`] | Aggregation (`aggregate_by_pair`) and CSV / Markdown writers (`write_per_word`, `write_per_pair`, `write_flagged`, `write_summary`) |

pub mod analysis;
pub mod heuristics;
pub mod report;
pub mod types;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::args::L2MorphotagEvalArgs;
use crate::cli::error::CliError;

use analysis::{AnalysisError, analyze_file};
use report::{
    ReportError, aggregate_by_pair, write_flagged, write_per_pair, write_per_word, write_summary,
};
use types::{FileAnalysis, PairKey};

// ---------------------------------------------------------------------------
// Eval-set JSONL shape
// ---------------------------------------------------------------------------

/// Minimal JSONL row schema from `eval-set.jsonl`.
///
/// The eval-set file is authoritative for the `pair_key` label on each
/// input; the analyzer never re-derives it from the CHAT file itself.
/// Extra fields in the JSONL are ignored silently.
#[derive(Debug, Clone, Deserialize)]
struct EvalSetEntry {
    path: PathBuf,
    pair_key: String,
}

/// Command-level error type. Wraps the sub-module error types plus
/// eval-set parsing failures.
#[derive(thiserror::Error, Debug)]
pub enum EvalCommandError {
    /// Analysis (parser / IO) failure.
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
    /// Report-writer failure.
    #[error(transparent)]
    Report(#[from] ReportError),
    /// Eval-set JSONL parsing failure.
    #[error("failed to read eval-set {path}: {source}")]
    EvalSetIo {
        /// Path of the eval-set file.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// One JSONL line could not be decoded.
    #[error("malformed eval-set line {line_no} in {path}: {source}")]
    EvalSetJson {
        /// Path of the eval-set file.
        path: PathBuf,
        /// 1-based line number that failed to decode.
        line_no: usize,
        /// Underlying serde_json error.
        #[source]
        source: serde_json::Error,
    },
    /// No post-morphotag file matched any eval-set entry.
    #[error("no morphotag-output CHAT files under {0} matched any eval-set basename")]
    NoMatches(PathBuf),
    /// Eval-set path does not exist.
    #[error("eval-set not found: {0}")]
    EvalSetMissing(PathBuf),
    /// Morphotag-output path does not exist.
    #[error("morphotag-output not found: {0}")]
    MorphotagMissing(PathBuf),
}

impl From<EvalCommandError> for CliError {
    fn from(err: EvalCommandError) -> Self {
        CliError::InvalidArgument(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Eval-set loading
// ---------------------------------------------------------------------------

/// Read `eval-set.jsonl`, returning a basename→pair_key map.
///
/// Matching by basename mirrors the Python runner's convention: the
/// `--morphotag-output` directory holds files whose basenames correspond
/// to eval-set input paths. The input paths themselves may live on a
/// machine the evaluator does not have access to.
fn load_eval_set_basenames(path: &Path) -> Result<BTreeMap<String, PairKey>, EvalCommandError> {
    let file = File::open(path).map_err(|source| EvalCommandError::EvalSetIo {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut out = BTreeMap::new();
    for (i, line_res) in reader.lines().enumerate() {
        let line = line_res.map_err(|source| EvalCommandError::EvalSetIo {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: EvalSetEntry =
            serde_json::from_str(&line).map_err(|source| EvalCommandError::EvalSetJson {
                path: path.to_path_buf(),
                line_no: i + 1,
                source,
            })?;
        let basename = entry
            .path
            .file_name()
            .map(|b| b.to_string_lossy().into_owned())
            .unwrap_or_default();
        if basename.is_empty() {
            continue;
        }
        out.insert(basename, PairKey::new(entry.pair_key));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Command entry point
// ---------------------------------------------------------------------------

/// Run the `eval l2-morphotag` subcommand — the public seam invoked by
/// `run_command` in `lib.rs`.
pub fn run(args: &L2MorphotagEvalArgs) -> Result<(), CliError> {
    run_impl(args).map_err(Into::into)
}

fn run_impl(args: &L2MorphotagEvalArgs) -> Result<(), EvalCommandError> {
    let eval_set = &args.eval_set;
    let morphotag_output = &args.morphotag_output;
    let output = &args.output;

    if !eval_set.exists() {
        return Err(EvalCommandError::EvalSetMissing(eval_set.clone()));
    }
    if !morphotag_output.exists() {
        return Err(EvalCommandError::MorphotagMissing(morphotag_output.clone()));
    }
    std::fs::create_dir_all(output).map_err(|source| EvalCommandError::EvalSetIo {
        path: output.clone(),
        source,
    })?;

    let pair_by_basename = load_eval_set_basenames(eval_set)?;

    // Walk the morphotag-output tree and analyze every `*.cha` whose
    // basename is in the eval set.
    let mut analyses: Vec<FileAnalysis> = Vec::new();
    for entry in walkdir::WalkDir::new(morphotag_output)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("cha") {
            continue;
        }
        let basename = match path.file_name().and_then(|b| b.to_str()) {
            Some(b) => b,
            None => continue,
        };
        let pair_key = match pair_by_basename.get(basename) {
            Some(k) => k.clone(),
            None => continue,
        };
        analyses.push(analyze_file(path, pair_key)?);
    }

    if analyses.is_empty() {
        return Err(EvalCommandError::NoMatches(morphotag_output.clone()));
    }

    let per_pair = aggregate_by_pair(&analyses);
    let flat: Vec<&types::AtSAnalysis> = analyses.iter().flat_map(|f| f.analyses.iter()).collect();

    write_per_word(&output.join("per-word.csv"), &flat)?;
    write_flagged(&output.join("flagged.csv"), &flat)?;
    write_per_pair(&output.join("per-pair.csv"), &per_pair)?;
    write_summary(
        &output.join("summary.md"),
        &per_pair,
        eval_set,
        morphotag_output,
    )?;

    let total_at_s: u64 = per_pair.values().map(|a| a.at_s_total).sum();
    let total_spliced: u64 = per_pair.values().map(|a| a.spliced).sum();
    let total_clean: u64 = per_pair.values().map(|a| a.heuristic_clean).sum();
    println!(
        "analyzed {} files, {} @s words across {} language pairs",
        analyses.len(),
        total_at_s,
        per_pair.len()
    );
    if total_at_s > 0 {
        println!(
            "  splice rate: {:.1}%",
            total_spliced as f64 / total_at_s as f64 * 100.0
        );
        println!(
            "  heuristic-clean rate: {:.1}%",
            total_clean as f64 / total_at_s as f64 * 100.0
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
