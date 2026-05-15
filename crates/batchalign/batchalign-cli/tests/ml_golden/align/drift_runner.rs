//! Align-pipeline driver + result aggregator for the env-gated drift
//! integration tests.
//!
//! Responsibilities of this module — deliberately narrow:
//!
//! - Run `batchalign3 align` on one staged file (via [`run_one_file`]) and
//!   classify the result as one of three [`FileOutcome`] variants.
//! - Dispatch the four drift assertions against the post-run parsed CHAT via
//!   the shared helpers in [`crate::common::drift_assertions`].
//! - Aggregate per-file outcomes into a single readable panic message (via
//!   [`assert_all_files_pass`]), prepending a 1-line summary header.
//! - Enforce the "empty outcomes → SKIP" contract via [`skip_if_empty`] so
//!   missing-media runs never pass vacuously.
//!
//! Pure filesystem staging lives one layer down in
//! [`crate::common::drift_staging`]; test bodies live one layer up in
//! `drift_integration.rs`.

use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::chat_ops::ChatFile;
use batchalign::options::{FaEngineName, WorTierPolicy};
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

use crate::common::LiveDirectJobClient;
use crate::common::drift_assertions::evaluate_drift_assertion;
use crate::common::drift_staging::{CorpusFileName, StagedDriftFile};
use crate::common::regression_manifest::FixtureAssertion;
use crate::ml_golden::align::helpers::align_options;

/// Evaluate the four drift assertions against one parsed output CHAT. Returns
/// a list of failure messages (empty on pass).
pub fn evaluate_all_drift_assertions(parsed: &ChatFile) -> Vec<String> {
    [
        FixtureAssertion::NoFaGroupInvalidAudioWindow,
        FixtureAssertion::NoMonotonicityRescueEmitted,
        FixtureAssertion::UtteranceBulletMonotonicityPreserved,
        FixtureAssertion::NoSilentTimingStrip,
    ]
    .iter()
    .filter_map(|a| evaluate_drift_assertion(parsed, a).err())
    .collect()
}

/// Record of how one file fared — used to build a single, readable panic
/// message at the end of each test. Modeled as an enum so the three failure
/// shapes (pipeline failure, parse failure, evaluated-with-failures) are
/// explicit at the type level and callers cannot forget to check one.
pub enum FileOutcome {
    /// The align pipeline itself did not complete successfully (crashed,
    /// returned non-Completed, or produced the wrong number of outputs).
    /// Treated as a `NoFaGroupInvalidAudioWindow`-class failure in the
    /// report.
    PipelineFailure {
        cha_name: CorpusFileName,
        message: String,
    },
    /// The pipeline completed, but the output CHAT failed to parse.
    ParseFailure {
        cha_name: CorpusFileName,
        message: String,
    },
    /// The pipeline completed AND the output parsed. The four drift
    /// assertions were evaluated; `assertion_failures` is empty on pass and
    /// populated on failure.
    Evaluated {
        cha_name: CorpusFileName,
        assertion_failures: Vec<String>,
    },
}

impl FileOutcome {
    pub fn passed(&self) -> bool {
        matches!(
            self,
            FileOutcome::Evaluated { assertion_failures, .. } if assertion_failures.is_empty()
        )
    }

    #[allow(dead_code)] // Part of the FileOutcome API; retained for future aggregation consumers.
    pub fn cha_name(&self) -> &CorpusFileName {
        match self {
            FileOutcome::PipelineFailure { cha_name, .. }
            | FileOutcome::ParseFailure { cha_name, .. }
            | FileOutcome::Evaluated { cha_name, .. } => cha_name,
        }
    }
}

/// Run `batchalign3 align` on one staged file and evaluate all four drift
/// assertions against the output. Returns `None` when the file had to be
/// skipped (missing media) — caller logs the skip and moves on.
pub async fn run_one_file(
    jobs: &LiveDirectJobClient<'_>,
    staged: &StagedDriftFile,
    lang: &str,
) -> Option<FileOutcome> {
    let cha_name = staged.cha_name();

    if staged.media_path.is_none() {
        eprintln!(
            "SKIP: {}: no adjacent media file found next to {}",
            cha_name,
            staged.cha_path.display()
        );
        return None;
    }

    // Direct the align output to a temp location under the session state
    // dir. The paths-mode helper derives the output CHAT filename from the
    // source stem, so we only need the output directory to be unique.
    let out_dir = jobs
        .state_dir()
        .join(format!("drift_out_{}", cha_name.as_str().replace('.', "_")));
    std::fs::create_dir_all(&out_dir).expect("mkdir drift out_dir");
    let output_path = out_dir.join("test.cha");

    let (info, outputs) = jobs
        .submit_paths_job(
            ReleasedCommand::Align,
            lang,
            vec![staged.cha_path.to_string_lossy().into_owned()],
            vec![output_path.to_string_lossy().into_owned()],
            align_options(FaEngineName::Wave2Vec, WorTierPolicy::Include),
        )
        .await;

    if info.status != JobStatus::Completed {
        return Some(FileOutcome::PipelineFailure {
            cha_name,
            message: format!(
                "align pipeline returned non-Completed status {:?} — surfaces as \
                 NoFaGroupInvalidAudioWindow-class failure (crash, InvalidAudioWindow, etc.)",
                info.status
            ),
        });
    }

    if outputs.len() != 1 {
        return Some(FileOutcome::PipelineFailure {
            cha_name,
            message: format!("expected exactly one output CHAT, got {}", outputs.len()),
        });
    }

    let parser = match TreeSitterParser::new() {
        Ok(p) => p,
        Err(e) => {
            return Some(FileOutcome::ParseFailure {
                cha_name,
                message: format!("tree-sitter construct: {e:?}"),
            });
        }
    };
    let (parsed, parse_errors) = parse_lenient(&parser, &outputs[0]);
    if !parse_errors.is_empty() {
        return Some(FileOutcome::ParseFailure {
            cha_name,
            message: format!("output CHAT failed to parse: {parse_errors:?}"),
        });
    }

    let assertion_failures = evaluate_all_drift_assertions(&parsed);
    Some(FileOutcome::Evaluated {
        cha_name,
        assertion_failures,
    })
}

/// Unified SKIP tail: if no file in the test had adjacent media, the whole
/// test SKIPs cleanly rather than passing vacuously via an empty outcome
/// vector. Returns `true` when the caller should `return` (the test was
/// skipped). Enforces I-2 (the missing-media vacuous-pass contract) by
/// construction across all four tests.
pub fn skip_if_empty(test_name: &str, outcomes: &[FileOutcome]) -> bool {
    if outcomes.is_empty() {
        eprintln!("SKIP: {test_name}: no stageable files had adjacent media");
        return true;
    }
    false
}

/// Format a readable report across all files for one convention, and panic if
/// any file failed. One panic at the end (rather than per-file) lets the test
/// report the full landscape in a single message.
///
/// The first line of the panic is a `{label}: {pass}/{total} passed, {fail}
/// failed` summary so the top-of-report tells you the split at a glance
/// before the per-file details.
pub fn assert_all_files_pass(label: &str, outcomes: &[FileOutcome]) {
    let failed: Vec<&FileOutcome> = outcomes.iter().filter(|o| !o.passed()).collect();
    if failed.is_empty() {
        return;
    }
    let total = outcomes.len();
    let passed = total - failed.len();
    let mut lines: Vec<String> = Vec::new();
    // 1-line summary header (M-6).
    lines.push(format!(
        "{label}: {passed}/{total} passed, {failed_count} failed",
        failed_count = failed.len(),
    ));
    for o in &failed {
        match o {
            FileOutcome::PipelineFailure { cha_name, message } => {
                lines.push(format!("  - {cha_name}: PipelineFailure — {message}"));
            }
            FileOutcome::ParseFailure { cha_name, message } => {
                lines.push(format!("  - {cha_name}: ParseFailure — {message}"));
            }
            FileOutcome::Evaluated {
                cha_name,
                assertion_failures,
            } => {
                lines.push(format!(
                    "  - {cha_name}: {} assertion failure(s):",
                    assertion_failures.len()
                ));
                for m in assertion_failures {
                    lines.push(format!("      {m}"));
                }
            }
        }
    }
    panic!("drift-integration {}", lines.join("\n"));
}
