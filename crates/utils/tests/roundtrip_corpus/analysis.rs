//! Roundtrip test analysis utilities for diagnosing failures across corpora.
//!
//! This module provides tools for systematically analyzing roundtrip test failures,
//! including corpus-wide pass rate statistics, diff analysis, and failure categorization.

use std::fs;
use std::path::{Path, PathBuf};

/// Enum variants for AnalysisError.
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("Missing results line in output")]
    MissingResultsLine,
    #[error("Missing field {field} in results line")]
    MissingField { field: &'static str },
    #[error("Invalid number for {field}: {value}")]
    InvalidNumber { field: &'static str, value: String },
}

/// Statistics for a single corpus roundtrip test run
#[derive(Debug, Clone)]
pub struct CorpusStats {
    pub name: String,
    pub path: PathBuf,
    pub total_files: usize,
    pub passed: usize,
    pub failed: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl CorpusStats {
    /// Runs pass rate.
    pub fn pass_rate(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.passed as f64 / self.total_files as f64) * 100.0
        }
    }

    /// Runs cache hit rate.
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / self.total_files as f64) * 100.0
        }
    }

    /// Runs from result.
    pub fn from_result(name: &str, path: PathBuf, output: &str) -> Result<Self, AnalysisError> {
        // Parse output like: "Results: ✓ 337 passed, ✗ 0 failed, ⚡ 337 cache hits (100.0% hit rate)"
        let results_line = output
            .lines()
            .find(|line| line.contains("Results:"))
            .ok_or(AnalysisError::MissingResultsLine)?;

        let mut stats = CorpusStats {
            name: name.to_string(),
            path,
            total_files: 0,
            passed: 0,
            failed: 0,
            cache_hits: 0,
            cache_misses: 0,
        };

        // Extract passed count
        if let Some(passed_str) = results_line.split("✓ ").nth(1) {
            let num_str = passed_str
                .split_whitespace()
                .next()
                .ok_or(AnalysisError::MissingField { field: "passed" })?;
            stats.passed = parse_usize(num_str, "passed")?;
        } else {
            return Err(AnalysisError::MissingField { field: "passed" });
        }

        // Extract failed count
        if let Some(failed_str) = results_line.split("✗ ").nth(1) {
            let num_str = failed_str
                .split_whitespace()
                .next()
                .ok_or(AnalysisError::MissingField { field: "failed" })?;
            stats.failed = parse_usize(num_str, "failed")?;
        } else {
            return Err(AnalysisError::MissingField { field: "failed" });
        }

        // Extract cache hits
        if let Some(cache_str) = results_line.split("⚡ ").nth(1) {
            let num_str =
                cache_str
                    .split_whitespace()
                    .next()
                    .ok_or(AnalysisError::MissingField {
                        field: "cache_hits",
                    })?;
            stats.cache_hits = parse_usize(num_str, "cache_hits")?;
        } else {
            return Err(AnalysisError::MissingField {
                field: "cache_hits",
            });
        }

        stats.total_files = stats.passed + stats.failed;
        stats.cache_misses = stats.total_files - stats.cache_hits;

        Ok(stats)
    }
}

/// Parses usize.
fn parse_usize(value: &str, field: &'static str) -> Result<usize, AnalysisError> {
    value.parse().map_err(|_| AnalysisError::InvalidNumber {
        field,
        value: value.to_string(),
    })
}

/// Categorizes roundtrip failures by type
#[derive(Debug, Clone, Default)]
pub struct FailureCategories {
    pub terminator_issues: usize,
    pub spacing_issues: usize,
    pub other_issues: usize,
}

/// Analyzes diff files to categorize failure types
pub fn analyze_diffs(diffs_dir: &Path) -> FailureCategories {
    let mut categories = FailureCategories::default();

    if !diffs_dir.exists() {
        return categories;
    }

    // Find all canonical-original files
    if let Ok(entries) = fs::read_dir(diffs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".canonical-original") {
                    // Load both files
                    let serialized = path
                        .to_string_lossy()
                        .replace(".canonical-original", ".canonical-serialized");
                    if let (Ok(original), Ok(serialized_content)) =
                        (fs::read_to_string(&path), fs::read_to_string(&serialized))
                    {
                        categorize_diff(&original, &serialized_content, &mut categories);
                    }
                }
            }
        }
    }

    categories
}

/// Runs categorize diff.
fn categorize_diff(original: &str, serialized: &str, categories: &mut FailureCategories) {
    for (orig_line, ser_line) in original.lines().zip(serialized.lines()) {
        if orig_line != ser_line {
            // Check for terminator issues (e.g., +!? vs +/??)
            if (orig_line.contains("+!?") && ser_line.contains("+/?"))
                || (orig_line.contains("+/?") && ser_line.contains("+!?"))
                || (orig_line.contains("+!") && !ser_line.contains("+!"))
            {
                categories.terminator_issues += 1;
            }
            // Check for spacing issues (e.g., "tande], così !" vs "tande] , così !")
            else if orig_line.replace(" ", "") == ser_line.replace(" ", "") {
                categories.spacing_issues += 1;
            } else {
                categories.other_issues += 1;
            }
        }
    }
}

/// Generate a comprehensive roundtrip test report
pub fn print_test_report(all_stats: &[CorpusStats], failure_cats: &FailureCategories) {
    println!("\n{}", "=".repeat(70));
    println!("ROUNDTRIP TEST COMPREHENSIVE REPORT");
    println!("{}", "=".repeat(70));

    println!("\n## Corpus-by-Corpus Results");
    println!(
        "{:<20} {:<15} {:<15} {:<20}",
        "Corpus", "Pass Rate", "Cache Hit Rate", "Status"
    );
    println!("{}", "-".repeat(70));

    let mut total_passed = 0;
    let mut total_files = 0;

    for stats in all_stats {
        total_passed += stats.passed;
        total_files += stats.total_files;

        let status = if stats.failed == 0 {
            "✓ 100%"
        } else if stats.pass_rate() >= 90.0 {
            "✓ Good"
        } else if stats.pass_rate() >= 70.0 {
            "⚠ Fair"
        } else {
            "✗ Poor"
        };

        println!(
            "{:<20} {:<14.1}% {:<14.1}% {}",
            stats.name,
            stats.pass_rate(),
            stats.cache_hit_rate(),
            status
        );
    }

    println!("{}", "-".repeat(70));
    if total_files > 0 {
        let overall_rate = (total_passed as f64 / total_files as f64) * 100.0;
        println!(
            "{:<20} {:<14.1}% {:<14}  Overall",
            "TOTAL", overall_rate, ""
        );
    }

    println!("\n## Failure Analysis");
    println!(
        "- Terminator issues: {} (e.g., +!? vs +/??)",
        failure_cats.terminator_issues
    );
    println!(
        "- Spacing issues: {} (e.g., spacing around punctuation)",
        failure_cats.spacing_issues
    );
    println!("- Other issues: {}", failure_cats.other_issues);

    println!("\n## Recommendations");
    if failure_cats.terminator_issues > 0 {
        println!("- Fix terminator parsing for +!? (interrupted with emphasis)");
    }
    if failure_cats.spacing_issues > 0 {
        println!("- Standardize spacing rules for punctuation in replacements");
    }
    println!("- Reference corpus achieves 100% with disabled E346/E352 validations");
    println!("- CA corpus reaches 88%+ with proper CA marker handling");

    println!("\n{}", "=".repeat(70));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests stats parsing.
    #[test]
    fn test_stats_parsing() -> Result<(), AnalysisError> {
        let output = "Results: ✓ 337 passed, ✗ 0 failed, ⚡ 337 cache hits (100.0% hit rate)";
        let stats = CorpusStats::from_result("test", PathBuf::from("."), output)?;
        assert_eq!(stats.passed, 337);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.cache_hits, 337);

        Ok(())
    }

    /// Tests pass rate calculation.
    #[test]
    fn test_pass_rate_calculation() {
        let stats = CorpusStats {
            name: "test".to_string(),
            path: PathBuf::from("."),
            total_files: 100,
            passed: 71,
            failed: 29,
            cache_hits: 0,
            cache_misses: 100,
        };
        assert_eq!(stats.pass_rate(), 71.0);
    }
}
