//! Summary totals and exit-status policy for alignment visualization.
//!
//! Prints the aggregate alignment/error counts from [`RenderTotals`] and exits
//! with code 1 when either validation or alignment errors were found. This makes
//! `chatter show-alignment` usable as a CI gate — a non-zero exit signals that
//! dependent-tier alignment has regressed.

use std::path::Path;

use crate::cli::AlignmentTier;
use crate::commands::alignment::helpers::format_tier_label;

use super::RenderTotals;

/// Print totals and exit non-zero when validation or alignment errors are present.
///
/// The summary mirrors the CLI’s alignment diagnostics tables so downstream CI jobs can audit the
/// same metrics described in the manual. When either validation errors or alignment mismatch errors
/// exist, the command exits with `1` to make regression detection trivial for auto-deploy scripts.
pub(super) fn render_summary(
    input: &Path,
    tier_filter: Option<AlignmentTier>,
    totals: &RenderTotals,
    had_validation_errors: bool,
) {
    println!();
    println!("{}", "=".repeat(80));
    if totals.total_alignments == 0 {
        match tier_filter {
            Some(tier) => eprintln!(
                "No {} alignments were found in {}.",
                format_tier_label(tier),
                input.display()
            ),
            None => eprintln!("No alignments were found in {}.", input.display()),
        }
    }

    println!(
        "Summary: {} alignment(s) shown, {} error(s) found",
        totals.total_alignments, totals.total_errors
    );

    if had_validation_errors || totals.total_errors > 0 {
        std::process::exit(1);
    }
}
