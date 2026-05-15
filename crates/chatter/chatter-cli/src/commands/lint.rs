//! Lint mode - detect and auto-fix common CHAT errors.
//!
//! This command identifies auto-fixable issues and optionally applies fixes.
//! Fixable issues come from the LSP code actions:
//! - E241: Replace `xx` with `xxx` (untranscribed marker)
//! - E242: Add `+...` for trailing off words
//! - E301: Add missing terminator (`.`, `?`, `!`)
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::fs;
use std::path::Path;
use talkbank_model::ParseValidateOptions;
use talkbank_model::{ErrorCode, ParseError};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::{PipelineError, parse_and_validate_with_parser};

/// Entry point for the `lint` subcommand that discovers fixable CHAT problems and applies
/// corrections when the user requests it.
///
/// The function is mindful of directories vs files, recursion, and the alignment flag so it
/// can emit diagnostics close to the examples in the Main Tier and Dependent Tier sections of
/// the CHAT manual (E241/E242/E301). Only the curated set of fixes that the LSP action catalog
/// advertises are applied, ensuring the resulting transcript remains faithful to CHAT practice.
pub fn lint_files(
    path: &Path,
    apply_fixes: bool,
    dry_run: bool,
    recursive: bool,
    check_alignment: bool,
) {
    if !path.exists() {
        eprintln!("Error: Path does not exist: {}", path.display());
        std::process::exit(1);
    }

    if path.is_dir() {
        if recursive {
            let summary = lint_directory(path, apply_fixes, dry_run, check_alignment);
            if summary.scan_errors > 0 {
                eprintln!(
                    "Lint scan was incomplete: {} file or directory error(s) occurred.",
                    summary.scan_errors
                );
                std::process::exit(1);
            }
        } else {
            eprintln!("Error: Path is a directory. Use --recursive to lint all files.");
            std::process::exit(1);
        }
    } else {
        let outcome = lint_single_file(path, apply_fixes, dry_run, check_alignment);
        if outcome.had_error {
            std::process::exit(1);
        }
    }
}

/// Lints directory.
fn lint_directory(
    dir: &Path,
    apply_fixes: bool,
    dry_run: bool,
    check_alignment: bool,
) -> LintRunSummary {
    let mut summary = LintRunSummary::default();

    for entry in walkdir::WalkDir::new(dir) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("Error walking {}: {}", dir.display(), error);
                summary.scan_errors += 1;
                continue;
            }
        };

        if entry.path().extension().and_then(|s| s.to_str()) != Some("cha") {
            continue;
        }

        summary.total_files += 1;
        let outcome = lint_single_file(entry.path(), apply_fixes, dry_run, check_alignment);
        summary.total_fixable += outcome.fixable;
        summary.total_fixed += outcome.fixed;
        if outcome.had_error {
            summary.scan_errors += 1;
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("Summary:");
    println!("  Files scanned:   {}", summary.total_files);
    println!("  Fixable issues:  {}", summary.total_fixable);
    if apply_fixes && !dry_run {
        println!("  Fixed:           {}", summary.total_fixed);
    }
    if summary.scan_errors > 0 {
        println!("  Scan errors:     {}", summary.scan_errors);
    }

    summary
}

/// Lints single file.
fn lint_single_file(
    path: &Path,
    apply_fixes: bool,
    dry_run: bool,
    check_alignment: bool,
) -> LintFileOutcome {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            return LintFileOutcome::error();
        }
    };

    // Parse and collect all errors
    let parser = match TreeSitterParser::new() {
        Ok(parser) => parser,
        Err(err) => {
            eprintln!("Error creating parser: {}", err);
            return LintFileOutcome::error();
        }
    };
    let options = ParseValidateOptions {
        alignment: check_alignment,
        validate: true,
        strict_linkers: false,
    };

    // Parse and validate (errors are collected internally)
    let result = parse_and_validate_with_parser(&parser, &source, options);

    // Extract validation errors (only validation errors are auto-fixable)
    let errors: Vec<ParseError> = match result {
        Ok(_) => Vec::new(), // No errors
        Err(PipelineError::Validation(errs)) => errs,
        Err(PipelineError::Parse(_)) => {
            eprintln!(
                "  ⚠️  Parse errors detected - file has syntax errors that prevent auto-fixing"
            );
            return LintFileOutcome::error();
        }
        _ => Vec::new(),
    };

    // Filter for fixable errors
    let fixable_errors: Vec<_> = errors
        .into_iter()
        .filter(|e| is_fixable_error(&e.code))
        .collect();

    if fixable_errors.is_empty() {
        if !apply_fixes {
            println!("✓ {}: No fixable issues", path.display());
        }
        return LintFileOutcome::default();
    }

    println!("📝 {}", path.display());
    let mut fixes = Vec::new();

    for error in &fixable_errors {
        if let Some(fix) = generate_fix(&source, error) {
            println!(
                "  {} [{}]: {}",
                if apply_fixes && !dry_run {
                    "🔧"
                } else {
                    "⚠️"
                },
                error.code.as_str(),
                error.message
            );
            println!(
                "    Line {}: \"{}\" → \"{}\"",
                fix.line, fix.old_text, fix.new_text
            );
            fixes.push(fix);
        }
    }

    let fixable_count = fixes.len();
    let mut fixed_count = 0;
    let mut had_error = false;

    if apply_fixes && !fixes.is_empty() {
        if dry_run {
            println!("  [dry-run] Would fix {} issue(s)", fixes.len());
        } else {
            match apply_fixes_to_file(&source, &fixes) {
                Ok(new_content) => {
                    if let Err(e) = fs::write(path, new_content) {
                        eprintln!("  Error writing file: {}", e);
                        had_error = true;
                    } else {
                        println!("  ✓ Fixed {} issue(s)", fixes.len());
                        fixed_count = fixes.len();
                    }
                }
                Err(e) => {
                    eprintln!("  Error applying fixes: {}", e);
                    had_error = true;
                }
            }
        }
    }

    println!();
    LintFileOutcome {
        fixable: fixable_count,
        fixed: fixed_count,
        had_error,
    }
}

#[derive(Default)]
struct LintRunSummary {
    total_files: usize,
    total_fixable: usize,
    total_fixed: usize,
    scan_errors: usize,
}

#[derive(Default)]
struct LintFileOutcome {
    fixable: usize,
    fixed: usize,
    had_error: bool,
}

impl LintFileOutcome {
    fn error() -> Self {
        Self {
            had_error: true,
            ..Self::default()
        }
    }
}

/// Description of one auto-fix operation that can be applied to source text.
#[derive(Clone)]
struct LintFix {
    byte_start: usize,
    byte_end: usize,
    old_text: String,
    new_text: String,
    line: usize,
}

/// Return `true` when an error code has an auto-fix implementation.
fn is_fixable_error(code: &ErrorCode) -> bool {
    matches!(code.as_str(), "E241" | "E242" | "E301")
}

/// Generates fix.
fn generate_fix(source: &str, error: &ParseError) -> Option<LintFix> {
    let byte_start = error.location.span.start as usize;
    let byte_end = error.location.span.end as usize;

    // Calculate line number from byte offset
    let line = source[..byte_start].chars().filter(|&c| c == '\n').count() + 1;

    // Extract the text at error location
    let error_text = match source.get(byte_start..byte_end) {
        Some(text) => text,
        None => {
            eprintln!(
                "Error extracting text for fix ({}..{})",
                byte_start, byte_end
            );
            return None;
        }
    };

    let (old_text, new_text) = match error.code.as_str() {
        "E241" => {
            // Replace "xx" with "xxx"
            if error_text == "xx" {
                ("xx".to_string(), "xxx".to_string())
            } else {
                return None;
            }
        }
        "E242" => {
            // Add "+..." after the word
            // The error points to the word that needs the marker
            (error_text.to_string(), format!("{}+...", error_text))
        }
        "E301" => {
            // Add terminator at end of utterance
            // The error points to where terminator should be
            ("".to_string(), " .".to_string())
        }
        _ => return None,
    };

    Some(LintFix {
        byte_start,
        byte_end,
        old_text,
        new_text,
        line,
    })
}

/// Applies fixes to file.
fn apply_fixes_to_file(source: &str, fixes: &[LintFix]) -> Result<String, String> {
    // Sort fixes by byte position (reverse order so we can apply without invalidating offsets)
    let mut sorted_fixes = fixes.to_vec();
    sorted_fixes.sort_by(|a, b| b.byte_start.cmp(&a.byte_start));

    let mut result = source.to_string();

    for fix in sorted_fixes {
        // Check bounds
        if fix.byte_start > result.len() || fix.byte_end > result.len() {
            return Err(format!(
                "Fix offset out of bounds: {}..{} (file length: {})",
                fix.byte_start,
                fix.byte_end,
                result.len()
            ));
        }

        // Apply the replacement
        result.replace_range(fix.byte_start..fix.byte_end, &fix.new_text);
    }

    Ok(result)
}
