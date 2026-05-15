//! Generate coverage dashboard showing validation implementation status
//!
//! Analyzes error corpus specs and generates a markdown dashboard with:
//! - Summary statistics (implemented vs not implemented)
//! - Progress bars by category
//! - Priority ordering
//! - Recent changes
//!
//! Can optionally parse test results to determine implementation status.

use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use generators::spec::error_corpus::ErrorCorpusSpec;

/// Generates a Markdown coverage dashboard showing validation implementation status by category and priority.
fn main() -> Result<()> {
    let spec_root = PathBuf::from("spec/errors");
    let output_path = PathBuf::from("docs/COVERAGE_DASHBOARD.md");

    println!("Loading error corpus specs from: {}", spec_root.display());

    let all_specs = ErrorCorpusSpec::load_all(&spec_root)
        .map_err(|e| anyhow::anyhow!("Failed to load error corpus specs: {}", e))?;

    println!("Found {} error spec files", all_specs.len());

    // Separate parser and validation errors
    let parser_specs: Vec<_> = all_specs
        .iter()
        .filter(|s| s.metadata.layer == "parser")
        .collect();
    let validation_specs: Vec<_> = all_specs
        .iter()
        .filter(|s| s.metadata.layer == "validation")
        .collect();

    // Group validation examples by error code
    let mut validation_by_code: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for spec in &validation_specs {
        for example in &spec.examples {
            if let Some(code) = &example.error_code {
                validation_by_code
                    .entry(code.clone())
                    .or_default()
                    .push(example.name.clone());
            }
        }
    }

    // Known implemented error codes (from test results)
    let implemented = get_implemented_error_codes();

    // Generate dashboard
    let dashboard = generate_dashboard(
        &all_specs,
        &parser_specs,
        &validation_specs,
        &validation_by_code,
        &implemented,
    );

    fs::write(&output_path, dashboard)?;
    println!("✓ Generated: {}", output_path.display());

    // Print summary
    println!();
    println!("Coverage Summary:");
    println!("  Total error codes: {}", validation_by_code.len());
    println!("  Implemented: {}", implemented.len());
    println!(
        "  Not implemented: {}",
        validation_by_code.len() - implemented.len()
    );
    println!(
        "  Coverage: {:.1}%",
        (implemented.len() as f64 / validation_by_code.len() as f64) * 100.0
    );

    Ok(())
}

/// Get list of implemented error codes
/// In the future, this could parse actual test results
fn get_implemented_error_codes() -> HashMap<String, ImplementationStatus> {
    let mut map = HashMap::new();

    // Known implemented (from validation test results)
    map.insert("E241".to_string(), ImplementationStatus::Partial);
    map.insert("E308".to_string(), ImplementationStatus::Complete);
    map.insert("E517".to_string(), ImplementationStatus::Complete);

    // Misclassified (marked as validation but actually parser)
    map.insert("E505".to_string(), ImplementationStatus::Misclassified);
    map.insert("E506".to_string(), ImplementationStatus::Misclassified);

    map
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ImplementationStatus {
    Complete,
    Partial,
    Misclassified,
}

fn generate_dashboard(
    all_specs: &[ErrorCorpusSpec],
    parser_specs: &[&ErrorCorpusSpec],
    validation_specs: &[&ErrorCorpusSpec],
    validation_by_code: &BTreeMap<String, Vec<String>>,
    implemented: &HashMap<String, ImplementationStatus>,
) -> String {
    let mut output = String::new();

    output.push_str("# Error Coverage Dashboard\n\n");
    output
        .push_str("Visual dashboard showing implementation status of error validation rules.\n\n");
    output.push_str(&format!(
        "**Last Updated**: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    output.push_str("---\n\n");

    // Summary section
    output.push_str("## Summary\n\n");

    let parser_examples: usize = parser_specs.iter().map(|s| s.examples.len()).sum();
    let validation_examples: usize = validation_specs.iter().map(|s| s.examples.len()).sum();
    let total_validation_codes = validation_by_code.len();

    let implemented_count = implemented
        .values()
        .filter(|s| {
            matches!(
                s,
                ImplementationStatus::Complete | ImplementationStatus::Partial
            )
        })
        .count();
    let misclassified_count = implemented
        .values()
        .filter(|s| matches!(s, ImplementationStatus::Misclassified))
        .count();
    let not_implemented_count = total_validation_codes - implemented_count - misclassified_count;

    output.push_str(&format!("- **Total error codes**: {}\n", all_specs.len()));
    output.push_str(&format!(
        "- **Parser errors**: {} (grammar-level, fully covered by tree-sitter)\n",
        parser_examples
    ));
    output.push_str(&format!(
        "- **Validation errors**: {} unique codes, {} examples\n",
        total_validation_codes, validation_examples
    ));
    output.push_str("\n### Validation Implementation Status\n\n");

    output.push_str("| Status | Count | Percentage |\n");
    output.push_str("|--------|-------|------------|\n");
    output.push_str(&format!(
        "| ✅ Implemented | {} | {:.1}% |\n",
        implemented_count,
        (implemented_count as f64 / total_validation_codes as f64) * 100.0
    ));
    output.push_str(&format!(
        "| ❌ Not Implemented | {} | {:.1}% |\n",
        not_implemented_count,
        (not_implemented_count as f64 / total_validation_codes as f64) * 100.0
    ));
    output.push_str(&format!(
        "| 🔄 Misclassified | {} | {:.1}% |\n",
        misclassified_count,
        (misclassified_count as f64 / total_validation_codes as f64) * 100.0
    ));

    output.push_str("\n---\n\n");

    // Overall progress bar
    output.push_str("## Overall Progress\n\n");
    let progress_pct = (implemented_count as f64 / total_validation_codes as f64) * 100.0;
    output.push_str(&format!(
        "**Validation Rules**: {}/{} ({:.1}%)\n\n",
        implemented_count, total_validation_codes, progress_pct
    ));
    output.push_str(&generate_progress_bar(progress_pct));
    output.push_str("\n\n---\n\n");

    // By category
    output.push_str("## By Category\n\n");

    let categories = get_categories_with_codes(validation_specs, validation_by_code);
    for (category, codes) in &categories {
        let category_implemented = codes
            .iter()
            .filter(|c| implemented.contains_key(*c))
            .count();
        let category_pct = (category_implemented as f64 / codes.len() as f64) * 100.0;

        output.push_str(&format!(
            "### {} ({}/{})\n\n",
            category,
            category_implemented,
            codes.len()
        ));
        output.push_str(&generate_progress_bar(category_pct));
        output.push_str("\n\n");

        for code in codes {
            let status = implemented.get(code);
            let icon = match status {
                Some(ImplementationStatus::Complete) => "✅",
                Some(ImplementationStatus::Partial) => "🔶",
                Some(ImplementationStatus::Misclassified) => "🔄",
                None => "❌",
            };

            let example_count = match validation_by_code.get(code) {
                Some(examples) => examples.len(),
                None => {
                    output.push_str(&format!("- ⚠️ **{}** - missing examples\n", code));
                    continue;
                }
            };
            output.push_str(&format!(
                "- {} **{}** - {} example(s)\n",
                icon, code, example_count
            ));
        }

        output.push('\n');
    }

    output.push_str("---\n\n");

    // By priority
    output.push_str("## By Priority\n\n");

    let (high_priority, medium_priority, low_priority) = categorize_by_priority(validation_by_code);

    output.push_str("### High Priority (Tier Alignment + Common Errors)\n\n");
    let high_implemented = count_implemented(&high_priority, implemented);
    let high_pct = (high_implemented as f64 / high_priority.len() as f64) * 100.0;
    output.push_str(&format!(
        "{}/{} ({:.1}%)\n\n",
        high_implemented,
        high_priority.len(),
        high_pct
    ));
    output.push_str(&generate_progress_bar(high_pct));
    output.push_str("\n\n");
    for code in &high_priority {
        let status = implemented.get(code);
        let icon = match status {
            Some(ImplementationStatus::Complete) => "✅",
            Some(ImplementationStatus::Partial) => "🔶",
            Some(ImplementationStatus::Misclassified) => "🔄",
            None => "❌",
        };
        output.push_str(&format!("- {} {}\n", icon, code));
    }

    output.push_str("\n### Medium Priority\n\n");
    let med_implemented = count_implemented(&medium_priority, implemented);
    let med_pct = (med_implemented as f64 / medium_priority.len() as f64) * 100.0;
    output.push_str(&format!(
        "{}/{} ({:.1}%)\n\n",
        med_implemented,
        medium_priority.len(),
        med_pct
    ));
    output.push_str(&generate_progress_bar(med_pct));
    output.push_str("\n\n");
    for code in &medium_priority {
        let status = implemented.get(code);
        let icon = match status {
            Some(ImplementationStatus::Complete) => "✅",
            Some(ImplementationStatus::Partial) => "🔶",
            Some(ImplementationStatus::Misclassified) => "🔄",
            None => "❌",
        };
        output.push_str(&format!("- {} {}\n", icon, code));
    }

    output.push_str("\n### Low Priority\n\n");
    let low_implemented = count_implemented(&low_priority, implemented);
    let low_pct = (low_implemented as f64 / low_priority.len() as f64) * 100.0;
    output.push_str(&format!(
        "{}/{} ({:.1}%)\n\n",
        low_implemented,
        low_priority.len(),
        low_pct
    ));
    output.push_str(&generate_progress_bar(low_pct));
    output.push_str("\n\n");
    for code in &low_priority {
        let status = implemented.get(code);
        let icon = match status {
            Some(ImplementationStatus::Complete) => "✅",
            Some(ImplementationStatus::Partial) => "🔶",
            Some(ImplementationStatus::Misclassified) => "🔄",
            None => "❌",
        };
        output.push_str(&format!("- {} {}\n", icon, code));
    }

    output.push_str("\n---\n\n");

    // Legend
    output.push_str("## Legend\n\n");
    output.push_str("- ✅ **Complete** - Validation rule fully implemented and tested\n");
    output.push_str(
        "- 🔶 **Partial** - Validation rule partially implemented (some cases covered)\n",
    );
    output.push_str("- ❌ **Not Implemented** - Validation rule needs implementation\n");
    output.push_str("- 🔄 **Misclassified** - Marked as validation but actually a parser error\n");

    output.push_str("\n---\n\n");

    // How to update
    output.push_str("## How to Update\n\n");
    output.push_str("This dashboard is auto-generated from error corpus specs.\n\n");
    output.push_str("**To regenerate**:\n");
    output.push_str("```bash\n");
    output.push_str("cargo run --bin gen_coverage_dashboard\n");
    output.push_str("```\n\n");
    output.push_str("**To update implementation status**:\n");
    output.push_str("1. Implement validation rule in `talkbank-model/src/validation/`\n");
    output.push_str(
        "2. Verify test passes: `cargo nextest run -p talkbank-parser-tests -E 'test(validation_tests)'`\n",
    );
    output.push_str("3. Update `get_implemented_error_codes()` in `gen_coverage_dashboard.rs`\n");
    output.push_str("4. Regenerate dashboard\n\n");

    output.push_str("---\n\n");
    output.push_str(&format!(
        "**Generated**: {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    output.push_str("**Source**: `generators/src/bin/gen_coverage_dashboard.rs`\n");

    output
}

fn generate_progress_bar(percentage: f64) -> String {
    let filled = (percentage / 5.0).round() as usize; // 20 segments (5% each)
    let empty = 20 - filled;

    format!(
        "[{}{}] {:.1}%",
        "█".repeat(filled),
        "░".repeat(empty),
        percentage
    )
}

fn get_categories_with_codes(
    validation_specs: &[&ErrorCorpusSpec],
    _validation_by_code: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut categories: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for spec in validation_specs {
        for example in &spec.examples {
            if let Some(code) = &example.error_code {
                categories
                    .entry(spec.metadata.description.clone())
                    .or_default()
                    .push(code.clone());
            }
        }
    }

    // Deduplicate codes in each category
    for codes in categories.values_mut() {
        codes.sort();
        codes.dedup();
    }

    categories
}

fn categorize_by_priority(
    validation_by_code: &BTreeMap<String, Vec<String>>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let high_priority = vec![
        "E601", "E602", "E603", "E604", "E605", // Tier alignment
        "E519", "E244", "E245", "E247", "E250", // Common errors
    ];

    let medium_priority = [
        "E701", "E702", "E704", // Tier content
        "E306", "E521", // Cross-utterance
    ];

    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();

    for code in validation_by_code.keys() {
        if high_priority.contains(&code.as_str()) {
            high.push(code.clone());
        } else if medium_priority.contains(&code.as_str()) {
            medium.push(code.clone());
        } else {
            low.push(code.clone());
        }
    }

    (high, medium, low)
}

fn count_implemented(
    codes: &[String],
    implemented: &HashMap<String, ImplementationStatus>,
) -> usize {
    codes
        .iter()
        .filter(|c| {
            matches!(
                implemented.get(*c),
                Some(ImplementationStatus::Complete | ImplementationStatus::Partial)
            )
        })
        .count()
}
