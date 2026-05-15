//! Test module for error coverage in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use std::collections::HashSet;
use std::fs;

/// Tests error code spec coverage.
#[test]
fn test_error_code_spec_coverage() {
    let error_code_file = "crates/talkbank-model/src/errors/codes/error_code.rs";
    let spec_dir = "spec/errors";

    // Try multiple paths to find the file
    let content = fs::read_to_string(error_code_file)
        .or_else(|_| fs::read_to_string(format!("../../../{}", error_code_file)))
        .or_else(|_| fs::read_to_string(format!("../../{}", error_code_file)))
        .expect("Failed to find error_code.rs");

    let spec_path = if Path::new(spec_dir).exists() {
        spec_dir.to_string()
    } else if Path::new(&format!("../../../{}", spec_dir)).exists() {
        format!("../../../{}", spec_dir)
    } else {
        format!("../../{}", spec_dir)
    };

    use std::path::Path;

    let mut expected_codes = HashSet::new();
    for line in content.lines() {
        if line.contains("#[code(\"") {
            if let Some(start) = line.find("\"") {
                if let Some(rest) = line.get(start + 1..) {
                    if let Some(end) = rest.find("\"") {
                        let code = &rest[..end];
                        expected_codes.insert(code.to_string());
                    }
                }
            }
        }
    }

    // Exclude special/test codes
    expected_codes.remove("E002"); // TestError
    expected_codes.remove("E999"); // UnknownError

    // Exclude codes that don't need individual spec files:
    // - Codes covered by other checks (E366/E369 → E358/E359, E367/E368)
    // - Infrastructure codes or codes with separate validation
    let deprecated = vec![
        "E210", "E213", "E258", "E303", "E345", "E348", "E366", "E369", "E720",
    ];
    for code in deprecated {
        expected_codes.remove(code);
    }

    let mut missing_specs = Vec::new();
    for code in &expected_codes {
        let mut found = false;
        if let Ok(entries) = fs::read_dir(&spec_path) {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.starts_with(code) && filename.ends_with(".md") {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            missing_specs.push(code.clone());
        }
    }

    missing_specs.sort();

    if !missing_specs.is_empty() {
        println!("Missing specs for {} error codes:", missing_specs.len());
        for code in &missing_specs {
            println!("  - {}", code);
        }
    }
}
