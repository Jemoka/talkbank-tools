//! Unit tests: lex real lines from ~/talkbank/data/*-data corpus.
//!
//! For each line type, we extract sample lines from the wild corpus
//! and verify they lex with ZERO error tokens. This catches lexer gaps
//! that curated reference corpus files don't expose.
//!
//! Uses `ChatLines` to correctly handle continuation lines.

use talkbank_parser_re2c::chat_lines::{ChatLineKind, ChatLines};
use talkbank_parser_re2c::lex;
use talkbank_parser_re2c::token::Token;

fn corpus_base() -> String {
    std::env::var("TALKBANK_DATA").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/talkbank/data")
    })
}

/// Collect sample logical lines from the wild corpus matching a prefix.
/// Uses `ChatLines` to correctly handle continuation lines.
/// Returns up to `max` unique lines.
fn collect_lines(prefix: &str, max: usize) -> Vec<String> {
    let mut lines = std::collections::BTreeSet::new();
    let base = corpus_base();
    let data_dirs: Vec<_> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with("-data")))
        .take(5) // Limit to first 5 corpus dirs for speed
        .collect();

    'outer: for dir in &data_dirs {
        let walker = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
            .take(200); // Limit files per dir

        for entry in walker {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for chat_line in ChatLines::new(&content) {
                    if chat_line.text.starts_with(prefix) {
                        lines.insert(chat_line.text.to_string());
                        if lines.len() >= max {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
    lines.into_iter().collect()
}

/// Collect sample logical lines by kind.
fn collect_lines_by_kind(kind: ChatLineKind, max: usize) -> Vec<String> {
    let mut lines = std::collections::BTreeSet::new();
    let base = corpus_base();
    let data_dirs: Vec<_> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with("-data")))
        .take(5)
        .collect();

    'outer: for dir in &data_dirs {
        let walker = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
            .take(200);

        for entry in walker {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for chat_line in ChatLines::new(&content) {
                    if chat_line.kind == kind {
                        lines.insert(chat_line.text.to_string());
                        if lines.len() >= max {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
    lines.into_iter().collect()
}

/// Lex a line and assert zero error tokens.
/// Returns the number of errors found (for reporting).
fn assert_clean_lex(line: &str) -> usize {
    let input = format!("{line}\n");
    let result = lex(&input);
    let errors = result.errors();
    if !errors.is_empty() {
        let report = result.error_report(&input);
        eprintln!("ERRORS in: {}", line.chars().take(80).collect::<String>());
        eprint!("{report}");
    }
    errors.len()
}

/// Run a batch of lines through the lexer, report errors.
fn batch_lex_check(prefix: &str, lines: &[String]) -> (usize, usize) {
    let mut total = 0;
    let mut error_count = 0;
    for line in lines {
        total += 1;
        error_count += assert_clean_lex(line);
    }
    if error_count > 0 {
        eprintln!(
            "  {prefix}: {error_count} error tokens in {total} lines ({} clean)",
            total - lines.iter().filter(|l| assert_clean_lex(l) > 0).count()
        );
    }
    (total, error_count)
}

// ═══════════════════════════════════════════════════════════════
// Main tier lines
// ═══════════════════════════════════════════════════════════════

#[test]
fn corpus_main_tiers() {
    let lines = collect_lines("*", 500);
    if lines.is_empty() {
        eprintln!("Skipping: no corpus data found");
        return;
    }
    let mut errors = 0;
    for line in &lines {
        errors += assert_clean_lex(line);
    }
    eprintln!("Main tiers: {}/{} lines clean", lines.len(), lines.len());
    assert_eq!(
        errors,
        0,
        "{errors} error tokens in {} main tier lines",
        lines.len()
    );
}

// ═══════════════════════════════════════════════════════════════
// Header lines
// ═══════════════════════════════════════════════════════════════

#[test]
fn corpus_headers_structured() {
    let prefixes = [
        "@ID:\t",
        "@Languages:\t",
        "@Participants:\t",
        "@Types:\t",
        "@Date:\t",
        "@Media:\t",
        "@Options:\t",
        "@Comment:\t",
    ];
    let mut total_errors = 0;
    let mut total_lines = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 100);
        for line in &lines {
            total_lines += 1;
            total_errors += assert_clean_lex(line);
        }
    }
    eprintln!("Structured headers: {total_lines} lines, {total_errors} errors");
    assert_eq!(total_errors, 0);
}

#[test]
fn corpus_headers_no_content() {
    let prefixes = ["@UTF8", "@Begin", "@End", "@Blank", "@New Episode"];
    let mut total_errors = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 50);
        for line in &lines {
            total_errors += assert_clean_lex(line);
        }
    }
    assert_eq!(total_errors, 0);
}

#[test]
fn corpus_headers_text() {
    let prefixes = [
        "@Location:\t",
        "@Situation:\t",
        "@Activities:\t",
        "@Recording Quality:\t",
        "@Room Layout:\t",
        "@Transcriber:\t",
        "@Transcription:\t",
        "@PID:\t",
        "@Font:\t",
        "@Window:\t",
        "@Color words:\t",
        "@Warning:\t",
        "@Number:\t",
    ];
    let mut total_errors = 0;
    let mut total_lines = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 50);
        for line in &lines {
            total_lines += 1;
            total_errors += assert_clean_lex(line);
        }
    }
    eprintln!("Text headers: {total_lines} lines, {total_errors} errors");
    assert_eq!(total_errors, 0);
}

#[test]
fn corpus_headers_optional_content() {
    let prefixes = ["@Bg", "@Eg", "@G"];
    let mut total_errors = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 50);
        for line in &lines {
            total_errors += assert_clean_lex(line);
        }
    }
    assert_eq!(total_errors, 0);
}

#[test]
fn corpus_headers_speaker_embedded() {
    let prefixes = ["@Birth of", "@Birthplace of", "@L1 of"];
    let mut total_errors = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 50);
        for line in &lines {
            total_errors += assert_clean_lex(line);
        }
    }
    assert_eq!(total_errors, 0);
}

// ═══════════════════════════════════════════════════════════════
// Dependent tier lines
// ═══════════════════════════════════════════════════════════════

#[test]
fn corpus_mor_tiers() {
    let lines = collect_lines("%mor:\t", 500);
    if lines.is_empty() {
        return;
    }
    let mut errors = 0;
    for line in &lines {
        errors += assert_clean_lex(line);
    }
    eprintln!(
        "%mor: {}/{} lines, {errors} errors",
        lines.len(),
        lines.len()
    );
    assert_eq!(errors, 0);
}

#[test]
fn corpus_gra_tiers() {
    let lines = collect_lines("%gra:\t", 500);
    if lines.is_empty() {
        return;
    }
    let mut errors = 0;
    for line in &lines {
        errors += assert_clean_lex(line);
    }
    eprintln!(
        "%gra: {}/{} lines, {errors} errors",
        lines.len(),
        lines.len()
    );
    assert_eq!(errors, 0);
}

#[test]
fn corpus_pho_tiers() {
    let lines = collect_lines("%pho:\t", 200);
    if lines.is_empty() {
        return;
    }
    let mut errors = 0;
    for line in &lines {
        errors += assert_clean_lex(line);
    }
    assert_eq!(errors, 0);
}

#[test]
fn corpus_text_tiers() {
    let prefixes = [
        "%com:\t", "%act:\t", "%sit:\t", "%exp:\t", "%eng:\t", "%flo:\t", "%ort:\t", "%spa:\t",
        "%add:\t", "%err:\t", "%gpx:\t", "%int:\t",
    ];
    let mut total_errors = 0;
    let mut total_lines = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 100);
        for line in &lines {
            total_lines += 1;
            total_errors += assert_clean_lex(line);
        }
    }
    eprintln!("Text tiers: {total_lines} lines, {total_errors} errors");
    assert_eq!(total_errors, 0);
}

#[test]
fn corpus_user_defined_tiers() {
    let prefixes = ["%xdb:\t", "%xpho:\t", "%xmod:\t", "%xcod:\t", "%xlang:\t"];
    let mut total_errors = 0;
    let mut total_lines = 0;
    for prefix in &prefixes {
        let lines = collect_lines(prefix, 100);
        for line in &lines {
            total_lines += 1;
            total_errors += assert_clean_lex(line);
        }
    }
    eprintln!("User-defined tiers: {total_lines} lines, {total_errors} errors");
    assert_eq!(total_errors, 0);
}

// ═══════════════════════════════════════════════════════════════
// Summary: lex entire files end-to-end
// ═══════════════════════════════════════════════════════════════

#[test]
fn corpus_full_files_sample() {
    // Lex a sample of complete files using ChatLines, check for error tokens
    let base = corpus_base();
    let data_dirs: Vec<_> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with("-data")))
        .take(3)
        .collect();

    let mut total_files = 0;
    let mut total_lines = 0;
    let mut total_errors = 0;
    let mut files_with_errors = 0;

    for dir in &data_dirs {
        let walker = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
            .take(50);

        for entry in walker {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                total_files += 1;
                let mut file_errors = 0;
                for chat_line in ChatLines::new(&content) {
                    total_lines += 1;
                    let result = lex(chat_line.text);
                    let errs = result.errors();
                    if !errs.is_empty() {
                        file_errors += errs.len();
                        // Print first few errors for debugging
                        if file_errors <= 3 {
                            let snippet = chat_line.text.chars().take(60).collect::<String>();
                            eprintln!(
                                "  Error in {:?}: {} ({} errors)",
                                entry.path().file_name().unwrap_or_default(),
                                snippet.escape_debug(),
                                errs.len()
                            );
                            for e in &errs {
                                eprintln!(
                                    "    {}: {:?}",
                                    e.context,
                                    e.token.text().escape_debug().to_string()
                                );
                            }
                        }
                    }
                }
                if file_errors > 0 {
                    files_with_errors += 1;
                    total_errors += file_errors;
                }
            }
        }
    }
    eprintln!(
        "Full files: {total_files} files, {total_lines} logical lines, {files_with_errors} files with errors, {total_errors} total error tokens"
    );
    assert_eq!(
        total_errors, 0,
        "{total_errors} error tokens in {files_with_errors}/{total_files} files"
    );
}

/// Lex ALL .cha files in ALL corpus dirs. Run with --ignored for the full sweep.
/// cargo nextest run -p talkbank-parser-re2c --test corpus_lex_tests -E 'test(corpus_full_sweep)' --run-ignored ignored-only --nocapture
#[test]
#[ignore]
fn corpus_full_sweep() {
    let base = corpus_base();
    let data_dirs: Vec<_> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with("-data")))
        .collect();

    let mut total_files = 0;
    let mut total_lines = 0;
    let mut total_errors = 0;
    let mut files_with_errors = 0;
    let mut error_examples: Vec<String> = Vec::new();

    for dir in &data_dirs {
        let dir_name = dir.file_name().to_string_lossy().to_string();
        let mut dir_files = 0;
        let mut dir_errors = 0;

        let walker = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"));

        for entry in walker {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                dir_files += 1;
                total_files += 1;
                let mut file_errors = 0;
                for chat_line in ChatLines::new(&content) {
                    total_lines += 1;
                    let result = lex(chat_line.text);
                    let errs = result.errors();
                    if !errs.is_empty() {
                        file_errors += errs.len();
                        if error_examples.len() < 50 {
                            let snippet = chat_line.text.chars().take(80).collect::<String>();
                            for e in &errs {
                                error_examples.push(format!(
                                    "{}: {} — {} ({:?})",
                                    entry
                                        .path()
                                        .file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy(),
                                    e.context,
                                    snippet.escape_debug(),
                                    e.token.text().escape_debug().to_string()
                                ));
                            }
                        }
                    }
                }
                if file_errors > 0 {
                    files_with_errors += 1;
                    total_errors += file_errors;
                    dir_errors += file_errors;
                }
            }
        }
        eprintln!("  {dir_name}: {dir_files} files, {dir_errors} errors");
    }

    eprintln!("\n=== FULL CORPUS SWEEP ===");
    eprintln!("Files: {total_files}");
    eprintln!("Logical lines: {total_lines}");
    eprintln!("Files with errors: {files_with_errors}");
    eprintln!("Total error tokens: {total_errors}");

    if !error_examples.is_empty() {
        eprintln!("\nError examples (first 50):");
        for ex in &error_examples {
            eprintln!("  {ex}");
        }
    }

    assert_eq!(
        total_errors, 0,
        "{total_errors} errors in {files_with_errors}/{total_files} files"
    );
}
