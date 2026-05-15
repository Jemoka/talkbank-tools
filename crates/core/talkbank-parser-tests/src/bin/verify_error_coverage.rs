//! Error Coverage Verification Tool
//!
//! Verifies that all error codes defined in talkbank-model have corresponding test files.
//! Generates a comprehensive coverage report showing which errors are tested and which are missing.
//!
//! ## Usage
//! ```bash
//! cargo run -p talkbank-parser-tests --bin verify_error_coverage
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use talkbank_parser_tests::test_error::TestError;

static CODE_CATEGORY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^([EW])(\\d)\\d{2}$").expect("valid regex"));

static CODE_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#\[code\("([EW]\d{3})"\)\]"#).expect("valid regex"));

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    println!("Error Corpus Coverage Report");
    println!("=============================\n");

    // Get all error codes from error_code.rs
    let all_error_codes = extract_error_codes()?;
    println!("Total error codes defined: {}\n", all_error_codes.len());

    // Find all test files
    let test_files = find_all_test_files()?;
    println!("Total test files found: {}\n", test_files.len());

    // Categorize error codes
    let categories = categorize_error_codes(&all_error_codes)?;

    // Check coverage for each category
    let mut total_tested = 0;
    let mut total_missing = 0;

    println!("Coverage by Category:");
    println!("---------------------\n");

    for (category_name, category_codes) in [
        ("E0-E1xx (Internal)", &categories.internal),
        ("E2xx (Word)", &categories.word),
        ("E3xx (Parser)", &categories.parser),
        ("E4xx (Dependent Tiers)", &categories.dependent_tier),
        ("E5xx (Headers)", &categories.header),
        ("E6xx (Tier Validation)", &categories.tier),
        ("E7xx (Alignment)", &categories.alignment),
        ("Wxxx (Warnings)", &categories.warnings),
    ] {
        let tested: HashSet<_> = category_codes
            .iter()
            .filter(|code| test_files.contains_key(*code))
            .collect();
        let missing: Vec<_> = category_codes
            .iter()
            .filter(|code| !test_files.contains_key(*code))
            .collect();

        let coverage = if category_codes.is_empty() {
            100.0
        } else {
            (tested.len() as f64 / category_codes.len() as f64) * 100.0
        };

        println!(
            "{:<30} {:>3}/{:<3} ({:>5.1}%)",
            category_name,
            tested.len(),
            category_codes.len(),
            coverage
        );

        if !missing.is_empty() && missing.len() <= 10 {
            for code in &missing {
                println!("  Missing: {}", code);
            }
        } else if !missing.is_empty() {
            println!("  Missing: {} codes", missing.len());
        }

        total_tested += tested.len();
        total_missing += missing.len();
    }

    println!("\n{}", "=".repeat(60));
    let overall_coverage = (total_tested as f64 / all_error_codes.len() as f64) * 100.0;
    println!(
        "TOTAL: {}/{} ({:.1}%)",
        total_tested,
        all_error_codes.len(),
        overall_coverage
    );
    println!("{}", "=".repeat(60));

    // Detailed analysis of untestable codes
    println!("\n\nUntestable Codes (System/Internal):");
    println!("-----------------------------------");
    let untestable = vec!["E001", "E002"];
    for code in &untestable {
        if all_error_codes.contains(*code) {
            println!(
                "  {} - Internal system error (not testable with CHAT files)",
                code
            );
        }
    }

    // Show test file distribution
    println!("\n\nTest File Distribution:");
    println!("----------------------");
    let parse_errors = test_files
        .iter()
        .filter(|(_, path)| path.contains("parse_errors"))
        .count();
    let validation_errors = test_files
        .iter()
        .filter(|(_, path)| path.contains("validation_errors"))
        .count();
    let warnings = test_files
        .iter()
        .filter(|(_, path)| path.contains("warnings"))
        .count();

    println!("  Parse errors:       {:>3} files", parse_errors);
    println!("  Validation errors:  {:>3} files", validation_errors);
    println!("  Warnings:           {:>3} files", warnings);
    println!("  Total:              {:>3} files", test_files.len());

    // Success/failure summary
    println!("\n");
    if total_missing == 0 {
        println!("✅ SUCCESS: 100% error code coverage!");
    } else {
        println!(
            "⚠️  {} error codes still need test files ({:.1}% coverage)",
            total_missing, overall_coverage
        );
    }

    Ok(())
}

/// Data container for ErrorCategories.
struct ErrorCategories {
    internal: Vec<String>,
    word: Vec<String>,
    parser: Vec<String>,
    dependent_tier: Vec<String>,
    header: Vec<String>,
    tier: Vec<String>,
    alignment: Vec<String>,
    warnings: Vec<String>,
}

/// Categorize error codes by numeric family for coverage reporting.
fn categorize_error_codes(codes: &HashSet<String>) -> Result<ErrorCategories, TestError> {
    let mut categories = ErrorCategories {
        internal: Vec::new(),
        word: Vec::new(),
        parser: Vec::new(),
        dependent_tier: Vec::new(),
        header: Vec::new(),
        tier: Vec::new(),
        alignment: Vec::new(),
        warnings: Vec::new(),
    };

    for code in codes {
        let caps = match CODE_CATEGORY_RE.captures(code) {
            Some(caps) => caps,
            None => {
                return Err(TestError::Failure(format!(
                    "Invalid error code format: {}",
                    code
                )));
            }
        };
        let prefix = match caps.get(1) {
            Some(value) => value.as_str(),
            None => {
                return Err(TestError::Failure(format!(
                    "Missing error code prefix in {}",
                    code
                )));
            }
        };
        let major_digit = match caps.get(2) {
            Some(value) => value.as_str(),
            None => {
                return Err(TestError::Failure(format!(
                    "Missing error code category digit in {}",
                    code
                )));
            }
        };

        match (prefix, major_digit) {
            ("W", _) => categories.warnings.push(code.clone()),
            ("E", "0") | ("E", "1") => categories.internal.push(code.clone()),
            ("E", "2") => categories.word.push(code.clone()),
            ("E", "3") => categories.parser.push(code.clone()),
            ("E", "4") => categories.dependent_tier.push(code.clone()),
            ("E", "5") => categories.header.push(code.clone()),
            ("E", "6") => categories.tier.push(code.clone()),
            ("E", "7") => categories.alignment.push(code.clone()),
            _ => {}
        }
    }

    // Sort all categories
    categories.internal.sort();
    categories.word.sort();
    categories.parser.sort();
    categories.dependent_tier.sort();
    categories.header.sort();
    categories.tier.sort();
    categories.alignment.sort();
    categories.warnings.sort();

    Ok(categories)
}

/// Extracts error codes.
fn extract_error_codes() -> Result<HashSet<String>, TestError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let error_code_path = PathBuf::from(manifest_dir)
        .parent()
        .ok_or_else(|| TestError::Failure("manifest dir missing parent".to_string()))?
        .join("talkbank-model/src/errors/codes/error_code.rs");

    let content = fs::read_to_string(error_code_path)?;

    let mut codes = HashSet::new();
    for line in content.lines() {
        // Look for lines like: #[code("E241")]
        if let Some(caps) = CODE_ATTR_RE.captures(line)
            && let Some(code) = caps.get(1)
        {
            codes.insert(code.as_str().to_string());
        }
    }

    Ok(codes)
}

/// Finds all test files.
fn find_all_test_files() -> Result<HashMap<String, String>, TestError> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_root = PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or_else(|| {
            TestError::Failure("manifest dir missing expected great-grandparent".to_string())
        })?
        .join("tests/error_corpus");

    let mut files = HashMap::new();

    for entry in walkdir::WalkDir::new(&corpus_root).into_iter() {
        let entry = entry
            .map_err(|err| TestError::Failure(format!("Failed to read corpus entry: {err}")))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("cha") {
            let filename = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
                TestError::Failure(format!("Invalid filename for {}", path.display()))
            })?;
            let error_code = filename
                .split_once('_')
                .map(|(code, _)| code)
                .ok_or_else(|| {
                    TestError::Failure(format!("Missing error code in filename {}", filename))
                })?;
            files.insert(error_code.to_string(), path.display().to_string());
        }
    }

    Ok(files)
}
