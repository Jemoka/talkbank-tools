//! Error parity test: run ALL implemented error specs against the re2c
//! parser and compare error codes with TreeSitter.
//!
//! This test reads each error spec file, extracts the input and expected
//! error codes, then runs both parsers. It reports which specs produce
//! matching errors, which diverge, and which are missing.
//!
//! Run: `cargo test -p talkbank-parser-re2c --test error_parity -- --nocapture`

mod fixture_utils;

use std::collections::BTreeSet;
use std::path::Path;

use talkbank_model::{ErrorCollector, ErrorSink, Validate};
use talkbank_parser::TreeSitterParser;

/// A single test case from a spec file: input + expected error codes.
struct SpecCase {
    input: String,
    expected_codes: Vec<String>,
}

/// Extract CHAT inputs and expected error codes from a spec markdown file.
/// Some specs have multiple examples; returns all of them.
fn parse_spec(content: &str, filename: &str) -> Vec<SpecCase> {
    let mut cases = Vec::new();

    // Extract error code from filename as fallback: E241_foo.md -> E241
    let filename_code = if filename.len() >= 4
        && filename.starts_with('E')
        && filename[1..4].chars().all(|c| c.is_ascii_digit())
    {
        Some(filename[..4].to_string())
    } else {
        None
    };

    // Find all ```chat blocks and their associated expected codes.
    // A spec may have multiple examples, each with its own code block.
    let mut search_from = 0;
    while let Some(block_start) = content[search_from..].find("```chat\n") {
        let abs_start = search_from + block_start + "```chat\n".len();
        let Some(block_end) = content[abs_start..].find("\n```") else {
            break;
        };
        let abs_end = abs_start + block_end;
        let input = content[abs_start..abs_end].to_string();

        // Look for expected error codes near this block.
        // Search backwards for "Expected Error Codes" line, or forward.
        let context_start = search_from;
        let context_end = (abs_end + 200).min(content.len());
        let context = &content[context_start..context_end];

        let mut codes = Vec::new();
        for line in context.lines() {
            // Match "**Expected Error Codes**: E316" or "**Expected Error Codes**: E316, E317"
            if let Some(after) = line
                .strip_prefix("**Expected Error Codes**:")
                .or_else(|| line.strip_prefix("- **Expected Error Codes**:"))
            {
                for part in after.split([',', ' ']) {
                    let part = part.trim();
                    if part.len() >= 4
                        && (part.starts_with('E') || part.starts_with('W'))
                        && part[1..4].chars().all(|c| c.is_ascii_digit())
                    {
                        codes.push(part[..4].to_string());
                    }
                }
            }
        }

        // Fallback: use filename code if no explicit expected codes found
        if codes.is_empty() {
            if let Some(ref fc) = filename_code {
                codes.push(fc.clone());
            }
        }

        if !codes.is_empty() {
            cases.push(SpecCase {
                input,
                expected_codes: codes,
            });
        }

        search_from = abs_end + 4; // skip past closing ```
    }

    cases
}

/// Run TreeSitter on input and collect all error codes (parse + validate).
fn collect_error_codes_ts(input: &str) -> BTreeSet<String> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(input, &errors);
    file.validate_with_alignment(&errors, None);
    errors
        .to_vec()
        .iter()
        .map(|e| e.code.as_str().to_string())
        .collect()
}

fn collect_error_codes_re2c(input: &str) -> BTreeSet<String> {
    let errors = ErrorCollector::new();
    let parsed = talkbank_parser_re2c::parser::parse_chat_file_streaming(input, &errors);
    let mut file = talkbank_model::model::ChatFile::from(&parsed);
    file.validate_with_alignment(&errors, None);
    errors
        .to_vec()
        .iter()
        .map(|e| e.code.as_str().to_string())
        .collect()
}

#[test]
fn error_parity_audit() {
    let spec_dir = format!("{}/spec/errors", fixture_utils::workspace_root().display());
    let spec_path = Path::new(&spec_dir);
    if !spec_path.exists() {
        eprintln!("Spec directory not found: {spec_dir}");
        return;
    }

    let mut total = 0;
    let mut exact_match = 0;
    let mut both_detect = 0; // both report errors, different codes
    let mut re2c_silent = Vec::new(); // re2c reports NOTHING, TS does
    let mut ts_silent = Vec::new(); // TS reports NOTHING, re2c does
    let mut both_empty = 0; // neither reports the expected code
    let mut both_empty_specs: Vec<(String, Vec<String>, Vec<String>, Vec<String>)> = Vec::new();
    let mut skipped = 0;

    let mut entries: Vec<_> = std::fs::read_dir(spec_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let content = std::fs::read_to_string(entry.path()).unwrap();

        // Skip not_implemented specs
        if content.contains("not_implemented") {
            skipped += 1;
            continue;
        }

        let filename = entry.file_name().to_string_lossy().to_string();
        let cases = parse_spec(&content, &filename);
        if cases.is_empty() {
            continue;
        }

        for (case_idx, case) in cases.iter().enumerate() {
            total += 1;

            let ts_codes = collect_error_codes_ts(&case.input);
            let re2c_codes = collect_error_codes_re2c(&case.input);

            let expected: BTreeSet<String> = case.expected_codes.iter().cloned().collect();

            let ts_has_expected = expected.iter().all(|c| ts_codes.contains(c));
            let re2c_has_expected = expected.iter().all(|c| re2c_codes.contains(c));

            let label = if cases.len() > 1 {
                format!("{filename}#{case_idx}")
            } else {
                filename.clone()
            };

            let ts_has_any_error = !ts_codes.is_empty();
            let re2c_has_any_error = !re2c_codes.is_empty();

            if ts_has_expected && re2c_has_expected {
                exact_match += 1;
            } else if ts_has_any_error && re2c_has_any_error {
                // Both detect something wrong, just different codes
                both_detect += 1;
            } else if ts_has_any_error && !re2c_has_any_error {
                // Re2c is SILENT on invalid input — the critical gap
                re2c_silent.push((label, expected.iter().cloned().collect::<Vec<_>>()));
            } else if !ts_has_any_error && re2c_has_any_error {
                ts_silent.push((label, re2c_codes.iter().cloned().collect::<Vec<_>>()));
            } else {
                // Neither reports the expected code
                both_empty += 1;
                both_empty_specs.push((
                    label,
                    expected.iter().cloned().collect::<Vec<_>>(),
                    ts_codes.iter().cloned().collect::<Vec<_>>(),
                    re2c_codes.iter().cloned().collect::<Vec<_>>(),
                ));
            }
        }
    }

    let detects_errors = exact_match + both_detect;
    let total_with_errors = total - both_empty;

    eprintln!("\n═══ Error Parity Audit ═══");
    eprintln!("Specs tested:       {total}");
    eprintln!("Exact code match:   {exact_match}");
    eprintln!("Both detect (diff): {both_detect}");
    eprintln!("Re2c SILENT:        {} ← critical gaps", re2c_silent.len());
    eprintln!("TS silent:          {}", ts_silent.len());
    eprintln!("Both empty:         {both_empty}");
    eprintln!("Skipped (not_impl): {skipped}");
    eprintln!();
    eprintln!(
        "Error detection: {detects_errors}/{total_with_errors} ({:.1}%)",
        if total_with_errors > 0 {
            detects_errors as f64 / total_with_errors as f64 * 100.0
        } else {
            0.0
        }
    );
    eprintln!(
        "Exact parity:    {exact_match}/{total_with_errors} ({:.1}%)",
        if total_with_errors > 0 {
            exact_match as f64 / total_with_errors as f64 * 100.0
        } else {
            0.0
        }
    );

    if !re2c_silent.is_empty() {
        eprintln!("\n── Re2c SILENT (invalid input, no error reported) ──");
        for (file, codes) in &re2c_silent {
            eprintln!("  {file}: TS reports {codes:?}");
        }
        // Debug: show what re2c actually produces for silent cases
        eprintln!("\n── Debug: re2c output for silent cases ──");
        for entry in &entries {
            let content = std::fs::read_to_string(entry.path()).unwrap();
            if content.contains("not_implemented") {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().to_string();
            let cases = parse_spec(&content, &filename);
            for (ci, case) in cases.iter().enumerate() {
                let label = if cases.len() > 1 {
                    format!("{filename}#{ci}")
                } else {
                    filename.clone()
                };
                if re2c_silent.iter().any(|(f, _)| f == &label) {
                    let re2c_codes = collect_error_codes_re2c(&case.input);
                    let ts_codes = collect_error_codes_ts(&case.input);
                    eprintln!("  {label}:");
                    eprintln!("    TS:   {:?}", ts_codes);
                    eprintln!("    Re2c: {:?}", re2c_codes);
                    eprintln!(
                        "    Input (first 100): {:?}",
                        &case.input[..case.input.len().min(100)]
                    );
                }
            }
        }
    }

    if !ts_silent.is_empty() {
        eprintln!("\n── TS silent (Re2c reports, TS doesn't) ──");
        for (file, codes) in &ts_silent {
            eprintln!("  {file}: re2c reports {codes:?}");
        }
    }

    if !both_empty_specs.is_empty() {
        eprintln!("\n── Both empty (neither finds expected code) ──");
        for (file, expected, ts_got, re2c_got) in &both_empty_specs {
            eprintln!("  {file}: expected {expected:?}");
            if !ts_got.is_empty() {
                eprintln!("    ts got:   {ts_got:?}");
            }
            if !re2c_got.is_empty() {
                eprintln!("    re2c got: {re2c_got:?}");
            }
        }
    }
}

/// Verify that the re2c parser NEVER panics or aborts on invalid input.
/// Every error spec must produce a ChatFile, even if the content is garbage.
#[test]
fn re2c_never_panics_on_invalid_input() {
    let spec_dir = format!("{}/spec/errors", fixture_utils::workspace_root().display());
    let spec_path = std::path::Path::new(&spec_dir);
    if !spec_path.exists() {
        return;
    }

    let mut tested = 0;
    let mut entries: Vec<_> = std::fs::read_dir(spec_path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let content = std::fs::read_to_string(entry.path()).unwrap();
        let filename = entry.file_name().to_string_lossy().to_string();
        let cases = parse_spec(&content, &filename);

        for case in &cases {
            tested += 1;
            // This must not panic
            let errors = ErrorCollector::new();
            let parsed =
                talkbank_parser_re2c::parser::parse_chat_file_streaming(&case.input, &errors);
            // Must produce a ChatFile (best-effort recovery)
            let _file = talkbank_model::model::ChatFile::from(&parsed);
        }
    }

    eprintln!("Recovery test: {tested} invalid inputs parsed without panic");
}
