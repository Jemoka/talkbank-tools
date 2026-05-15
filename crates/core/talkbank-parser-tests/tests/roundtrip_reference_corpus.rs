//! TreeSitter serialization roundtrip test on the reference corpus.
//!
//! For every `.cha` file in the reference corpus:
//! 1. Parse with TreeSitterParser
//! 2. Serialize back to CHAT via `WriteChat`
//! 3. Re-parse the serialized output
//! 4. Compare original and re-parsed ASTs with `SemanticEq`
//!
//! This is the primary gate for serialization correctness. Any model or
//! serialization change that breaks roundtrip idempotency will fail here.
//!
//! ## Usage
//!
//! ```bash
//! cargo nextest run -p talkbank-parser-tests -E 'test(roundtrip_reference)'
//! cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
//! ```

use std::path::PathBuf;

use rstest::rstest;
use talkbank_model::model::{SemanticEq, WriteChat};
use talkbank_parser::TreeSitterParser;

/// Parse → serialize → reparse → SemanticEq for a single reference corpus file.
#[rstest]
fn roundtrip_reference(#[files("../../resources/corpus/reference/**/*.cha")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));

    let parser = TreeSitterParser::new()
        .unwrap_or_else(|e| panic!("Failed to create TreeSitterParser: {}", e));

    // Pass 1: parse original
    let original = parser
        .parse_chat_file(&content)
        .unwrap_or_else(|e| panic!("Parse failed for {}: {}", path.display(), e));

    // Serialize back to CHAT text
    let serialized = original.to_chat_string();

    // Pass 2: reparse the serialized output
    let reparsed = parser.parse_chat_file(&serialized).unwrap_or_else(|e| {
        panic!(
            "Reparse of serialized output failed for {}: {}",
            path.display(),
            e
        )
    });

    // Semantic comparison
    assert!(
        original.semantic_eq(&reparsed),
        "Roundtrip semantic mismatch for {}\n\
         Original serialized length: {}\n\
         Reparsed serialized length: {}",
        path.display(),
        serialized.len(),
        reparsed.to_chat_string().len(),
    );
}
