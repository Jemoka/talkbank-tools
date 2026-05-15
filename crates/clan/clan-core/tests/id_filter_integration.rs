//! End-to-end runner test for `--id-filter` / `+t@ID="..."`.
//!
//! Wires the public `clan_core::framework::AnalysisRunner` against a
//! real fixture (`resources/corpus/reference/edge-cases/postcodes-and-gems.cha`,
//! which has both `CHI` and `MOT` `@ID` lines plus utterances from each
//! speaker). Verifies that an `IdFilter` set on `FilterConfig` drops
//! utterances whose speaker's `@ID` does not match the pattern.
//!
//! Companion to the type-level unit tests inside
//! `clan-core/src/framework/id_filter.rs::tests`.

mod common;

use std::path::PathBuf;

use clan_core::commands::freq::FreqCommand;
use clan_core::framework::{AnalysisRunner, FilterConfig, IdFilter};

use crate::common::corpus_file;

fn fixture() -> PathBuf {
    corpus_file("edge-cases/postcodes-and-gems.cha")
}

fn run_with_filter(filter: FilterConfig) -> clan_core::commands::freq::FreqResult {
    let runner = AnalysisRunner::with_filter(filter);
    let command = FreqCommand::default();
    runner
        .run(&command, &[fixture()])
        .expect("runner should accept a single existing fixture")
}

/// Convenience: run the freq command on the fixture with `--id-filter
/// <pattern>` only (other filters at their defaults).
fn run_with_id_pattern(pattern: &str) -> clan_core::commands::freq::FreqResult {
    run_with_filter(FilterConfig {
        id_filter: Some(pattern.parse::<IdFilter>().unwrap()),
        ..FilterConfig::default()
    })
}

/// Sum the per-speaker token counts to get an overall count.
///
/// `FreqResult` exposes per-speaker totals; the overall "did the filter
/// drop the right utterances" question is decided by summing them.
fn total_tokens(result: &clan_core::commands::freq::FreqResult) -> u64 {
    // `WordCount` is a `u64` type alias, so a plain sum is fine.
    result.speakers.iter().map(|s| s.total_tokens).sum()
}

#[test]
fn id_filter_chi_keeps_only_chi_utterances() {
    let baseline = run_with_filter(FilterConfig::default());
    let baseline_tokens = total_tokens(&baseline);

    let chi_only = run_with_id_pattern("*|*|CHI|*");
    let chi_tokens = total_tokens(&chi_only);

    // CHI is one of two speakers; the filter must reduce token counts.
    assert!(
        chi_tokens > 0,
        "CHI utterances should produce some tokens after filtering"
    );
    assert!(
        chi_tokens < baseline_tokens,
        "--id-filter CHI must drop MOT utterances (chi={chi_tokens}, baseline={baseline_tokens})"
    );
}

#[test]
fn id_filter_mot_keeps_only_mot_utterances() {
    let baseline = run_with_filter(FilterConfig::default());
    let baseline_tokens = total_tokens(&baseline);

    let mot_only = run_with_id_pattern("*|*|MOT|*");
    let mot_tokens = total_tokens(&mot_only);

    assert!(
        mot_tokens > 0,
        "MOT utterances should produce some tokens after filtering"
    );
    assert!(
        mot_tokens < baseline_tokens,
        "--id-filter MOT must drop CHI utterances (mot={mot_tokens}, baseline={baseline_tokens})"
    );
}

#[test]
fn id_filter_chi_plus_mot_equals_baseline() {
    let baseline = run_with_filter(FilterConfig::default());
    let baseline_tokens = total_tokens(&baseline);

    let chi_only = run_with_id_pattern("*|*|CHI|*");
    let mot_only = run_with_id_pattern("*|*|MOT|*");

    // CHI-only + MOT-only must add up to the unfiltered baseline. If
    // tokens went missing, the filter is silently dropping more than
    // it should.
    assert_eq!(
        total_tokens(&chi_only) + total_tokens(&mot_only),
        baseline_tokens,
        "CHI tokens + MOT tokens must equal baseline tokens"
    );
}

#[test]
fn id_filter_non_matching_language_skips_file_entirely() {
    // The fixture is all `eng`. A filter that requires `fra` should
    // file-prefilter the whole input out, leaving the freq result
    // empty.
    let result = run_with_id_pattern("fra|*|*|*");
    assert_eq!(
        total_tokens(&result),
        0,
        "fra-only filter on an eng-only file should produce zero tokens"
    );
}

#[test]
fn id_filter_no_constraint_equals_no_filter() {
    let baseline = run_with_filter(FilterConfig::default());
    let wide_open = run_with_id_pattern("");
    assert_eq!(
        total_tokens(&baseline),
        total_tokens(&wide_open),
        "empty id-filter pattern should be the same as no filter"
    );
}
