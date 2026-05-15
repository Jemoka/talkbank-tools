//! Per-file parser validation tests.
//!
//! Each `.cha` file in the reference corpus becomes its own `#[test]`.
//!
//! ## Usage
//!
//! ```bash
//! cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
//! ```

use std::path::PathBuf;

use rstest::rstest;
use talkbank_parser::TreeSitterParser;

/// Test that the parser successfully parses each reference corpus file.
#[rstest]
fn parser_equivalence(#[files("../../resources/corpus/reference/**/*.cha")] path: PathBuf) {
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let parser = TreeSitterParser::new().expect("TreeSitterParser init");
    let result = parser.parse_chat_file(&content);

    assert!(
        result.is_ok(),
        "Parser failed for {}: {:?}",
        path.display(),
        result.err()
    );
}
