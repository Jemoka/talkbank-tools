//! Check coverage of construct and error specifications
//!
//! Analyzes which constructs and errors are documented.
//! For errors, cross-references against the ErrorCode enum to report
//! full coverage metrics.

use clap::Parser;
use generators::spec::{ConstructSpec, ErrorSpec};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// CLI arguments: flags to select construct/error coverage, spec directory, and optional enum file path.
#[derive(Parser)]
#[command(name = "coverage")]
#[command(about = "Check construct and error coverage")]
struct Args {
    /// Check construct coverage
    #[arg(long)]
    constructs: bool,

    /// Check error coverage
    #[arg(long)]
    errors: bool,

    /// Root directory for specs
    #[arg(short, long, default_value = "spec")]
    spec_dir: PathBuf,

    /// Path to ErrorCode enum source file (auto-detected if not specified)
    #[arg(long)]
    enum_file: Option<PathBuf>,
}

/// Reports coverage of construct and error specs, cross-referencing against the ErrorCode enum.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !args.constructs && !args.errors {
        anyhow::bail!("Specify --constructs or --errors (or both)");
    }

    if args.constructs {
        check_construct_coverage(&args.spec_dir.join("constructs"))?;
    }

    if args.errors {
        let enum_file = args.enum_file.unwrap_or_else(|| {
            // Auto-detect: look relative to spec_dir
            let spec_root = args.spec_dir.parent().unwrap_or(Path::new("."));
            spec_root.join("crates/talkbank-model/src/errors/codes/error_code.rs")
        });
        check_error_coverage(&args.spec_dir.join("errors"), &enum_file)?;
    }

    Ok(())
}

fn check_construct_coverage(dir: &PathBuf) -> anyhow::Result<()> {
    println!("=== Construct Coverage ===\n");

    let specs = ConstructSpec::load_all(dir)
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {}", e))?;

    let mut by_level = std::collections::HashMap::new();

    for spec in &specs {
        let entry = by_level
            .entry(spec.metadata.level.clone())
            .or_insert_with(Vec::new);
        entry.push(&spec.metadata.category);
    }

    for (level, categories) in by_level {
        println!("{} ({} categories):", level, categories.len());
        for category in categories {
            println!("  - {}", category);
        }
        println!();
    }

    println!("Total: {} construct specifications\n", specs.len());

    Ok(())
}

/// Extract all error/warning codes from the ErrorCode enum source file
fn extract_enum_codes(enum_file: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let content = std::fs::read_to_string(enum_file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", enum_file.display(), e))?;

    let code_re = Regex::new(r#"#\[code\("([EW]\d+)"\)\]\s*\n\s*(\w+)"#).expect("invalid regex");

    let mut codes = BTreeMap::new();
    for cap in code_re.captures_iter(&content) {
        let code = cap[1].to_string();
        let variant = cap[2].to_string();
        codes.insert(code, variant);
    }

    Ok(codes)
}

fn check_error_coverage(dir: &PathBuf, enum_file: &Path) -> anyhow::Result<()> {
    println!("=== Error Coverage ===\n");

    // Load all specs
    let specs = ErrorSpec::load_all(dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    // Collect spec info: code -> (has_example, layer, category)
    let mut spec_codes: BTreeMap<String, SpecInfo> = BTreeMap::new();
    for spec in &specs {
        for error_def in &spec.errors {
            let code = error_def.code.clone();
            if code.is_empty() || (!code.starts_with('E') && !code.starts_with('W')) {
                continue;
            }
            spec_codes.insert(
                code,
                SpecInfo {
                    has_example: !error_def.examples.is_empty(),
                    layer: spec.metadata.error_type.clone(),
                    category: spec.metadata.category.clone(),
                    source_file: spec.source_file.clone(),
                },
            );
        }
    }

    // Load enum codes
    let enum_codes = if enum_file.exists() {
        extract_enum_codes(enum_file)?
    } else {
        println!("Warning: Enum file not found at {}", enum_file.display());
        println!("         Showing spec-only coverage.\n");
        BTreeMap::new()
    };

    // Cross-reference
    let all_codes: BTreeSet<String> = enum_codes
        .keys()
        .chain(spec_codes.keys())
        .cloned()
        .collect();

    let mut with_spec = 0;
    let mut with_example = 0;
    let mut stubs = 0;
    let mut missing: Vec<(String, String)> = Vec::new();
    let mut extra: Vec<String> = Vec::new(); // in specs but not in enum

    // Group by category
    let mut by_category: BTreeMap<String, CategoryStats> = BTreeMap::new();

    for code in &all_codes {
        let in_enum = enum_codes.contains_key(code);
        let spec_info = spec_codes.get(code);

        if in_enum {
            if let Some(info) = spec_info {
                with_spec += 1;
                if info.has_example {
                    with_example += 1;
                } else {
                    stubs += 1;
                }

                let cat = categorize_code(code);
                let stats = by_category.entry(cat).or_default();
                stats.total += 1;
                stats.with_spec += 1;
                if info.has_example {
                    stats.with_example += 1;
                }
            } else {
                missing.push((
                    code.clone(),
                    enum_codes.get(code).cloned().unwrap_or_default(),
                ));

                let cat = categorize_code(code);
                let stats = by_category.entry(cat).or_default();
                stats.total += 1;
            }
        } else if spec_info.is_some() {
            extra.push(code.clone());
        }
    }

    let total_enum = enum_codes.len();

    // Print category breakdown
    println!("Category breakdown:");
    println!(
        "{:<25} {:>5} {:>5} {:>8} {:>8}",
        "Category", "Total", "Specs", "Examples", "Stubs"
    );
    println!("{}", "-".repeat(56));
    for (cat, stats) in &by_category {
        println!(
            "{:<25} {:>5} {:>5} {:>8} {:>8}",
            cat,
            stats.total,
            stats.with_spec,
            stats.with_example,
            stats.with_spec - stats.with_example,
        );
    }
    println!("{}", "-".repeat(56));
    println!(
        "{:<25} {:>5} {:>5} {:>8} {:>8}",
        "TOTAL", total_enum, with_spec, with_example, stubs,
    );
    println!();

    // Print coverage percentage
    let pct = if total_enum > 0 {
        (with_spec as f64 / total_enum as f64) * 100.0
    } else {
        0.0
    };
    println!("Coverage: {}/{} ({:.1}%)", with_spec, total_enum, pct);
    println!("  With CHAT examples: {}", with_example);
    println!("  Stub specs (no example): {}", stubs);
    println!();

    // Print missing codes
    if !missing.is_empty() {
        println!("Missing specs ({}):", missing.len());
        for (code, variant) in &missing {
            println!("  {} — {}", code, variant);
        }
        println!();
    }

    // Print extra codes (in specs but not in enum)
    if !extra.is_empty() {
        println!("Extra specs (not in enum): {:?}", extra);
        println!();
    }

    Ok(())
}

fn categorize_code(code: &str) -> String {
    if code.starts_with('W') {
        return "Warnings (Wxxx)".to_string();
    }
    let prefix = &code[1..2];
    match prefix {
        "0" | "1" => "Internal (E0xx/E1xx)".to_string(),
        "2" => "Word errors (E2xx)".to_string(),
        "3" => "Parser errors (E3xx)".to_string(),
        "4" => "Dep. tier (E4xx)".to_string(),
        "5" => "Header errors (E5xx)".to_string(),
        "6" => "Tier errors (E6xx)".to_string(),
        "7" => "Temporal/media (E7xx)".to_string(),
        "9" => "Unknown (E9xx)".to_string(),
        _ => format!("Other ({}xx)", prefix),
    }
}

#[derive(Default)]
struct CategoryStats {
    total: usize,
    with_spec: usize,
    with_example: usize,
}

#[allow(dead_code)]
struct SpecInfo {
    has_example: bool,
    layer: String,
    category: String,
    source_file: String,
}
