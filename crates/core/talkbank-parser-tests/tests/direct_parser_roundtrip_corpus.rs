//! Direct Parser Roundtrip Test on Reference Corpus
//!
//! This test verifies that the TreeSitterParser can roundtrip all reference corpus
//! files: parse → serialize → re-parse → compare with SemanticEq.
//!
//! This is a prerequisite for integrating the direct parser into `chatter validate`.
//!
//! ## Usage
//!
//! ```bash
//! # Run the roundtrip test
//! cargo test --release -p talkbank-parser-tests --test direct_parser_roundtrip_corpus
//!
//! # Show detailed output for failures
//! cargo test --release -p talkbank-parser-tests --test direct_parser_roundtrip_corpus -- --nocapture
//! ```

use std::path::{Path, PathBuf};
use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;
use walkdir::WalkDir;

/// Find all .cha files in a directory tree.
fn find_cha_files(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("cha"))
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// Files containing constructs the direct parser does not support.
const DIRECT_PARSER_SKIP: &[&str] = &[];

/// Parse a file with TreeSitterParser and roundtrip it.
///
/// Returns Ok(()) if the file roundtrips successfully, or an error message.
fn roundtrip_file(path: &Path, parser: &TreeSitterParser) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("IO error reading {}: {}", path.display(), e))?;

    // Parse with TreeSitterParser
    let chat_file = parser
        .parse_chat_file(&content)
        .map_err(|e| format!("TreeSitterParser failed to parse {}: {}", path.display(), e))?;

    // Serialize back to CHAT
    let serialized = chat_file.to_chat_string();

    // Re-parse the serialized CHAT
    let reparsed = parser.parse_chat_file(&serialized).map_err(|e| {
        format!(
            "TreeSitterParser failed to re-parse serialized {}: {}",
            path.display(),
            e
        )
    })?;

    // Semantic comparison
    if !chat_file.semantic_eq(&reparsed) {
        return Err(format!(
            "Semantic mismatch after roundtrip for {}",
            path.display()
        ));
    }

    Ok(())
}

/// Verifies TreeSitterParser round-trip stability on the reference corpus.
#[test]
fn direct_parser_roundtrip_reference_corpus() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let corpus_dir = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("missing expected grandparent of manifest dir")
        .join("resources/corpus/reference");

    assert!(
        corpus_dir.exists(),
        "Reference corpus not found at {}",
        corpus_dir.display()
    );

    let mut files = find_cha_files(&corpus_dir);
    files.sort();

    assert!(
        !files.is_empty(),
        "No .cha files found in {}",
        corpus_dir.display()
    );

    let parser = TreeSitterParser::new().expect("Failed to create TreeSitterParser");

    let mut passed = 0;
    let mut failures: Vec<(PathBuf, String)> = Vec::new();

    for (i, file) in files.iter().enumerate() {
        if (i + 1) % 50 == 0 {
            println!("Progress: {}/{}", i + 1, files.len());
        }

        // Skip files with known direct parser limitations
        if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
            if DIRECT_PARSER_SKIP.contains(&stem) {
                println!("SKIP (unsupported): {}", file.display());
                passed += 1;
                continue;
            }
        }

        match roundtrip_file(file, &parser) {
            Ok(()) => passed += 1,
            Err(msg) => {
                eprintln!("✗ {}", msg);
                failures.push((file.clone(), msg));
            }
        }
    }

    println!();
    println!("=== Direct Parser Roundtrip Summary ===");
    println!("Total files: {}", files.len());
    println!("✓ Passed:    {}", passed);
    println!("✗ Failed:    {}", failures.len());
    println!("=======================================");

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failed files:");
        for (path, reason) in &failures {
            eprintln!("  {}: {}", path.display(), reason);
        }
        panic!(
            "FAILED: {} of {} files did not pass direct parser roundtrip",
            failures.len(),
            files.len()
        );
    }
}
